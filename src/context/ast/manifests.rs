//! Tier 3 Manifest and Schema IDL Extractor.
//! Extracts macro architecture components and dependency links from build manifests and schemas.

use regex::Regex;
use std::sync::LazyLock;
use super::types::{AstCall, AstLanguage, AstSymbol};

static PROTO_SVC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*service\s+([A-Za-z0-9_]+)"#).unwrap()
});
static PROTO_RPC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*rpc\s+([A-Za-z0-9_]+)\s*\(\s*([A-Za-z0-9_]+)\s*\)\s*returns\s*\(\s*([A-Za-z0-9_]+)\s*\)"#).unwrap()
});
static PROTO_MSG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*message\s+([A-Za-z0-9_]+)"#).unwrap()
});
static GRADLE_DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:implementation|api|testImplementation)\s*\(?\s*project\s*\(?["']:([^"']+)["']\)?\)?|(?:implementation|api)\s*\(?["']([^:"']+):([^:"']+)"#).unwrap()
});
static CMAKE_TARGET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)(?:add_executable|add_library|project)\s*\(\s*([A-Za-z0-9_]+)"#).unwrap()
});
static CMAKE_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)target_link_libraries\s*\(\s*([A-Za-z0-9_]+)\s+([^)]+)\)"#).unwrap()
});
static MAVEN_DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<dependency>\s*.*?<artifactId>([^<]+)</artifactId>"#).unwrap()
});

/// Extracts symbols from manifests and IDLs.
pub fn extract_symbols(content: &str, lang: AstLanguage) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    match lang {
        AstLanguage::Protobuf => {
            for cap in PROTO_SVC_RE.captures_iter(content) {
                if let Some(m) = cap.get(1) {
                    let line = line_number(content, m.start());
                    symbols.push(build_sym(m.as_str(), "service", line, &lines, lang, None));
                }
            }
            for cap in PROTO_MSG_RE.captures_iter(content) {
                if let Some(m) = cap.get(1) {
                    let line = line_number(content, m.start());
                    symbols.push(build_sym(m.as_str(), "message", line, &lines, lang, None));
                }
            }
        }
        AstLanguage::CargoManifest => {
            if let Ok(val) = content.parse::<toml::Value>() {
                if let Some(pkg) = val.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
                    symbols.push(build_sym(pkg, "manifest", 1, &lines, lang, None));
                }
            }
        }
        AstLanguage::NpmManifest => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
                if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                    symbols.push(build_sym(name, "manifest", 1, &lines, lang, None));
                }
            }
        }
        AstLanguage::Cmake => {
            for cap in CMAKE_TARGET_RE.captures_iter(content) {
                if let Some(m) = cap.get(1) {
                    let line = line_number(content, m.start());
                    symbols.push(build_sym(m.as_str(), "target", line, &lines, lang, None));
                }
            }
        }
        AstLanguage::Maven => {
            let art_re = Regex::new(r#"<artifactId>([^<]+)</artifactId>"#).unwrap();
            let name = art_re.captures(content).and_then(|c| c.get(1)).map_or("build_manifest", |m| m.as_str().trim());
            symbols.push(build_sym(name, "manifest", 1, &lines, lang, None));
        }
        AstLanguage::Gradle => {
            symbols.push(build_sym("build_manifest", "manifest", 1, &lines, lang, None));
        }
        _ => {}
    }

    symbols
}

/// Extracts cross-dependency and invocation edges from manifests and IDLs.
pub fn extract_calls(content: &str, lang: AstLanguage) -> Vec<AstCall> {
    let mut calls = Vec::new();

    match lang {
        AstLanguage::Protobuf => {
            for cap in PROTO_RPC_RE.captures_iter(content) {
                let rpc_name = cap.get(1).map_or("", |m| m.as_str());
                let req_type = cap.get(2).map_or("", |m| m.as_str());
                let resp_type = cap.get(3).map_or("", |m| m.as_str());
                let line = cap.get(1).map_or(1, |m| line_number(content, m.start()));

                calls.push(AstCall {
                    caller_name: Some(rpc_name.to_string()),
                    callee_name: req_type.to_string(),
                    line,
                    receiver: Some("request".to_string()),
                    namespace: None,
                });
                calls.push(AstCall {
                    caller_name: Some(rpc_name.to_string()),
                    callee_name: resp_type.to_string(),
                    line,
                    receiver: Some("response".to_string()),
                    namespace: None,
                });
            }
        }
        AstLanguage::Gradle => {
            for (idx, line) in content.lines().enumerate() {
                for cap in GRADLE_DEP_RE.captures_iter(line) {
                    let dep_name = cap.get(1).or_else(|| cap.get(3)).map(|m| m.as_str()).unwrap_or("");
                    if !dep_name.is_empty() {
                        calls.push(AstCall {
                            caller_name: None,
                            callee_name: dep_name.to_string(),
                            line: idx + 1,
                            receiver: Some("dependency".to_string()),
                            namespace: None,
                        });
                    }
                }
            }
        }
        AstLanguage::CargoManifest => {
            if let Ok(val) = content.parse::<toml::Value>() {
                for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(deps) = val.get(section).and_then(|d| d.as_table()) {
                        for (dep_name, _) in deps {
                            calls.push(AstCall {
                                caller_name: None,
                                callee_name: dep_name.clone(),
                                line: 1,
                                receiver: Some("dependency".to_string()),
                                namespace: None,
                            });
                        }
                    }
                }
            }
        }
        AstLanguage::NpmManifest => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
                for section in ["dependencies", "devDependencies", "peerDependencies"] {
                    if let Some(deps) = val.get(section).and_then(|d| d.as_object()) {
                        for (dep_name, _) in deps {
                            calls.push(AstCall {
                                caller_name: None,
                                callee_name: dep_name.clone(),
                                line: 1,
                                receiver: Some("dependency".to_string()),
                                namespace: None,
                            });
                        }
                    }
                }
            }
        }
        AstLanguage::Cmake => {
            for cap in CMAKE_LINK_RE.captures_iter(content) {
                let target = cap.get(1).map_or("", |m| m.as_str());
                if let Some(deps_str) = cap.get(2) {
                    for dep in deps_str.as_str().split_whitespace() {
                        if !dep.eq_ignore_ascii_case("PUBLIC") && !dep.eq_ignore_ascii_case("PRIVATE") && !dep.eq_ignore_ascii_case("INTERFACE") {
                            calls.push(AstCall {
                                caller_name: Some(target.to_string()),
                                callee_name: dep.to_string(),
                                line: line_number(content, cap.get(0).map_or(0, |m| m.start())),
                                receiver: Some("link_library".to_string()),
                                namespace: None,
                            });
                        }
                    }
                }
            }
        }
        AstLanguage::Maven => {
            for cap in MAVEN_DEP_RE.captures_iter(content) {
                if let Some(art) = cap.get(1) {
                    let line = line_number(content, art.start());
                    calls.push(AstCall {
                        caller_name: None,
                        callee_name: art.as_str().trim().to_string(),
                        line,
                        receiver: Some("dependency".to_string()),
                        namespace: None,
                    });
                }
            }
        }
        _ => {}
    }

    calls
}

fn line_number(content: &str, byte_idx: usize) -> usize {
    content[..byte_idx.min(content.len())].lines().count().max(1)
}

fn build_sym(
    name: &str,
    kind: &str,
    start_line: usize,
    lines: &[&str],
    lang: AstLanguage,
    parent: Option<String>,
) -> AstSymbol {
    let sig = lines.get(start_line - 1).unwrap_or(&"").trim().to_string();
    AstSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        start_line,
        end_line: start_line,
        start_byte: 0,
        end_byte: name.len(),
        signature: sig,
        body_start_line: Some(start_line),
        body_end_line: Some(start_line),
        parent_symbol: parent,
        package: None,
        language: lang.as_str().to_string(),
    }
}
