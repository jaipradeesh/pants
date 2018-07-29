use build::bazel::remote::execution::v2 as remote_execution;
use hashing;

impl<'a> From<&'a hashing::Digest> for remote_execution::Digest {
  fn from(d: &hashing::Digest) -> Self {
    remote_execution::Digest {
      hash: d.0.to_hex(),
      size_bytes: d.1 as i64,
    }
  }
}

impl<'a> From<&'a remote_execution::Digest> for Result<hashing::Digest, String> {
  fn from(d: &remote_execution::Digest) -> Self {
    hashing::Fingerprint::from_hex_string(&d.hash)
      .map_err(|err| format!("Bad fingerprint in Digest {:?}: {:?}", d.hash, err))
      .map(|fingerprint| hashing::Digest(fingerprint, d.size_bytes as usize))
  }
}

#[cfg(test)]
mod tests {
  use hashing;

  #[test]
  fn from_our_digest() {
    let our_digest = &hashing::Digest(
      hashing::Fingerprint::from_hex_string(
        "0123456789abcdeffedcba98765432100000000000000000ffffffffffffffff",
      ).unwrap(),
      10,
    );
    let converted: super::remote_execution::Digest = our_digest.into();
    let want = super::remote_execution::Digest {
      hash: "0123456789abcdeffedcba98765432100000000000000000ffffffffffffffff".to_owned(),
      size_bytes: 10,
    };
    assert_eq!(converted, want);
  }

  #[test]
  fn from_bazel_digest() {
    let bazel_digest = super::remote_execution::Digest {
      hash: "0123456789abcdeffedcba98765432100000000000000000ffffffffffffffff".to_owned(),
      size_bytes: 10,
    };
    let converted: Result<hashing::Digest, String> = (&bazel_digest).into();
    let want = hashing::Digest(
      hashing::Fingerprint::from_hex_string(
        "0123456789abcdeffedcba98765432100000000000000000ffffffffffffffff",
      ).unwrap(),
      10,
    );
    assert_eq!(converted, Ok(want));
  }

  #[test]
  fn from_bad_bazel_digest() {
    let bazel_digest = super::remote_execution::Digest {
      hash: "0".to_owned(),
      size_bytes: 10,
    };
    let converted: Result<hashing::Digest, String> = (&bazel_digest).into();
    let err = converted.expect_err("Want Err converting bad digest");
    assert!(
      err.starts_with("Bad fingerprint in Digest \"0\":"),
      "Bad error message: {}",
      err
    );
  }
}
