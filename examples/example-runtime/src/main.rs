use futures::channel::oneshot;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use structopt::StructOpt;
use ya_runtime_sdk::serialize::json;
use ya_runtime_sdk::*;

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct ExampleCli {
    #[allow(unused)]
    example_path: Option<std::path::PathBuf>,
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
    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move {
            Ok(json::json!(
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
        async move { Ok(json::json!({})) }.boxed_local()
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
        let mut emitter = ctx.emitter.clone().unwrap();
        let (tx, rx) = oneshot::channel();

        // handle execution in background
        tokio::task::spawn_local(async move {
            emitter.command_started(seq).await;

            // unblock `run_command` execution and continue in background
            let _ = tx.send(seq);

            let stdout = format!("[{}] output for command: {:?}", seq, command)
                .as_bytes()
                .to_vec();

            tokio::time::delay_for(std::time::Duration::from_secs(1)).await;

            emitter.command_stdout(seq, stdout).await;
            emitter.command_stopped(seq, 0).await;
        });

        async move {
            let _ = rx.await;
            Ok(seq)
        }
        .boxed_local()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ya_runtime_sdk::run::<ExampleRuntime>().await
}
