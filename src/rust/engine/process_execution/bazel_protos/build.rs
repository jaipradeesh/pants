extern crate build_utils;
extern crate tower_grpc_build;

use std::path::PathBuf;

use build_utils::BuildRoot;

fn main() {
  let build_root = BuildRoot::find().unwrap();
  let thirdpartyprotobuf = build_root.join("3rdparty").join("protobuf");
  println!(
    "cargo:rerun-if-changed={}",
    thirdpartyprotobuf.to_str().unwrap()
  );
  let merged = thirdpartyprotobuf.join("merged");

  // TODO: Set up directory watches
  tower_grpc_build::Config::new()
    .enable_server(true)
    .enable_client(true)
    .build(
      &[
        merged
          .join("build")
          .join("bazel")
          .join("remote")
          .join("execution")
          .join("v2")
          .join("remote_execution.proto"),
        merged
          .join("google")
          .join("bytestream")
          .join("bytestream.proto"),
      ],
      &[merged],
    )
    .expect("Protobuf compilation failed");

  for package in &[
    "build.bazel.remote.execution.v2",
    "google.api",
    "google.bytestream",
    "google.longrunning",
    "google.rpc",
    "google.protobuf",
  ] {
    copy_generated_file(package).expect("Copying generated protobuf rust failed");
  }
}

fn copy_generated_file(package_path: &str) -> Result<(), std::io::Error> {
  let mut generated_filename = String::from(package_path);
  generated_filename.extend(".rs".chars());
  let src = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join(generated_filename);

  let mut dst = PathBuf::from("src").join("gen");
  dst.extend(package_path.split('.'));
  dst.set_extension("rs");

  std::fs::create_dir_all(dst.parent().unwrap())?;
  println!("DWH: Copying {:?} to {:?}", src, dst);
  std::fs::copy(src, dst)?;
  Ok(())
}

//fn main() {
//  let build_root = BuildRoot::find().unwrap();
//  let thirdpartyprotobuf = build_root.join("3rdparty/protobuf");
//  println!(
//    "cargo:rerun-if-changed={}",
//    thirdpartyprotobuf.to_str().unwrap()
//  );
//
//  let gen_dir = PathBuf::from("src/gen_old");
//
//  // Re-gen if, say, someone does a git clean on the gen dir but not the target dir. This ensures
//  // generated sources are available for reading by programmers and tools like rustfmt alike.
//  println!("cargo:rerun-if-changed={}", gen_dir.to_str().unwrap());
//
//  protoc_grpcio::compile_grpc_protos(
//    &[
//      "build/bazel/remote/execution/v2/remote_execution.proto",
//      "google/bytestream/bytestream.proto",
//      "google/rpc/code.proto",
//      "google/rpc/error_details.proto",
//      "google/rpc/status.proto",
//      "google/longrunning/operations.proto",
//      "google/protobuf/empty.proto",
//    ],
//    &[
//      thirdpartyprotobuf.join("bazelbuild_remote-apis"),
//      thirdpartyprotobuf.join("googleapis"),
//      thirdpartyprotobuf.join("standard"),
//      thirdpartyprotobuf.join("rust-protobuf"),
//    ],
//    &gen_dir,
//  ).expect("Failed to compile protos!");
//
//  let listing = gen_dir.read_dir().unwrap();
//  let mut pub_mod_stmts = listing
//    .filter_map(|d| {
//      let dirent = d.unwrap();
//      let file_name = dirent.file_name().into_string().unwrap();
//      match file_name.trim_right_matches(".rs") {
//        "mod" | ".gitignore" => None,
//        module_name => Some(format!("pub mod {};", module_name)),
//      }
//    })
//    .collect::<Vec<_>>();
//  pub_mod_stmts.sort();
//  let contents = format!(
//    "\
//// This file is generated. Do not edit.
//{}
//",
//    pub_mod_stmts.join("\n")
//  );
//
//  File::create(gen_dir.join("mod.rs"))
//    .and_then(|mut f| f.write_all(contents.as_bytes()))
//    .expect("Failed to write mod.rs")
//}
