use anyhow::Result;
use std::path::Path;

use super::bundle_snippet_extractor::{extract_call_site, extract_symbol_body, CodeSnippet};
use crate::context::db::CodeGraphDb;

#[derive(Debug, Clone)]
pub struct SubgraphTarget {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubgraphCaller {
    pub symbol_name: String,
    pub file_path: String,
    pub call_line: usize,
    pub edge_type: String,
    pub weight: f64,
    pub snippet: Option<CodeSnippet>,
}

#[derive(Debug, Clone)]
pub struct SubgraphCallee {
    pub symbol_name: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub edge_type: String,
    pub weight: f64,
    pub snippet: Option<CodeSnippet>,
}

pub struct SubgraphBundle {
    pub target: SubgraphTarget,
    pub target_snippet: Option<CodeSnippet>,
    pub callers: Vec<SubgraphCaller>,
    pub callees: Vec<SubgraphCallee>,
    pub blast_radius_score: f64,
    pub risk_level: String,
    pub total_loc: usize,
    pub loc_budget: usize,
}

pub fn build_subgraph_bundle(
    proj_path: &Path,
    target_query: &str,
    loc_budget: usize,
) -> Result<Option<SubgraphBundle>> {
    let clean_target = target_query.trim();
    if clean_target.is_empty() {
        return Ok(None);
    }

    let db = CodeGraphDb::open_for_project(proj_path)?;

    // 1. Locate target symbol
    let mut target_stmt = db.conn.prepare(
        "SELECT id, name, kind, file_path, start_line, end_line, signature
         FROM symbols
         WHERE name = ?1 OR id = ?1
         ORDER BY (end_line - start_line) DESC
         LIMIT 1",
    )?;

    let target_opt = target_stmt
        .query_row([clean_target], |row| {
            Ok(SubgraphTarget {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                file_path: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                signature: row.get(6)?,
            })
        })
        .ok();

    let target = match target_opt {
        Some(t) => t,
        None => return Ok(None),
    };

    // 2. Locate callers (1-hop inbound)
    let mut callers_stmt = db.conn.prepare(
        "SELECT s.name, s.file_path, COALESCE(e.call_line, s.start_line), e.edge_type, e.weight
         FROM symbol_edges e
         JOIN symbols s ON e.source_id = s.id
         WHERE e.target_id = ?1 OR e.target_id IN (SELECT id FROM symbols WHERE name = ?1)
         ORDER BY e.weight DESC, e.call_line ASC
         LIMIT 5",
    )?;

    let raw_callers: Vec<(String, String, usize, String, f64)> = callers_stmt
        .query_map([clean_target], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 3. Locate callees (1-hop outbound)
    let mut callees_stmt = db.conn.prepare(
        "SELECT s.name, s.file_path, s.start_line, s.end_line, e.edge_type, e.weight
         FROM symbol_edges e
         JOIN symbols s ON e.target_id = s.id
         WHERE e.source_id = ?1 OR e.source_id IN (SELECT id FROM symbols WHERE name = ?1)
         ORDER BY e.weight DESC, s.start_line ASC
         LIMIT 8",
    )?;

    let raw_callees: Vec<(String, String, usize, usize, String, f64)> = callees_stmt
        .query_map([clean_target], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 4. Calculate blast radius
    let raw_score = (raw_callers.len() * 2 + raw_callees.len() + 1) as f64;
    let blast_radius_score = raw_score.log2().clamp(1.0, 10.0);
    let risk_level = if blast_radius_score > 6.0 {
        "HIGH".to_string()
    } else if blast_radius_score > 3.0 {
        "MEDIUM".to_string()
    } else {
        "LOW".to_string()
    };

    // 5. Budget distribution
    let budget = loc_budget.clamp(50, 500);
    let target_budget = (budget * 40 / 100).clamp(25, 100);

    let full_target_path = proj_path.join(&target.file_path);
    let target_snippet = extract_symbol_body(
        &full_target_path,
        &target.file_path,
        target.start_line,
        target.end_line,
        target_budget,
    );

    let mut current_loc = target_snippet.as_ref().map(|s| s.line_count()).unwrap_or(0);

    // Callers snippets (Top 3)
    let mut callers = Vec::new();
    for (name, file, call_line, edge_type, weight) in raw_callers.into_iter().take(3) {
        let snippet = if current_loc + 8 <= budget {
            let full_p = proj_path.join(&file);
            let s = extract_call_site(&full_p, &file, call_line, 3);
            if let Some(ref snip) = s {
                current_loc += snip.line_count();
            }
            s
        } else {
            None
        };
        callers.push(SubgraphCaller {
            symbol_name: name,
            file_path: file,
            call_line,
            edge_type,
            weight,
            snippet,
        });
    }

    // Callees snippets (Top 5)
    let mut callees = Vec::new();
    for (name, file, start_line, end_line, edge_type, weight) in raw_callees.into_iter().take(5) {
        let snippet = if current_loc + 8 <= budget {
            let full_p = proj_path.join(&file);
            let max_callee_lines = 10.min(budget.saturating_sub(current_loc));
            let s = extract_symbol_body(&full_p, &file, start_line, end_line, max_callee_lines);
            if let Some(ref snip) = s {
                current_loc += snip.line_count();
            }
            s
        } else {
            None
        };
        callees.push(SubgraphCallee {
            symbol_name: name,
            file_path: file,
            start_line,
            end_line,
            edge_type,
            weight,
            snippet,
        });
    }

    Ok(Some(SubgraphBundle {
        target,
        target_snippet,
        callers,
        callees,
        blast_radius_score,
        risk_level,
        total_loc: current_loc,
        loc_budget: budget,
    }))
}
