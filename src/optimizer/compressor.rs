use regex::Regex;

pub fn estimate_tokens(text: &str, is_code: bool) -> usize {
    let ratio = if is_code { 2.8 } else { 4.0 };
    ((text.len() as f64) / ratio).ceil() as usize
}

pub fn compress_markdown(content: &str) -> String {
    let re_comments = Regex::new(r"(?s)<!--.*?-->").unwrap();
    let re_badges = Regex::new(r"!\[.*?\]\(https://img\.shields\.io/.*?\)|!\[.*?\]\(https://badge.*?\)\s*").unwrap();
    
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
