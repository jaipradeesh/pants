extern crate env_logger;
extern crate futures;
extern crate http;
extern crate prost;
#[macro_use]
extern crate prost_derive;
extern crate prost_types;
extern crate tokio_core;
extern crate tower_grpc;
extern crate tower_http;
extern crate tower_h2;

use futures::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tower_grpc::Request;
use tower_h2::client::Connection;

pub mod build {
    pub mod bazel {
        pub mod remote {
            pub mod execution {
                pub mod v2 {
                    include!(concat!(env!("OUT_DIR"), "/build.bazel.remote.execution.v2.rs"));
                }
            }
        }
    }
}

pub mod google {
    pub mod api {
        include!(concat!(env!("OUT_DIR"), "/google.api.rs"));
    }

    pub mod longrunning {
        include!(concat!(env!("OUT_DIR"), "/google.longrunning.rs"));
    }

    pub mod rpc {
        include!(concat!(env!("OUT_DIR"), "/google.rpc.rs"));
    }

    pub mod protobuf {
        include!(concat!(env!("OUT_DIR"), "/google.protobuf.rs"));

        pub type Empty = ();
    }
}

fn main() {
    let _ = ::env_logger::init();

    let mut core = Core::new().unwrap();
    let reactor = core.handle();

    let addr = "127.0.0.1:50001".parse().unwrap();
    let uri: http::Uri = format!("http://127.0.0.1:50001").parse().unwrap();

    let find_blobs = TcpStream::connect(&addr, &reactor)
        .and_then(move |socket| {
            // Bind the HTTP/2.0 connection
            Connection::handshake(socket, reactor)
                .map_err(|_| panic!("failed HTTP/2.0 handshake"))
        })
        .map(move |conn| {
            use build::bazel::remote::execution::v2::client::ContentAddressableStorage;
            use tower_http::add_origin;

            let conn = add_origin::Builder::new()
                .uri(uri)
                .build(conn)
                .unwrap();

            ContentAddressableStorage::new(conn)
        })
        .and_then(|mut client| {
            use build::bazel::remote::execution::v2::FindMissingBlobsRequest;

            client.find_missing_blobs(Request::new(FindMissingBlobsRequest {
                instance_name: "".to_string(),
                blob_digests: vec![],
            })).map_err(|e| panic!("gRPC request failed; err={:?}", e))
        })
        .and_then(|response| {
            println!("RESPONSE = {:?}", response);
            Ok(())
        })
        .map_err(|e| {
            println!("ERR = {:?}", e);
        });

    core.run(find_blobs).unwrap();
}
