use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod community;
pub mod leiden;
pub mod persistence;
pub mod query;

pub use community::{Community, CommunityHierarchy, CommunityLevel, CommunitySummary, GraphEdge, GraphEntity};
pub use query::{GraphRagQueryMode, QueryResult};

use crate::context::db::CodeGraphDb;

pub struct GraphRagEngine {
    project_path: PathBuf,
}

impl GraphRagEngine {
    pub fn new(project_path: &Path) -> Self {
        Self {
            project_path: project_path.to_path_buf(),
        }
    }

    /// Builds or updates the community hierarchy from the project's code graph database.
    pub fn build_or_update(&self, detected_architecture: &str) -> Result<CommunityHierarchy> {
        let db = CodeGraphDb::open_for_project(&self.project_path)?;

        let project_name = self
            .project_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".into());

        // Extract all entities (symbols) from database
        let mut stmt = db.conn.prepare(
            "SELECT id, name, kind, file_path, start_line, end_line, signature FROM symbols",
        )?;
        let entity_iter = stmt.query_map([], |row| {
            Ok(GraphEntity {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                file_path: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                signature: row.get(6)?,
            })
        })?;

        let entities: Vec<GraphEntity> = entity_iter.filter_map(|r| r.ok()).collect();

        // Extract all edges (calls/imports) from database
        let mut edge_stmt = db.conn.prepare(
            "SELECT source_id, target_id, edge_type, weight FROM symbol_edges",
        )?;
        let edge_iter = edge_stmt.query_map([], |row| {
            Ok(GraphEdge {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                edge_type: row.get(2)?,
                weight: row.get(3)?,
            })
        })?;

        let edges: Vec<GraphEdge> = edge_iter.filter_map(|r| r.ok()).collect();

        // Perform hierarchical Leiden clustering
        let hierarchy = leiden::build_community_hierarchy(
            &project_name,
            &entities,
            &edges,
            detected_architecture,
        );

        // Persist the hierarchy
        let _ = persistence::save_hierarchy(&self.project_path, &hierarchy);

        Ok(hierarchy)
    }

    /// Loads active hierarchy from disk cache or builds it on-demand.
    pub fn load_or_build(&self, detected_architecture: &str) -> CommunityHierarchy {
        if let Some(h) = persistence::load_hierarchy(&self.project_path) {
            h
        } else {
            self.build_or_update(detected_architecture)
                .unwrap_or_else(|_| CommunityHierarchy::new("project", detected_architecture))
        }
    }

    /// Executes GraphRAG query in specified mode (global, local, drift).
    pub fn query(&self, query_text: &str, mode: GraphRagQueryMode, detected_architecture: &str) -> Result<QueryResult> {
        let hierarchy = self.load_or_build(detected_architecture);
        let db = CodeGraphDb::open_for_project(&self.project_path)?;

        match mode {
            GraphRagQueryMode::Global => Ok(query::execute_global_search(query_text, &hierarchy)),
            GraphRagQueryMode::Local => query::execute_local_search(query_text, &db, &hierarchy),
            GraphRagQueryMode::Drift => query::execute_drift_search(query_text, &db, &hierarchy),
            GraphRagQueryMode::Basic => Ok(query::execute_global_search(query_text, &hierarchy)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_rag_engine_hierarchy() {
        let entities = vec![
            GraphEntity {
                id: "sym1".into(),
                name: "UserService".into(),
                kind: "struct".into(),
                file_path: "src/service/user.rs".into(),
                start_line: 1,
                end_line: 20,
                signature: Some("pub struct UserService".into()),
            },
            GraphEntity {
                id: "sym2".into(),
                name: "UserRepository".into(),
                kind: "trait".into(),
                file_path: "src/domain/repo.rs".into(),
                start_line: 1,
                end_line: 15,
                signature: Some("pub trait UserRepository".into()),
            },
        ];

        let edges = vec![GraphEdge {
            source_id: "sym1".into(),
            target_id: "sym2".into(),
            edge_type: "IMPORTS".into(),
            weight: 1.0,
        }];

        let hierarchy = leiden::build_community_hierarchy("test_app", &entities, &edges, "Clean_Architecture");
        assert_eq!(hierarchy.project_name, "test_app");
        assert_eq!(hierarchy.detected_architecture, "Clean_Architecture");
        assert!(!hierarchy.communities.is_empty());

        let global = query::execute_global_search("user", &hierarchy);
        assert!(!global.sections.is_empty());
    }
}
