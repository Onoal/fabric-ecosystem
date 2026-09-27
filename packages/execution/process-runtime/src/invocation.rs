use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessEnvironmentVariable {
    pub name: String,
    pub value: String,
}

impl ProcessEnvironmentVariable {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub environment: Vec<ProcessEnvironmentVariable>,
    pub stdin: Option<Vec<u8>>,
}

impl ProcessInvocation {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            working_directory: None,
            environment: Vec::new(),
            stdin: None,
        }
    }

    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_working_directory(mut self, working_directory: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(working_directory.into());
        self
    }

    pub fn with_environment(
        mut self,
        environment: impl IntoIterator<Item = ProcessEnvironmentVariable>,
    ) -> Self {
        self.environment = environment.into_iter().collect();
        self
    }

    pub fn with_stdin(mut self, stdin: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(stdin.into());
        self
    }
}
