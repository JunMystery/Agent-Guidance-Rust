//! Strongly typed serde data contracts for multi-view graph API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileGraphResponse {
    pub graph_available: bool,
    pub view: String,
    pub project_path: String,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes: Vec<FileGraphNode>,
    pub edges: Vec<FileGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileGraphNode {
    pub id: String,
    pub path: String,
    pub loc: usize,
    pub total_symbols: usize,
    pub in_degree: usize,
    pub out_degree: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileGraphEdge {
    pub source: String,
    pub target: String,
    pub weight: f64,
    pub calls: Vec<FileCallDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCallDetail {
    pub source_func: String,
    pub target_func: String,
    pub caller: String,
    pub callee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    pub edge_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFunctionsResponse {
    pub graph_available: bool,
    pub view: String,
    pub project_path: String,
    pub file: String,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes: Vec<FunctionGraphNode>,
    pub edges: Vec<FunctionGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionGraphNode {
    pub id: String,
    pub name: String,
    pub label: String,
    pub kind: String,
    pub file_path: String,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub loc: usize,
    pub is_external: bool,
    pub scope: String,
    pub deg: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionGraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub direction: String,
    pub weight: f64,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_line: Option<usize>,
    pub origin: String,
    pub dashed: bool,
}

pub struct GraphQueryResult {
    pub nodes: Vec<serde_json::Value>,
    pub edges: Vec<serde_json::Value>,
}
