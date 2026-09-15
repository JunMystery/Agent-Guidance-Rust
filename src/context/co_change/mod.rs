//! Evolutionary Co-Change Coupling Graph (Milestone v1.9.0).
//!
//! Tracks historical pairwise co-modifications between files and predicts forgotten coupled files.

pub mod predictor;
pub mod recorder;

pub use predictor::{CoupledFileWarning, predict_coupled_files};
pub use recorder::{normalize_file_pair, record_co_change, record_session_co_changes};

#[cfg(test)]
mod tests;
