//! Distributed Graph Memory & Snapshot Synchronization (Milestone v1.9.0).
//!
//! Exports and imports verified GraphRAG snapshots for decentralized agent networks and team caches.

pub mod snapshot;

#[cfg(test)]
pub use snapshot::*;

#[cfg(test)]
mod tests;
