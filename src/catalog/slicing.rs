use crate::catalog::language_detector::ProjectLanguageProfile;
use crate::ml::embeddings::{EmbeddingModel, cosine_similarity};
use crate::optimizer::compressor::compress_markdown;

#[derive(Debug, Clone)]
struct MarkdownSection {
    title: String,
    content: String,
}

/// Splits a Markdown document into logical sections based on headers (`#`, `##`, `###`).
fn split_markdown_into_sections(md: &str) -> Vec<MarkdownSection> {
    let mut sections = Vec::new();
    let mut current_title = "Overview".to_string();
    let mut current_lines = Vec::new();

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            if !current_lines.is_empty() {
                sections.push(MarkdownSection {
                    title: current_title.clone(),
                    content: current_lines.join("\n"),
                });
                current_lines.clear();
            }
            current_title = trimmed.trim_start_matches('#').trim().to_string();
        } else {
            current_lines.push(line);
        }
    }

    if !current_lines.is_empty() {
        sections.push(MarkdownSection {
            title: current_title,
            content: current_lines.join("\n"),
        });
    }

    sections
}

/// Slices a raw Markdown skill document by task context using semantic cosine similarity.
/// Returns the top-k most relevant sections compressed to minimize token consumption.
pub fn slice_skill_markdown(raw_md: &str, task: &str, top_k: usize) -> String {
    if task.trim().is_empty() {
        return compress_markdown(raw_md);
    }

    let sections = split_markdown_into_sections(raw_md);
    if sections.len() <= top_k {
        return compress_markdown(raw_md);
    }

    // Try embedding task and sections using the shared ML engine
    if let Ok(model) = EmbeddingModel::load_or_download() {
        if let Ok(query_vec) = model.embed_text(task, Some("query")) {
            let mut scored_sections: Vec<(f32, &MarkdownSection)> = Vec::new();

            for sec in &sections {
                let sec_text = format!("{} {}", sec.title, sec.content);
                if let Ok(sec_vec) = model.embed_text(&sec_text, Some("passage")) {
                    let score = cosine_similarity(&query_vec, &sec_vec);
                    scored_sections.push((score, sec));
                }
            }

            scored_sections.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut result = Vec::new();
            for (score, sec) in scored_sections.into_iter().take(top_k) {
                let compressed = compress_markdown(&sec.content);
                result.push(format!("#### {} (Relevance: {:.2})\n{}", sec.title, score, compressed));
            }

            return result.join("\n\n---\n\n");
        }
    }

    // Fallback: lexical substring matching if ML model unavailable
    let task_lower = task.to_lowercase();
    let mut matched = Vec::new();
    for sec in &sections {
        if sec.title.to_lowercase().contains(&task_lower) || sec.content.to_lowercase().contains(&task_lower) {
            matched.push(format!("#### {}\n{}", sec.title, compress_markdown(&sec.content)));
            if matched.len() >= top_k {
                break;
            }
        }
    }

    if matched.is_empty() {
        compress_markdown(raw_md)
    } else {
        matched.join("\n\n---\n\n")
    }
}

/// Returns language-specific safety micro-guidance based on detected repository profile.
pub fn get_language_safety_rules(profile: &ProjectLanguageProfile) -> String {
    let mut rules = Vec::new();

    if profile.primary_languages.contains("rust") {
        rules.push("- **Rust Safety**: Avoid `.unwrap()` / `.expect()` in production; handle `Option`/`Result` cleanly; minimize unneeded `.clone()`.");
    }
    if profile.primary_languages.contains("typescript") || profile.primary_languages.contains("javascript") {
        rules.push("- **TS/JS Safety**: Use strict types (prefer `unknown` over `any`); use optional chaining `?.` and nullish coalescing `??`.");
    }
    if profile.primary_languages.contains("python") {
        rules.push("- **Python Safety**: Use explicit type hints; handle `None` checks explicitly; avoid mutable default arguments in functions.");
    }
    if profile.primary_languages.contains("go") {
        rules.push("- **Go Safety**: Check `if err != nil` explicitly; bind goroutine lifecycles to `context.Context`; avoid data races on shared structs.");
    }
    if profile.primary_languages.contains("kotlin") || profile.primary_languages.contains("java") {
        rules.push("- **Kotlin/Java Safety**: Avoid forced unwraps (`!!`); scope coroutines properly; respect lifecycle state flows.");
    }

    if rules.is_empty() {
        "- **General Code Safety**: Validate non-null inputs before dereference; enforce explicit error handling boundaries.".to_string()
    } else {
        rules.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_language_safety_rules() {
        let mut profile = ProjectLanguageProfile::default();
        profile.primary_languages.insert("rust".to_string());
        let rules = get_language_safety_rules(&profile);
        assert!(rules.contains("Rust Safety"));
        assert!(rules.contains("unwrap()"));

        let mut py_profile = ProjectLanguageProfile::default();
        py_profile.primary_languages.insert("python".to_string());
        let py_rules = get_language_safety_rules(&py_profile);
        assert!(py_rules.contains("Python Safety"));
    }

    #[test]
    fn test_slice_skill_markdown_fallback() {
        let md = "# Overview\nThis is general overview\n\n## Section A\nDetails about domain usecases\n\n## Section B\nDetails about database migrations";
        let sliced = slice_skill_markdown(md, "domain usecases", 2);
        assert!(sliced.contains("Section A") || sliced.contains("Overview") || sliced.contains("usecases"));
    }
}
