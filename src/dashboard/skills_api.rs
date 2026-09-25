use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use tiny_http::Request;

use crate::catalog::checksum::{load_manifest, skills_manifest_path};
use crate::catalog::staging::{scan_staging_skills, staging_dir};
use crate::catalog::tombstone::{load_tombstones, tombstones_path};
use crate::ml::embeddings::binary_format::SkillsBinary;
use crate::ml::embeddings::compactor::delete_skills_and_compact;
use crate::ml::embeddings::compiler::compile_staging_to_binary;
use crate::ml::embeddings::precomputed::{skills_bin_path, vectors_path};

#[derive(Deserialize)]
struct DeleteBulkRequest {
    names: Vec<String>,
    #[serde(default = "default_true")]
    purge_staging: bool,
}

#[derive(Deserialize)]
struct CompileRequest {
    #[serde(default)]
    force: bool,
}

fn default_true() -> bool {
    true
}

pub fn handle_api_skills_registry(request: Request) {
    let tombstones: HashSet<String> = load_tombstones().into_iter().collect();
    let staging_skills = scan_staging_skills();
    let staging_names: HashSet<String> = staging_skills.iter().map(|s| s.name.clone()).collect();

    let mut skill_items = Vec::new();
    let bin_path = skills_bin_path();

    if bin_path.exists() {
        if let Ok(bytes) = fs::read(&bin_path) {
            if let Ok(bin) = SkillsBinary::deserialize(&bytes) {
                for rec in bin.skills {
                    let name = rec.name;
                    let sections = rec.content.matches("\n## ").count() + 1;
                    let is_staged = staging_names.contains(&name);
                    let is_tomb = tombstones.contains(&name);
                    let source = if is_staged { "staged_override" } else { "binary" };

                    skill_items.push(json!({
                        "name": name,
                        "sections": sections,
                        "source": source,
                        "tombstoned": is_tomb,
                        "content_len": rec.content.len()
                    }));
                }
            }
        }
    }

    // Also include any staged skills not yet compiled into binary
    for staged in staging_skills {
        if !skill_items.iter().any(|s| s["name"].as_str() == Some(&staged.name)) {
            let sections = staged.content.matches("\n## ").count() + 1;
            let is_tomb = tombstones.contains(&staged.name);
            skill_items.push(json!({
                "name": staged.name,
                "sections": sections,
                "source": "staging_only",
                "tombstoned": is_tomb,
                "content_len": staged.content.len()
            }));
        }
    }

    let payload = json!({
        "status": "ok",
        "skills": skill_items,
        "total": skill_items.len()
    });
    super::router::json_response(request, 200, &payload);
}

pub fn handle_api_skills_binary_stats(request: Request) {
    let get_len = |p: std::path::PathBuf| fs::metadata(p).map(|m| m.len()).unwrap_or(0);

    let skills_bytes = get_len(skills_bin_path());
    let vectors_bytes = get_len(vectors_path());
    let manifest_bytes = get_len(skills_manifest_path());
    let tombstones_bytes = get_len(tombstones_path());
    let manifest = load_manifest();

    let total_skills = manifest.as_ref().map(|m| m.count).unwrap_or(0);
    let catalog_hash = manifest.as_ref().map(|m| m.catalog_hash.clone()).unwrap_or_default();
    let tombstones_count = load_tombstones().len();

    let payload = json!({
        "status": "ok",
        "skills_bin_bytes": skills_bytes,
        "vectors_bin_bytes": vectors_bytes,
        "manifest_bytes": manifest_bytes,
        "tombstones_bytes": tombstones_bytes,
        "total_skills": total_skills,
        "catalog_hash": catalog_hash,
        "tombstones_count": tombstones_count,
        "staging_dir": staging_dir().to_string_lossy()
    });
    super::router::json_response(request, 200, &payload);
}

pub fn handle_api_skills_delete_bulk(mut request: Request) {
    if request.method() != &tiny_http::Method::Post {
        super::router::json_response(request, 405, &json!({ "error": "POST required" }));
        return;
    }

    let mut body = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut body) {
        super::router::json_response(request, 400, &json!({ "error": format!("Read error: {}", e) }));
        return;
    }

    let req_data: DeleteBulkRequest = match serde_json::from_str(&body) {
        Ok(d) => d,
        Err(e) => {
            super::router::json_response(request, 400, &json!({ "error": format!("Invalid JSON: {}", e) }));
            return;
        }
    };

    match delete_skills_and_compact(&req_data.names, req_data.purge_staging) {
        Ok(res) => {
            super::router::json_response(request, 200, &json!({
                "status": "ok",
                "result": res
            }));
        }
        Err(e) => {
            super::router::json_response(request, 500, &json!({ "error": e.to_string() }));
        }
    }
}

pub fn handle_api_skills_compile(mut request: Request) {
    if request.method() != &tiny_http::Method::Post {
        super::router::json_response(request, 405, &json!({ "error": "POST required" }));
        return;
    }

    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    let req_data: CompileRequest = serde_json::from_str(&body).unwrap_or(CompileRequest { force: false });

    match compile_staging_to_binary(req_data.force) {
        Ok(stats) => {
            super::router::json_response(request, 200, &json!({
                "status": "ok",
                "stats": stats
            }));
        }
        Err(e) => {
            super::router::json_response(request, 500, &json!({ "error": e.to_string() }));
        }
    }
}
