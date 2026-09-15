use anyhow::Result;
use std::path::Path;

use super::bundle_snippet_extractor::{build_safe_path, extract_call_site, extract_symbol_body, CodeSnippet};
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
    pub total_callers_count: usize,
    pub total_callees_count: usize,
    pub blast_radius_score: f64,
    pub risk_level: String,
    pub total_loc: usize,
    pub loc_budget: usize,
}

pub fn build_subgraph_bundle(
    proj_path: &Path,
    target_query: &str,
    rel_path_hint: Option<&str>,
    loc_budget: usize,
) -> Result<Option<SubgraphBundle>> {
    let clean_query = target_query.trim();
    if clean_query.is_empty() {
        return Ok(None);
    }

    // Support "file#symbol" or "file::symbol" format
    let (inferred_path, clean_target) = if let Some((file, sym)) = clean_query.split_once('#') {
        (Some(file.trim()), sym.trim())
    } else if let Some((file, sym)) = clean_query.split_once("::").filter(|(f, _)| f.ends_with(".rs") || f.ends_with(".py") || f.ends_with(".ts") || f.ends_with(".js")) {
        (Some(file.trim()), sym.trim())
    } else {
        (None, clean_query)
    };

    if clean_target.is_empty() {
        return Ok(None);
    }

    let target_file_raw = rel_path_hint.or(inferred_path).unwrap_or("").trim().replace('\\', "/");
    let target_file = target_file_raw.trim_start_matches('/').to_string();

    let db = CodeGraphDb::open_for_project(proj_path)?;

    // 1. Locate target symbol (cross-platform path matching via REPLACE)
    let mut target_stmt = db.conn.prepare(
        "SELECT id, name, kind, file_path, start_line, end_line, signature
         FROM symbols
         WHERE (name = ?1 OR id = ?1)
           AND (?2 = '' OR REPLACE(file_path, '\\', '/') = ?2 OR REPLACE(file_path, '\\', '/') LIKE '%' || ?2)
         ORDER BY (REPLACE(file_path, '\\', '/') = ?2) DESC, (end_line - start_line) DESC
         LIMIT 1",
    )?;

    let target_opt = target_stmt
        .query_row(rusqlite::params![clean_target, target_file], |row| {
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

    // 2. Count total callers & fetch top callers (1-hop inbound calls only, excluding self-recursion)
    let total_callers_count: usize = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM symbol_edges WHERE (target_id = ?1 OR target_id = ?2) AND edge_type = 'calls' AND source_id != ?1 AND source_id != ?2",
            rusqlite::params![target.id, target.name],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut callers_stmt = db.conn.prepare(
        "SELECT s.name, s.file_path, COALESCE(e.call_line, s.start_line), e.edge_type, e.weight
         FROM symbol_edges e
         JOIN symbols s ON e.source_id = s.id
         WHERE (e.target_id = ?1 OR e.target_id = ?2) AND e.edge_type = 'calls' AND s.id != ?1 AND s.id != ?2
         ORDER BY (e.call_line IS NOT NULL) DESC, e.weight DESC, e.call_line ASC
         LIMIT 5",
    )?;

    let raw_callers: Vec<(String, String, usize, String, f64)> = callers_stmt
        .query_map(rusqlite::params![target.id, target.name], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 3. Count total callees & fetch top callees (1-hop outbound calls only, excluding self-recursion)
    let total_callees_count: usize = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM symbol_edges WHERE (source_id = ?1 OR source_id = ?2) AND edge_type = 'calls' AND target_id != ?1 AND target_id != ?2",
            rusqlite::params![target.id, target.name],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut callees_stmt = db.conn.prepare(
        "SELECT s.name, s.file_path, s.start_line, s.end_line, e.edge_type, e.weight
         FROM symbol_edges e
         JOIN symbols s ON e.target_id = s.id
         WHERE (e.source_id = ?1 OR e.source_id = ?2) AND e.edge_type = 'calls' AND s.id != ?1 AND s.id != ?2
         ORDER BY e.weight DESC, s.start_line ASC
         LIMIT 8",
    )?;

    let raw_callees: Vec<(String, String, usize, usize, String, f64)> = callees_stmt
        .query_map(rusqlite::params![target.id, target.name], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 4. Calculate blast radius accurately from full counts
    let raw_score = (total_callers_count * 3 + total_callees_count + 1) as f64;
    let blast_radius_score = raw_score.log2().clamp(1.0, 10.0);
    let risk_level = if blast_radius_score > 5.0 {
        "HIGH".to_string()
    } else if blast_radius_score > 2.5 {
        "MEDIUM".to_string()
    } else {
        "LOW".to_string()
    };

    // 5. Budget distribution
    let budget = loc_budget.clamp(50, 500);
    let target_budget = (budget * 40 / 100).clamp(25, 100);

    let full_target_path = build_safe_path(proj_path, &target.file_path);
    let target_snippet = extract_symbol_body(
        &full_target_path,
        &target.file_path,
        target.start_line,
        target.end_line,
        target_budget,
    );

    let mut current_loc = target_snippet.as_ref().map(|s| s.line_count()).unwrap_or(0);

    // Callers snippets (preserve full list, attach snippets within budget)
    let mut callers = Vec::new();
    for (name, file, call_line, edge_type, weight) in raw_callers {
        let snippet = if current_loc + 8 <= budget {
            let full_p = build_safe_path(proj_path, &file);
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

    // Callees snippets (preserve full list, attach snippets within budget)
    let mut callees = Vec::new();
    for (name, file, start_line, end_line, edge_type, weight) in raw_callees {
        let snippet = if current_loc + 8 <= budget {
            let full_p = build_safe_path(proj_path, &file);
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
        total_callers_count,
        total_callees_count,
        blast_radius_score,
        risk_level,
        total_loc: current_loc,
        loc_budget: budget,
    }))
}
