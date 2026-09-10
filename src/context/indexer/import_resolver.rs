// Modular Polyglot Import Path & Symbol Resolver
use std::collections::HashMap;
use std::path::Path;

/// Extracts the target import file or module path from an import/include line.
pub fn extract_import_path(line: &str) -> Option<String> {
    if let Some(idx) = line.find("@import ") {
        let rest = line[idx + 8..].trim();
        let part = rest.split_whitespace().next().unwrap_or("").trim_matches(|c| c == '\'' || c == '"' || c == ';');
        if !part.is_empty() {
            return Some(part.to_string());
        }
    }
    if let Some(idx) = line.find("@use ") {
        let rest = line[idx + 5..].trim();
        let part = rest.split_whitespace().next().unwrap_or("").trim_matches(|c| c == '\'' || c == '"' || c == ';');
        if !part.is_empty() {
            return Some(part.to_string());
        }
    }
    if let Some(idx) = line.find("from ") {
        let rest = line[idx + 5..].trim();
        let part = rest.split_whitespace().next().unwrap_or("").trim_matches(|c| c == '\'' || c == '"' || c == ';');
        if !part.is_empty() {
            return Some(part.to_string());
        }
    }
    if let Some(idx) = line.find("#include") {
        let part = &line[idx + 8..].trim();
        return Some(part.trim_matches(|c| c == '<' || c == '>' || c == '"').to_string());
    }
    if let Some(idx) = line.find("import ") {
        let rest = line[idx + 7..].trim();
        // Quoted import: import "./styles.css" or import "fmt"
        if rest.starts_with('"') || rest.starts_with('\'') {
            return Some(rest.trim_matches(|c| c == '\'' || c == '"' || c == ';').to_string());
        }
        // Dotted package: import com.example.app.Service
        let token = rest.split_whitespace().next().unwrap_or("").trim_end_matches(';');
        if token.contains('.') && !token.contains('{') && !token.contains('*') {
            return Some(token.replace('.', "/"));
        }
    }
    if let Some(idx) = line.find("use ") {
        let rest = line[idx + 4..].trim();
        if let Some(crate_path) = rest.strip_prefix("crate::") {
            let token = crate_path.split("::").take(2).collect::<Vec<_>>().join("/");
            let token = token.split('{').next().unwrap_or("").trim_matches(|c| c == ';' || c == ' ');
            if !token.is_empty() {
                return Some(format!("src/{}", token));
            }
        }
    }
    None
}

/// Extracts symbol identifiers imported in an import line.
pub fn extract_imported_symbol_names(line: &str) -> Vec<String> {
    line.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty() && *s != "use" && *s != "import" && *s != "from" && *s != "crate" && *s != "super")
        .map(|s| s.to_string())
        .collect()
}

/// Resolves an import path into an exact tracked relative project file path.
pub fn resolve_relative_import_path(
    from_file: &str,
    import_path: &str,
    file_modules: &HashMap<String, String>,
) -> Option<String> {
    let clean = import_path.trim_matches(|c| c == '\'' || c == '"' || c == ';' || c == '<' || c == '>');
    let clean = clean.strip_prefix("./").unwrap_or(clean);
    let clean = clean.strip_prefix('.').unwrap_or(clean);
    let parent = Path::new(from_file).parent().unwrap_or(Path::new(""));
    let target = parent.join(clean);
    let normalized = target.to_string_lossy().replace('\\', "/").replace("/./", "/");

    let candidates = [
        normalized.clone(),
        format!("{}.ts", normalized),
        format!("{}.tsx", normalized),
        format!("{}.js", normalized),
        format!("{}.jsx", normalized),
        format!("{}.rs", normalized),
        format!("{}.py", normalized),
        format!("{}.kt", normalized),
        format!("{}.java", normalized),
        format!("{}.go", normalized),
        format!("{}.c", normalized),
        format!("{}.cpp", normalized),
        format!("{}.h", normalized),
        format!("{}.hpp", normalized),
        format!("{}.cs", normalized),
        format!("{}.vue", normalized),
        format!("{}.svelte", normalized),
        format!("{}.astro", normalized),
        format!("{}.css", normalized),
        format!("{}.scss", normalized),
        format!("{}.sql", normalized),
        format!("{}.prisma", normalized),
        format!("{}.graphql", normalized),
        format!("{}/index.ts", normalized),
        format!("{}/index.js", normalized),
        format!("{}/mod.rs", normalized),
        format!("{}/__init__.py", normalized),
        clean.to_string(),
        format!("{}.py", clean),
        format!("{}.rs", clean),
    ];

    for cand in &candidates {
        if file_modules.contains_key(cand) {
            return Some(cand.clone());
        }
    }

    // Direct suffix match on tracked file paths
    file_modules
        .keys()
        .find(|k| k.ends_with(&format!("/{}.kt", clean)) || k.ends_with(&format!("/{}.java", clean)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_import_paths() {
        assert_eq!(extract_import_path("from .models import User"), Some(".models".to_string()));
        assert_eq!(extract_import_path("#include <vector>"), Some("vector".to_string()));
        assert_eq!(extract_import_path("import \"./styles.css\";"), Some("./styles.css".to_string()));
        assert_eq!(extract_import_path("import com.example.Service;"), Some("com/example/Service".to_string()));
        assert_eq!(extract_import_path("use crate::catalog::store;"), Some("src/catalog/store".to_string()));
    }

    #[test]
    fn test_resolve_relative_polyglot_paths() {
        let mut modules = HashMap::new();
        modules.insert("src/utils.py".to_string(), "m1".to_string());
        modules.insert("app/src/main/java/com/example/Service.kt".to_string(), "m2".to_string());

        let res_py = resolve_relative_import_path("src/main.py", ".utils", &modules);
        assert_eq!(res_py, Some("src/utils.py".to_string()));

        let res_kt = resolve_relative_import_path("app/Main.kt", "Service", &modules);
        assert_eq!(res_kt, Some("app/src/main/java/com/example/Service.kt".to_string()));
    }
}
