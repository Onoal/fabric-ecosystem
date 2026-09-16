use std::process::Child;
use std::sync::{Arc, Mutex};

use crate::{
    LocalProcessExit, LocalProcessObservation, LocalProcessRuntimeError, LocalProcessStatus,
};

#[derive(Clone)]
pub struct LocalProcessHandle {
    pub(crate) state: Arc<Mutex<ProcessState>>,
}

pub(crate) struct ProcessState {
    pub(crate) child: Option<Child>,
    pub(crate) status: LocalProcessStatus,
}

impl LocalProcessHandle {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProcessState {
                child: None,
                status: LocalProcessStatus::NotStarted,
            })),
        }
    }

    pub fn observe(&self) -> Result<LocalProcessObservation, LocalProcessRuntimeError> {
        let mut state =
            self.state
                .lock()
                .map_err(|_| LocalProcessRuntimeError::StatusUnavailable {
                    message: "local process state lock is poisoned".to_owned(),
                })?;
        refresh(&mut state)?;
        Ok(LocalProcessObservation::new(state.status.clone()))
    }
}

pub(crate) fn refresh(state: &mut ProcessState) -> Result<(), LocalProcessRuntimeError> {
    let Some(child) = state.child.as_mut() else {
        return Ok(());
    };
    match child.try_wait() {
        Ok(Some(exit)) => {
            let pid = child.id();
            state.child = None;
            state.status = LocalProcessStatus::Exited {
                pid,
                exit: LocalProcessExit::from_status(exit),
            };
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(error) => Err(LocalProcessRuntimeError::status_unavailable(error)),
    }
}
