use futures::future::{AbortHandle, Abortable};
use futures::{FutureExt, TryFutureExt};
use tokio::time::Duration;
use ya_runtime_sdk::*;

const COUNTER_NAME: &'static str = "golem.usage.custom.counter";
const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default, RuntimeDef)]
pub struct ExampleRuntime {
    handle: Option<AbortHandle>,
}

async fn metric_reporter(mut emitter: EventEmitter) {
    let mut value = 10f64;

    loop {
        tokio::time::delay_for(INTERVAL).await;
        value += 1f64;
        emitter
            .counter(RuntimeCounter {
                name: COUNTER_NAME.to_string(),
                value,
            })
            .await;
    }
}

impl Runtime for ExampleRuntime {
    fn deploy<'a>(&mut self, _: &mut Context<Self>) -> OutputResponse<'a> {
        async move { Ok(None) }.boxed_local()
    }

    fn start<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a> {
        let emitter = match ctx.emitter.clone() {
            Some(emitter) => emitter,
            None => {
                let err = anyhow::anyhow!("not running in server mode");
                return futures::future::err(err.into()).boxed_local();
            }
        };

        let (handle, reg) = AbortHandle::new_pair();
        tokio::task::spawn_local(Abortable::new(metric_reporter(emitter.clone()), reg));
        self.handle = Some(handle);

        async move {
            emitter
                .clone()
                .counter(RuntimeCounter {
                    name: COUNTER_NAME.to_string(),
                    value: 1f64,
                })
                .await;

            Ok(None)
        }
        .boxed_local()
    }

    fn stop<'a>(&mut self, _: &mut Context<Self>) -> EmptyResponse<'a> {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        async move { Ok(()) }.boxed_local()
    }

    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        _mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a> {
        ctx.command(|mut run_ctx| {
            async move {
                match command.bin.as_str() {
                    "sleep" => {
                        let delay_str = command
                            .args
                            .get(1)
                            .ok_or_else(|| anyhow::anyhow!("Missing delay value"))?;

                        let delay_ms: u64 = delay_str.as_str().parse()?;
                        let delay = Duration::from_millis(delay_ms);

                        run_ctx
                            .stdout(format!("Entering sleep for {}ms", delay_ms))
                            .await;

                        tokio::time::delay_for(delay).await;
                        run_ctx.stdout("Done sleeping").await;
                    }
                    "stop" => {
                        run_ctx.stdout("Stopping runtime").await;
                        run_ctx.control().shutdown();
                    }
                    _ => {
                        anyhow::bail!("Unsupported command {} {:?}", command.bin, command.args);
                    }
                }
                Ok(())
            }
            .map_err(Into::into)
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    ya_runtime_sdk::run::<ExampleRuntime>().await
}
