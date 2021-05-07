use crate::cli::CommandCli;
use crate::error::Error;
use crate::runner::ServiceMode;
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
pub trait Service: ServiceDef + Default {
    const MODE: ServiceMode = ServiceMode::Server;

    /// Deploy and configure the service
    fn deploy<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a>;

    /// Start the service
    fn start<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a>;

    /// Stop the service
    fn stop<'a>(&mut self, ctx: &mut Context<Self>) -> EmptyResponse<'a>;

    /// Start a service command
    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: ServiceMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a>;

    /// Stop service command execution
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

/// Service definition trait.
/// Auto-generated via `#[derive(ServiceDef)]`
pub trait ServiceDef {
    const NAME: &'static str;
    const VERSION: &'static str;

    type Cli: CommandCli;
    type Conf: Default + Serialize + for<'de> Deserialize<'de>;
}

/// Service execution context
pub struct Context<Svc: Service + ?Sized> {
    /// Command line parameters
    pub cli: <Svc as ServiceDef>::Cli,
    /// Configuration read from the configuration file
    pub conf: <Svc as ServiceDef>::Conf,
    /// Configuration file path
    pub conf_path: PathBuf,
    /// Event emitter, available when
    /// `Service::MODE == ServiceMode::Server`
    /// and
    /// `command != Command::Deploy`
    pub emitter: Option<EventEmitter>,
}

impl<Svc: Service + ?Sized> Clone for Context<Svc>
where
    <Svc as ServiceDef>::Cli: Clone,
    <Svc as ServiceDef>::Conf: Clone,
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

impl<Svc: Service + ?Sized> Context<Svc> {
    const CONF_EXTENSIONS: [&'static str; 3] = ["toml", "yaml", "json"];

    pub fn try_new() -> anyhow::Result<Self> {
        let app = <Svc as ServiceDef>::Cli::clap()
            .name(Svc::NAME)
            .version(Svc::VERSION);
        let cli = <Svc as ServiceDef>::Cli::from_clap(&app.get_matches());

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

    pub fn read_config<P: AsRef<Path>>(path: P) -> anyhow::Result<<Svc as ServiceDef>::Conf> {
        use anyhow::Context;

        let path = path.as_ref();
        let extension = file_extension(path)?;
        let err = || format!("Unable to read the configuration file: {}", path.display());

        let contents = std::fs::read_to_string(path).with_context(err)?;
        let conf: <Svc as ServiceDef>::Conf = match extension.as_str() {
            "toml" => toml::de::from_str(&contents).with_context(err)?,
            "yaml" => serde_yaml::from_str(&contents).with_context(err)?,
            "json" => serde_json::from_str(&contents).with_context(err)?,
            _ => anyhow::bail!("Unsupported extension: {}", extension),
        };

        Ok(conf)
    }

    pub fn write_config<P: AsRef<Path>>(
        conf: &<Svc as ServiceDef>::Conf,
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
            "yaml" => serde_yaml::to_string(conf).with_context(err)?,
            "json" => serde_json::to_string_pretty(conf).with_context(err)?,
            _ => anyhow::bail!("Unsupported extension: {}", extension),
        };
        std::fs::write(path, contents).with_context(err)?;

        Ok(())
    }

    pub fn config_path() -> anyhow::Result<PathBuf> {
        const ORGANIZATION: &'static str = "GolemFactory";

        let dir = directories::ProjectDirs::from("", ORGANIZATION, Svc::NAME)
            .map(|dirs| dirs.data_dir().into())
            .unwrap_or_else(|| PathBuf::from(ORGANIZATION).join(Svc::NAME));
        let candidates = Self::CONF_EXTENSIONS
            .iter()
            .map(|ext| dir.join(format!("{}.{}", Svc::NAME, ext)))
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
    // Functions below return futures
    // in preparation for changes in the Runtime API

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
        self.inner.on_process_status(status);
        async move { () }.boxed_local()
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
