use build::bazel::remote::execution::v2 as remote_execution;
use prost;

use std::collections::HashSet;

pub fn verify_directory_canonical(directory: &remote_execution::Directory) -> Result<(), String> {
  //verify_no_unknown_fields(directory)?;
  verify_nodes(&directory.files, |n| &n.name, |n| &n.digest)?;
  verify_nodes(&directory.directories, |n| &n.name, |n| &n.digest)?;
  let file_names: HashSet<&String> = directory
    .files
    .iter()
    .map(|file| &file.name)
    .chain(directory.directories.iter().map(|dir| &dir.name))
    .collect();
  if file_names.len() != directory.files.len() + directory.directories.len() {
    return Err(format!(
      "Children must be unique, but a path was both a file and a directory: {:?}",
      directory
    ));
  }
  Ok(())
}

fn verify_nodes<Node, GetName, GetDigest>(
  nodes: &[Node],
  get_name: GetName,
  get_digest: GetDigest,
) -> Result<(), String>
where
  Node: prost::Message,
  GetName: Fn(&Node) -> &str,
  GetDigest: Fn(&Node) -> &Option<remote_execution::Digest>,
{
  let mut prev: Option<&Node> = None;
  for node in nodes {
    //verify_no_unknown_fields(node)?;
    //verify_no_unknown_fields(get_digest(node))?;
    let name = get_name(node);
    if let None = get_digest(node) {
      return Err(format!(
        "All children must have a digest, but {} had none",
        name
      ));
    }
    if name.contains('/') {
      return Err(format!(
        "All children must have one path segment, but found {}",
        name
      ));
    }
    if let Some(p) = prev {
      if name <= get_name(p) {
        return Err(format!(
          "Children must be sorted and unique, but {} was before {}",
          get_name(p),
          name
        ));
      }
    }
    prev = Some(node);
  }
  Ok(())
}

// TODO: Verify no unknown fields when prost supports
//fn verify_no_unknown_fields(message: &prost::Message) -> Result<(), String> {
//  if message.get_unknown_fields().fields.is_some() {
//    return Err(format!(
//      "Found unknown fields: {:?}",
//      message.get_unknown_fields()
//    ));
//  }
//  Ok(())
//}

#[cfg(test)]
mod canonical_directory_tests {
  use super::remote_execution::{Digest, Directory, DirectoryNode, FileNode};
  use super::verify_directory_canonical;

  const HASH: &str = "693d8db7b05e99c6b7a7c0616456039d89c555029026936248085193559a0b5d";
  const FILE_SIZE: i64 = 16;
  const DIRECTORY_HASH: &str = "63949aa823baf765eff07b946050d76ec0033144c785a94d3ebd82baa931cd16";
  const DIRECTORY_SIZE: i64 = 80;
  const OTHER_DIRECTORY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb924\
                                      27ae41e4649b934ca495991b7852b855";
  const OTHER_DIRECTORY_SIZE: i64 = 0;

  #[test]
  fn empty_directory() {
    assert_eq!(
      Ok(()),
      verify_directory_canonical(&Directory {
        files: vec![],
        directories: vec![],
      })
    );
  }

  #[test]
  fn canonical_directory() {
    let directory = Directory {
      files: vec![
        FileNode {
          name: "roland".to_owned(),
          digest: Some(Digest {
            hash: HASH.to_owned(),
            size_bytes: FILE_SIZE,
          }),
          is_executable: false,
        },
        FileNode {
          name: "simba".to_owned(),
          digest: Some(Digest {
            hash: HASH.to_owned(),
            size_bytes: FILE_SIZE,
          }),
          is_executable: false,
        },
      ],
      directories: vec![
        DirectoryNode {
          name: "cats".to_owned(),
          digest: Some(Digest {
            hash: DIRECTORY_HASH.to_owned(),
            size_bytes: DIRECTORY_SIZE,
          }),
        },
        DirectoryNode {
          name: "dogs".to_owned(),
          digest: Some(Digest {
            hash: OTHER_DIRECTORY_HASH.to_owned(),
            size_bytes: OTHER_DIRECTORY_SIZE,
          }),
        },
      ],
    };
    assert_eq!(Ok(()), verify_directory_canonical(&directory));
  }

  //  #[test]
  //  fn unknown_field() {
  //    let mut directory = Directory::new();
  //    directory.mut_unknown_fields().add_fixed32(42, 42);
  //    let error = verify_directory_canonical(&directory).expect_err("Want error");
  //    assert!(
  //      error.contains("unknown"),
  //      format!("Bad error message: {}", error)
  //    );
  //  }

  //  #[test]
  //  fn unknown_field_in_file_node() {
  //    let mut directory = Directory::new();
  //
  //    directory.mut_files().push({
  //      let mut file = FileNode::new();
  //      file.set_name("roland".to_owned());
  //      file.set_digest({
  //        let mut digest = Digest::new();
  //        digest.set_size_bytes(FILE_SIZE);
  //        digest.set_hash(HASH.to_owned());
  //        digest
  //      });
  //      file.mut_unknown_fields().add_fixed32(42, 42);
  //      file
  //    });
  //
  //    let error = verify_directory_canonical(&directory).expect_err("Want error");
  //    assert!(
  //      error.contains("unknown"),
  //      format!("Bad error message: {}", error)
  //    );
  //  }

  #[test]
  fn multiple_path_segments_in_directory() {
    let directory = Directory {
      directories: vec![DirectoryNode {
        name: "pets/cats".to_owned(),
        digest: Some(Digest {
          hash: DIRECTORY_HASH.to_owned(),
          size_bytes: DIRECTORY_SIZE,
        }),
      }],
      files: vec![],
    };

    let error = verify_directory_canonical(&directory).expect_err("Want error");
    assert!(
      error.contains("pets/cats"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn multiple_path_segments_in_file() {
    let directory = Directory {
      files: vec![FileNode {
        name: "cats/roland".to_owned(),
        digest: Some(Digest {
          hash: HASH.to_owned(),
          size_bytes: FILE_SIZE,
        }),
        is_executable: false,
      }],
      directories: vec![],
    };

    let error = verify_directory_canonical(&directory).expect_err("Want error");
    assert!(
      error.contains("cats/roland"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn duplicate_path_in_directory() {
    let dir = DirectoryNode {
      name: "cats".to_owned(),
      digest: Some(Digest {
        hash: DIRECTORY_HASH.to_owned(),
        size_bytes: DIRECTORY_SIZE,
      }),
    };
    let directory = Directory {
      directories: vec![dir.clone(), dir],
      files: vec![],
    };
    let error = verify_directory_canonical(&directory).expect_err("Want error");
    assert!(
      error.contains("cats"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn duplicate_path_in_file() {
    let file = FileNode {
      name: "roland".to_owned(),
      digest: Some(Digest {
        hash: HASH.to_owned(),
        size_bytes: FILE_SIZE,
      }),
      is_executable: false,
    };
    let directory = Directory {
      directories: vec![],
      files: vec![file.clone(), file],
    };
    let error = verify_directory_canonical(&directory).expect_err("Want error");
    assert!(
      error.contains("roland"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn duplicate_path_in_file_and_directory() {
    let directory = Directory {
      files: vec![FileNode {
        name: "roland".to_owned(),
        digest: Some(Digest {
          hash: HASH.to_owned(),
          size_bytes: FILE_SIZE,
        }),
        is_executable: false,
      }],
      directories: vec![DirectoryNode {
        name: "roland".to_owned(),
        digest: Some(Digest {
          hash: DIRECTORY_HASH.to_owned(),
          size_bytes: DIRECTORY_SIZE,
        }),
      }],
    };

    verify_directory_canonical(&directory).expect_err("Want error");
  }

  #[test]
  fn unsorted_path_in_directory() {
    let directory = Directory {
      directories: vec![
        DirectoryNode {
          name: "dogs".to_owned(),
          digest: Some(Digest {
            hash: DIRECTORY_HASH.to_owned(),
            size_bytes: DIRECTORY_SIZE,
          }),
        },
        DirectoryNode {
          name: "cats".to_owned(),
          digest: Some(Digest {
            hash: DIRECTORY_HASH.to_owned(),
            size_bytes: DIRECTORY_SIZE,
          }),
        },
      ],
      files: vec![],
    };
    let error = verify_directory_canonical(&directory).expect_err("Want error");
    assert!(
      error.contains("dogs was before cats"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn unsorted_path_in_file() {
    let directory = Directory {
      files: vec![
        FileNode {
          name: "simba".to_owned(),
          digest: Some(Digest {
            hash: HASH.to_owned(),
            size_bytes: FILE_SIZE,
          }),
          is_executable: false,
        },
        FileNode {
          name: "roland".to_owned(),
          digest: Some(Digest {
            hash: HASH.to_owned(),
            size_bytes: FILE_SIZE,
          }),
          is_executable: false,
        },
      ],
      directories: vec![],
    };

    let error = verify_directory_canonical(&directory).expect_err("Want error");
    assert!(
      error.contains("simba was before roland"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn file_node_missing_digest() {
    let directory = Directory {
      files: vec![FileNode {
        name: "some_file".to_owned(),
        digest: None,
        is_executable: false,
      }],
      directories: vec![],
    };
    let error = verify_directory_canonical(&directory).expect_err("Want err");
    assert!(
      error.contains("some_file"),
      format!("Bad error message: {}", error)
    );
    assert!(
      error.contains("must have a digest"),
      format!("Bad error message: {}", error)
    );
  }

  #[test]
  fn directory_node_missing_digest() {
    let directory = Directory {
      directories: vec![DirectoryNode {
        name: "some_dir".to_owned(),
        digest: None,
      }],
      files: vec![],
    };
    let error = verify_directory_canonical(&directory).expect_err("Want err");
    assert!(
      error.contains("some_dir"),
      format!("Bad error message: {}", error)
    );
    assert!(
      error.contains("must have a digest"),
      format!("Bad error message: {}", error)
    );
  }
}
