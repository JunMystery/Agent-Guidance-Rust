//! Document and structured data file classification and single-symbol representation.

use super::parsers::ExtractedSymbol;

/// Categorizes file format into document or structured data if applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDocCategory {
    Document,
    Data,
}

impl FileDocCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Data => "data",
        }
    }
}

/// Checks if a file path is a documentation or structured data file.
pub fn classify_doc_or_data(rel_path: &str) -> Option<FileDocCategory> {
    let lower = rel_path.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");

    match ext {
        "md" | "markdown" | "mdown" | "mkdn" | "txt" | "rst" | "adoc" | "pdf" => {
            Some(FileDocCategory::Document)
        }
        "json" | "yaml" | "yml" | "toml" | "csv" | "tsv" | "xml" | "css" | "scss" | "sass"
        | "less" | "html" | "htm" | "svg" | "ini" | "env" | "lock" | "properties" | "conf" => {
            Some(FileDocCategory::Data)
        }
        _ => None,
    }
}

/// Emits a single file-level symbol for documentation and structured data files.
pub fn extract_doc_or_data_symbol(
    rel_path: &str,
    total_lines: usize,
    category: FileDocCategory,
) -> ExtractedSymbol {
    let file_name = std::path::Path::new(rel_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string());

    let kind = category.as_str().to_string();
    let id = format!("{}::{}::{}::L1", rel_path, kind, file_name);

    ExtractedSymbol {
        id,
        name: file_name,
        kind,
        parent: None,
        start_line: 1,
        end_line: total_lines,
        signature: Some(format!("{} {}", category.as_str(), rel_path)),
        language: Some(category.as_str().to_string()),
        namespace: None,
        receiver: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_doc_and_data() {
        assert_eq!(classify_doc_or_data("skills/foo/SKILL.md"), Some(FileDocCategory::Document));
        assert_eq!(classify_doc_or_data("README.md"), Some(FileDocCategory::Document));
        assert_eq!(classify_doc_or_data("Cargo.toml"), Some(FileDocCategory::Data));
        assert_eq!(classify_doc_or_data("package.json"), Some(FileDocCategory::Data));
        assert_eq!(classify_doc_or_data("dashboard.css"), Some(FileDocCategory::Data));
        assert_eq!(classify_doc_or_data("src/main.rs"), None);
        assert_eq!(classify_doc_or_data("scripts/instinct-cli.py"), None);
    }

    #[test]
    fn test_markdown_and_data_emits_single_symbol_no_functions() {
        let md_content = "# Title\n```python\ndef dummy_func():\n    pass\n```\nclass DummyClass:\n    pass\n";
        let symbols = crate::context::indexer::parsers::extract_symbols_from_content("skills/sample/SKILL.md", md_content);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, "document");
        assert_eq!(symbols[0].name, "SKILL.md");

        let edges = crate::context::indexer::edge_parser::extract_edges_from_content("skills/sample/SKILL.md", md_content, &symbols);
        assert!(edges.is_empty());

        let json_content = "{\n  \"name\": \"agent-guidance\",\n  \"version\": \"1.5.9\"\n}";
        let data_symbols = crate::context::indexer::parsers::extract_symbols_from_content("package.json", json_content);
        assert_eq!(data_symbols.len(), 1);
        assert_eq!(data_symbols[0].kind, "data");
        assert_eq!(data_symbols[0].name, "package.json");
    }
}
