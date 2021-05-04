use crate::{Context, Service, ServiceDef, ServiceMode};
use futures::{FutureExt, TryFutureExt};
use std::cell::RefCell;
use std::rc::Rc;
use ya_runtime_api::server::{
    AsyncResponse, KillProcess, RunProcess, RunProcessResp, RuntimeService,
};

pub struct Server<Svc: Service> {
    pub(crate) service: Rc<RefCell<Svc>>,
    pub(crate) ctx: Rc<RefCell<Context<Svc>>>,
}

impl<Svc: Service> Server<Svc> {
    pub fn new(service: Svc, ctx: Context<Svc>) -> Self {
        Self {
            service: Rc::new(RefCell::new(service)),
            ctx: Rc::new(RefCell::new(ctx)),
        }
    }
}

impl<Svc: Service> RuntimeService for Server<Svc> {
    fn hello(&self, _version: &str) -> AsyncResponse<'_, String> {
        async { Ok(<Svc as ServiceDef>::VERSION.to_owned()) }.boxed_local()
    }

    fn run_process(&self, run: RunProcess) -> AsyncResponse<'_, RunProcessResp> {
        let mut service = self.service.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        service
            .run_command(run, ServiceMode::Server, &mut ctx)
            .then(|result| async move {
                match result {
                    Ok(pid) => Ok(RunProcessResp { pid }),
                    Err(err) => Err(err.into()),
                }
            })
            .boxed_local()
    }

    fn kill_process(&self, kill: KillProcess) -> AsyncResponse<'_, ()> {
        let mut service = self.service.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        service
            .kill_command(kill, &mut ctx)
            .map_err(Into::into)
            .boxed_local()
    }

    fn shutdown(&self) -> AsyncResponse<'_, ()> {
        let mut service = self.service.borrow_mut();
        let mut ctx = self.ctx.borrow_mut();
        service.stop(&mut ctx).map_err(Into::into).boxed_local()
    }
}
