use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;

use futures::channel::mpsc;
use futures::future::{BoxFuture, LocalBoxFuture};
use futures::{Future, FutureExt, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::cli::CommandCli;
use crate::common::IntoVec;
use crate::env::{DefaultEnv, Env};
use crate::error::Error;
use crate::runtime_api::server::RuntimeEvent;
use crate::{KillProcess, ProcessStatus, RunProcess};

pub type ProcessId = u64;
pub type EmptyResponse<'a> = LocalBoxFuture<'a, Result<(), Error>>;
pub type OutputResponse<'a> = LocalBoxFuture<'a, Result<Option<serde_json::Value>, Error>>;
pub type ProcessIdResponse<'a> = LocalBoxFuture<'a, Result<ProcessId, Error>>;

/// Command handling interface for runtimes
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
        async move {
            Ok(Some(crate::serialize::json::json!({
                "constraints": "",
                "properties": {}
            })))
        }
        .boxed_local()
    }

    /// Perform a self-test
    fn test<'a>(&mut self, _ctx: &mut Context<Self>) -> EmptyResponse<'a> {
        async move { Ok(()) }.boxed_local()
    }
}

/// Defines the mode of execution for commands within the runtime.
#[derive(Clone, Copy, Debug)]
pub enum RuntimeMode {
    /// Server (blocking) mode
    /// Uses Runtime API to communicate with the ExeUnit Supervisor.
    /// `Command::Deploy` remains a one-shot command.
    Server,
    /// One-shot execution mode
    /// Each command is a separate invocation of the runtime binary.
    Command,
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
    /// Process ID sequence
    pid_seq: AtomicU64,
}

impl<R: Runtime + ?Sized> Context<R> {
    const CONF_EXTENSIONS: [&'static str; 4] = ["toml", "yaml", "yml", "json"];

    /// Create a new instance with a default environment configuration
    pub fn try_new() -> anyhow::Result<Self> {
        Self::try_with(DefaultEnv::default())
    }

    /// Create a new instance with provided environment configuration
    pub fn try_with<E: Env>(env: E) -> anyhow::Result<Self> {
        let app = <R as RuntimeDef>::Cli::clap()
            .name(R::NAME)
            .version(R::VERSION);

        let cli = <R as RuntimeDef>::Cli::from_clap(&app.get_matches_from(env.args()));

        let conf_dir = env.data_directory(R::NAME)?;
        let conf_path = Self::config_path(conf_dir)?;

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
            pid_seq: Default::default(),
        })
    }

    /// Read configuration from file
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

    /// Write configuration to file
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

    fn config_path<P: AsRef<Path>>(dir: P) -> anyhow::Result<PathBuf> {
        let dir = dir.as_ref();
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

    pub(crate) fn next_pid(&self) -> ProcessId {
        self.pid_seq.fetch_add(1, Relaxed)
    }

    pub(crate) fn set_emitter(&mut self, emitter: impl RuntimeEvent + Send + Sync + 'static) {
        self.emitter.replace(EventEmitter::spawn(emitter));
    }
}

pub trait RunCommandExt<R, F, T>
where
    R: Runtime + ?Sized,
    F: Future<Output = Result<(), Error>> + 'static,
    T: 'static,
{
    fn as_command<'a, H>(self, ctx: &mut Context<R>, handler: H) -> ProcessIdResponse<'a>
    where
        H: (FnOnce(T, RunCommandContext) -> F) + 'a;

    fn command<'a, H, Fi, Ei>(ctx: &mut Context<R>, fut: Fi, handler: H) -> ProcessIdResponse<'a>
    where
        H: (FnOnce(T, RunCommandContext) -> F) + 'a,
        Fi: Future<Output = Result<T, Ei>> + 'a,
        Error: From<Ei>,
    {
        let pid = ctx.next_pid();
        let mut cmd_ctx = RunCommandContext {
            id: pid,
            emitter: ctx.emitter.clone(),
        };

        async move {
            let val = fut.await?;
            cmd_ctx.started().await;

            let fut = handler(val, cmd_ctx.clone());
            tokio::task::spawn_local(async move {
                let return_code = fut.await.is_err() as i32;
                cmd_ctx.stopped(return_code).await;
            });

            Ok(pid)
        }
        .boxed_local()
    }
}

impl<R, F, T, E> RunCommandExt<R, F, T> for Result<T, E>
where
    R: Runtime + ?Sized,
    F: Future<Output = Result<(), Error>> + 'static,
    T: 'static,
    E: 'static,
    Error: From<E>,
{
    fn as_command<'a, H>(self, ctx: &mut Context<R>, handler: H) -> ProcessIdResponse<'a>
    where
        H: (FnOnce(T, RunCommandContext) -> F) + 'a,
    {
        Self::command(ctx, async move { self }, handler)
    }
}

/// Command execution handler
#[derive(Clone)]
pub struct RunCommandContext {
    id: ProcessId,
    emitter: Option<EventEmitter>,
}

impl RunCommandContext {
    /// Get command ID
    pub fn id(&self) -> &ProcessId {
        &self.id
    }

    fn started(&mut self) -> BoxFuture<()> {
        let id = self.id;
        self.emitter
            .as_mut()
            .map(|e| e.command_started(id))
            .unwrap_or_else(|| futures::future::ready(()).boxed())
    }

    fn stopped(&mut self, return_code: i32) -> BoxFuture<()> {
        let id = self.id;
        self.emitter
            .as_mut()
            .map(|e| e.command_stopped(id, return_code))
            .unwrap_or_else(|| futures::future::ready(()).boxed())
    }

    /// Emit a RUN command output event (stdout)
    pub fn stdout(&mut self, output: impl IntoVec<u8>) -> BoxFuture<()> {
        let id = self.id;
        let output = output.into_vec();
        match self.emitter {
            Some(ref mut e) => e.command_stdout(id, output),
            None => Self::print_output(output),
        }
    }

    /// Emit a RUN command output event (stderr)
    pub fn stderr(&mut self, output: impl IntoVec<u8>) -> BoxFuture<()> {
        let id = self.id;
        let output = output.into_vec();
        match self.emitter {
            Some(ref mut e) => e.command_stderr(id, output),
            None => Self::print_output(output),
        }
    }

    fn print_output<'a>(output: impl IntoVec<u8>) -> BoxFuture<'a, ()> {
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(output.into_vec().as_slice());
        let _ = stdout.flush();
        futures::future::ready(()).boxed()
    }
}

/// Runtime event emitter
#[derive(Clone)]
pub struct EventEmitter {
    tx: mpsc::Sender<ProcessStatus>,
}

impl EventEmitter {
    pub fn spawn(emitter: impl RuntimeEvent + Send + Sync + 'static) -> Self {
        let (tx, rx) = mpsc::channel(1);
        tokio::task::spawn(rx.for_each(move |status| emitter.on_process_status(status)));
        Self { tx }
    }
}

impl EventEmitter {
    /// Emit a command started event
    pub fn command_started(&mut self, process_id: ProcessId) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: Default::default(),
            stderr: Default::default(),
        })
    }

    /// Emit a command stopped event
    pub fn command_stopped(&mut self, process_id: ProcessId, return_code: i32) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: false,
            return_code,
            stdout: Default::default(),
            stderr: Default::default(),
        })
    }

    /// Emit a command output event (stdout)
    pub fn command_stdout(
        &mut self,
        process_id: ProcessId,
        stdout: impl IntoVec<u8>,
    ) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: stdout.into_vec(),
            stderr: Default::default(),
        })
    }

    /// Emit a command output event (stderr)
    pub fn command_stderr(
        &mut self,
        process_id: ProcessId,
        stderr: impl IntoVec<u8>,
    ) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: Default::default(),
            stderr: stderr.into_vec(),
        })
    }

    /// Emit an event
    pub fn emit(&mut self, status: ProcessStatus) -> BoxFuture<()> {
        self.tx.send(status).then(|_| async {}).boxed()
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
