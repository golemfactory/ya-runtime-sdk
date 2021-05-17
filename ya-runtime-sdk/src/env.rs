use std::path::PathBuf;

const ORGANIZATION: &'static str = "GolemFactory";

/// Runtime environment configuration
pub trait Env {
    /// Directory to store the configuration, caches and state at
    fn data_directory(&self, runtime_name: &str) -> anyhow::Result<PathBuf>;

    /// Command line arguments
    fn args(&self) -> Box<dyn Iterator<Item = String>> {
        Box::new(std::env::args())
    }
}

/// Default runtime environment provider.
///
/// - data directory is located in the home user folder
/// - provides unaltered command line arguments
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultEnv;

impl Env for DefaultEnv {
    fn data_directory(&self, runtime_name: &str) -> anyhow::Result<PathBuf> {
        Ok(
            directories::ProjectDirs::from("", ORGANIZATION, runtime_name)
                .map(|dirs| dirs.data_dir().into())
                .unwrap_or_else(|| PathBuf::from(ORGANIZATION).join(runtime_name)),
        )
    }
}
