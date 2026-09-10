//! AST parsing and symbol extraction module using Tree-Sitter & Polyglot Engine.

pub mod database;
pub mod engine;
pub mod frontend;
pub mod manifests;
pub mod polyglot;
pub mod types;
pub mod walker;
pub mod zoom;

pub use engine::AstEngine;
pub use types::{AstCall, AstLanguage, AstSymbol};
pub use zoom::ZoomSliceResult;

#[cfg(test)]
mod tests;
