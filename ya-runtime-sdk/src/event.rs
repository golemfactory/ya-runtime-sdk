use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::{FutureExt, SinkExt, StreamExt};

use ya_runtime_api::server::*;

use crate::common::IntoVec;
use crate::ProcessId;

/// Runtime event kind
#[derive(Clone, Debug)]
pub enum EventKind {
    Process(ProcessStatus),
    Runtime(RuntimeStatus),
}

impl From<ProcessStatus> for EventKind {
    fn from(status: ProcessStatus) -> Self {
        Self::Process(status)
    }
}

impl From<RuntimeStatus> for EventKind {
    fn from(status: RuntimeStatus) -> Self {
        Self::Runtime(status)
    }
}

impl From<RuntimeStatusKind> for EventKind {
    fn from(kind: RuntimeStatusKind) -> Self {
        Self::Runtime(RuntimeStatus { kind: Some(kind) })
    }
}

/// Runtime event emitter
#[derive(Clone)]
pub struct EventEmitter {
    tx_process: mpsc::Sender<ProcessStatus>,
    tx_runtime: mpsc::Sender<RuntimeStatus>,
}

impl EventEmitter {
    pub fn spawn(emitter: impl RuntimeHandler + 'static) -> Self {
        let (tx_p, rx_p) = mpsc::channel(1);
        let (tx_r, rx_r) = mpsc::channel(1);
        let e_p = Rc::new(RefCell::new(emitter));
        let e_r = e_p.clone();

        tokio::task::spawn_local(
            rx_p.for_each(move |status| e_p.borrow().on_process_status(status)),
        );
        tokio::task::spawn_local(
            rx_r.for_each(move |status| e_r.borrow().on_runtime_status(status)),
        );

        Self {
            tx_process: tx_p,
            tx_runtime: tx_r,
        }
    }
}

impl EventEmitter {
    /// Emit a command started event
    pub fn command_started(&mut self, process_id: ProcessId) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: Default::default(),
            stderr: Default::default(),
        })
    }

    /// Emit a command stopped event
    pub fn command_stopped(&mut self, process_id: ProcessId, return_code: i32) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: false,
            return_code,
            stdout: Default::default(),
            stderr: Default::default(),
        })
    }

    /// Emit a command output event (stdout)
    pub fn command_stdout(
        &mut self,
        process_id: ProcessId,
        stdout: impl IntoVec<u8>,
    ) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: stdout.into_vec(),
            stderr: Default::default(),
        })
    }

    /// Emit a command output event (stderr)
    pub fn command_stderr(
        &mut self,
        process_id: ProcessId,
        stderr: impl IntoVec<u8>,
    ) -> BoxFuture<()> {
        self.emit(ProcessStatus {
            pid: process_id,
            running: true,
            return_code: 0,
            stdout: Default::default(),
            stderr: stderr.into_vec(),
        })
    }

    /// Emit a state event
    pub fn state(&mut self, state: RuntimeState) -> BoxFuture<()> {
        self.emit(RuntimeStatusKind::State(state))
    }

    /// Emit a counter event
    pub fn counter(&mut self, counter: RuntimeCounter) -> BoxFuture<()> {
        self.emit(RuntimeStatusKind::Counter(counter))
    }

    /// Emit an event
    pub fn emit(&mut self, event: impl Into<EventKind>) -> BoxFuture<()> {
        match event.into() {
            EventKind::Process(status) => self.tx_process.send(status).then(|_| async {}).boxed(),
            EventKind::Runtime(status) => self.tx_runtime.send(status).then(|_| async {}).boxed(),
        }
    }
}
