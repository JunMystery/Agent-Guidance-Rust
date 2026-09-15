//! Cross-file data flow resolution and persistence into data_flow_edges.

use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use crate::context::ast::dataflow_def::{CallArgDetail, FormalParam};
use crate::context::ast::{AstEngine, AstLanguage, extract_dataflow};
use super::cross_edges::SymbolMeta;

/// Analyzes all files in the project for formal parameters and intra-procedural argument flow.
pub fn resolve_dataflow_edges(
    conn: &mut Connection,
    files: &[String],
    project_path: &Path,
    name_to_syms: &HashMap<String, Vec<SymbolMeta>>,
) -> Result<usize> {
    let mut file_func_params: HashMap<String, HashMap<String, Vec<FormalParam>>> = HashMap::new();
    let mut all_call_args: Vec<(String, String, CallArgDetail)> = Vec::new(); // (file_path, caller_name, arg)

    // 1. AST scan for parameter signatures and call-site arguments
    for rel_path in files {
        let full_path = project_path.join(rel_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lang = AstLanguage::from_path(rel_path);
        if lang.is_tier1() {
            if let Some(tree) = AstEngine::parse(&content, lang) {
                let df_data = extract_dataflow(tree.root_node(), content.as_bytes(), lang);
                for (func_name, params, call_args) in df_data {
                    file_func_params
                        .entry(rel_path.clone())
                        .or_default()
                        .insert(func_name.clone(), params);

                    for arg in call_args {
                        all_call_args.push((rel_path.clone(), func_name.clone(), arg));
                    }
                }
            }
        }
    }

    if all_call_args.is_empty() {
        return Ok(0);
    }

    // 2. Query call edges to map (caller_id, call_line) -> Vec<callee_id>
    let mut call_edge_map: HashMap<(String, usize), Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, call_line FROM symbol_edges WHERE edge_type = 'calls' AND call_line IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, usize>(2)?,
            ))
        })?;
        for r in rows.flatten() {
            let (src, tgt, line) = r;
            call_edge_map.entry((src, line)).or_default().push(tgt);
        }
    }

    // Precompute reverse lookup map (symbol_id -> (file_path, symbol_name)) for O(1) param lookup
    let mut id_to_meta: HashMap<&str, (&str, &str)> = HashMap::new();
    for (name, metas) in name_to_syms {
        for meta in metas {
            id_to_meta.insert(meta.id.as_str(), (meta.file_path.as_str(), name.as_str()));
        }
    }

    let mut records: Vec<(String, String, String, String, usize, usize, String, f64)> = Vec::new();

    // 3. Resolve caller symbol ID, callee symbol ID, and target parameter name
    for (file_path, caller_name, arg) in all_call_args {
        // Resolve caller ID
        let caller_id = if let Some(candidates) = name_to_syms.get(&caller_name) {
            candidates.iter().find(|c| c.file_path == file_path).map(|c| c.id.clone())
        } else {
            None
        };

        let Some(c_id) = caller_id else {
            continue;
        };

        // Resolve callee ID from exact call edge at this call line (matching callee_name if multiple calls on line)
        let callee_id = call_edge_map.get(&(c_id.clone(), arg.call_line)).and_then(|targets| {
            if targets.len() == 1 {
                Some(targets[0].clone())
            } else {
                targets.iter()
                    .find(|t| t.contains(&format!("::{}", arg.callee_name)) || t.ends_with(&arg.callee_name))
                    .or_else(|| targets.first())
                    .cloned()
            }
        }).or_else(|| {
            name_to_syms.get(&arg.callee_name).and_then(|cands| {
                if cands.len() == 1 {
                    Some(cands[0].id.clone())
                } else {
                    cands.iter().find(|c| c.file_path == file_path).map(|c| c.id.clone())
                }
            })
        });

        let Some(target_id) = callee_id else {
            continue;
        };

        // Resolve target parameter name if formal parameters were extracted (O(1))
        let target_param = resolve_target_param(&target_id, arg.arg_index, &file_func_params, &id_to_meta);

        let conf = if arg.flow_type == "direct_param" { 0.95 } else { 0.85 };

        records.push((
            c_id,
            target_id,
            arg.source_var,
            target_param,
            arg.arg_index,
            arg.call_line,
            arg.flow_type,
            conf,
        ));
    }

    let count = records.len();
    if count > 0 {
        let tx = conn.transaction()?;
        {
            let mut insert_stmt = tx.prepare(
                "INSERT OR REPLACE INTO data_flow_edges (caller_symbol_id, callee_symbol_id, source_variable, target_parameter, arg_index, call_line, flow_type, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            )?;
            for (caller, callee, src_var, tgt_param, idx, line, f_type, conf) in records {
                let _ = insert_stmt.execute(params![caller, callee, src_var, tgt_param, idx as i64, line as i64, f_type, conf]);
            }
        }
        tx.commit()?;
    }

    Ok(count)
}

fn resolve_target_param(
    target_id: &str,
    arg_index: usize,
    file_func_params: &HashMap<String, HashMap<String, Vec<FormalParam>>>,
    id_to_meta: &HashMap<&str, (&str, &str)>,
) -> String {
    if let Some(&(file_path, name)) = id_to_meta.get(target_id) {
        if let Some(func_map) = file_func_params.get(file_path) {
            if let Some(params) = func_map.get(name) {
                if let Some(param) = params.get(arg_index) {
                    return param.name.clone();
                }
            }
        }
    }
    format!("arg{}", arg_index)
}
