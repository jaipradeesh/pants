use bazel_protos::build::bazel::remote::execution::v2::{Directory, DirectoryNode, FileNode};
use bytes;
use digest::FixedOutput;
use hashing;
use prost::Message;
use sha2::{self, Digest};

pub struct TestData {
  string: String,
}

impl TestData {
  pub fn empty() -> TestData {
    TestData::new("")
  }

  pub fn roland() -> TestData {
    TestData::new("European Burmese")
  }

  pub fn catnip() -> TestData {
    TestData::new("catnip")
  }

  pub fn robin() -> TestData {
    TestData::new("Pug")
  }

  pub fn fourty_chars() -> TestData {
    TestData::new(
      "0123456789012345678901234567890123456789\
       0123456789012345678901234567890123456789",
    )
  }

  pub fn new(s: &str) -> TestData {
    TestData {
      string: s.to_owned(),
    }
  }

  pub fn bytes(&self) -> bytes::Bytes {
    bytes::Bytes::from(self.string.as_str())
  }

  pub fn fingerprint(&self) -> hashing::Fingerprint {
    hash(&self.bytes())
  }

  pub fn digest(&self) -> hashing::Digest {
    hashing::Digest(self.fingerprint(), self.string.len())
  }

  pub fn string(&self) -> String {
    self.string.clone()
  }

  pub fn len(&self) -> usize {
    self.string.len()
  }
}

pub struct TestDirectory {
  directory: Directory,
}

impl TestDirectory {
  pub fn empty() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /roland
  pub fn containing_roland() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![FileNode {
          name: "roland".to_owned(),
          digest: Some((&TestData::roland().digest()).into()),
          is_executable: false,
        }],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /robin
  pub fn containing_robin() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![FileNode {
          name: "robin".to_owned(),
          digest: Some((&TestData::robin().digest()).into()),
          is_executable: false,
        }],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /treats
  pub fn containing_treats() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![FileNode {
          name: "treats".to_owned(),
          digest: Some((&TestData::catnip().digest()).into()),
          is_executable: false,
        }],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /cats/roland
  pub fn nested() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        directories: vec![DirectoryNode {
          name: "cats".to_owned(),
          digest: Some((&TestDirectory::containing_roland().digest()).into()),
        }],
        files: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /dnalor
  pub fn containing_dnalor() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![FileNode {
          name: "dnalor".to_owned(),
          digest: Some((&TestData::roland().digest()).into()),
          is_executable: false,
        }],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /roland
  pub fn containing_wrong_roland() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![FileNode {
          name: "roland".to_owned(),
          digest: Some((&TestData::catnip().digest()).into()),
          is_executable: false,
        }],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /roland
  // /treats
  pub fn containing_roland_and_treats() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![
          FileNode {
            name: "roland".to_owned(),
            digest: Some((&TestData::roland().digest()).into()),
            is_executable: false,
          },
          FileNode {
            name: "treats".to_owned(),
            digest: Some((&TestData::catnip().digest()).into()),
            is_executable: false,
          },
        ],
        directories: vec![],
      },
    }
  }

  // Directory structure:
  //
  // /cats/roland
  // /treats
  pub fn recursive() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        directories: vec![DirectoryNode {
          name: "cats".to_string(),
          digest: Some((&TestDirectory::containing_roland().digest()).into()),
        }],
        files: vec![FileNode {
          name: "treats".to_string(),
          digest: Some((&TestData::catnip().digest()).into()),
          is_executable: false,
        }],
      },
    }
  }

  // Directory structure:
  //
  // /feed (executable)
  // /food
  pub fn with_mixed_executable_files() -> TestDirectory {
    TestDirectory {
      directory: Directory {
        files: vec![
          FileNode {
            name: "feed".to_string(),
            digest: Some((&TestData::catnip().digest()).into()),
            is_executable: true,
          },
          FileNode {
            name: "food".to_string(),
            digest: Some((&TestData::catnip().digest()).into()),
            is_executable: false,
          },
        ],
        directories: vec![],
      },
    }
  }

  pub fn directory(&self) -> Directory {
    self.directory.clone()
  }

  pub fn bytes(&self) -> bytes::Bytes {
    let mut buf = bytes::BytesMut::with_capacity(self.directory.encoded_len());
    self
      .directory
      .encode(&mut buf)
      .expect("Error serializing proto");
    buf.freeze()
  }

  pub fn fingerprint(&self) -> hashing::Fingerprint {
    hash(&self.bytes())
  }

  pub fn digest(&self) -> hashing::Digest {
    hashing::Digest(self.fingerprint(), self.bytes().len())
  }
}

fn hash(bytes: &bytes::Bytes) -> hashing::Fingerprint {
  let mut hasher = sha2::Sha256::default();
  hasher.input(bytes);
  hashing::Fingerprint::from_bytes_unsafe(hasher.fixed_result().as_slice())
}
