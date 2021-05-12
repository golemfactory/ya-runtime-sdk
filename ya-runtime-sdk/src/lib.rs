pub mod cli;
pub mod error;
pub mod runner;
mod runtime;
pub mod serialize;
mod server;

pub use ya_runtime_api as runtime_api;
pub use ya_runtime_api::server::{
    ErrorResponse, KillProcess, ProcessStatus, RunProcess, RuntimeEvent,
};

pub use cli::Command;
pub use runner::{run, RuntimeMode};
pub use runtime::*;
pub use server::Server;

#[cfg(feature = "macros")]
#[allow(unused_imports)]
#[macro_use]
extern crate ya_runtime_sdk_derive;
#[cfg(feature = "macros")]
#[doc(hidden)]
pub use ya_runtime_sdk_derive::*;

#[cfg(feature = "macros")]
#[macro_export]
macro_rules! main {
    ($ty:ty) => {
        #[tokio::main]
        async fn main() -> anyhow::Result<()> {
            ya_runtime_sdk::run::<$ty>().await
        }
    };
}
