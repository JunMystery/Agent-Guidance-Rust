use super::super::community::{Community, CommunityHierarchy, CommunityLevel, CommunitySummary, GraphEdge};
use super::*;

#[test]
fn test_sanitization() {
    assert_eq!(sanitize_mermaid_id("crate::service::User-Service"), "crate__service__User_Service");
    assert_eq!(sanitize_mermaid_id("123node"), "node_123node");
    assert_eq!(sanitize_mermaid_label("User<T>[Auth] (v1)"), "User T  Auth   v1");
}

#[test]
fn test_generate_architecture_mermaid_empty() {
    let hierarchy = CommunityHierarchy::new("sample_proj", "Clean_Architecture");
    let mermaid = generate_architecture_mermaid(&hierarchy, &[]);
    assert!(mermaid.starts_with("graph TD\n"));
    assert!(mermaid.contains("subgraph Architecture"));
    assert!(mermaid.contains("Clean_Architecture"));
}

#[test]
fn test_generate_architecture_mermaid_with_communities() {
    let mut hierarchy = CommunityHierarchy::new("sample_proj", "Clean_Architecture");
    let comm1 = Community {
        id: "comm_ui".to_string(),
        level: CommunityLevel::MacroSubsystem,
        parent_id: None,
        member_entity_ids: vec!["sym_view".to_string()],
        member_files: vec!["src/ui/view.rs".to_string()],
        summary: CommunitySummary {
            title: "UI Views".to_string(),
            description: "Presentation controllers".to_string(),
            layer: "Presentation".to_string(),
            key_entities: vec!["sym_view".to_string()],
            export_interfaces: vec![],
            dependencies: vec![],
        },
    };
    let comm2 = Community {
        id: "comm_repo".to_string(),
        level: CommunityLevel::MacroSubsystem,
        parent_id: None,
        member_entity_ids: vec!["sym_db".to_string()],
        member_files: vec!["src/db/repo.rs".to_string()],
        summary: CommunitySummary {
            title: "Repository Layer".to_string(),
            description: "Database adapters".to_string(),
            layer: "Infrastructure".to_string(),
            key_entities: vec!["sym_db".to_string()],
            export_interfaces: vec![],
            dependencies: vec![],
        },
    };
    hierarchy.communities = vec![comm1, comm2];

    let edges = vec![GraphEdge {
        source_id: "sym_view".to_string(),
        target_id: "sym_db".to_string(),
        edge_type: "CALLS".to_string(),
        weight: 1.0,
    }];

    let mermaid = generate_architecture_mermaid(&hierarchy, &edges);
    assert!(mermaid.contains("subgraph Presentation [\"Presentation Layer\"]"));
    assert!(mermaid.contains("subgraph Infrastructure [\"Infrastructure Layer\"]"));
    assert!(mermaid.contains("comm_ui --> comm_repo"));
}

#[test]
fn test_generate_blast_radius_mermaid() {
    let callers = vec!["handle_request".to_string(), "api_endpoint".to_string()];
    let callees = vec!["query_db".to_string(), "log_event".to_string()];
    let mermaid = generate_blast_radius_mermaid("UserService", &callers, &callees);

    assert!(mermaid.starts_with("graph LR\n"));
    assert!(mermaid.contains("[\"UserService\"]"));
    assert!(mermaid.contains("handle_request[\"handle_request\"] --> UserService"));
    assert!(mermaid.contains("UserService --> query_db[\"query_db\"]"));
    assert!(mermaid.contains("style UserService fill:#4a3b2b"));
}
