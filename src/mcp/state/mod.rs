pub mod types;
pub mod uri;
pub mod gates;
pub mod permissions;
pub mod persistence;

pub use types::ServerState;

#[cfg(test)]
pub use uri::parse_file_uri;
#[cfg(test)]
pub use std::sync::Arc;
#[cfg(test)]
pub use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
pub use std::fs;

#[cfg(test)]
#[path = "../state_tests.rs"]
mod tests;
