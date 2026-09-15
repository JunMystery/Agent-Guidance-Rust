//! AST parsing and symbol extraction module using Tree-Sitter & Polyglot Engine.

pub mod database;
pub mod dataflow_def;
pub mod dataflow_walker;
pub mod engine;
pub mod frontend;
pub mod manifests;
pub mod polyglot;
pub mod trait_def;
pub mod trait_walker;
pub mod types;
pub mod walker;
pub mod zoom;

pub use dataflow_def::{CallArgDetail, DataFlowRecord, FormalParam};
pub use dataflow_walker::extract_dataflow;
pub use engine::AstEngine;
pub use trait_def::{TraitBinding, TraitSpec};
pub use trait_walker::extract_trait_bindings;
pub use types::{AstCall, AstLanguage, AstSymbol};
pub use zoom::ZoomSliceResult;

#[cfg(test)]
mod tests;
