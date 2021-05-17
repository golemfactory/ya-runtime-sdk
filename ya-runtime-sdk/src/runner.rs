use tokio::io::AsyncWriteExt;
use ya_runtime_api::server::proto::{output::Type, request::RunProcess, Output};

use crate::cli::{Command, CommandCli};
use crate::env::{DefaultEnv, Env};
use crate::runtime::{Context, Runtime, RuntimeMode};
use crate::server::Server;

/// Starts the runtime
pub async fn run<R: Runtime + 'static>() -> anyhow::Result<()> {
    run_with::<R, _>(DefaultEnv::default()).await
}

/// Starts the runtime using a custom environment configuration provider
pub async fn run_with<R: Runtime + 'static, E: Env>(env: E) -> anyhow::Result<()> {
    let mut runtime = R::default();
    let mut ctx = Context::<R>::try_with(env)?;

    match ctx.cli.command() {
        Command::Deploy { args: _ } => {
            let deployment = runtime.deploy(&mut ctx).await?;
            output(deployment).await?;
        }
        Command::Start { args: _ } => match R::MODE {
            RuntimeMode::Command => {
                let started = runtime.start(&mut ctx).await?;
                output(started).await?;
            }
            RuntimeMode::Server => {
                ya_runtime_api::server::run_async(|emitter| async move {
                    let start = {
                        ctx.set_emitter(Box::new(emitter));
                        runtime.start(&mut ctx)
                    };
                    start.await.expect("Failed to start the runtime");
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
                work_dir: ctx.cli.workdir().unwrap().display().to_string(),
                stdout: capture.clone(),
                stderr: capture,
            };

            let pid = runtime
                .run_command(command, RuntimeMode::Command, &mut ctx)
                .await?;

            output(serde_json::json!(pid)).await?;
        }
        Command::OfferTemplate { args: _ } => {
            let template = runtime.offer(&mut ctx).await?;
            output(template).await?;
        }
        Command::Test { args: _ } => runtime.test(&mut ctx).await?,
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
