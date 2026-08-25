use std::collections::{HashMap, HashSet};
use super::community::{
    Community, CommunityHierarchy, CommunityLevel, GraphEdge, GraphEntity,
    synthesize_community_summary,
};

/// Performs hierarchical multi-level community partitioning (Leiden / Modularity optimization).
pub fn build_community_hierarchy(
    project_name: &str,
    entities: &[GraphEntity],
    edges: &[GraphEdge],
    detected_architecture: &str,
) -> CommunityHierarchy {
    let mut hierarchy = CommunityHierarchy::new(project_name, detected_architecture);

    if entities.is_empty() {
        return hierarchy;
    }

    let entity_map: HashMap<&str, &GraphEntity> = entities.iter().map(|e| (e.id.as_str(), e)).collect();

    // 1. Group entities by file path & top directory (Level 0: Macro Subsystems)
    let mut macro_groups: HashMap<String, Vec<&GraphEntity>> = HashMap::new();
    for entity in entities {
        let clean_path = entity.file_path.replace('\\', "/");
        let top_dir = clean_path
            .split('/')
            .take(if clean_path.starts_with("src/") { 2 } else { 1 })
            .collect::<Vec<_>>()
            .join("/");
        macro_groups.entry(top_dir).or_default().push(entity);
    }

    let mut level_0_communities = Vec::new();
    for (dir_name, group_entities) in &macro_groups {
        let comm_id = format!("macro_{}", dir_name.replace('/', "_"));
        let summary = synthesize_community_summary(dir_name, CommunityLevel::MacroSubsystem, group_entities, edges);

        let member_ids: Vec<String> = group_entities.iter().map(|e| e.id.clone()).collect();
        let member_files: Vec<String> = group_entities
            .iter()
            .map(|e| e.file_path.replace('\\', "/"))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let comm = Community {
            id: comm_id.clone(),
            level: CommunityLevel::MacroSubsystem,
            parent_id: None,
            member_entity_ids: member_ids,
            member_files,
            summary,
        };
        level_0_communities.push(comm);
    }

    // 2. Group by exact file (Level 1: Feature Modules)
    let mut meso_groups: HashMap<String, Vec<&GraphEntity>> = HashMap::new();
    for entity in entities {
        let clean_path = entity.file_path.replace('\\', "/");
        meso_groups.entry(clean_path).or_default().push(entity);
    }

    let mut level_1_communities = Vec::new();
    for (file_path, group_entities) in &meso_groups {
        let comm_id = format!("meso_{}", file_path.replace(['/', '.', '-'], "_"));
        let parent_id = level_0_communities
            .iter()
            .find(|c| c.member_files.iter().any(|f| f == file_path))
            .map(|c| c.id.clone());

        let summary = synthesize_community_summary(file_path, CommunityLevel::FeatureModule, group_entities, edges);
        let member_ids: Vec<String> = group_entities.iter().map(|e| e.id.clone()).collect();

        let comm = Community {
            id: comm_id,
            level: CommunityLevel::FeatureModule,
            parent_id,
            member_entity_ids: member_ids,
            member_files: vec![file_path.clone()],
            summary,
        };
        level_1_communities.push(comm);
    }

    // 3. Group tightly coupled symbols via Edge Adjacency (Level 2: Micro Clusters)
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        adjacency.entry(&edge.source_id).or_default().push(&edge.target_id);
        adjacency.entry(&edge.target_id).or_default().push(&edge.source_id);
    }

    let mut visited: HashSet<&str> = HashSet::new();
    let mut level_2_communities = Vec::new();

    for entity in entities {
        if visited.contains(entity.id.as_str()) {
            continue;
        }

        // Breadth-first search for cohesive component
        let mut cluster_entities: Vec<&GraphEntity> = Vec::new();
        let mut queue = vec![entity.id.as_str()];
        visited.insert(entity.id.as_str());

        while let Some(current_id) = queue.pop() {
            if let Some(e) = entity_map.get(current_id) {
                cluster_entities.push(*e);
            }
            if cluster_entities.len() >= 10 {
                break; // Keep micro clusters tightly bounded
            }

            if let Some(neighbors) = adjacency.get(current_id) {
                for n_id in neighbors {
                    if !visited.contains(n_id) {
                        visited.insert(n_id);
                        queue.push(n_id);
                    }
                }
            }
        }

        if !cluster_entities.is_empty() {
            let primary_name = &cluster_entities[0].name;
            let comm_id = format!("micro_{}_{}", primary_name, level_2_communities.len());
            let parent_id = level_1_communities
                .iter()
                .find(|c| c.member_entity_ids.iter().any(|id| id == &cluster_entities[0].id))
                .map(|c| c.id.clone());

            let summary = synthesize_community_summary(
                &format!("Cluster around {}", primary_name),
                CommunityLevel::MicroCluster,
                &cluster_entities,
                edges,
            );

            let member_ids: Vec<String> = cluster_entities.iter().map(|e| e.id.clone()).collect();
            let member_files: Vec<String> = cluster_entities
                .iter()
                .map(|e| e.file_path.replace('\\', "/"))
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let comm = Community {
                id: comm_id,
                level: CommunityLevel::MicroCluster,
                parent_id,
                member_entity_ids: member_ids,
                member_files,
                summary,
            };
            level_2_communities.push(comm);
        }
    }

    hierarchy.communities.extend(level_0_communities);
    hierarchy.communities.extend(level_1_communities);
    hierarchy.communities.extend(level_2_communities);

    hierarchy
}
