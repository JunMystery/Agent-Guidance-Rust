use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::context::db::{bytes_to_f32_vec, CodeGraphDb};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReusableSymbol {
    pub name: String,
    pub file_path: String,
    pub symbol_kind: String,
    pub fan_in: usize,
    pub callers: Vec<String>,
    pub is_shared_location: bool,
    pub reusability_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticClonePair {
    pub symbol_a: String,
    pub file_a: String,
    pub symbol_b: String,
    pub file_b: String,
    pub similarity: f32,
    pub recommendation: String,
}

pub fn is_shared_path(path: &str) -> bool {
    let lower = path.to_lowercase().replace('\\', "/");
    lower.contains("/shared/")
        || lower.starts_with("shared/")
        || lower.contains("/common/")
        || lower.starts_with("common/")
        || lower.contains("/utils/")
        || lower.starts_with("utils/")
        || lower.contains("/helpers/")
        || lower.starts_with("helpers/")
        || lower.contains("/core/")
        || lower.starts_with("core/")
        || lower.contains("/components/shared/")
}

pub fn calculate_reusability_score(fan_in: usize, is_shared_dir: bool, is_public: bool) -> f32 {
    let mut score = (fan_in as f32) * 2.0;
    if is_shared_dir {
        score += 5.0;
    }
    if is_public {
        score += 1.5;
    }
    score
}

pub fn compute_cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut n1 = 0.0f32;
    let mut n2 = 0.0f32;
    for (a, b) in v1.iter().zip(v2.iter()) {
        dot += a * b;
        n1 += a * a;
        n2 += b * b;
    }
    if n1 <= 0.0 || n2 <= 0.0 {
        0.0
    } else {
        (dot / (n1.sqrt() * n2.sqrt())).clamp(-1.0, 1.0)
    }
}

pub fn analyze_project_reusability(project_path: &Path) -> Result<String> {
    let db = CodeGraphDb::open_for_project(project_path)?;

    // 1. Fetch symbols & compute fan-in topology
    let mut stmt = db.conn.prepare(
        "SELECT id, name, kind, file_path, signature FROM symbols WHERE kind IN ('function', 'method', 'struct', 'class')"
    )?;
    let symbol_rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut symbols = Vec::new();
    for row in symbol_rows.flatten() {
        symbols.push(row);
    }

    // 2. Fetch edges to calculate fan-in (callers)
    let mut edge_stmt = db.conn.prepare("SELECT target_id, source_id FROM symbol_edges")?;
    let mut callers_map: HashMap<String, HashSet<String>> = HashMap::new();
    let edge_rows = edge_stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for (target, source) in edge_rows.flatten() {
        callers_map.entry(target).or_default().insert(source);
    }

    // 3. Compute reusable symbol rankings
    let mut reusable_symbols = Vec::new();
    for (id, name, kind, file_path, sig) in &symbols {
        let callers = callers_map.get(id).map(|c| c.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let fan_in = callers.len();
        let is_shared = is_shared_path(file_path);
        let is_public = sig.as_ref().map(|s| s.contains("pub ") || s.contains("export ")).unwrap_or(false);

        if is_shared || fan_in > 0 {
            let score = calculate_reusability_score(fan_in, is_shared, is_public);
            reusable_symbols.push(ReusableSymbol {
                name: name.clone(),
                file_path: file_path.clone(),
                symbol_kind: kind.clone(),
                fan_in,
                callers,
                is_shared_location: is_shared,
                reusability_score: score,
            });
        }
    }
    reusable_symbols.sort_by(|a, b| b.reusability_score.partial_cmp(&a.reusability_score).unwrap_or(std::cmp::Ordering::Equal));

    // 4. ML Semantic Duplicate Detection across different files
    let mut vec_stmt = db.conn.prepare(
        "SELECT s.id, s.name, s.file_path, sv.vector FROM symbols s JOIN symbol_vectors sv ON s.id = sv.symbol_id WHERE s.kind IN ('function', 'method')"
    )?;
    let vec_rows = vec_stmt.query_map([], |row| {
        let raw_bytes: Vec<u8> = row.get(3)?;
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            bytes_to_f32_vec(&raw_bytes),
        ))
    })?;

    let mut function_vectors = Vec::new();
    for row in vec_rows.flatten() {
        if !row.3.is_empty() {
            function_vectors.push(row);
        }
    }

    let mut clone_pairs = Vec::new();
    let count = function_vectors.len();
    for i in 0..count {
        for j in (i + 1)..count {
            let (_id_a, name_a, file_a, vec_a) = &function_vectors[i];
            let (_id_b, name_b, file_b, vec_b) = &function_vectors[j];

            if file_a != file_b {
                let sim = compute_cosine_similarity(vec_a, vec_b);
                if sim >= 0.88 {
                    clone_pairs.push(SemanticClonePair {
                        symbol_a: name_a.clone(),
                        file_a: file_a.clone(),
                        symbol_b: name_b.clone(),
                        file_b: file_b.clone(),
                        similarity: sim,
                        recommendation: format!("Consider extracting into shared module (e.g. `shared/{}` or common helper)", name_a),
                    });
                }
            }
        }
    }
    clone_pairs.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    Ok(format_reusable_report(&reusable_symbols, &clone_pairs))
}

pub fn format_reusable_report(
    reusable_symbols: &[ReusableSymbol],
    clone_pairs: &[SemanticClonePair],
) -> String {
    let mut out = String::new();
    out.push_str("# ♻️ Code Reusability & Shared Function Analysis\n\n");

    if !reusable_symbols.is_empty() {
        out.push_str("### 🌟 Identified Shared / Highly-Reusable Functions\n\n");
        out.push_str("| Symbol | Location | Kind | Fan-in (Callers) | Shared Path | Score |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for sym in reusable_symbols.iter().take(15) {
            let shared_tag = if sym.is_shared_location { "✅ Yes" } else { "No" };
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} | {:.1} |\n",
                sym.name, sym.file_path, sym.symbol_kind, sym.fan_in, shared_tag, sym.reusability_score
            ));
        }
        out.push_str("\n");
    }

    if !clone_pairs.is_empty() {
        out.push_str("### ⚠️ Potential Duplicate Logic / Semantic Clones (DRY Warning)\n\n");
        out.push_str("| Function A | Function B | Semantic Similarity | Recommended Action |\n");
        out.push_str("|---|---|---|---|\n");
        for clone in clone_pairs.iter().take(10) {
            out.push_str(&format!(
                "| `{}:{}` | `{}:{}` | {:.1}% | {} |\n",
                clone.file_a, clone.symbol_a, clone.file_b, clone.symbol_b,
                clone.similarity * 100.0, clone.recommendation
            ));
        }
        out.push_str("\n💡 **Action**: Unify duplicated logic into a shared helper module.\n");
    }

    if reusable_symbols.is_empty() && clone_pairs.is_empty() {
        out.push_str("No duplicate or cross-module shared symbols detected in the current index.\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_shared_path() {
        assert!(is_shared_path("src/utils/format.rs"));
        assert!(is_shared_path("components/shared/Button.tsx"));
        assert!(is_shared_path("src/common/auth.ts"));
        assert!(!is_shared_path("src/features/login/login_view.rs"));
    }

    #[test]
    fn test_reusability_score() {
        let score_shared = calculate_reusability_score(3, true, true);
        assert_eq!(score_shared, 12.5);
        let score_private = calculate_reusability_score(0, false, false);
        assert_eq!(score_private, 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let v3 = vec![0.0, 1.0, 0.0];
        assert!((compute_cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-5);
        assert!((compute_cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-5);
    }
}
