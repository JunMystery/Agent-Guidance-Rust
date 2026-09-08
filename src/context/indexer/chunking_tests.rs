//! Unit tests for dynamic semantic chunking and boundary windowing.

#[cfg(test)]
mod tests {
    use crate::context::indexer::chunking::build_semantic_chunks;
    use crate::context::indexer::parsers::ExtractedSymbol;

    #[test]
    fn test_atomic_symbol_chunking() {
        let code = r#"
// File Header
use std::collections::HashMap;
use std::path::Path;

pub fn calculate_metrics(data: &[i32]) -> i32 {
    let mut sum = 0;
    for x in data {
        sum += *x;
    }
    sum
}

pub struct Config {
    pub timeout: u64,
}
"#;
        let symbols = vec![
            ExtractedSymbol {
                id: "test::function::calculate_metrics::L6".to_string(),
                name: "calculate_metrics".to_string(),
                kind: "function".to_string(),
                parent: None,
                start_line: 6,
                end_line: 12,
                signature: Some("pub fn calculate_metrics(data: &[i32]) -> i32".to_string()),
            },
            ExtractedSymbol {
                id: "test::struct::Config::L14".to_string(),
                name: "Config".to_string(),
                kind: "struct".to_string(),
                parent: None,
                start_line: 14,
                end_line: 16,
                signature: Some("pub struct Config".to_string()),
            },
        ];

        let chunks = build_semantic_chunks("test.rs", code, &symbols, 50, 10);
        assert!(!chunks.is_empty());

        // Header chunk
        assert!(chunks.iter().any(|c| c.text.contains("use std::collections::HashMap;")));

        // Atomic function chunk
        let fn_chunk = chunks.iter().find(|c| c.start_line == 6);
        assert!(fn_chunk.is_some());
        let c = fn_chunk.unwrap();
        assert_eq!(c.end_line, 12);
        assert!(c.text.contains("pub fn calculate_metrics"));
        assert!(c.text.contains("sum += *x;"));

        // Atomic struct chunk
        let struct_chunk = chunks.iter().find(|c| c.start_line == 14);
        assert!(struct_chunk.is_some());
        assert_eq!(struct_chunk.unwrap().end_line, 16);
    }

    #[test]
    fn test_oversized_symbol_windowing() {
        let mut lines = Vec::new();
        lines.push("pub fn big_processor() {".to_string());
        for i in 1..=100 {
            lines.push(format!("    let step_{} = {};", i, i));
        }
        lines.push("}".to_string());
        let code = lines.join("\n");

        let symbols = vec![ExtractedSymbol {
            id: "test::function::big_processor::L1".to_string(),
            name: "big_processor".to_string(),
            kind: "function".to_string(),
            parent: None,
            start_line: 1,
            end_line: 102,
            signature: Some("pub fn big_processor()".to_string()),
        }];

        let chunks = build_semantic_chunks("test.rs", &code, &symbols, 50, 10);
        assert!(chunks.len() >= 2);

        // First sub-chunk has direct code
        assert!(chunks[0].text.contains("pub fn big_processor"));

        // Subsequent sub-chunk has prepended context header
        assert!(chunks[1].text.contains("// Context: function big_processor (continued from L1)"));
    }

    #[test]
    fn test_empty_and_fallback_chunking() {
        let empty_chunks = build_semantic_chunks("empty.rs", "", &[], 50, 10);
        assert!(empty_chunks.is_empty());

        let plain_text = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let chunks = build_semantic_chunks("notes.txt", plain_text, &[], 50, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 5);
    }
}
