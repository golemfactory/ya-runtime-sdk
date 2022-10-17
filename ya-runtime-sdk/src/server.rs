use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::oneshot;
use futures::{FutureExt, TryFutureExt};
use ya_runtime_api::server::proto::response::create_network::Endpoint;
use ya_runtime_api::server::{
    AsyncResponse, CreateNetwork, CreateNetworkResp, KillProcess, RunProcess, RunProcessResp,
    RuntimeService,
};

pub use ya_runtime_api::deploy::ContainerEndpoint;

use crate::runtime::RuntimeMode;
use crate::{Context, Runtime, RuntimeDef};

pub struct Server<R: Runtime> {
    pub(crate) runtime: Rc<RefCell<R>>,
    pub(crate) ctx: Rc<RefCell<Context<R>>>,
}

impl<R: Runtime + 'static> Server<R> {
    pub fn new(runtime: R, mut ctx: Context<R>) -> Self {
        let (tx, rx) = oneshot::channel();
        ctx.set_shutdown_tx(tx);

        let server = Self {
            runtime: Rc::new(RefCell::new(runtime)),
            ctx: Rc::new(RefCell::new(ctx)),
        };

        server.shutdown_on(rx);
        server
    }

    pub fn shutdown_on(&self, rx: oneshot::Receiver<()>) {
        let server = self.clone();
        tokio::task::spawn_local(rx.then(move |result| async move {
            if result.is_ok() {
                let _ = server.shutdown().await;
                std::process::exit(0);
            }
        }));
    }
}

impl<R: Runtime> Clone for Server<R> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
            ctx: self.ctx.clone(),
        }
    }
}

impl<R: Runtime> RuntimeService for Server<R> {
    fn hello(&self, _version: &str) -> AsyncResponse<'_, String> {
        async { Ok(<R as RuntimeDef>::VERSION.to_owned()) }.boxed_local()
    }

    fn run_process(&self, run: RunProcess) -> AsyncResponse<'_, RunProcessResp> {
        let mut runtime = self.runtime.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        runtime
            .run_command(run, RuntimeMode::Server, &mut ctx)
            .then(|result| async move {
                match result {
                    Ok(pid) => Ok(RunProcessResp { pid }),
                    Err(err) => Err(err.into()),
                }
            })
            .boxed_local()
    }

    fn kill_process(&self, kill: KillProcess) -> AsyncResponse<'_, ()> {
        let mut runtime = self.runtime.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        runtime
            .kill_command(kill, &mut ctx)
            .map_err(Into::into)
            .boxed_local()
    }

    fn create_network(&self, network: CreateNetwork) -> AsyncResponse<'_, CreateNetworkResp> {
        let mut runtime = self.runtime.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        runtime
            .join_network(network, &mut ctx)
            .map(|result| {
                result.map(|e| CreateNetworkResp {
                    endpoint: match &e {
                        ContainerEndpoint::UnixStream(_) => {
                            Some(Endpoint::UnixStream(e.to_string()))
                        }
                        ContainerEndpoint::UnixDatagram(_) => {
                            Some(Endpoint::UnixDatagram(e.to_string()))
                        }
                        ContainerEndpoint::UdpDatagram(_) => {
                            Some(Endpoint::UdpDatagram(e.to_string()))
                        }
                        ContainerEndpoint::TcpStream(_) => Some(Endpoint::TcpStream(e.to_string())),
                        _ => None,
                    },
                })
            })
            .map_err(Into::into)
            .boxed_local()
    }

    fn shutdown(&self) -> AsyncResponse<'_, ()> {
        let mut runtime = self.runtime.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        runtime.stop(&mut ctx).map_err(Into::into).boxed_local()
    }
}
