extern crate tower_grpc_build;

fn main() {
  // TODO: Set up directory watches
  tower_grpc_build::Config::new()
        .enable_server(true)
        .enable_client(true)
        .build(
            &[
                "../../../../3rdparty/protobuf/merged/build/bazel/remote/execution/v2/remote_execution.proto",
                "../../../../3rdparty/protobuf/merged/google/protobuf/empty.proto",
            ],
            &[
                "../../../../3rdparty/protobuf/merged",
            ],
        )
        .unwrap_or_else(|e| panic!("protobuf compilation failed: {}", e));
}
