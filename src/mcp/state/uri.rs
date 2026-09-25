/// Parse cross-platform file:// URIs safely into native OS path strings.
pub fn parse_file_uri(uri: &str) -> String {
    let mut decoded = uri.replace("%20", " ");
    if decoded.starts_with("file://") {
        decoded = decoded.trim_start_matches("file://").to_string();
    }

    if cfg!(windows) {
        // Handle Windows leading slash e.g. /C:/path or /e:/path -> C:\path or E:\path
        if decoded.starts_with('/') && decoded.chars().nth(2) == Some(':') {
            decoded = decoded.trim_start_matches('/').to_string();
        }
        decoded = decoded.replace('/', "\\");
        if decoded.len() >= 2 && decoded.as_bytes()[1] == b':' {
            let first = decoded.chars().next().unwrap();
            if first.is_ascii_lowercase() {
                let upper = first.to_ascii_uppercase().to_string();
                decoded.replace_range(..1, &upper);
            }
        }
    }

    decoded
}
