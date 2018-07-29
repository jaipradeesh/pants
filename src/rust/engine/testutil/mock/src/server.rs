use futures::{self, stream::poll_fn, Async, Future, Poll, Stream};
use futures_timer;
use h2;
use http;
use std;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use tokio_core;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tower_grpc::codegen::server::tower::NewService;
use tower_h2::{Body, RecvBody, Server};

pub struct StopOnDropServer {
  dropped: Arc<Mutex<bool>>,
  local_addr: SocketAddr,
}

impl StopOnDropServer {
  pub fn new<S, B, IE, SE>(new_service: S) -> std::io::Result<StopOnDropServer>
  where
    S: NewService<
        Request = http::Request<RecvBody>,
        Response = http::Response<B>,
        InitError = IE,
        Error = SE,
      >
      + Send
      + 'static,
    B: Body + 'static,
    IE: Debug,
    SE: Debug,
  {
    let stop = Arc::new(Mutex::new(false));
    let stop2 = stop.clone();

    let (sender, receiver) = channel();

    std::thread::spawn(move || {
      let addr = "127.0.0.1:0".parse().unwrap();
      let result = (|| {
        let core = Core::new()?;
        let listener = TcpListener::bind(&addr, &core.handle())?;
        let addr = listener.local_addr()?;
        Ok((core, listener, addr))
      })();
      let (mut core, listener) = match result {
        Ok((core, listener, addr)) => {
          sender
            .send(Ok(addr))
            .expect("Error sending Ok from started server thread");
          (core, listener)
        }
        Err(err) => {
          sender
            .send(Err(err))
            .expect("Error sending Err from started server thread");
          return;
        }
      };

      // Select from three streams:
      //  * Stream 1 gets requests from the network, and continues listening.
      //  * Stream 2 gets a signal that the server has been dropped, and should stop listening.
      //  * Stream 3 gets a re-poll signal at a fixed interval, so that stream 2 is re-polled.
      let serve = listener
        .incoming()
        .map_err(|err| TerminateOrError::Error(err))
        .map(|(sock, _)| HandleOrTerminate::SocketRequest(sock))
        .select(poll_fn(
          move || -> Poll<Option<HandleOrTerminate>, TerminateOrError> {
            if *stop.lock().unwrap() {
              Ok(Async::Ready(Some(HandleOrTerminate::Terminate)))
            } else {
              Ok(Async::NotReady)
            }
          },
        ))
        .select(futures::stream::unfold((), |()| {
          Some(
            // Check for whether the server has been dropped evert 50ms.
            futures_timer::Delay::new(std::time::Duration::from_millis(50))
              .map(|()| (HandleOrTerminate::Continue, ()))
              .map_err(|err| TerminateOrError::Error(err)),
          )
        }))
        .fold(
          (
            Server::new(new_service, h2::server::Builder::default(), core.handle()),
            core.handle(),
          ),
          |(server, reactor), req_or_die| match req_or_die {
            HandleOrTerminate::SocketRequest(sock) => {
              if let Err(e) = sock.set_nodelay(true) {
                return Err(TerminateOrError::Error(e));
              }
              let serve = server.serve(sock);
              reactor.spawn(serve.map_err(|e| error!("Error serving: {:?}", e)));

              Ok((server, reactor))
            }
            HandleOrTerminate::Terminate => Err(TerminateOrError::Terminate),
            HandleOrTerminate::Continue => Ok((server, reactor)),
          },
        );

      core
        .run(serve)
        .map(|_| ())
        .or_else(|result| match result {
          TerminateOrError::Terminate => Ok(()),
          TerminateOrError::Error(err) => Err(err),
        })
        .expect("Error from server");
    });

    match receiver.recv() {
      Ok(Ok(local_addr)) => Ok(StopOnDropServer {
        dropped: stop2,
        local_addr: local_addr,
      }),
      Ok(Err(err)) => Err(err),
      Err(err) => Err(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "Error starting or while serving server",
      )),
    }
  }

  pub fn local_addr(&self) -> SocketAddr {
    self.local_addr
  }
}

impl Drop for StopOnDropServer {
  fn drop(&mut self) {
    *self.dropped.lock().unwrap() = true;
  }
}

enum HandleOrTerminate {
  // Indicates a request which should be served has been received.
  SocketRequest(tokio_core::net::TcpStream),
  // Indicates the server should cleanly terminate.
  Terminate,
  // Continue waiting for either of the other conditions.
  Continue,
}

enum TerminateOrError {
  // Indicates the server should cleanly terminate.
  Terminate,
  // Indicates an error occurred, and the the server terminated in the background.
  Error(std::io::Error),
}
