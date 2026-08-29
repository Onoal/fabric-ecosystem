#![forbid(unsafe_code)]

mod adapter;
mod config;

#[cfg(test)]
mod tests;

pub use adapter::PingoraIngressAdapter;
pub use config::PingoraIngressConfig;
