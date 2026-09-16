/// Public observation of one local process occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcessObservation {
    status: LocalProcessStatus,
}

/// Runtime status of one local process occurrence.
///
/// OS PID is runtime observation/debug information. It is not durable semantic
/// identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalProcessStatus {
    NotStarted,
    Running { pid: u32 },
    Exited { pid: u32, exit: LocalProcessExit },
    Stopped { pid: Option<u32> },
    SpawnFailed { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcessExit {
    code: Option<i32>,
    successful: bool,
}

impl LocalProcessObservation {
    pub(crate) fn new(status: LocalProcessStatus) -> Self {
        Self { status }
    }

    pub fn status(&self) -> &LocalProcessStatus {
        &self.status
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, LocalProcessStatus::Running { .. })
    }
}

impl LocalProcessExit {
    pub(crate) fn from_status(status: std::process::ExitStatus) -> Self {
        Self {
            code: status.code(),
            successful: status.success(),
        }
    }

    pub fn code(&self) -> Option<i32> {
        self.code
    }

    pub fn successful(&self) -> bool {
        self.successful
    }
}
