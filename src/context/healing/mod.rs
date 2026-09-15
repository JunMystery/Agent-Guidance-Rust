//! Architecture Healing Sentinel (Milestone v1.9.0).
//!
//! Autonomous detection of circular dependencies and dead/orphan symbols across codebase.

pub mod cycle_detector;
pub mod orphan_scanner;

pub use cycle_detector::detect_file_cycles;
pub use orphan_scanner::{OrphanSymbol, detect_orphan_symbols};

#[cfg(test)]
mod tests;
