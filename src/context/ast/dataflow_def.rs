//! Data models for intra-procedural data flow tracking and argument extraction.

use serde::{Deserialize, Serialize};

/// Represents a formal parameter of a function or method definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormalParam {
    /// Name of the formal parameter
    pub name: String,
    /// 0-indexed parameter position
    pub index: usize,
    /// Declared type hint if present
    pub type_hint: Option<String>,
}

/// Represents an argument passed to a function call within a caller body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallArgDetail {
    /// Name of the called function
    pub callee_name: String,
    /// 0-indexed argument position
    pub arg_index: usize,
    /// Full source text of argument expression
    pub source_expr: String,
    /// Canonical source variable identifier if resolved (e.g. "user_id")
    pub source_var: String,
    /// 1-indexed call line number
    pub call_line: usize,
    /// Flow classification: "direct_param", "intermediate_var", "field_access", "return_val"
    pub flow_type: String,
}
