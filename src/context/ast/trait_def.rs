//! Data model for trait, interface definitions and implementation bindings.

use serde::{Deserialize, Serialize};

/// Represents an explicit or inferred binding where an implementor struct/class
/// implements a trait/interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitBinding {
    /// Name of the trait or interface (e.g. "UserRepository", "io.Reader")
    pub trait_name: String,
    /// Name of the implementing struct or class (e.g. "SqlUserRepository")
    pub implementor_name: String,
    /// 1-indexed line where the implementation or class heritage starts
    pub line: usize,
    /// Method names declared/implemented in this block
    pub methods: Vec<String>,
    /// Language identifier (e.g. "rust", "typescript", "python", "go")
    pub language: String,
    /// True if inferred via duck-typing or type assertion
    pub is_inferred: bool,
}

/// Represents an interface or trait specification and its expected contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitSpec {
    /// Name of the trait/interface
    pub name: String,
    /// Method names defined by the trait
    pub method_signatures: Vec<String>,
    /// 1-indexed start line
    pub line: usize,
    /// Whether this is a Python Protocol, Go interface, or TS interface
    pub kind: String,
}
