extern crate bytes;
extern crate hashing;
extern crate prost;
#[macro_use]
extern crate prost_derive;
extern crate prost_types;
extern crate tower_grpc;

mod gen {
    pub mod build {
        pub mod bazel {
            pub mod remote {
                pub mod execution {
                    pub mod v2;
                }
            }
        }
    }

    pub mod google {
        pub mod api;
        pub mod longrunning;
        pub mod protobuf {
            pub type Empty = ();
        }
        pub mod rpc;
    }
}

pub use gen::*;

mod conversions;
mod verification;
pub use verification::verify_directory_canonical;
