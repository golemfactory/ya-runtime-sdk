use futures::channel::oneshot;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use structopt::StructOpt;
use ya_runtime_sdk::*;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    /// Task package path (ignored in case of services)
    #[allow(unused)]
    task_package: Option<std::path::PathBuf>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ExampleConf {
    value: usize,
}

#[derive(Default, RuntimeDef)]
#[cli(ExampleCli)]
#[conf(ExampleConf)]
pub struct ExampleRuntime {
    seq: AtomicU64,
}

impl Runtime for ExampleRuntime {
    const MODE: RuntimeMode = RuntimeMode::Server;

    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move {
            Ok(serialize::json::json!(
                {
                    "startMode":"blocking",
                    "valid":{"Ok":""},
                    "vols":[]
                }
            ))
        }
        .boxed_local()
    }

    fn start<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok("start".into()) }.boxed_local()
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
        let seq = self.seq.fetch_add(1, Relaxed);
        let emitter = ctx.emitter.clone().unwrap();

        let (tx, rx) = oneshot::channel();

        tokio::task::spawn_local(async move {
            // command execution started
            emitter.command_started(seq).await;
            // resolves the future returned by `run_command`
            let _ = tx.send(seq);

            let stdout = format!("[{}] output for command: {:?}", seq, command)
                .as_bytes()
                .to_vec();

            tokio::time::delay_for(std::time::Duration::from_secs(2)).await;

            emitter.command_stdout(seq, stdout).await;
            emitter.command_stopped(seq, 0).await;
        });

        async move {
            // awaits `tx.send`
            Ok(rx.await.unwrap())
        }
        .boxed_local()
    }

    fn offer<'a>(&mut self, _ctx: &mut Context<Self>) -> OutputResponse<'a> {
        async move {
            Ok(serialize::json::json!(
                {
                    "constraints": "",
                    "properties": {}
                }
            ))
        }
        .boxed_local()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ya_runtime_sdk::run::<ExampleRuntime>().await
}
