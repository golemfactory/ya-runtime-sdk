use tokio::io::AsyncWriteExt;

use ya_runtime_api::server::proto::{output::Type, request::RunProcess, Output};

use crate::cli::{Command, CommandCli};
use crate::env::{DefaultEnv, Env};
use crate::runtime::{Context, Runtime, RuntimeMode};
use crate::server::Server;
use crate::RuntimeDef;

/// Starts the runtime
pub async fn run<R: Runtime + 'static>() -> anyhow::Result<()> {
    run_with::<R, _>(DefaultEnv::<<R as RuntimeDef>::Cli>::default()).await
}

/// Starts the runtime using a custom environment configuration provider
pub async fn run_with<R, E>(env: E) -> anyhow::Result<()>
where
    R: Runtime + 'static,
    E: Env<<R as RuntimeDef>::Cli> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(async {
            let set = tokio::task::LocalSet::new();
            set.run_until(inner::<R, E>(env)).await
        })
    })
    .await?
}

async fn inner<R, E>(env: E) -> anyhow::Result<()>
where
    R: Runtime + 'static,
    E: Env<<R as RuntimeDef>::Cli> + Send + 'static,
{
    let mut runtime = R::default();
    let mut ctx = Context::<R>::try_with(env)?;

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
            output(deployment).await?;
        }
        Command::Start { .. } => match R::MODE {
            RuntimeMode::Command => {
                if let Some(started) = runtime.start(&mut ctx).await? {
                    output(started).await?;
                }
            }
            RuntimeMode::Server => {
                ya_runtime_api::server::run_async(|emitter| async move {
                    let start = {
                        ctx.set_emitter(emitter);
                        runtime.start(&mut ctx)
                    };

                    if let Some(out) = start.await.expect("Failed to start the runtime") {
                        crate::runtime::RunCommandContext {
                            id: ctx.next_pid(),
                            emitter: ctx.emitter.clone(),
                            control: Default::default(),
                        }
                        .stdout(out.to_string())
                        .await;
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
                output(serde_json::json!(pid)).await?;
            }
        }
        Command::OfferTemplate { .. } => {
            if let Some(template) = runtime.offer(&mut ctx).await? {
                output(template).await?;
            }
        }
        Command::Test { .. } => runtime.test(&mut ctx).await?,
    }

    Ok(())
}

async fn output(json: serde_json::Value) -> anyhow::Result<()> {
    let string = json.to_string();
    let mut stdout = tokio::io::stdout();
    stdout.write_all(string.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}
