use fabric_host::{HostCompatibilityError, HostRequirement};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SystemdProcessAdapterError {
    #[error(
        "systemd process adapter host is incompatible with requirement {requirement:?}: {source}"
    )]
    IncompatibleHost {
        requirement: Box<HostRequirement>,
        #[source]
        source: Box<HostCompatibilityError>,
    },
}
