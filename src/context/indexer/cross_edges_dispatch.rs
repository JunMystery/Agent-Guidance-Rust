//! Cross-file dynamic dispatch and trait implementation edge resolver.

use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use crate::context::ast::trait_def::{TraitBinding, TraitSpec};
use crate::context::ast::{AstEngine, AstLanguage, extract_trait_bindings};
use super::cross_edges::SymbolMeta;

const GENERIC_VERBS: &[&str] = &[
    "new", "run", "init", "get", "set", "build", "create", "update", "delete",
    "start", "stop", "handle", "process", "execute", "call", "apply", "main",
    "test", "load", "save", "parse", "read", "write", "check", "add", "remove", "default", "clone", "string", "error", "len"
];

/// Collects all trait definitions and bindings from all files in the project.
pub fn scan_project_traits(
    files: &[String],
    project_path: &Path,
) -> (Vec<TraitSpec>, Vec<TraitBinding>) {
    let mut all_specs = Vec::new();
    let mut all_bindings = Vec::new();

    for rel_path in files {
        let full_path = project_path.join(rel_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lang = AstLanguage::from_path(rel_path);
        if lang.is_tier1() {
            if let Some(tree) = AstEngine::parse(&content, lang) {
                let (specs, bindings) = extract_trait_bindings(tree.root_node(), content.as_bytes(), lang);
                all_specs.extend(specs);
                all_bindings.extend(bindings);
            }
        }
    }
    (all_specs, all_bindings)
}

/// Resolves and commits trait implementations and dynamic dispatch edges to SQLite.
pub fn resolve_dispatch_edges(
    conn: &mut Connection,
    files: &[String],
    project_path: &Path,
    name_to_syms: &HashMap<String, Vec<SymbolMeta>>,
) -> Result<usize> {
    let (_specs, bindings) = scan_project_traits(files, project_path);
    let mut edges: Vec<(String, String, String, f64, f64, String, Option<usize>)> = Vec::new();
    let mut trait_method_to_impls: HashMap<String, Vec<String>> = HashMap::new();

    // 1. Resolve struct -> trait and concrete method -> trait method bindings
    for binding in &bindings {
        let trait_leaf = binding.trait_name.split("::").last()
            .and_then(|s| s.split('.').last())
            .unwrap_or(&binding.trait_name);
        let struct_leaf = binding.implementor_name.split("::").last()
            .and_then(|s| s.split('.').last())
            .unwrap_or(&binding.implementor_name);

        let trait_matches = name_to_syms.get(&binding.trait_name).or_else(|| name_to_syms.get(trait_leaf));
        let struct_matches = name_to_syms.get(&binding.implementor_name).or_else(|| name_to_syms.get(struct_leaf));

        if let (Some(t_syms), Some(s_syms)) = (trait_matches, struct_matches) {
            for t_sym in t_syms {
                for s_sym in s_syms {
                    // Struct implements Trait
                    edges.push((
                        s_sym.id.clone(),
                        t_sym.id.clone(),
                        "implements_trait".to_string(),
                        1.0,
                        1.0,
                        "structural".to_string(),
                        Some(binding.line),
                    ));

                    // Concrete methods implement Trait methods
                    for method in &binding.methods {
                        if let Some(method_syms) = name_to_syms.get(method) {
                            let concrete_opt = method_syms.iter().find(|m| m.parent.as_deref() == Some(&s_sym.name) || m.file_path == s_sym.file_path);
                            let trait_method_opt = method_syms.iter().find(|m| m.parent.as_deref() == Some(&t_sym.name) || m.file_path == t_sym.file_path);

                            if let (Some(c_m), Some(t_m)) = (concrete_opt, trait_method_opt) {
                                edges.push((
                                    c_m.id.clone(),
                                    t_m.id.clone(),
                                    "implements_method".to_string(),
                                    0.95,
                                    0.95,
                                    "symbol".to_string(),
                                    Some(binding.line),
                                ));
                                trait_method_to_impls.entry(t_m.id.clone()).or_default().push(c_m.id.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Synthesize dynamic_dispatch edges from callers of trait methods to concrete implementations
    if !trait_method_to_impls.is_empty() {
        let mut call_stmt = conn.prepare(
            "SELECT source_id, target_id, call_line FROM symbol_edges WHERE edge_type = 'calls'"
        )?;
        let call_rows = call_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<usize>>(2)?,
            ))
        })?;

        for r in call_rows.flatten() {
            let (caller_id, target_id, call_line) = r;
            if let Some(impls) = trait_method_to_impls.get(&target_id) {
                let method_name = target_id
                    .split("::function::")
                    .nth(1)
                    .and_then(|s| s.split("::").next())
                    .unwrap_or("");
                if GENERIC_VERBS.contains(&method_name.to_lowercase().as_str()) {
                    continue;
                }

                let n = impls.len() as f64;
                let conf = (0.95 / n.sqrt()).clamp(0.65, 0.95);

                for impl_id in impls {
                    if impl_id != &caller_id {
                        edges.push((
                            caller_id.clone(),
                            impl_id.clone(),
                            "dynamic_dispatch".to_string(),
                            conf,
                            conf,
                            "dispatch".to_string(),
                            call_line,
                        ));
                    }
                }
            }
        }
    }

    let count = edges.len();
    if count > 0 {
        let tx = conn.transaction()?;
        {
            let mut insert_stmt = tx.prepare(
                "INSERT OR REPLACE INTO symbol_edges (source_id, target_id, edge_type, weight, confidence, category, call_line)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
            )?;
            for (src, tgt, etype, weight, conf, cat, call_line) in edges {
                let _ = insert_stmt.execute(params![src, tgt, etype, weight, conf, cat, call_line.map(|l| l as i64)]);
            }
        }
        tx.commit()?;
    }

    Ok(count)
}
