use super::handlers::*;
use super::state::WorkerState;
use super::types::*;
use crate::ml::embeddings::binary_format::SkillRecord;

#[test]
fn test_worker_state_hot_swap_and_search() {
    let state = WorkerState::new_empty();

    let skills = vec![
        SkillRecord {
            name: "test-auth".to_string(),
            relative_path: "skills/test-auth/SKILL.md".to_string(),
            content: "# Authentication\n## Rules\nHandle tokens safely.\n## Examples\nJWT tokens.".to_string(),
        },
        SkillRecord {
            name: "test-db".to_string(),
            relative_path: "skills/test-db/SKILL.md".to_string(),
            content: "# Database\n## Rules\nUse transactions.".to_string(),
        },
    ];

    let v_auth = vec![1.0f32, 0.0, 0.0];
    let v_db = vec![0.0f32, 1.0, 0.0];
    let vectors = vec![v_auth, v_db];

    state.hot_swap(skills, vectors, "test-catalog-hash-123".to_string());

    // Health check
    let health = handle_health(&state);
    assert_eq!(health.status, "ok");
    assert_eq!(health.total_skills, 2);

    // Stats check
    let stats = handle_stats(&state);
    assert_eq!(stats.total_skills, 2);
    assert_eq!(stats.catalog_hash, "test-catalog-hash-123");
    assert_eq!(stats.vectors_dim, 3);

    // List check
    let list = handle_list(&state);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "test-auth");

    // Search check
    let q_auth = vec![0.9f32, 0.1, 0.0];
    let hits = state.search_top_k(&q_auth, 5, 0.5);
    assert!(!hits.is_empty());
    assert_eq!(hits[0].0, 0); // index 0 is test-auth

    // Slicing check
    let slice_req = SliceRequest {
        skill: "test-auth".to_string(),
        task: "Need JWT tokens guidance".to_string(),
        max_sections: Some(2),
    };
    let slice_res = handle_slice(&state, slice_req).unwrap();
    assert_eq!(slice_res.skill, "test-auth");
    assert!(slice_res.content.contains("JWT tokens") || slice_res.content.contains("Rules"));
    assert!(slice_res.token_count <= slice_res.original_token_count);
}

#[test]
fn test_worker_stats_serialization() {
    let stats = SkillStats {
        total_skills: 280,
        total_sections: 1100,
        vectors_dim: 384,
        catalog_hash: "abcd1234efgh".to_string(),
        last_reindex_at: 1700000000,
        tombstones_count: 2,
        token_savings_ratio: 0.81,
    };
    let json = serde_json::to_string(&stats).unwrap();
    let decoded: SkillStats = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.total_skills, 280);
    assert_eq!(decoded.catalog_hash, "abcd1234efgh");
}
