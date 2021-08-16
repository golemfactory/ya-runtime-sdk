use futures::FutureExt;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use ya_runtime_sdk::error::Error;
use ya_runtime_sdk::*;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    #[allow(unused)]
    path: Option<std::path::PathBuf>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ExampleConf {
    value: usize,
}

#[derive(Default, RuntimeDef)]
#[cli(ExampleCli)]
#[conf(ExampleConf)]
pub struct ExampleRuntime;

impl Runtime for ExampleRuntime {
    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        // SDK will auto-generate the following code:
        //
        // async move {
        //     Ok(Some(serialize::json::json!({
        //         "startMode": "blocking",
        //         "valid": {"Ok": ""},
        //         "vols": []
        //     })))
        // }
        // .boxed_local()

        async move { Ok(None) }.boxed_local()
    }

    fn start<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move {
            Ok(Some(serialize::json::json!({
                "exampleProperty": "running",
            })))
        }
        .boxed_local()
    }

    fn stop<'a>(&mut self, _: &mut Context<Self>) -> EmptyResponse<'a> {
        // Gracefully shutdown the service
        async move { Ok(()) }.boxed_local()
    }

    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a> {
        use std::process::Stdio;

        if let RuntimeMode::Command = mode {
            return Error::response("Command mode is not supported");
        }

        // Echo the executed command and its arguments
        let started = tokio::process::Command::new("/bin/echo")
            .arg(command.bin)
            .args(command.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn();

        // Wraps command's lifecycle. The handler is executed in background.
        // Result of `started` is handled prior to emitting command lifecycle events.
        futures::future::ready(started).as_command(ctx, |child, mut run_ctx| async move {
            let output = child.wait_with_output().await?;
            run_ctx.stdout(output.stdout).await;
            run_ctx.stderr(output.stderr).await;
            Ok(())
        })
    }

    // Simple command handler
    //
    // fn run_command<'a>(
    //     &mut self,
    //     command: RunProcess,
    //     _mode: RuntimeMode,
    //     ctx: &mut Context<Self>,
    // ) -> ProcessIdResponse<'a> {
    //     ctx.command(|mut run_ctx| async move {
    //         run_ctx.stdout(format!("[{:?}] stdout", command)).await;
    //         Ok(())
    //     })
    // }

    // Remaining trait functions have default implementations
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ya_runtime_sdk::run::<ExampleRuntime>().await
}
