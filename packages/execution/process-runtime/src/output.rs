#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.status_code == Some(0)
    }

    /// Lossy UTF-8 display convenience. Raw `stdout` bytes remain canonical.
    pub fn stdout_utf8(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Lossy UTF-8 display convenience. Raw `stderr` bytes remain canonical.
    pub fn stderr_utf8(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}
