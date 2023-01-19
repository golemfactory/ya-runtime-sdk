use futures::channel::oneshot;
use futures::future::BoxFuture;
use futures::FutureExt;
use serde::Serialize;
use std::cell::RefCell;
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;

use ya_runtime_api::server::{RuntimeCounter, RuntimeHandler, RuntimeState};

use crate::common::{write_output, IntoVec};
use crate::env::{DefaultEnv, Env};
use crate::error::Error;
use crate::event::EventEmitter;
use crate::runtime::{ProcessId, ProcessIdResponse};
use crate::runtime::{Runtime, RuntimeControl, RuntimeDef};
use crate::serialize::json;
use crate::RuntimeMode;

/// Runtime execution context
pub struct Context<R: Runtime + ?Sized> {
    /// Command line parameters
    pub cli: <R as RuntimeDef>::Cli,
    /// Configuration read from the configuration file
    pub conf: <R as RuntimeDef>::Conf,
    /// Configuration file path
    pub conf_path: PathBuf,
    /// Environment instance
    pub env: Box<dyn Env<<R as RuntimeDef>::Cli>>,
    /// Event emitter, available when
    /// `Runtime::MODE == RuntimeMode::Server`
    /// and
    /// `command != Command::Deploy`
    pub emitter: Option<EventEmitter>,
    /// Process ID sequence
    pid_seq: AtomicU64,
    /// Runtime control
    pub(crate) control: RuntimeControl,
}

impl<R> Context<R>
where
    R: Runtime + ?Sized,
    <R as RuntimeDef>::Cli: 'static,
{
    const CONF_EXTENSIONS: [&'static str; 4] = ["toml", "yaml", "yml", "json"];

    /// Create a new instance with a default environment configuration
    pub fn try_new() -> anyhow::Result<Self> {
        Self::try_with(DefaultEnv::default())
    }

    /// Create a new instance with provided environment configuration
    pub fn try_with<E>(mut env: E) -> anyhow::Result<Self>
    where
        E: Env<<R as RuntimeDef>::Cli> + 'static,
    {
        let cli = env.cli(R::NAME, R::VERSION)?;
        let name = env.runtime_name().unwrap_or_else(|| R::NAME.to_string());
        let conf_dir = env.data_directory(name.as_str())?;
        let conf_path = Self::config_path(conf_dir, name.as_str())?;

        let conf = if conf_path.exists() {
            Self::read_config(&conf_path)?
        } else {
            Default::default()
        };

        Ok(Self {
            cli,
            conf,
            conf_path,
            env: Box::new(env),
            emitter: None,
            pid_seq: Default::default(),
            control: Default::default(),
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
            std::fs::create_dir_all(parent_dir).with_context(err)?;
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

    /// Return a runtime control object
    pub fn control(&self) -> RuntimeControl {
        self.control.clone()
    }

    fn config_path<P: AsRef<Path>>(dir: P, name: &str) -> anyhow::Result<PathBuf> {
        let dir = dir.as_ref();
        let candidates = Self::CONF_EXTENSIONS
            .iter()
            .map(|ext| dir.join(format!("{}.{}", name, ext)))
            .collect::<Vec<_>>();
        let conf_path = candidates
            .iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| candidates.last().unwrap())
            .clone();

        Ok(conf_path)
    }

    pub(crate) fn next_run_ctx(&self) -> RunCommandContext {
        let id = self.pid_seq.fetch_add(1, Relaxed);
        RunCommandContext {
            id,
            emitter: self.emitter.clone(),
            control: self.control.clone(),
        }
    }

    pub(crate) fn set_emitter(&mut self, emitter: impl RuntimeHandler + Send + Sync + 'static) {
        self.emitter.replace(EventEmitter::spawn(emitter));
    }

    pub(crate) fn set_shutdown_tx(&mut self, tx: oneshot::Sender<()>) {
        self.control.shutdown_tx = Rc::new(RefCell::new(Some(tx)));
    }
}

impl<R> Context<R>
where
    R: Runtime + ?Sized,
    <R as RuntimeDef>::Cli: 'static,
{
    pub fn command<'a, H, T, Fut>(&mut self, handler: H) -> ProcessIdResponse<'a>
    where
        H: (FnOnce(RunCommandContext) -> Fut) + 'static,
        T: Serialize,
        Fut: Future<Output = Result<T, Error>> + 'a,
    {
        let run_ctx = self.next_run_ctx();
        run_command(run_ctx, move |run_ctx| {
            async move {
                let id = run_ctx.id;
                let emitter = run_ctx.emitter.clone();
                let output = handler(run_ctx).await?;
                let value = json::to_value(&output).map_err(Error::from_string)?;

                if value.is_null() {
                    return Ok(());
                }

                match R::MODE {
                    RuntimeMode::Command => {
                        let _ = write_output(value).await;
                    }
                    RuntimeMode::Server if emitter.is_some() => {
                        emitter.unwrap().command_stdout(id, value.to_string()).await;
                    }
                    RuntimeMode::Server => (),
                }
                Ok(())
            }
            .boxed_local()
        })
    }
}

/// Command execution handler
#[derive(Clone)]
pub struct RunCommandContext {
    pub(crate) id: ProcessId,
    pub(crate) emitter: Option<EventEmitter>,
    pub(crate) control: RuntimeControl,
}

impl RunCommandContext {
    /// Get command ID
    pub fn id(&self) -> &ProcessId {
        &self.id
    }

    pub(crate) fn started(&mut self) -> BoxFuture<()> {
        let id = self.id;
        self.emitter
            .as_mut()
            .map(|e| e.command_started(id))
            .unwrap_or_else(|| futures::future::ready(()).boxed())
    }

    pub(crate) fn stopped(&mut self, return_code: i32) -> BoxFuture<()> {
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

    /// Emit a STATE event
    pub fn state(&mut self, name: String, value: json::Value) -> BoxFuture<Result<(), Error>> {
        match self.emitter {
            Some(ref mut e) => async move {
                let json_str = json::to_string(&value)
                    .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;
                let json_bytes = json_str.into_bytes();

                e.state(RuntimeState {
                    name,
                    value: json_bytes,
                })
                .await;

                Ok(())
            }
            .boxed(),
            None => futures::future::ok(()).boxed(),
        }
    }

    /// Emit a COUNTER event
    pub fn counter(&mut self, name: String, value: f64) -> BoxFuture<()> {
        match self.emitter {
            Some(ref mut e) => e.counter(RuntimeCounter { name, value }),
            None => futures::future::ready(()).boxed(),
        }
    }

    /// Return runtime control object
    pub fn control(&self) -> RuntimeControl {
        self.control.clone()
    }

    fn print_output<'a>(output: impl IntoVec<u8>) -> BoxFuture<'a, ()> {
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(output.into_vec().as_slice());
        let _ = stdout.flush();
        futures::future::ready(()).boxed()
    }
}

/// Wraps command lifecycle in the following manner:
/// - manages command sequence numbers
/// - emits command start & stop events
/// - provides a RunCommandContext object for easier output event emission
pub trait RunCommandExt<R: Runtime + ?Sized> {
    type Item: 'static;

    #[allow(clippy::wrong_self_convention)]
    /// Wrap `self` in `run_command`
    fn as_command<'a, H, Fh>(self, ctx: &mut Context<R>, handler: H) -> ProcessIdResponse<'a>
    where
        H: (FnOnce(Self::Item, RunCommandContext) -> Fh) + 'static,
        Fh: Future<Output = Result<(), Error>> + 'static;
}

/// Implements `RunCommandExt` for `Future`s outputting `Result`s.
/// The output result is checked prior to emitting any command lifecycle events.
impl<R, F, Rt, Re> RunCommandExt<R> for F
where
    R: Runtime + ?Sized,
    <R as RuntimeDef>::Cli: 'static,
    F: Future<Output = Result<Rt, Re>> + 'static,
    Rt: 'static,
    Re: 'static,
    Error: From<Re>,
{
    type Item = Rt;

    fn as_command<'a, H, Fh>(self, ctx: &mut Context<R>, handler: H) -> ProcessIdResponse<'a>
    where
        H: (FnOnce(Self::Item, RunCommandContext) -> Fh) + 'static,
        Fh: Future<Output = Result<(), Error>> + 'static,
    {
        let run_ctx = ctx.next_run_ctx();
        async move {
            let value = self.await?;
            run_command(run_ctx, move |run_ctx| async move {
                handler(value, run_ctx).await
            })
            .await
        }
        .boxed_local()
    }
}

fn run_command<'a, H, F>(mut run_ctx: RunCommandContext, handler: H) -> ProcessIdResponse<'a>
where
    H: (FnOnce(RunCommandContext) -> F) + 'static,
    F: Future<Output = Result<(), Error>> + 'static,
{
    async move {
        let pid = run_ctx.id;
        run_ctx.started().await;

        let fut = handler(run_ctx.clone());
        tokio::task::spawn_local(async move {
            let return_code = fut.await.is_err() as i32;
            run_ctx.stopped(return_code).await;
        });

        Ok(pid)
    }
    .boxed_local()
}

fn file_extension<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    Ok(path
        .as_ref()
        .extension()
        .ok_or_else(|| anyhow::anyhow!("Invalid config path"))?
        .to_string_lossy()
        .to_lowercase())
}
