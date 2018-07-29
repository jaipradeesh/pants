use bazel_protos::build::bazel::remote::execution::v2::server::{Execution, ExecutionServer};
use bazel_protos::build::bazel::remote::execution::v2::{ExecuteRequest, WaitExecutionRequest};
use bazel_protos::google::longrunning::server::Operations;
use bazel_protos::google::longrunning::Operation;
use bazel_protos::google::longrunning::{
  CancelOperationRequest, DeleteOperationRequest, GetOperationRequest, ListOperationsRequest,
  ListOperationsResponse,
};
use bazel_protos::google::protobuf::Empty;

use std::collections::VecDeque;
use std::fmt::Debug;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use super::StopOnDropServer;
use futures;
use http;
use prost;
use tower_grpc;

#[derive(Clone, Debug)]
pub struct MockExecution {
  name: String,
  execute_request: ExecuteRequest,
  operation_responses: Arc<Mutex<VecDeque<(Operation, Option<Duration>)>>>,
}

impl MockExecution {
  ///
  /// # Arguments:
  ///  * `name` - The name of the operation. It is assumed that all operation_responses use this
  ///             name.
  ///  * `execute_request` - The expected ExecuteRequest.
  ///  * `operation_responses` - Vec of Operation response for Execution or GetOperation requests.
  ///                            Will be returned in order.
  ///
  pub fn new(
    name: String,
    execute_request: ExecuteRequest,
    operation_responses: Vec<(Operation, Option<Duration>)>,
  ) -> MockExecution {
    MockExecution {
      name: name,
      execute_request: execute_request,
      operation_responses: Arc::new(Mutex::new(VecDeque::from(operation_responses))),
    }
  }
}

///
/// A server which will answer ExecuteRequest and GetOperation gRPC requests with pre-canned
/// responses.
///
pub struct TestServer {
  pub mock_responder: MockResponder,
  server_transport: StopOnDropServer,
}

impl TestServer {
  ///
  /// # Arguments
  /// * `mock_execution` - The canned responses to issue. Returns the MockExecution's
  ///                      operation_responses in order to any ExecuteRequest or GetOperation
  ///                      requests.
  ///                      If an ExecuteRequest request is received which is not equal to this
  ///                      MockExecution's execute_request, an error will be returned.
  ///                      If a GetOperation request is received whose name is not equal to this
  ///                      MockExecution's name, or more requests are received than stub responses
  ///                      are available for, an error will be returned.
  pub fn new(mock_execution: MockExecution) -> TestServer {
    let mock_responder = MockResponder::new(mock_execution);

    // TODO: Also register Operations...
    let new_service = ExecutionServer::new(mock_responder.clone());
    let server_transport = StopOnDropServer::new(new_service).expect("Starting");

    TestServer {
      mock_responder,
      server_transport,
    }
  }

  ///
  /// The address on which this server is listening over insecure HTTP transport.
  ///
  pub fn address(&self) -> SocketAddr {
    self.server_transport.local_addr()
  }
}

impl Drop for TestServer {
  fn drop(&mut self) {
    let remaining_expected_responses = self
      .mock_responder
      .mock_execution
      .operation_responses
      .lock()
      .unwrap()
      .len();
    assert_eq!(
      remaining_expected_responses,
      0,
      "Expected {} more requests. Remaining expected responses:\n{}\nReceived requests:\n{}",
      remaining_expected_responses,
      MockResponder::display_all(&Vec::from_iter(
        self
          .mock_responder
          .mock_execution
          .operation_responses
          .lock()
          .unwrap()
          .clone(),
      )),
      MockResponder::display_all(&self
        .mock_responder
        .received_messages
        .deref()
        .lock()
        .unwrap())
    )
  }
}

#[derive(Clone, Debug)]
pub struct MockResponder {
  mock_execution: MockExecution,
  pub received_messages: Arc<Mutex<Vec<(String, Box<prost::Message>, Instant)>>>,
}

impl MockResponder {
  fn new(mock_execution: MockExecution) -> MockResponder {
    MockResponder {
      mock_execution: mock_execution,
      received_messages: Arc::new(Mutex::new(vec![])),
    }
  }

  fn log<T: prost::Message + Sized + 'static>(&self, request_type: String, message: Box<T>) {
    self
      .received_messages
      .lock()
      .unwrap()
      .push((request_type, message, Instant::now()));
  }

  fn display_all<D: Debug>(items: &[D]) -> String {
    items
      .iter()
      .map(|i| format!("{:?}\n", i))
      .collect::<Vec<_>>()
      .concat()
  }

  fn next_operation(&self) -> Result<Operation, tower_grpc::Error> {
    match self
      .mock_execution
      .operation_responses
      .lock()
      .unwrap()
      .pop_front()
    {
      Some((op, duration)) => {
        if let Some(d) = duration {
          sleep(d);
        }
        Ok(op.clone())
      }
      None => {
        // TODO: Error message
        Err(tower_grpc::Error::Grpc(
          tower_grpc::Status::INVALID_ARGUMENT,
          http::HeaderMap::new(),
        ))
        //        sink.fail(grpcio::RpcStatus::new(
        //          grpcio::RpcStatusCode::InvalidArgument,
        //          Some("Did not expect further requests from client.".to_string()),
        //        ));
      }
    }
  }
}

impl Execution for MockResponder {
  type ExecuteStream = futures::stream::Once<Operation, tower_grpc::Error>;
  type ExecuteFuture =
    futures::future::FutureResult<tower_grpc::Response<Self::ExecuteStream>, tower_grpc::Error>;
  type WaitExecutionStream = futures::stream::Once<Operation, tower_grpc::Error>;
  type WaitExecutionFuture =
    futures::future::FutureResult<tower_grpc::Response<Self::ExecuteStream>, tower_grpc::Error>;

  // We currently only support the one-shot "stream and disconnect" client behavior.
  // If we start supporting the "stream updates" variant, we will need to do so here.
  fn execute(&mut self, req: tower_grpc::Request<ExecuteRequest>) -> Self::ExecuteFuture {
    self.log(
      "Execution.execute".to_owned(),
      Box::new(req.get_ref().clone()),
    );

    if &self.mock_execution.execute_request != req.get_ref() {
      // TODO: Error message
      return futures::future::err(tower_grpc::Error::Grpc(
        tower_grpc::Status::INVALID_ARGUMENT,
        http::HeaderMap::new(),
      ));
      //      ctx.spawn(
      //        sink
      //          .fail(grpcio::RpcStatus::new(
      //            grpcio::RpcStatusCode::InvalidArgument,
      //            Some("Did not expect this request".to_string()),
      //          ))
      //          .map_err(|_| ()),
      //      );
      //      return;
    }

    futures::future::done(
      self
        .next_operation()
        .map(|operation| tower_grpc::Response::new(futures::stream::once(Ok(operation)))),
    )
  }

  fn wait_execution(
    &mut self,
    request: tower_grpc::Request<WaitExecutionRequest>,
  ) -> Self::WaitExecutionFuture {
    unimplemented!()
  }
}

impl Operations for MockResponder {
  type ListOperationsFuture =
    futures::future::FutureResult<tower_grpc::Response<ListOperationsResponse>, tower_grpc::Error>;
  type GetOperationFuture =
    futures::future::FutureResult<tower_grpc::Response<Operation>, tower_grpc::Error>;
  type DeleteOperationFuture =
    futures::future::FutureResult<tower_grpc::Response<Empty>, tower_grpc::Error>;
  type CancelOperationFuture =
    futures::future::FutureResult<tower_grpc::Response<Empty>, tower_grpc::Error>;

  fn get_operation(
    &mut self,
    req: tower_grpc::Request<GetOperationRequest>,
  ) -> Self::GetOperationFuture {
    self.log(
      "Operations.get_operation".to_owned(),
      Box::new(req.get_ref().clone()),
    );

    futures::future::done(self.next_operation().map(tower_grpc::Response::new))
  }

  fn list_operations(
    &mut self,
    request: tower_grpc::Request<ListOperationsRequest>,
  ) -> Self::ListOperationsFuture {
    unimplemented!()
  }

  fn delete_operation(
    &mut self,
    request: tower_grpc::Request<DeleteOperationRequest>,
  ) -> Self::DeleteOperationFuture {
    unimplemented!()
  }

  fn cancel_operation(
    &mut self,
    request: tower_grpc::Request<CancelOperationRequest>,
  ) -> Self::CancelOperationFuture {
    unimplemented!()
  }
}
