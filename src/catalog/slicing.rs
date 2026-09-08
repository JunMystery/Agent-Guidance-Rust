use crate::catalog::language_detector::ProjectLanguageProfile;
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

/// Slices a raw Markdown skill document by task context using fast token-overlap and title relevance scoring.
/// Returns the top-k most relevant sections compressed to minimize token consumption (< 1ms).
pub fn slice_skill_markdown(raw_md: &str, task: &str, top_k: usize) -> String {
    if task.trim().is_empty() {
        return compress_markdown(raw_md);
    }

    let sections = split_markdown_into_sections(raw_md);
    if sections.len() <= top_k {
        return compress_markdown(raw_md);
    }

    // Extract significant keywords from task (len >= 3, excluding common stop words)
    let stop_words = ["the", "and", "for", "with", "this", "that", "from", "have", "been", "will", "your", "about"];
    let keywords: Vec<String> = task
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3 && !stop_words.contains(w))
        .map(|s| s.to_string())
        .collect();

    if keywords.is_empty() {
        let compressed = sections.iter().take(top_k).map(|s| format!("#### {}\n{}", s.title, compress_markdown(&s.content))).collect::<Vec<_>>().join("\n\n---\n\n");
        return compressed;
    }

    let mut scored: Vec<(f32, &MarkdownSection)> = Vec::new();
    for sec in &sections {
        let title_lower = sec.title.to_lowercase();
        let content_lower = sec.content.to_lowercase();
        let mut score = 0.0f32;

        if title_lower.contains("overview") || title_lower.contains("quick start") || title_lower.contains("key rules") {
            score += 0.5;
        }

        for kw in &keywords {
            if title_lower.contains(kw) {
                score += 3.0;
            }
            if content_lower.contains(kw) {
                score += 1.0;
            }
        }

        if score > 0.0 {
            scored.push((score, sec));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    if scored.is_empty() {
        sections
            .iter()
            .take(top_k)
            .map(|s| format!("#### {}\n{}", s.title, compress_markdown(&s.content)))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    } else {
        scored
            .into_iter()
            .take(top_k)
            .map(|(score, s)| format!("#### {} (Relevance: {:.1})\n{}", s.title, score, compress_markdown(&s.content)))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
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
