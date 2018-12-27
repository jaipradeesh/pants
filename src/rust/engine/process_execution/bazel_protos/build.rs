use protoc_grpcio;

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

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

  generate_for_grpcio(&thirdpartyprotobuf);

  let merged_root = make_merged_dir(&thirdpartyprotobuf);

  generate_for_tower(merged_root.path().to_owned());

}


fn generate_for_grpcio(thirdpartyprotobuf: &Path) {
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
            for line in original.lines() {
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

fn generate_for_tower(merged_root: PathBuf) {
  tower_grpc_build::Config::new()
      .enable_server(true)
      .enable_client(true)
      .build(
        &[
          merged_root.join("build/bazel/remote/execution/v2/remote_execution.proto"),
          merged_root.join("google/protobuf/empty.proto"),
        ],
        &[merged_root],
      ).unwrap_or_else(|e| panic!("protobuf compilation failed: {}", e));

  // TODO: File a bug
  for f in walkdir::WalkDir::new(std::env::var("OUT_DIR").unwrap()) {
    let f = f.unwrap();
    if f.path().extension() == Some("rs".as_ref()) {
      let contents: Vec<_> = std::fs::read_to_string(f.path())
          .unwrap()
          .lines()
          .map(|line| {
            if line.trim().starts_with("use") && line.contains(", (), ") {
              line.replace(", (), ", ", ")
            } else {
              line.to_owned()
            }
          }).collect();
      std::fs::write(f.path(), contents.join("\n")).unwrap();
    }
  }
}

fn make_merged_dir(thirdpartyprotobuf: &Path) -> tempfile::TempDir {
  let dir = tempfile::TempDir::new().unwrap();

  for file in walkdir::WalkDir::new(&thirdpartyprotobuf) {
    let file = file.unwrap();
    if file.path().extension() == Some("proto".as_ref()) {
      let relative_path: PathBuf = file
          .path()
          .strip_prefix(&thirdpartyprotobuf)
          .unwrap()
          .components()
          .skip(1)
          .collect();
      let dst = dir.path().join(&relative_path);
      std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
      std::fs::copy(file.path(), dst).unwrap();
    }
  }
  dir
}
