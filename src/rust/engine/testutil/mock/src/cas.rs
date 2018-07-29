use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bazel_protos::build::bazel::remote::execution::v2::server::ContentAddressableStorage;
use bazel_protos::build::bazel::remote::execution::v2::server::ContentAddressableStorageServer;
use bazel_protos::build::bazel::remote::execution::v2::{
  BatchUpdateBlobsRequest, BatchUpdateBlobsResponse, FindMissingBlobsRequest,
  FindMissingBlobsResponse, GetTreeRequest, GetTreeResponse,
};
use bazel_protos::google::bytestream::server::ByteStream;
use bazel_protos::google::bytestream::{
  QueryWriteStatusRequest, QueryWriteStatusResponse, ReadRequest, ReadResponse, WriteRequest,
  WriteResponse,
};
use boxfuture::{BoxFuture, Boxable};
use futures;

use super::StopOnDropServer;
use bytes::Bytes;
use futures::{Future, IntoFuture, Stream};
use hashing::{Digest, Fingerprint};
use http;
use std::net::SocketAddr;
use std::vec::IntoIter;
use testutil::data::{TestData, TestDirectory};
use tower_grpc;

///
/// Implements the ContentAddressableStorage gRPC API, answering read requests with either known
/// content, NotFound for valid but unknown content, or InvalidArguments for bad arguments.
///
pub struct StubCAS {
  server_transport: StopOnDropServer,
  read_request_count: Arc<Mutex<usize>>,
  pub write_message_sizes: Arc<Mutex<Vec<usize>>>,
  pub blobs: Arc<Mutex<HashMap<Fingerprint, Bytes>>>,
}

impl StubCAS {
  pub fn with_content(
    chunk_size_bytes: i64,
    files: Vec<TestData>,
    directories: Vec<TestDirectory>,
  ) -> StubCAS {
    let mut blobs = HashMap::new();
    for file in files {
      blobs.insert(file.fingerprint(), file.bytes());
    }
    for directory in directories {
      blobs.insert(directory.fingerprint(), directory.bytes());
    }
    StubCAS::with_unverified_content(chunk_size_bytes, blobs)
  }

  ///
  /// # Arguments
  /// * `chunk_size_bytes` - The maximum number of bytes of content to include per streamed message.
  ///                        Messages will saturate until the last one, which may be smaller than
  ///                        this value.
  ///                        If a negative value is given, all requests will receive an error.
  /// * `blobs`            - Known Fingerprints and their content responses. These are not checked
  ///                        for correctness.
  pub fn with_unverified_content(
    chunk_size_bytes: i64,
    blobs: HashMap<Fingerprint, Bytes>,
  ) -> StubCAS {
    let read_request_count = Arc::new(Mutex::new(0));
    let write_message_sizes = Arc::new(Mutex::new(Vec::new()));
    let blobs = Arc::new(Mutex::new(blobs));
    let responder = StubCASResponder {
      chunk_size_bytes: chunk_size_bytes,
      blobs: blobs.clone(),
      read_request_count: read_request_count.clone(),
      write_message_sizes: write_message_sizes.clone(),
    };
    // TODO: Also register Bytestream...
    let new_service = ContentAddressableStorageServer::new(responder);
    let server_transport = StopOnDropServer::new(new_service).expect("Starting");

    StubCAS {
      server_transport,
      read_request_count,
      write_message_sizes,
      blobs,
    }
  }

  pub fn with_roland_and_directory(chunk_size_bytes: i64) -> StubCAS {
    StubCAS::with_content(
      chunk_size_bytes,
      vec![TestData::roland()],
      vec![TestDirectory::containing_roland()],
    )
  }

  pub fn empty() -> StubCAS {
    StubCAS::with_unverified_content(1024, HashMap::new())
  }

  pub fn always_errors() -> StubCAS {
    StubCAS::with_unverified_content(-1, HashMap::new())
  }

  ///
  /// The address on which this server is listening over insecure HTTP transport.
  ///
  pub fn address(&self) -> SocketAddr {
    self.server_transport.local_addr()
  }

  pub fn read_request_count(&self) -> usize {
    self.read_request_count.lock().unwrap().clone()
  }
}

#[derive(Clone, Debug)]
pub struct StubCASResponder {
  chunk_size_bytes: i64,
  blobs: Arc<Mutex<HashMap<Fingerprint, Bytes>>>,
  pub read_request_count: Arc<Mutex<usize>>,
  pub write_message_sizes: Arc<Mutex<Vec<usize>>>,
}

impl StubCASResponder {
  fn should_always_fail(&self) -> bool {
    self.chunk_size_bytes < 0
  }

  fn read_internal(&self, req: &ReadRequest) -> Result<Vec<ReadResponse>, tower_grpc::Error> {
    let parts: Vec<_> = req.resource_name.splitn(4, '/').collect();
    if parts.len() != 4 || parts.get(0) != Some(&"") || parts.get(1) != Some(&"blobs") {
      // TODO: Error message
      return Err(tower_grpc::Error::Grpc(
        tower_grpc::Status::INVALID_ARGUMENT,
        http::HeaderMap::new(),
      ));
      //      return Err(grpcio::RpcStatus::new(
      //        grpcio::RpcStatusCode::InvalidArgument,
      //        Some(format!(
      //          "Bad resource name format {} - want /blobs/some-sha256/size",
      //          req.resource_name
      //        )),
      //      ));
    }
    let digest = parts[2];
    let fingerprint = Fingerprint::from_hex_string(digest).map_err(|e| {
      // TODO: Error message
      tower_grpc::Error::Grpc(tower_grpc::Status::INVALID_ARGUMENT, http::HeaderMap::new())
      //      grpcio::RpcStatus::new(
      //        grpcio::RpcStatusCode::InvalidArgument,
      //        Some(format!("Bad digest {}: {}", digest, e)),
      //      )
    })?;
    if self.should_always_fail() {
      // TODO: Error message
      return Err(tower_grpc::Error::Grpc(
        tower_grpc::Status::INTERNAL,
        http::HeaderMap::new(),
      ));
      //      return Err(grpcio::RpcStatus::new(
      //        grpcio::RpcStatusCode::Internal,
      //        Some("StubCAS is configured to always fail".to_owned()),
      //      ));
    }
    let blobs = self.blobs.lock().unwrap();
    let maybe_bytes = blobs.get(&fingerprint);
    match maybe_bytes {
      Some(bytes) => Ok(
        bytes
          .chunks(self.chunk_size_bytes as usize)
          .map(|b| ReadResponse { data: Vec::from(b) })
          .collect(),
      ),
      // TODO: Error message
      None => Err(tower_grpc::Error::Grpc(
        tower_grpc::Status::NOT_FOUND,
        http::HeaderMap::new(),
      )),
      //      None => Err(grpcio::RpcStatus::new(
      //        grpcio::RpcStatusCode::NotFound,
      //        Some(format!("Did not find digest {}", fingerprint)),
      //      )),
    }
  }
}

impl ByteStream for StubCASResponder {
  type ReadStream = futures::stream::IterOk<IntoIter<ReadResponse>, tower_grpc::Error>;
  type ReadFuture =
    futures::future::FutureResult<tower_grpc::Response<Self::ReadStream>, tower_grpc::Error>;
  type WriteFuture = BoxFuture<tower_grpc::Response<WriteResponse>, tower_grpc::Error>;
  type QueryWriteStatusFuture = futures::future::FutureResult<
    tower_grpc::Response<QueryWriteStatusResponse>,
    tower_grpc::Error,
  >;

  fn read(&mut self, req: tower_grpc::Request<ReadRequest>) -> Self::ReadFuture {
    {
      let mut request_count = self.read_request_count.lock().unwrap();
      *request_count = *request_count + 1;
    }
    futures::future::done(
      self
        .read_internal(req.get_ref())
        .map(|response| tower_grpc::Response::new(futures::stream::iter_ok(response))),
    )
  }

  fn write(
    &mut self,
    request: tower_grpc::Request<tower_grpc::Streaming<WriteRequest>>,
  ) -> Self::WriteFuture {
    let should_always_fail = self.should_always_fail();
    let write_message_sizes = self.write_message_sizes.clone();
    let blobs = self.blobs.clone();
    request
      .into_inner()
      .collect()
      .into_future()
      .and_then(move |reqs| {
        let mut maybe_resource_name = None;
        let mut want_next_offset = 0;
        let mut bytes = Bytes::new();
        for req in reqs {
          match maybe_resource_name {
            None => maybe_resource_name = Some(req.resource_name),
            Some(ref resource_name) => {
              if resource_name != &req.resource_name {
                // TODO: Error message
                return Err(tower_grpc::Error::Grpc(
                  tower_grpc::Status::INVALID_ARGUMENT,
                  http::HeaderMap::new(),
                ));
                //                return Err(grpcio::Error::RpcFailure(grpcio::RpcStatus::new(
                //                  grpcio::RpcStatusCode::InvalidArgument,
                //                  Some(format!(
                //                    "All resource names in stream must be the same. Got {} but earlier saw {}",
                //                    req.resource_name,
                //                    resource_name
                //                  )),
                //                )));
              }
            }
          }
          if req.write_offset != want_next_offset {
            // TODO: Error message
            return Err(tower_grpc::Error::Grpc(
              tower_grpc::Status::INVALID_ARGUMENT,
              http::HeaderMap::new(),
            ));
            //            return Err(grpcio::Error::RpcFailure(grpcio::RpcStatus::new(
            //              grpcio::RpcStatusCode::InvalidArgument,
            //              Some(format!(
            //                "Missing chunk. Expected next offset {}, got next offset: {}",
            //                want_next_offset,
            //                req.write_offset
            //              )),
            //            )));
          }
          want_next_offset += req.data.len() as i64;
          write_message_sizes.lock().unwrap().push(req.data.len());
          bytes.extend(req.data);
        }
        Ok((maybe_resource_name, bytes))
      })
      .map_err(move |err: tower_grpc::Error| match err {
        tower_grpc::Error::Grpc(status, _) => status,
        // TODO: Error message
        e => tower_grpc::Status::UNKNOWN,
        //        e => grpcio::RpcStatus::new(grpcio::RpcStatusCode::Unknown, Some(format!("{:?}", e))),
      })
      .and_then(
        move |(maybe_resource_name, bytes)| match maybe_resource_name {
          // TODO: Error message
          None => Err(tower_grpc::Status::INVALID_ARGUMENT),
          //          None => Err(grpcio::RpcStatus::new(
          //            grpcio::RpcStatusCode::InvalidArgument,
          //            Some("Stream saw no messages".to_owned()),
          //          )),
          Some(resource_name) => {
            let parts: Vec<_> = resource_name.splitn(6, '/').collect();
            if parts.len() != 6
              || parts.get(1) != Some(&"uploads")
              || parts.get(3) != Some(&"blobs")
            {
              // TODO: Error message
              return Err(tower_grpc::Status::INVALID_ARGUMENT);
              //              return Err(grpcio::RpcStatus::new(
              //                grpcio::RpcStatusCode::InvalidArgument,
              //                Some(format!("Bad resource name: {}", resource_name)),
              //              ));
            }
            let fingerprint = match Fingerprint::from_hex_string(parts[4]) {
              Ok(f) => f,
              Err(err) => {
                // TODO: Error message
                return Err(tower_grpc::Status::INVALID_ARGUMENT);
                //                return Err(grpcio::RpcStatus::new(
                //                  grpcio::RpcStatusCode::InvalidArgument,
                //                  Some(format!(
                //                    "Bad fingerprint in resource name: {}: {}",
                //                    parts[4], err
                //                  )),
                //                ))
              }
            };
            let size = match parts[5].parse::<usize>() {
              Ok(s) => s,
              Err(err) => {
                // TODO: Error message
                return Err(tower_grpc::Status::INVALID_ARGUMENT);
                //                return Err(grpcio::RpcStatus::new(
                //                  grpcio::RpcStatusCode::InvalidArgument,
                //                  Some(format!("Bad size in resource name: {}: {}", parts[5], err)),
                //                ))
              }
            };
            if size != bytes.len() {
              // TODO: Error message
              return Err(tower_grpc::Status::INVALID_ARGUMENT);
              //              return Err(grpcio::RpcStatus::new(
              //                grpcio::RpcStatusCode::InvalidArgument,
              //                Some(format!(
              //                  "Size was incorrect: resource name said size={} but got {}",
              //                  size,
              //                  bytes.len()
              //                )),
              //              ));
            }

            if should_always_fail {
              // TODO: Error message
              return Err(tower_grpc::Status::INTERNAL);
              //              return Err(grpcio::RpcStatus::new(
              //                grpcio::RpcStatusCode::Internal,
              //                Some("StubCAS is configured to always fail".to_owned()),
              //              ));
            }

            {
              let mut blobs = blobs.lock().unwrap();
              blobs.insert(fingerprint, bytes);
            }

            Ok(tower_grpc::Response::new(WriteResponse {
              committed_size: size as i64,
            }))
          }
        },
      )
      .map_err(|status| tower_grpc::Error::Grpc(status, http::HeaderMap::new()))
      .to_boxed()
  }

  fn query_write_status(
    &mut self,
    _req: tower_grpc::Request<QueryWriteStatusRequest>,
  ) -> Self::QueryWriteStatusFuture {
    unimplemented!()
  }
}

impl ContentAddressableStorage for StubCASResponder {
  type FindMissingBlobsFuture = futures::future::FutureResult<
    tower_grpc::Response<FindMissingBlobsResponse>,
    tower_grpc::Error,
  >;
  type BatchUpdateBlobsFuture = futures::future::FutureResult<
    tower_grpc::Response<BatchUpdateBlobsResponse>,
    tower_grpc::Error,
  >;
  type GetTreeStream = futures::stream::IterOk<IntoIter<GetTreeResponse>, tower_grpc::Error>;
  type GetTreeFuture =
    futures::future::FutureResult<tower_grpc::Response<Self::GetTreeStream>, tower_grpc::Error>;

  fn find_missing_blobs(
    &mut self,
    req: tower_grpc::Request<FindMissingBlobsRequest>,
  ) -> Self::FindMissingBlobsFuture {
    if self.should_always_fail() {
      return futures::future::err(tower_grpc::Error::Grpc(
        tower_grpc::Status::INTERNAL,
        http::HeaderMap::new(),
      ));
      // TODO: Error message
      //      sink.fail(grpcio::RpcStatus::new(
      //        grpcio::RpcStatusCode::Internal,
      //        Some("StubCAS is configured to always fail".to_owned()),
      //      ));
      //      return;
    }
    let blobs = self.blobs.lock().unwrap();
    let mut missing_blob_digests = vec![];
    for digest in &req.get_ref().blob_digests {
      let hashing_digest_result: Result<Digest, String> = digest.into();
      let hashing_digest = hashing_digest_result.expect("Bad digest");
      if !blobs.contains_key(&hashing_digest.0) {
        missing_blob_digests.push(digest.clone())
      }
    }
    return futures::future::ok(tower_grpc::Response::new(FindMissingBlobsResponse {
      missing_blob_digests,
    }));
  }

  fn batch_update_blobs(
    &mut self,
    _req: tower_grpc::Request<BatchUpdateBlobsRequest>,
  ) -> Self::BatchUpdateBlobsFuture {
    unimplemented!();
  }

  fn get_tree(&mut self, _req: tower_grpc::Request<GetTreeRequest>) -> Self::GetTreeFuture {
    // Our client doesn't currently use get_tree, so we don't bother implementing it.
    // We will need to if the client starts wanting to use it.
    unimplemented!();
  }
}
