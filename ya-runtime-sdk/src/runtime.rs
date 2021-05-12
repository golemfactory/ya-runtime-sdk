use crate::cli::CommandCli;
use crate::error::Error;
use crate::runner::RuntimeMode;
use crate::{KillProcess, ProcessStatus, RunProcess, RuntimeEvent};
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use structopt::StructOpt;

pub type ProcessId = u64;
pub type EmptyResponse<'a> = LocalBoxFuture<'a, Result<(), Error>>;
pub type OutputResponse<'a> = LocalBoxFuture<'a, Result<serde_json::Value, Error>>;
pub type ProcessIdResponse<'a> = LocalBoxFuture<'a, Result<ProcessId, Error>>;

/// Command handler interface.
pub trait Runtime: RuntimeDef + Default {
    const MODE: RuntimeMode = RuntimeMode::Server;

    /// Deploy and configure the runtime
    fn deploy<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a>;

    /// Start the runtime
    fn start<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a>;

    /// Stop the runtime
    fn stop<'a>(&mut self, ctx: &mut Context<Self>) -> EmptyResponse<'a>;

    /// Start a runtime command
    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a>;

    /// Stop runtime command execution
    fn kill_command<'a>(
        &mut self,
        _kill: KillProcess,
        _ctx: &mut Context<Self>,
    ) -> EmptyResponse<'a> {
        async move { Err(Error::from_string("Not supported")) }.boxed_local()
    }

    /// Output a market Offer template stub
    fn offer<'a>(&mut self, _ctx: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok(serde_json::Value::default()) }.boxed_local()
    }

    /// Perform a self-test
    fn test<'a>(&mut self, _ctx: &mut Context<Self>) -> EmptyResponse<'a> {
        async move { Ok(()) }.boxed_local()
    }
}

/// Runtime definition trait.
/// Auto-generated via `#[derive(RuntimeDef)]`
pub trait RuntimeDef {
    const NAME: &'static str;
    const VERSION: &'static str;

    type Cli: CommandCli;
    type Conf: Default + Serialize + for<'de> Deserialize<'de>;
}

/// Runtime execution context
pub struct Context<R: Runtime + ?Sized> {
    /// Command line parameters
    pub cli: <R as RuntimeDef>::Cli,
    /// Configuration read from the configuration file
    pub conf: <R as RuntimeDef>::Conf,
    /// Configuration file path
    pub conf_path: PathBuf,
    /// Event emitter, available when
    /// `Runtime::MODE == RuntimeMode::Server`
    /// and
    /// `command != Command::Deploy`
    pub emitter: Option<EventEmitter>,
}

impl<R: Runtime + ?Sized> Clone for Context<R>
where
    <R as RuntimeDef>::Cli: Clone,
    <R as RuntimeDef>::Conf: Clone,
{
    fn clone(&self) -> Self {
        Context {
            cli: self.cli.clone(),
            conf: self.conf.clone(),
            conf_path: self.conf_path.clone(),
            emitter: self.emitter.clone(),
        }
    }
}

impl<R: Runtime + ?Sized> Context<R> {
    const CONF_EXTENSIONS: [&'static str; 4] = ["toml", "yaml", "yml", "json"];

    pub fn try_new() -> anyhow::Result<Self> {
        let app = <R as RuntimeDef>::Cli::clap()
            .name(R::NAME)
            .version(R::VERSION);
        let cli = <R as RuntimeDef>::Cli::from_clap(&app.get_matches());

        let conf_path = Self::config_path()?;
        let conf = if conf_path.exists() {
            Self::read_config(&conf_path)?
        } else {
            let conf = Default::default();
            Self::write_config(&conf, &conf_path)?;
            conf
        };

        Ok(Self {
            cli,
            conf,
            conf_path,
            emitter: None,
        })
    }

    pub fn read_config<P: AsRef<Path>>(path: P) -> anyhow::Result<<R as RuntimeDef>::Conf> {
        use anyhow::Context;

        let path = path.as_ref();
        let extension = file_extension(path)?;
        let err = || format!("Unable to read the configuration file: {}", path.display());

        let contents = std::fs::read_to_string(path).with_context(err)?;
        let conf: <R as RuntimeDef>::Conf = match extension.as_str() {
            "toml" => toml::de::from_str(&contents).with_context(err)?,
            "yaml" | "yml" => serde_yaml::from_str(&contents).with_context(err)?,
            "json" => serde_json::from_str(&contents).with_context(err)?,
            _ => anyhow::bail!("Unsupported extension: {}", extension),
        };

        Ok(conf)
    }

    pub fn write_config<P: AsRef<Path>>(
        conf: &<R as RuntimeDef>::Conf,
        path: P,
    ) -> anyhow::Result<()> {
        use anyhow::Context;

        let path = path.as_ref();
        let extension = file_extension(path)?;
        let err = || format!("Unable to write configuration: {}", path.display());

        let parent_dir = path.parent().ok_or_else(|| {
            anyhow::anyhow!("Unable to resolve parent directory of {}", path.display())
        })?;
        if !parent_dir.exists() {
            std::fs::create_dir_all(&parent_dir).with_context(err)?;
        }

        let contents = match extension.as_str() {
            "toml" => toml::ser::to_string_pretty(conf).with_context(err)?,
            "yaml" | "yml" => serde_yaml::to_string(conf).with_context(err)?,
            "json" => serde_json::to_string_pretty(conf).with_context(err)?,
            _ => anyhow::bail!("Unsupported extension: {}", extension),
        };
        std::fs::write(path, contents).with_context(err)?;

        Ok(())
    }

    pub fn config_path() -> anyhow::Result<PathBuf> {
        const ORGANIZATION: &'static str = "GolemFactory";

        let dir = directories::ProjectDirs::from("", ORGANIZATION, R::NAME)
            .map(|dirs| dirs.data_dir().into())
            .unwrap_or_else(|| PathBuf::from(ORGANIZATION).join(R::NAME));
        let candidates = Self::CONF_EXTENSIONS
            .iter()
            .map(|ext| dir.join(format!("{}.{}", R::NAME, ext)))
            .collect::<Vec<_>>();
        let conf_path = candidates
            .iter()
            .filter(|path| path.exists())
            .next()
            .unwrap_or_else(|| candidates.last().unwrap())
            .clone();

        Ok(conf_path)
    }

    pub(crate) fn set_emitter(&mut self, emitter: Box<dyn RuntimeEvent + Send + Sync>) {
        self.emitter.replace(EventEmitter::new(emitter));
    }
}

#[derive(Clone)]
pub struct EventEmitter {
    inner: Arc<Box<dyn RuntimeEvent + Send + Sync>>,
}

impl EventEmitter {
    pub fn new(emitter: Box<dyn RuntimeEvent + Send + Sync>) -> Self {
        Self {
            inner: Arc::new(emitter),
        }
    }
}

impl EventEmitter {
    /// Emit a command started event
    pub fn command_started<'a>(&self, process_id: ProcessId) -> LocalBoxFuture<'a, ()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: Default::default(),
            stderr: Default::default(),
        })
    }

    /// Emit a command stopped event
    pub fn command_stopped<'a>(
        &self,
        process_id: ProcessId,
        return_code: i32,
    ) -> LocalBoxFuture<'a, ()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: false,
            return_code,
            stdout: Default::default(),
            stderr: Default::default(),
        })
    }

    /// Emit a command output event (stdout)
    pub fn command_stdout<'a>(
        &self,
        process_id: ProcessId,
        stdout: Vec<u8>,
    ) -> LocalBoxFuture<'a, ()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout,
            stderr: Default::default(),
        })
    }

    /// Emit a command output event (stderr)
    pub fn command_stderr<'a>(
        &self,
        process_id: ProcessId,
        stderr: Vec<u8>,
    ) -> LocalBoxFuture<'a, ()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: Default::default(),
            stderr,
        })
    }

    /// Emit an event
    pub fn emit<'a>(&self, status: ProcessStatus) -> LocalBoxFuture<'a, ()> {
        self.inner.on_process_status(status).boxed_local()
    }
}

fn file_extension<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    Ok(path
        .as_ref()
        .extension()
        .ok_or_else(|| anyhow::anyhow!("Invalid config path"))?
        .to_string_lossy()
        .to_lowercase())
}