use protoc_grpcio;

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use build_utils::BuildRoot;

const EXTRA_HEADER: &'static str = r#"import "rustproto.proto";
option (rustproto.carllerche_bytes_for_bytes_all) = true;
"#;

fn main() {
  let build_root = BuildRoot::find().unwrap();
  let thirdpartyprotobuf = build_root.join("3rdparty/protobuf");
  for file in walkdir::WalkDir::new(&thirdpartyprotobuf) {
    println!("cargo:rerun-if-changed={}", file.unwrap().path().to_str().unwrap());
  }


  let amended_proto_root = tempfile::TempDir::new().unwrap();
  for f in &["bazelbuild_remote-apis", "googleapis"] {
    let src_root = thirdpartyprotobuf.join(f);
    for entry in walkdir::WalkDir::new(&src_root)
      .into_iter()
      .filter_map(|entry| entry.ok())
    {
      if entry.file_type().is_file() && entry.file_name().to_string_lossy().ends_with(".proto") {
        let dst = amended_proto_root
          .path()
          .join(entry.path().strip_prefix(&src_root).unwrap());
        std::fs::create_dir_all(dst.parent().unwrap())
          .expect("Error making dir in temp proto root");
        let original = std::fs::read_to_string(entry.path())
          .expect(&format!("Error reading {}", entry.path().display()));
        let mut copy = String::with_capacity(original.len() + EXTRA_HEADER.len());
        for line in original.split("\n") {
          copy += line;
          copy += "\n";
          if line.starts_with("package ") {
            copy += EXTRA_HEADER
          }
        }
        std::fs::write(&dst, copy.as_bytes()).expect(&format!("Error writing {}", dst.display()));
      }
    }
  }

  let gen_dir = PathBuf::from("src/gen");

  // Re-gen if, say, someone does a git clean on the gen dir but not the target dir. This ensures
  // generated sources are available for reading by programmers and tools like rustfmt alike.
  println!("cargo:rerun-if-changed={}", gen_dir.to_str().unwrap());

  protoc_grpcio::compile_grpc_protos(
    &[
      "build/bazel/remote/execution/v2/remote_execution.proto",
      "google/bytestream/bytestream.proto",
      "google/rpc/code.proto",
      "google/rpc/error_details.proto",
      "google/rpc/status.proto",
      "google/longrunning/operations.proto",
      "google/protobuf/empty.proto",
    ],
    &[
      amended_proto_root.path().to_owned(),
      thirdpartyprotobuf.join("standard"),
      thirdpartyprotobuf.join("rust-protobuf"),
    ],
    &gen_dir,
  )
  .expect("Failed to compile protos!");

  let listing = gen_dir.read_dir().unwrap();
  let mut pub_mod_stmts = listing
    .filter_map(|d| {
      let dirent = d.unwrap();
      let file_name = dirent.file_name().into_string().unwrap();
      match file_name.trim_right_matches(".rs") {
        "mod" | ".gitignore" => None,
        module_name => Some(format!("pub mod {};", module_name)),
      }
    })
    .collect::<Vec<_>>();
  pub_mod_stmts.sort();
  let contents = format!(
    "\
// This file is generated. Do not edit.
{}
",
    pub_mod_stmts.join("\n")
  );

  File::create(gen_dir.join("mod.rs"))
    .and_then(|mut f| f.write_all(contents.as_bytes()))
    .expect("Failed to write mod.rs")
}
