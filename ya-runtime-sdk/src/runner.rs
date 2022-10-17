use futures::future::LocalBoxFuture;
use futures::FutureExt;
use std::future::Future;

use ya_runtime_api::server::proto::{output::Type, request::RunProcess, Output};

use crate::cli::{Command, CommandCli};
use crate::common::write_output;
use crate::context::Context;
use crate::env::{DefaultEnv, Env};
use crate::runtime::{Runtime, RuntimeDef, RuntimeMode};
use crate::server::Server;

/// Starts the runtime within a new `tokio::task::LocalSet`
#[inline]
pub async fn run<R>() -> anyhow::Result<()>
where
    R: Runtime + Default + 'static,
{
    run_with::<R, _>(DefaultEnv::<<R as RuntimeDef>::Cli>::default()).await
}

/// Starts the runtime within a new `tokio::task::LocalSet`,
/// using a custom environment configuration provider
pub async fn run_with<R, E>(env: E) -> anyhow::Result<()>
where
    R: Runtime + Default + 'static,
    E: Env<<R as RuntimeDef>::Cli> + Send + 'static,
{
    build(env, move |_| async move { Ok(R::default()) }).await
}

/// Creates a new runtime execution future within a new `tokio::task::LocalSet`,
/// using a custom environment configuration provider and a runtime factory
pub fn build<R, E, F, Fut>(env: E, factory: F) -> LocalBoxFuture<'static, anyhow::Result<()>>
where
    R: Runtime + 'static,
    E: Env<<R as RuntimeDef>::Cli> + Send + 'static,
    F: FnOnce(&mut Context<R>) -> Fut + 'static,
    Fut: Future<Output = anyhow::Result<R>> + 'static,
{
    async move {
        let set = tokio::task::LocalSet::new();
        set.run_until(inner::<R, E, _>(env, |ctx| {
            async move { factory(ctx).await }.boxed_local()
        }))
        .await
    }
    .boxed_local()
}

async fn inner<R, E, F>(env: E, factory: F) -> anyhow::Result<()>
where
    R: Runtime + 'static,
    E: Env<<R as RuntimeDef>::Cli> + Send + 'static,
    F: FnOnce(&mut Context<R>) -> LocalBoxFuture<anyhow::Result<R>>,
{
    #[cfg(feature = "logger")]
    {
        if let Err(error) = crate::logger::start_file_logger() {
            crate::logger::start_logger().expect("Failed to start logging");
            log::warn!("Using fallback logging due to an error: {:?}", error);
        };

        std::panic::set_hook(Box::new(|e| {
            log::error!("Runtime panic: {e}");
        }));
    }

    let mut ctx = Context::<R>::try_with(env)?;
    let mut runtime = factory(&mut ctx).await?;

    match ctx.cli.command() {
        Command::Deploy { .. } => {
            let deployment = match runtime.deploy(&mut ctx).await? {
                Some(deployment) => deployment,
                None => {
                    crate::serialize::json::json!({
                        "startMode": match R::MODE {
                            RuntimeMode::Server => "blocking",
                            RuntimeMode::Command => "empty",
                        },
                        "valid": {"Ok": ""},
                        "vols": []
                    })
                }
            };
            write_output(deployment).await?;
        }
        Command::Start { .. } => match R::MODE {
            RuntimeMode::Command => {
                if let Some(started) = runtime.start(&mut ctx).await? {
                    write_output(started).await?;
                }
            }
            RuntimeMode::Server => {
                ya_runtime_api::server::run_async(|emitter| async move {
                    let start = {
                        ctx.set_emitter(emitter);
                        runtime.start(&mut ctx)
                    };

                    match start.await {
                        Ok(Some(out)) => {
                            ctx.next_run_ctx().stdout(out.to_string()).await;
                        }
                        Err(err) => {
                            panic!("Failed to start the runtime: {}", err);
                        }
                        _ => (),
                    }

                    Server::new(runtime, ctx)
                })
                .await;
            }
        },
        Command::Run { args } => {
            if args.is_empty() {
                anyhow::bail!("not enough arguments");
            }

            let mut args = args.clone();
            let bin = args.remove(0);
            let capture = Some(Output {
                r#type: Some(Type::AtEnd(40960)),
            });
            let command = RunProcess {
                bin,
                args,
                work_dir: ctx
                    .cli
                    .workdir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                stdout: capture.clone(),
                stderr: capture,
            };

            let pid = runtime
                .run_command(command, RuntimeMode::Command, &mut ctx)
                .await?;

            if let RuntimeMode::Server = R::MODE {
                write_output(serde_json::json!(pid)).await?;
            }
        }
        Command::OfferTemplate { .. } => {
            if let Some(template) = runtime.offer(&mut ctx).await? {
                write_output(template).await?;
            }
        }
        Command::Test { .. } => runtime.test(&mut ctx).await?,
    }

    Ok(())
}
