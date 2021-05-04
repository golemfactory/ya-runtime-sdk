use futures::FutureExt;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use ya_service_sdk::*;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    /// Example service param
    #[allow(unused)]
    #[structopt(long, default_value = "1")]
    param: usize,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ExampleConf {
    value: usize,
}

#[derive(Default, ServiceDef)]
#[cli(ExampleCli)]
#[conf(ExampleConf)]
pub struct ExampleService;

impl Service for ExampleService {
    const MODE: ServiceMode = ServiceMode::Command;

    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok("deploy".into()) }.boxed_local()
    }

    fn start<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok("start".into()) }.boxed_local()
    }

    fn stop<'a>(&mut self, _: &mut Context<Self>) -> EmptyResponse<'a> {
        println!("stop");
        async move { Ok(()) }.boxed_local()
    }

    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: ServiceMode,
        _: &mut Context<Self>,
    ) -> ProcessIdResponse<'a> {
        println!("start_command: {:?} in {:?} mode", command, mode);
        async move { Ok(0) }.boxed_local()
    }
}

// Macro expansion is equivalent to:
//
// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     ServiceRunner::<ExampleService>::run().await
// }

main!(ExampleService);
