use crate::{ProcessExecutionError, ProcessInvocation, ProcessOutput};

fabric::resource! {
    pub ProcessRuntime {
        id: "onoal.package.execution.process-runtime";

        api {
            async fn execute(
                &self,
                invocation: ProcessInvocation,
            ) -> Result<ProcessOutput, ProcessExecutionError>;
        }
    }
}
