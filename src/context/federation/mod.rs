//! Deep Graph Federation & Multi-Workspace Linked Graph (Milestone v1.9.0).
//!
//! Connects and traverses symbol graphs across distinct repositories and monorepo packages.

pub mod federated_traversal;
pub mod workspace_detector;

pub use federated_traversal::{federated_search_callers, federated_search_callees};

#[cfg(test)]
pub use workspace_detector::auto_discover_workspaces;

#[cfg(test)]
mod tests;
