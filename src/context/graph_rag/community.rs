use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommunityLevel {
    MacroSubsystem = 0, // Level 0: Macro subsystems (e.g. Domain, MCP, ML, Context)
    FeatureModule = 1,  // Level 1: Feature modules & service boundaries
    MicroCluster = 2,   // Level 2: Micro symbol clusters (Structs + Methods)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntity {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitySummary {
    pub title: String,
    pub layer: String,
    pub description: String,
    pub key_entities: Vec<String>,
    pub export_interfaces: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    pub id: String,
    pub level: CommunityLevel,
    pub parent_id: Option<String>,
    pub member_entity_ids: Vec<String>,
    pub member_files: Vec<String>,
    pub summary: CommunitySummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommunityHierarchy {
    pub project_name: String,
    pub updated_at: u64,
    pub detected_architecture: String,
    pub communities: Vec<Community>,
}

impl CommunityHierarchy {
    pub fn new(project_name: &str, detected_architecture: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            project_name: project_name.to_string(),
            updated_at: now,
            detected_architecture: detected_architecture.to_string(),
            communities: Vec::new(),
        }
    }

    pub fn get_by_level(&self, level: CommunityLevel) -> Vec<&Community> {
        self.communities.iter().filter(|c| c.level == level).collect()
    }

    pub fn find_community_for_entity(&self, entity_id: &str) -> Option<&Community> {
        self.communities
            .iter()
            .find(|c| c.member_entity_ids.iter().any(|id| id == entity_id))
    }

    pub fn find_community_for_file(&self, file_path: &str) -> Option<&Community> {
        let clean = file_path.replace('\\', "/");
        self.communities
            .iter()
            .find(|c| c.member_files.iter().any(|f| f == &clean))
    }
}

/// Generates a structured bottom-up summary for a detected community.
pub fn synthesize_community_summary(
    title: &str,
    level: CommunityLevel,
    entities: &[&GraphEntity],
    edges: &[GraphEdge],
) -> CommunitySummary {
    let mut file_set = HashSet::new();
    let mut key_entities = Vec::new();
    let mut export_interfaces = Vec::new();
    let mut deps_set = HashSet::new();

    let mut layer_scores: HashMap<&str, usize> = HashMap::new();

    for entity in entities {
        file_set.insert(&entity.file_path);
        let path_lower = entity.file_path.to_lowercase();

        // Layer inference
        if path_lower.contains("domain") || path_lower.contains("entities") {
            *layer_scores.entry("Domain").or_insert(0) += 2;
        } else if path_lower.contains("usecase") || path_lower.contains("service") {
            *layer_scores.entry("Service/Usecase").or_insert(0) += 2;
        } else if path_lower.contains("mcp") || path_lower.contains("router") || path_lower.contains("handler") {
            *layer_scores.entry("Presentation/Interface").or_insert(0) += 2;
        } else if path_lower.contains("db") || path_lower.contains("storage") || path_lower.contains("infra") {
            *layer_scores.entry("Infrastructure").or_insert(0) += 2;
        } else if path_lower.contains("ml") || path_lower.contains("embedding") {
            *layer_scores.entry("Machine Learning / Vector").or_insert(0) += 2;
        }

        if key_entities.len() < 8 {
            key_entities.push(format!("{} ({})", entity.name, entity.kind));
        }

        if entity.kind == "trait" || entity.kind == "interface" || entity.signature.as_deref().unwrap_or("").starts_with("pub fn") {
            if export_interfaces.len() < 6 {
                export_interfaces.push(entity.name.clone());
            }
        }
    }

    let detected_layer = layer_scores
        .into_iter()
        .max_by_key(|(_, score)| *score)
        .map(|(l, _)| l.to_string())
        .unwrap_or_else(|| match level {
            CommunityLevel::MacroSubsystem => "Core Subsystem".to_string(),
            CommunityLevel::FeatureModule => "Feature Module".to_string(),
            CommunityLevel::MicroCluster => "Symbol Cluster".to_string(),
        });

    for edge in edges {
        if !deps_set.contains(&edge.target_id) && deps_set.len() < 6 {
            deps_set.insert(edge.target_id.clone());
        }
    }

    let description = format!(
        "Community '{}' encompassing {} symbols across {} files. Acts as {} layer.",
        title,
        entities.len(),
        file_set.len(),
        detected_layer
    );

    CommunitySummary {
        title: title.to_string(),
        layer: detected_layer,
        description,
        key_entities,
        export_interfaces,
        dependencies: deps_set.into_iter().collect(),
    }
}
