use crate::cli::{Command, CommandCli};
use crate::server::Server;
use crate::service::{Context, Service};
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use std::io;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use ya_runtime_api::server::proto::{output::Type, request::RunProcess, Output};

/// Returns a parent directory of this binary.
#[allow(unused)]
pub fn exe_dir() -> io::Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?
        .to_path_buf())
}

/// Defines the mode of execution for commands within the service.
#[derive(Clone, Copy, Debug)]
pub enum ServiceMode {
    /// Server (blocking) mode
    /// Uses Runtime API to communicate with the ExeUnit Supervisor.
    /// `Command::Deploy` remains a one-shot command.
    Server,
    /// One-shot execution mode
    /// Each command is a separate invocation of the service binary.
    Command,
}

pub struct ServiceRunner<Svc: Service> {
    phantom: std::marker::PhantomData<Svc>,
}

impl<Svc: Service + 'static> ServiceRunner<Svc> {
    pub fn run<'a>() -> LocalBoxFuture<'a, anyhow::Result<()>>
    where
        Svc: 'a,
    {
        async move {
            let mut ctx = Context::<Svc>::try_new()?;

            match ctx.cli.command().clone() {
                Command::Deploy { args: _ } => {
                    let mut service = Svc::default();
                    let deployment = service.deploy(&mut ctx).await?;
                    output(deployment).await?;
                }
                Command::Start { args: _ } => match Svc::MODE {
                    ServiceMode::Command => {
                        let mut service = Svc::default();
                        let started = service.start(&mut ctx).await?;
                        output(started).await?;
                    }
                    ServiceMode::Server => {
                        // `run_async` accepts `Fn`, thus outer variable capturing is not possible
                        // FIXME: refactor `Fn` to `FnMut` in Runtime API
                        ya_runtime_api::server::run_async(|emitter| async move {
                            let mut service = Svc::default();
                            let mut ctx = Context::<Svc>::try_new().unwrap();

                            let start = {
                                ctx.set_emitter(Box::new(emitter));
                                service.start(&mut ctx)
                            };
                            start.await.expect("Failed to start the service");

                            Server::new(service, ctx)
                        })
                        .await;
                    }
                },
                Command::Run { args } => {
                    if args.len() < 1 {
                        anyhow::bail!("not enough arguments");
                    }

                    let capture = Some(Output {
                        r#type: Some(Type::AtEnd(40960)),
                    });
                    let command = RunProcess {
                        bin: args.get(0).cloned().unwrap(),
                        args: args.iter().skip(1).cloned().collect(),
                        work_dir: ctx.cli.workdir().unwrap().display().to_string(),
                        stdout: capture.clone(),
                        stderr: capture,
                    };

                    let mut service = Svc::default();
                    let pid = service
                        .run_command(command, ServiceMode::Command, &mut ctx)
                        .await?;

                    output(serde_json::json!(pid)).await?;
                }
                Command::OfferTemplate { args: _ } => {
                    let mut service = Svc::default();
                    let template = service.offer(&mut ctx).await?;
                    output(template).await?;
                }
                Command::Test { args: _ } => {
                    let mut service = Svc::default();
                    service.test(&mut ctx).await?
                }
            }

            Ok(())
        }
        .boxed_local()
    }
}

async fn output(json: serde_json::Value) -> anyhow::Result<()> {
    let string = json.to_string();
    let mut stdout = tokio::io::stdout();
    stdout.write_all(string.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}
