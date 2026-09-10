use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::Result;
use rusqlite::{params, Connection};
use crate::context::ast::{AstEngine, AstLanguage};
use super::import_resolver::{extract_import_path, extract_imported_symbol_names, resolve_relative_import_path};
use super::parsers::ExtractedSymbol;
use super::resolver::find_enclosing_symbol;

const GENERIC_VERBS: &[&str] = &[
    "new", "run", "init", "get", "set", "build", "create", "update", "delete",
    "start", "stop", "handle", "process", "execute", "call", "apply", "main",
    "test", "load", "save", "parse", "read", "write", "check", "add", "remove", "default", "clone"
];

#[derive(Clone, Debug)]
pub struct SymbolMeta {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub parent: Option<String>,
}

pub fn resolve_all_cross_edges(conn: &mut Connection, project_path: &Path) -> Result<usize> {
    let (file_symbols, name_to_syms, file_modules, package_to_files) = {
        let mut stmt = conn.prepare("SELECT id, name, kind, file_path, parent, start_line, end_line FROM symbols")?;
        let mut file_symbols: HashMap<String, Vec<ExtractedSymbol>> = HashMap::new();
        let mut name_to_syms: HashMap<String, Vec<SymbolMeta>> = HashMap::new();
        let mut file_modules: HashMap<String, String> = HashMap::new();
        let mut package_to_files: HashMap<String, Vec<String>> = HashMap::new();

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, usize>(5)?,
                row.get::<_, usize>(6)?,
            ))
        })?;

        for r in rows.flatten() {
            let (id, name, kind, file_path, parent, start_line, end_line) = r;
            if kind == "module" {
                file_modules.insert(file_path.clone(), id.clone());
            }
            if let Some(ref p) = parent {
                if kind == "package" {
                    package_to_files.entry(p.clone()).or_default().push(file_path.clone());
                }
            }
            file_symbols.entry(file_path.clone()).or_default().push(ExtractedSymbol {
                id: id.clone(),
                name: name.clone(),
                kind: kind.clone(),
                parent: parent.clone(),
                start_line,
                end_line,
                signature: None,
                language: None,
                namespace: None,
                receiver: None,
            });
            name_to_syms.entry(name.clone()).or_default().push(SymbolMeta {
                id,
                name,
                kind,
                file_path,
                parent,
            });
        }
        (file_symbols, name_to_syms, file_modules, package_to_files)
    };

    let mut edges: Vec<(String, String, String, f64, f64, String, Option<usize>)> = Vec::new();
    let mut file_imports_map: HashMap<String, HashMap<String, Option<String>>> = HashMap::new();

    // 1. Resolve imports and file-to-file relationships
    for (file_path, _) in &file_symbols {
        let full_path = project_path.join(file_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mod_id = file_modules.get(file_path).cloned().unwrap_or_default();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("use ") || trimmed.starts_with("import ") || trimmed.starts_with("from ") || trimmed.starts_with("#include") || trimmed.starts_with("@import") || trimmed.starts_with("@use") {
                let mut target_file_opt = None;
                // File-to-file import resolution
                if let Some(import_target) = extract_import_path(trimmed) {
                    if let Some(target_file) = resolve_relative_import_path(file_path, &import_target, &file_modules) {
                        if let Some(target_mod_id) = file_modules.get(&target_file) {
                            if !mod_id.is_empty() && &mod_id != target_mod_id {
                                edges.push((mod_id.clone(), target_mod_id.clone(), "imports_file".to_string(), 0.95, 0.95, "file".to_string(), None));
                            }
                        }
                        target_file_opt = Some(target_file);
                    }
                }

                // Extracted imported symbol names mapped to resolved target file
                for token in extract_imported_symbol_names(trimmed) {
                    file_imports_map.entry(file_path.clone()).or_default().insert(token, target_file_opt.clone());
                }
            }
        }
    }

    // 2. Package-level cohesion (shares_package)
    for (_pkg, files) in &package_to_files {
        for (i, f1) in files.iter().enumerate() {
            for f2 in files.iter().skip(i + 1) {
                if let (Some(m1), Some(m2)) = (file_modules.get(f1), file_modules.get(f2)) {
                    edges.push((m1.clone(), m2.clone(), "shares_package".to_string(), 0.85, 0.85, "file".to_string(), None));
                }
            }
        }
    }

    // 3. 5-Tier Scoped Disambiguation for AST & Polyglot Calls
    for (file_path, syms) in &file_symbols {
        let full_path = project_path.join(file_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mod_id = file_modules.get(file_path).cloned().unwrap_or_default();
        let lang = AstLanguage::from_path(file_path);
        let ast_calls = if lang.is_supported() {
            AstEngine::extract_calls(&content, lang)
        } else {
            Vec::new()
        };

        let imported_symbols = file_imports_map.get(file_path);

        for call in ast_calls {
            let caller = find_enclosing_symbol(syms, call.line);
            let caller_id = caller.map(|c| c.id.as_str()).unwrap_or(mod_id.as_str());
            if caller_id.is_empty() { continue; }

            let target = resolve_call_target(
                &call.callee_name,
                call.receiver.as_deref(),
                file_path,
                caller_id,
                syms,
                &name_to_syms,
                imported_symbols,
            );

            if let Some((target_id, weight)) = target {
                edges.push((caller_id.to_string(), target_id, "calls".to_string(), weight, weight, "symbol".to_string(), Some(call.line)));
            }
        }
    }

    let count = edges.len();
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
    Ok(count)
}

fn resolve_call_target(
    callee: &str,
    receiver: Option<&str>,
    file_path: &str,
    caller_id: &str,
    local_syms: &[ExtractedSymbol],
    name_to_syms: &HashMap<String, Vec<SymbolMeta>>,
    imported_symbols: Option<&HashMap<String, Option<String>>>,
) -> Option<(String, f64)> {
    // Tier 1: Local scope
    if receiver.is_none() || receiver == Some("self") || receiver == Some("this") {
        if let Some(local_match) = local_syms.iter().find(|s| s.name == callee && s.id != caller_id) {
            return Some((local_match.id.clone(), 1.0));
        }
    }

    // Tier 2: Explicitly imported symbols with file matching
    if let Some(imports) = imported_symbols {
        if let Some(target_file_opt) = imports.get(callee) {
            if let Some(candidates) = name_to_syms.get(callee) {
                let matched = target_file_opt
                    .as_deref()
                    .and_then(|tf| candidates.iter().find(|c| c.file_path == tf))
                    .or_else(|| candidates.iter().find(|c| c.file_path != file_path));

                if let Some(target) = matched {
                    return Some((target.id.clone(), 0.95));
                }
            }
        }
    }

    // Heuristic Safeguard: Generic verbs must not cross files without explicit import
    if GENERIC_VERBS.contains(&callee) {
        return None;
    }

    // Tier 3: Same directory scope
    let dir = Path::new(file_path).parent();
    if let Some(candidates) = name_to_syms.get(callee) {
        let same_dir_candidates: Vec<&SymbolMeta> = candidates
            .iter()
            .filter(|c| c.id != caller_id && Path::new(&c.file_path).parent() == dir)
            .collect();
        if same_dir_candidates.len() == 1 {
            return Some((same_dir_candidates[0].id.clone(), 0.85));
        }
    }

    // Tier 4: Typed receiver / namespace match
    if let Some(rec) = receiver {
        if rec != "self" && rec != "this" {
            if let Some(candidates) = name_to_syms.get(callee) {
                if let Some(matched) = candidates.iter().find(|c| c.parent.as_deref() == Some(rec)) {
                    return Some((matched.id.clone(), 0.80));
                }
            }
        }
    }

    // Tier 5: Unique global match with heuristic safeguard
    if let Some(candidates) = name_to_syms.get(callee) {
        let valid_candidates: Vec<&SymbolMeta> = candidates.iter().filter(|c| c.id != caller_id).collect();
        if valid_candidates.len() == 1 {
            return Some((valid_candidates[0].id.clone(), 0.60));
        }
    }

    None
}
