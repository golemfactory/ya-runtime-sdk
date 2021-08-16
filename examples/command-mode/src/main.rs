use futures::FutureExt;
use structopt::StructOpt;
use ya_runtime_sdk::cli::CommandCli;
use ya_runtime_sdk::error::Error;
use ya_runtime_sdk::*;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    #[allow(unused)]
    path: Option<std::path::PathBuf>,
}

#[derive(Default, RuntimeDef)]
#[cli(ExampleCli)]
pub struct ExampleRuntime;

impl Runtime for ExampleRuntime {
    const MODE: RuntimeMode = RuntimeMode::Command;

    fn deploy<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a> {
        if ctx.cli.workdir().is_none() {
            return Error::response("Workdir argument not provided");
        }
        async move { Ok(None) }.boxed_local()
    }

    fn start<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a> {
        if ctx.cli.workdir().is_none() {
            return Error::response("Workdir argument not provided");
        }
        async move {
            Ok(Some(serialize::json::json!({
                "exampleProperty": "running",
            })))
        }
        .boxed_local()
    }

    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a> {
        use anyhow::Context;
        use std::process::Stdio;

        if let RuntimeMode::Server = mode {
            return Error::response("Server mode is not supported");
        }
        if ctx.cli.workdir().is_none() {
            return Error::response("Workdir argument not provided");
        }

        async move {
            // Echo the executed command and its arguments
            let child = tokio::process::Command::new("/bin/echo")
                .arg(command.bin)
                .args(command.args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()?;
            let output = child.wait_with_output().await?;

            print!(
                "stdout: {}",
                String::from_utf8(output.stdout).context("stdout is not a UTF-8 string")?
            );
            print!(
                "stderr: {}",
                String::from_utf8(output.stderr).context("stderr is not a UTF-8 string")?
            );
            println!();

            Ok(0)
        }
        .boxed_local()
    }

    // Remaining trait functions have default implementations
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ya_runtime_sdk::run::<ExampleRuntime>().await
}
