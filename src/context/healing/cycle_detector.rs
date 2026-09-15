//! Circular dependency detection between files using directed DFS.

use anyhow::Result;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

/// Detects circular file-to-file dependencies in the code graph.
pub fn detect_file_cycles(conn: &Connection) -> Result<Vec<Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT s1.file_path, s2.file_path
         FROM symbol_edges e
         JOIN symbols s1 ON e.source_id = s1.id
         JOIN symbols s2 ON e.target_id = s2.id
         WHERE s1.file_path != s2.file_path",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for r in rows.flatten() {
        adj.entry(r.0).or_default().insert(r.1);
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut rec_stack: Vec<String> = Vec::new();
    let mut rec_set: HashSet<String> = HashSet::new();
    let mut cycles: Vec<Vec<String>> = Vec::new();

    let all_nodes: Vec<String> = adj.keys().cloned().collect();
    for node in all_nodes {
        if !visited.contains(&node) {
            dfs_detect(
                &node,
                &adj,
                &mut visited,
                &mut rec_stack,
                &mut rec_set,
                &mut cycles,
            );
        }
    }

    // Deduplicate rotation-equivalent cycles
    let mut canonical_cycles: HashSet<String> = HashSet::new();
    let mut unique_cycles = Vec::new();

    for mut c in cycles {
        if c.is_empty() {
            continue;
        }
        // Normalize rotation to start with minimum element
        let min_idx = c
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| *v)
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        c.rotate_left(min_idx);
        let key = c.join(" -> ");
        if canonical_cycles.insert(key) {
            unique_cycles.push(c);
        }
    }

    Ok(unique_cycles)
}

fn dfs_detect(
    u: &str,
    adj: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut Vec<String>,
    rec_set: &mut HashSet<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(u.to_string());
    rec_stack.push(u.to_string());
    rec_set.insert(u.to_string());

    if let Some(neighbors) = adj.get(u) {
        for v in neighbors {
            if !visited.contains(v) {
                dfs_detect(v, adj, visited, rec_stack, rec_set, cycles);
            } else if rec_set.contains(v) {
                // Cycle found: slice from v to end of rec_stack
                if let Some(pos) = rec_stack.iter().position(|x| x == v) {
                    let cycle = rec_stack[pos..].to_vec();
                    if cycle.len() >= 2 {
                        cycles.push(cycle);
                    }
                }
            }
        }
    }

    rec_stack.pop();
    rec_set.remove(u);
}
