use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerHealthResponse {
    pub status: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub uptime_secs: u64,
    #[serde(default)]
    pub model_loaded: bool,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub total_skills: usize,
    #[serde(default)]
    pub total_sections: usize,
    #[serde(default)]
    pub memory_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SkillStatsResponse {
    pub total_skills: usize,
    pub total_sections: usize,
    pub vectors_dim: usize,
    #[serde(default)]
    pub last_reindex_at: u64,
    pub catalog_hash: String,
    #[serde(default)]
    pub tombstones_count: usize,
    #[serde(default)]
    pub token_savings_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSearchRequest {
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub threshold: Option<f32>,
}

fn default_max_results() -> Option<usize> {
    Some(5)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillHit {
    pub name: String,
    pub score: f32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub sections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillSearchHit {
    pub name: String,
    pub score: f32,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub sections: usize,
    #[serde(default)]
    pub matched_section: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillSearchResponse {
    #[serde(alias = "results")]
    pub skills: Vec<SkillSearchHit>,
    #[serde(default)]
    pub total_found: usize,
    #[serde(default)]
    pub catalog_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSliceRequest {
    #[serde(alias = "skill_name")]
    pub skill: String,
    #[serde(alias = "prompt")]
    pub task: String,
    #[serde(default)]
    pub max_sections: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillSliceResponse {
    #[serde(alias = "skill_name")]
    pub skill: String,
    #[serde(alias = "sections_content")]
    pub content: String,
    #[serde(default)]
    pub token_count: usize,
    #[serde(default)]
    pub original_token_count: usize,
    #[serde(default)]
    pub catalog_hash: String,
}
