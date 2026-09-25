use super::cache::{ensure_cache_tables, get_cached_remote_stats, get_cached_slice, save_cached_remote_stats, save_cached_slice};
use super::http::RemoteMlClient;
use super::types::*;
use crate::config::ServerConfig;
use rusqlite::Connection;

#[test]
fn test_remote_client_construction() {
    let mut config = ServerConfig::default();
    config.url = "http://10.0.0.1:9000".to_string();
    config.timeout_ms = 4000;
    config.api_key = "bearer-test".to_string();

    let client = RemoteMlClient::new(&config);
    assert_eq!(client.config.url, "http://10.0.0.1:9000");
    assert_eq!(client.config.api_key, "bearer-test");
}

#[test]
fn test_skill_stats_json_roundtrip() {
    let stats = SkillStatsResponse {
        total_skills: 284,
        total_sections: 1142,
        vectors_dim: 384,
        last_reindex_at: 1700000000,
        catalog_hash: "a3f5b72e91".to_string(),
        tombstones_count: 2,
        token_savings_ratio: 0.81,
    };

    let serialized = serde_json::to_string(&stats).expect("Serialize JSON");
    let deserialized: SkillStatsResponse = serde_json::from_str(&serialized).expect("Deserialize JSON");

    assert_eq!(stats, deserialized);
}

#[test]
fn test_skill_search_response_json_roundtrip() {
    let res = SkillSearchResponse {
        skills: vec![SkillSearchHit {
            name: "android-clean-architecture".to_string(),
            score: 0.92,
            title: "Android Clean Architecture".to_string(),
            intent: "Clean architecture guidance".to_string(),
            sections: 5,
            summary: "Clean architecture patterns for Android".to_string(),
            matched_section: Some("Repository Pattern".to_string()),
        }],
        total_found: 1,
        catalog_hash: "hash123".to_string(),
    };

    let serialized = serde_json::to_string(&res).expect("Serialize JSON");
    let deserialized: SkillSearchResponse = serde_json::from_str(&serialized).expect("Deserialize JSON");

    assert_eq!(res, deserialized);
}

#[test]
fn test_skill_slice_response_json_roundtrip() {
    let res = SkillSliceResponse {
        skill: "rust-patterns".to_string(),
        content: "## Idiomatic Rust\nUse Result and Option.".to_string(),
        token_count: 45,
        original_token_count: 320,
        catalog_hash: "cat_hash_77".to_string(),
    };

    let serialized = serde_json::to_string(&res).expect("Serialize JSON");
    let deserialized: SkillSliceResponse = serde_json::from_str(&serialized).expect("Deserialize JSON");

    assert_eq!(res, deserialized);
}

#[test]
fn test_sqlite_cache_table_creation() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_cache_tables(&conn).unwrap();

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='remote_skill_cache'")
        .unwrap();
    let exists = stmt.exists([]).unwrap();
    assert!(exists);
}
