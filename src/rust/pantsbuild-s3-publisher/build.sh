#!/bin/bash -eu

repo_root="$(git rev-parse --show-toplevel)"
publisher_root="${repo_root}/src/rust/pantsbuild-s3-publisher"

"${repo_root}/build-support/bin/native/cargo" build --release --target x86_64-unknown-linux-musl --manifest-path="${publisher_root}/Cargo.toml"
zip -j "${publisher_root}/deploy.zip" "${publisher_root}/target/x86_64-unknown-linux-musl/release/bootstrap"
