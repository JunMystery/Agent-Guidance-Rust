use regex::Regex;
use std::sync::OnceLock;

static RE_COMMENTS: OnceLock<Regex> = OnceLock::new();
static RE_BADGES: OnceLock<Regex> = OnceLock::new();

pub fn estimate_tokens(text: &str, is_code: bool) -> usize {
    let ratio = if is_code { 2.8 } else { 4.0 };
    ((text.len() as f64) / ratio).ceil() as usize
}

pub fn compress_markdown(content: &str) -> String {
    let re_comments = RE_COMMENTS.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").expect("Invalid comment regex"));
    let re_badges = RE_BADGES.get_or_init(|| {
        Regex::new(r"!\[.*?\]\(https://img\.shields\.io/.*?\)|!\[.*?\]\(https://badge.*?\)\s*")
            .expect("Invalid badge regex")
    });

    let stripped = re_comments.replace_all(content, "");
    let stripped_badges = re_badges.replace_all(&stripped, "");
    
    let mut lines = Vec::new();
    let mut blank_count = 0;
    for line in stripped_badges.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                lines.push("");
            }
        } else {
            blank_count = 0;
            lines.push(trimmed);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let text = "Hello world";
        assert!(estimate_tokens(text, false) > 0);
        assert!(estimate_tokens(text, true) > 0);
    }

    #[test]
    fn test_compress_markdown() {
        let md = "# Title\n<!-- comment -->\n![badge](https://img.shields.io/badge/status-ok)\n\n\n\nBody text";
        let compressed = compress_markdown(md);
        assert!(!compressed.contains("comment"));
        assert!(!compressed.contains("shields.io"));
        assert!(compressed.contains("# Title"));
        assert!(compressed.contains("Body text"));
    }
}
