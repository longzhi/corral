//! Corral CLI library exports for testing

pub mod adapters;
pub mod audit;
pub mod broker;
pub mod manifest;
pub mod platform;
pub mod watchdog;

// Re-export corral-core
pub use corral_core;
