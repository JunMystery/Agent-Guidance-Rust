use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;
use rusqlite::{params, Connection};
use crate::context::ast::{AstEngine, AstLanguage};
use super::parsers::ExtractedSymbol;
use super::resolver::find_enclosing_symbol;

#[derive(Clone, Debug)]
struct SymbolMeta {
    id: String,
    kind: String,
    file_path: String,
}

pub fn resolve_all_cross_edges(conn: &mut Connection, project_path: &Path) -> Result<usize> {
    let (file_symbols, name_to_syms, file_modules) = {
        let mut stmt = conn.prepare("SELECT id, name, kind, file_path, start_line, end_line FROM symbols")?;
        let mut file_symbols: HashMap<String, Vec<ExtractedSymbol>> = HashMap::new();
        let mut name_to_syms: HashMap<String, Vec<SymbolMeta>> = HashMap::new();
        let mut file_modules: HashMap<String, String> = HashMap::new();

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, usize>(4)?,
                row.get::<_, usize>(5)?,
            ))
        })?;

        for r in rows.flatten() {
            let (id, name, kind, file_path, start_line, end_line) = r;
            if kind == "module" {
                file_modules.insert(file_path.clone(), id.clone());
            }
            file_symbols.entry(file_path.clone()).or_default().push(ExtractedSymbol {
                id: id.clone(),
                name: name.clone(),
                kind: kind.clone(),
                parent: None,
                start_line,
                end_line,
                signature: None,
            });
            name_to_syms.entry(name).or_default().push(SymbolMeta { id, kind, file_path });
        }
        (file_symbols, name_to_syms, file_modules)
    };

    let mut edges = Vec::new();

    for (file_path, syms) in &file_symbols {
        let full_path = project_path.join(file_path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mod_id = file_modules.get(file_path).cloned().unwrap_or_else(|| {
            syms.first().map(|s| s.id.clone()).unwrap_or_default()
        });

        // 1. Resolve imports
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("use ") || trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                let token = trimmed.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .filter(|s| !s.is_empty() && *s != "use" && *s != "import" && *s != "from" && *s != "crate" && *s != "super")
                    .last();
                if let Some(target_name) = token {
                    if let Some(candidates) = name_to_syms.get(target_name) {
                        if let Some(target) = candidates.iter().find(|c| c.file_path != *file_path) {
                            if !mod_id.is_empty() && mod_id != target.id {
                                edges.push((mod_id.clone(), target.id.clone(), "imports".to_string(), 1.0));
                            }
                        }
                    }
                }
            }
        }

        // 2. Resolve AST calls
        let lang = AstLanguage::from_path(file_path);
        let ast_calls = if lang.is_supported() {
            AstEngine::extract_calls(&content, lang)
        } else {
            Vec::new()
        };

        for call in ast_calls {
            let caller = find_enclosing_symbol(syms, call.line);
            let caller_id = caller.map(|c| c.id.as_str()).unwrap_or(mod_id.as_str());
            if caller_id.is_empty() { continue; }

            if let Some(candidates) = name_to_syms.get(&call.callee_name) {
                // Prefer local match, then cross-file candidate
                let chosen = candidates.iter().find(|c| c.file_path == *file_path && c.id != caller_id)
                    .or_else(|| candidates.iter().find(|c| c.id != caller_id));

                if let Some(target) = chosen {
                    edges.push((caller_id.to_string(), target.id.clone(), "calls".to_string(), 1.0));
                }
            }
        }
    }

    let count = edges.len();
    let tx = conn.transaction()?;
    {
        let mut insert_stmt = tx.prepare(
            "INSERT OR REPLACE INTO symbol_edges (source_id, target_id, edge_type, weight)
             VALUES (?1, ?2, ?3, ?4)"
        )?;
        for (src, tgt, etype, weight) in edges {
            let _ = insert_stmt.execute(params![src, tgt, etype, weight]);
        }
    }
    tx.commit()?;
    Ok(count)
}
