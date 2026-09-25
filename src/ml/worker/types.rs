use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
    pub threshold: Option<f32>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHit {
    pub name: String,
    pub score: f32,
    pub title: String,
    pub intent: String,
    pub sections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SkillHit>,
    pub total_found: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SliceRequest {
    pub skill: String,
    pub task: String,
    pub max_sections: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceResponse {
    pub skill: String,
    pub content: String,
    pub token_count: usize,
    pub original_token_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteRequest {
    pub names: Vec<String>,
    pub purge_staging: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReindexRequest {
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedRequest {
    pub texts: Vec<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub engine: String,
    pub backend: String,
    pub total_skills: usize,
    pub total_sections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStats {
    pub total_skills: usize,
    pub total_sections: usize,
    pub vectors_dim: usize,
    pub catalog_hash: String,
    pub last_reindex_at: u64,
    pub tombstones_count: usize,
    pub token_savings_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub name: String,
    pub sections: usize,
    pub updated_at: u64,
}
