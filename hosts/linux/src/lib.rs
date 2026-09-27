//! Linux Host artifact for Fabric.
//!
//! This crate projects Linux environment evidence into Fabric's canonical
//! `HostDescriptor` and `HostRequirement` model. It does not define a Linux
//! Resource, System, Component, Adapter, Composition, or parallel Host model.
//!
//! `detect_linux_host()` is the public detection entry point. Stable Linux
//! operating-system and facility identifiers are exposed for requirements and
//! ecosystem compatibility checks; raw probing functions remain implementation
//! details of detection.

mod detection;
mod error;
mod facts;
mod requirements;

pub use detection::detect_linux_host;
pub use error::{LinuxHostError, LinuxHostErrorKind};
pub use facts::{cgroup_v2_facility, linux_operating_system, native_architecture, procfs_facility};
pub use requirements::{linux_cgroup_v2_requirement, linux_procfs_requirement, linux_requirement};
