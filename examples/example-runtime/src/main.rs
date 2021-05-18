use futures::FutureExt;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use ya_runtime_sdk::*;
use serde_json::Value;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    /// Task package path (ignored in case of services)
    // FIXME: currently, the ExeUnit Supervisor always passes this argument to a binary
    #[allow(unused)]
    task_package: Option<std::path::PathBuf>,
    /// Example runtime param
    #[allow(unused)]
    #[structopt(long, default_value = "1")]
    param: usize,
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
    const MODE: RuntimeMode = RuntimeMode::Server;

    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        let data = r#"
        {"startMode":"blocking","valid":{"Ok":""},"vols":[]}
        "#;
        let json_v: Value = serde_json::from_str(data).expect("Invalid JSON string");
        async move { Ok(json_v) }.boxed_local()
    }

    fn start<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move {
            // tokio::time::delay_for(std::time::Duration::from_secs(3600)).await;
            Ok("start".into())
        }.boxed_local()
    }

    fn stop<'a>(&mut self, _: &mut Context<Self>) -> EmptyResponse<'a> {
        async move { Ok(()) }.boxed_local()
    }
    fn offer<'a>(&mut self, _ctx: &mut Context<Self>) -> OutputResponse<'a> {
        let data = r#"
            {
                "constraints":"",
                "properties":{}
            }"#;
        let json_v: Value = serde_json::from_str(data).expect("Invalid JSON string");
        async move { Ok(json_v) }.boxed_local()
    }

    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a> {
        // println!("start_command: {:?} in {:?} mode", command, mode);
        let emitter = ctx.emitter.clone().unwrap();
        async move {
            let seq = 0u64;
            emitter.command_started(seq).await;
            emitter
                .command_stdout(seq, format!("response {}", seq).as_bytes().to_vec())
                .await;
            emitter.command_stopped(seq, 0).await;
            Ok(seq)
        }
        .boxed_local()
    }
}

// Macro expansion is equivalent to:
//
// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     ya_runtime_sdk::::run::<ExampleRuntime>().await
// }

main!(ExampleRuntime);
