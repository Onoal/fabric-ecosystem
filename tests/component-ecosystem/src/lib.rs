#![forbid(unsafe_code)]
//! Cross-package ecosystem regressions for official Fabric components.
//!
//! The generic `fabric-component` kernel crate owns generic component laws.
//! This crate preserves the concrete realization and integration laws for the
//! maintained Gateway, Namespace, and Publication packages that now live in
//! `fabric-packages`.

#[cfg(test)]
mod support;

#[cfg(test)]
mod activation_semantics;
#[cfg(test)]
mod component_runtime_semantics;
#[cfg(test)]
mod component_scope_semantics;
#[cfg(test)]
mod generation_semantics;
#[cfg(test)]
mod invocation_semantics;
#[cfg(test)]
mod operation_catalog_semantics;
#[cfg(test)]
mod runtime_host_semantics;
#[cfg(test)]
mod shell_closure_semantics;
#[cfg(test)]
mod source_guards;
