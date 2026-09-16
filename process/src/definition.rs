use fabric::core::ModuleId;

use crate::LocalProcessDefinitionError;

/// Declarative local process configuration.
///
/// This is process configuration, not an OS process occurrence. The process
/// occurrence is created only when the Fabric runtime starts the module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcessDefinition {
    module_id: ModuleId,
    program: String,
    args: Vec<String>,
}

impl LocalProcessDefinition {
    pub fn new(
        module_id: ModuleId,
        program: impl Into<String>,
    ) -> Result<Self, LocalProcessDefinitionError> {
        Self::with_args(module_id, program, Vec::<String>::new())
    }

    pub fn with_args(
        module_id: ModuleId,
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, LocalProcessDefinitionError> {
        let program = program.into();
        validate_argument(&program).map_err(|_| LocalProcessDefinitionError::InvalidProgram)?;
        let args = args
            .into_iter()
            .map(Into::into)
            .map(|arg| {
                validate_argument(&arg)
                    .map(|_| arg)
                    .map_err(|_| LocalProcessDefinitionError::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            module_id,
            program,
            args,
        })
    }

    pub fn module_id(&self) -> &ModuleId {
        &self.module_id
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

fn validate_argument(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.contains('\0') {
        Err(())
    } else {
        Ok(())
    }
}
