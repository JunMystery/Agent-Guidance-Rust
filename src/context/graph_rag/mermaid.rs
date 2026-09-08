use std::collections::{HashMap, HashSet};
use super::community::{CommunityHierarchy, CommunityLevel, GraphEdge};

pub fn sanitize_mermaid_id(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if sanitized.is_empty() || sanitized.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
        format!("node_{}", sanitized)
    } else {
        sanitized
    }
}

pub fn sanitize_mermaid_label(raw: &str) -> String {
    raw.replace('"', "'")
        .replace(['[', ']', '(', ')', '{', '}', '<', '>'], " ")
        .trim()
        .to_string()
}

pub fn generate_architecture_mermaid(
    hierarchy: &CommunityHierarchy,
    edges: &[GraphEdge],
) -> String {
    let mut out = String::from("graph TD\n");

    let macro_comms = hierarchy.get_by_level(CommunityLevel::MacroSubsystem);
    if macro_comms.is_empty() {
        out.push_str("    subgraph Architecture [\"Detected Architecture\"]\n");
        out.push_str(&format!(
            "        Arch[\"{}\"]\n",
            sanitize_mermaid_label(&hierarchy.detected_architecture)
        ));
        out.push_str("    end\n");
        return out;
    }

    let mut layer_map: HashMap<String, Vec<&super::community::Community>> = HashMap::new();
    let mut entity_to_comm: HashMap<&str, String> = HashMap::new();

    for comm in macro_comms {
        let layer = if comm.summary.layer.trim().is_empty() {
            "Shared".to_string()
        } else {
            comm.summary.layer.clone()
        };
        layer_map.entry(layer).or_default().push(comm);

        let safe_comm_id = sanitize_mermaid_id(&comm.id);
        for member in &comm.member_entity_ids {
            entity_to_comm.insert(member.as_str(), safe_comm_id.clone());
        }
    }

    // Render Subgraphs by Layer
    for (layer_name, comms) in layer_map {
        let safe_subgraph_id = sanitize_mermaid_id(&layer_name);
        out.push_str(&format!(
            "    subgraph {} [\"{} Layer\"]\n",
            safe_subgraph_id,
            sanitize_mermaid_label(&layer_name)
        ));
        for comm in comms {
            let safe_id = sanitize_mermaid_id(&comm.id);
            let safe_title = sanitize_mermaid_label(&comm.summary.title);
            out.push_str(&format!("        {}[\"{}\"]\n", safe_id, safe_title));
        }
        out.push_str("    end\n");
    }

    // Connect Inter-Community Edges
    let mut rendered_edges: HashSet<(String, String)> = HashSet::new();
    for edge in edges {
        if let (Some(src_comm), Some(tgt_comm)) = (
            entity_to_comm.get(edge.source_id.as_str()),
            entity_to_comm.get(edge.target_id.as_str()),
        ) {
            if src_comm != tgt_comm && !rendered_edges.contains(&(src_comm.clone(), tgt_comm.clone())) {
                out.push_str(&format!("    {} --> {}\n", src_comm, tgt_comm));
                rendered_edges.insert((src_comm.clone(), tgt_comm.clone()));
            }
        }
    }

    out
}

pub fn generate_blast_radius_mermaid(
    target_symbol: &str,
    callers: &[String],
    callees: &[String],
) -> String {
    let mut out = String::from("graph LR\n");
    let safe_target = sanitize_mermaid_id(target_symbol);
    let target_label = sanitize_mermaid_label(target_symbol);

    out.push_str(&format!("    {}[\"{}\"]\n", safe_target, target_label));

    for caller in callers {
        let s_id = sanitize_mermaid_id(caller);
        let s_label = sanitize_mermaid_label(caller);
        out.push_str(&format!("    {}[\"{}\"] --> {}\n", s_id, s_label, safe_target));
        out.push_str(&format!("    style {} fill:#2b3a4a,stroke:#4a90e2,stroke-width:1px\n", s_id));
    }

    for callee in callees {
        let s_id = sanitize_mermaid_id(callee);
        let s_label = sanitize_mermaid_label(callee);
        out.push_str(&format!("    {} --> {}[\"{}\"]\n", safe_target, s_id, s_label));
        out.push_str(&format!("    style {} fill:#2b4a3a,stroke:#50e3c2,stroke-width:1px\n", s_id));
    }

    out.push_str(&format!(
        "    style {} fill:#4a3b2b,stroke:#f5a623,stroke-width:2px\n",
        safe_target
    ));

    out
}

#[cfg(test)]
#[path = "mermaid_tests.rs"]
mod tests;

