use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::cli::CommandCli;
use crate::context::Context;
use crate::error::Error;
use crate::runtime_api::server::*;

use ya_runtime_api::deploy::ContainerEndpoint;

pub type ProcessId = u64;
pub type EmptyResponse<'a> = LocalBoxFuture<'a, Result<(), Error>>;
pub type OutputResponse<'a> = LocalBoxFuture<'a, Result<Option<serde_json::Value>, Error>>;
pub type EndpointResponse<'a> = LocalBoxFuture<'a, Result<ContainerEndpoint, Error>>;
pub type ProcessIdResponse<'a> = LocalBoxFuture<'a, Result<ProcessId, Error>>;

/// Command handling interface for runtimes
pub trait Runtime: RuntimeDef {
    const MODE: RuntimeMode = RuntimeMode::Server;

    /// Deploy and configure the runtime
    fn deploy<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a>;

    /// Start the runtime
    fn start<'a>(&mut self, ctx: &mut Context<Self>) -> OutputResponse<'a>;

    /// Stop the runtime
    fn stop<'a>(&mut self, _ctx: &mut Context<Self>) -> EmptyResponse<'a> {
        async move { Ok(()) }.boxed_local()
    }

    /// Start a runtime command
    fn run_command<'a>(
        &mut self,
        command: RunProcess,
        mode: RuntimeMode,
        ctx: &mut Context<Self>,
    ) -> ProcessIdResponse<'a>;

    /// Stop runtime command execution
    fn kill_command<'a>(
        &mut self,
        _kill: KillProcess,
        _ctx: &mut Context<Self>,
    ) -> EmptyResponse<'a> {
        async move { Err(Error::from_string("Not supported")) }.boxed_local()
    }

    /// Output a market Offer template stub
    fn offer<'a>(&mut self, _ctx: &mut Context<Self>) -> OutputResponse<'a> {
        async move {
            Ok(Some(crate::serialize::json::json!({
                "constraints": "",
                "properties": {}
            })))
        }
        .boxed_local()
    }

    /// Perform a self-test
    fn test<'a>(&mut self, _ctx: &mut Context<Self>) -> EmptyResponse<'a> {
        async move { Ok(()) }.boxed_local()
    }

    /// Join a VPN network
    fn join_network<'a>(
        &mut self,
        _network: CreateNetwork,
        _ctx: &mut Context<Self>,
    ) -> EndpointResponse<'a> {
        async move { Err(Error::from_string("Not supported")) }.boxed_local()
    }
}

/// Runtime definition trait.
/// Auto-generated with `#[derive(RuntimeDef)]`
pub trait RuntimeDef {
    const NAME: &'static str;
    const VERSION: &'static str;

    type Cli: CommandCli;
    type Conf: Default + Serialize + for<'de> Deserialize<'de>;
}

/// Defines the mode of execution for commands within the runtime.
#[derive(Clone, Copy, Debug)]
pub enum RuntimeMode {
    /// Server (blocking) mode
    /// Uses Runtime API to communicate with the ExeUnit Supervisor.
    /// `Command::Deploy` remains a one-shot command.
    Server,
    /// One-shot execution mode
    /// Each command is a separate invocation of the runtime binary.
    Command,
}

/// Runtime control helper
#[derive(Clone, Default)]
pub struct RuntimeControl {
    pub(crate) shutdown_tx: Rc<RefCell<Option<oneshot::Sender<()>>>>,
}

impl RuntimeControl {
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.borrow_mut().take() {
            let _ = tx.send(());
        }
    }
}
