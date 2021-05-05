pub mod cli;
pub mod error;
pub mod runner;
pub mod serialize;
mod server;
mod service;

pub use ya_runtime_api::server::{
    ErrorResponse, KillProcess, ProcessStatus, RunProcess, RuntimeEvent,
};

pub use cli::Command;
pub use runner::{run, ServiceMode};
pub use service::*;

#[cfg(feature = "macros")]
#[allow(unused_imports)]
#[macro_use]
extern crate ya_service_sdk_derive;
#[cfg(feature = "macros")]
#[doc(hidden)]
pub use ya_service_sdk_derive::*;

#[cfg(feature = "macros")]
#[macro_export]
macro_rules! main {
    ($ty:ty) => {
        #[tokio::main]
        async fn main() -> anyhow::Result<()> {
            crate::run::<$ty>().await
        }
    };
}
