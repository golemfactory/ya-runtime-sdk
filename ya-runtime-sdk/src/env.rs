use crate::cli::{parse_cli, CommandCli};
use std::path::PathBuf;

/// Runtime environment configuration
pub trait Env<C: CommandCli> {
    /// Runtime name. Return `None` for project name
    fn runtime_name(&self) -> Option<String> {
        None
    }

    /// Directory to store the configuration, caches and state at
    fn data_directory(&self, runtime_name: &str) -> anyhow::Result<PathBuf> {
        Ok(directories::ProjectDirs::from("", "", runtime_name)
            .map(|dirs| dirs.data_dir().into())
            .unwrap_or_else(|| PathBuf::from(runtime_name)))
    }

    /// Command line arguments
    fn args(&self) -> Box<dyn Iterator<Item = String>> {
        Box::new(std::env::args())
    }

    /// Parse command line arguments
    fn cli(&mut self, project_name: &str, project_version: &str) -> anyhow::Result<C> {
        let name = self
            .runtime_name()
            .unwrap_or_else(|| project_name.to_string());
        parse_cli(&name, project_version, self.args())
    }
}

/// Default runtime environment provider.
///
/// - data directory is located in the home user folder
/// - provides unaltered command line arguments
#[derive(Clone, Copy, Debug)]
pub struct DefaultEnv<C> {
    phantom: std::marker::PhantomData<C>,
}

impl<C> Default for DefaultEnv<C> {
    fn default() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<C: CommandCli> Env<C> for DefaultEnv<C> {}
