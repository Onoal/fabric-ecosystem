use std::sync::Arc;

use fabric_host::{HostDescriptor, HostFacilityId, HostOperatingSystem, HostRequirement};
use fabric_resource_process::{
    ProcessAdapter, ProcessError, ProcessExecution, ProcessExecutionRequest, ProcessSpec,
};

use crate::SystemdProcessAdapterError;
use crate::adapter::runtime::{ProcessRuntime, ProcessSupervisor};
use crate::adapter::systemd::SystemdProcessSupervisor;
use crate::artifact::ProcessArtifactResolver;
use crate::config::SystemdProcessConfig;

pub struct SystemdProcessAdapter {
    runtime: ProcessRuntime,
}

impl SystemdProcessAdapter {
    pub fn for_host(
        host: &HostDescriptor,
        config: SystemdProcessConfig,
        resolver: Arc<dyn ProcessArtifactResolver>,
    ) -> Result<Self, SystemdProcessAdapterError> {
        let requirement = Self::host_requirement();
        requirement.evaluate(host).map_err(|source| {
            SystemdProcessAdapterError::IncompatibleHost {
                requirement: Box::new(requirement.clone()),
                source: Box::new(source),
            }
        })?;
        let supervisor: Arc<dyn ProcessSupervisor> = Arc::new(SystemdProcessSupervisor::new());
        Ok(Self {
            runtime: ProcessRuntime::new(config, resolver, supervisor),
        })
    }

    pub fn host_requirement() -> HostRequirement {
        HostRequirement::new()
            .allow_operating_system(
                HostOperatingSystem::new("linux").expect("static operating system"),
            )
            .require_facility(user_systemd_supervision_facility())
    }

    #[cfg(test)]
    pub(crate) fn with_supervisor(
        config: SystemdProcessConfig,
        resolver: Arc<dyn ProcessArtifactResolver>,
        supervisor: Arc<dyn ProcessSupervisor>,
    ) -> Result<Self, ProcessError> {
        Ok(Self {
            runtime: ProcessRuntime::new(config, resolver, supervisor),
        })
    }
}

fn user_systemd_supervision_facility() -> HostFacilityId {
    HostFacilityId::new("fabric.host.user-systemd-supervision").expect("static host facility")
}

impl ProcessAdapter for SystemdProcessAdapter {
    fn prepare(&mut self, process: &ProcessSpec) -> Result<(), ProcessError> {
        self.runtime.prepare_process(process)
    }

    fn start(
        &mut self,
        request: ProcessExecutionRequest,
    ) -> Result<Box<dyn ProcessExecution>, ProcessError> {
        self.runtime.start_execution(request)
    }

    fn clear(&mut self) {
        self.runtime.clear();
    }
}
