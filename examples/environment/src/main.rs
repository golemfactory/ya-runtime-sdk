use futures::FutureExt;
use structopt::StructOpt;
use ya_runtime_sdk::cli::parse_cli;
use ya_runtime_sdk::env::Env;
use ya_runtime_sdk::*;

type RuntimeCli = <ExampleRuntime as RuntimeDef>::Cli;

#[derive(Default, RuntimeDef)]
#[cli(ExampleCli)]
pub struct ExampleRuntime;

#[derive(Default)]
pub struct ExampleEnv {
    runtime_name: Option<String>,
}

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    #[structopt(long = "name")]
    flag_name: Option<String>,
    name: Option<String>,
}

impl Env<RuntimeCli> for ExampleEnv {
    fn runtime_name(&self) -> Option<String> {
        self.runtime_name.clone()
    }

    fn cli(&mut self, project_name: &str, project_version: &str) -> anyhow::Result<RuntimeCli> {
        let cli: RuntimeCli = parse_cli(project_name, project_version, self.args())?;

        if cli.runtime.flag_name.is_some() {
            // set runtime name from a flag argument
            self.runtime_name = cli.runtime.flag_name.clone();
        }
        if cli.runtime.name.is_some() {
            // set runtime name from a positional argument
            self.runtime_name = cli.runtime.name.clone();
        }

        Ok(cli)
    }
}

impl Runtime for ExampleRuntime {
    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok(None) }.boxed_local()
    }

    fn start<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok(None) }.boxed_local()
    }

    fn stop<'a>(&mut self, _: &mut Context<Self>) -> EmptyResponse<'a> {
        async move { Ok(()) }.boxed_local()
    }

    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        _mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a> {
        ctx.command(|mut run_ctx| async move {
            run_ctx.stdout(format!("[{:?}] stdout", command)).await;
            Ok(())
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ya_runtime_sdk::run_with::<ExampleRuntime, _>(ExampleEnv::default()).await
}
