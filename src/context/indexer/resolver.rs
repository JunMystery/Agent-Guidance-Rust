use super::parsers::ExtractedSymbol;

/// Finds the smallest enclosing symbol for a given line number.
pub fn find_enclosing_symbol<'a>(
    symbols: &'a [ExtractedSymbol],
    line: usize,
) -> Option<&'a ExtractedSymbol> {
    symbols
        .iter()
        .filter(|s| s.kind != "module" && s.start_line <= line && line <= s.end_line)
        .min_by_key(|s| s.end_line.saturating_sub(s.start_line))
}

/// Finds the top-level module symbol for a file.
pub fn find_module_symbol<'a>(symbols: &'a [ExtractedSymbol]) -> Option<&'a ExtractedSymbol> {
    symbols.iter().find(|s| s.kind == "module")
}

/// Resolves a callee name within the same file symbols.
pub fn resolve_local_callee<'a>(
    symbols: &'a [ExtractedSymbol],
    callee_name: &str,
    caller_id: &str,
) -> Option<&'a ExtractedSymbol> {
    symbols
        .iter()
        .find(|s| s.name == callee_name && s.id != caller_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_enclosing_symbol() {
        let syms = vec![
            ExtractedSymbol {
                id: "file.rs::module::mod::L1".into(),
                name: "file.rs".into(),
                kind: "module".into(),
                parent: None,
                start_line: 1,
                end_line: 100,
                signature: None,
            },
            ExtractedSymbol {
                id: "file.rs::struct::Foo::L10".into(),
                name: "Foo".into(),
                kind: "struct".into(),
                parent: None,
                start_line: 10,
                end_line: 50,
                signature: None,
            },
            ExtractedSymbol {
                id: "file.rs::function::bar::L20".into(),
                name: "bar".into(),
                kind: "function".into(),
                parent: Some("Foo".into()),
                start_line: 20,
                end_line: 30,
                signature: None,
            },
        ];

        let found = find_enclosing_symbol(&syms, 25).unwrap();
        assert_eq!(found.id, "file.rs::function::bar::L20");

        let found_outer = find_enclosing_symbol(&syms, 15).unwrap();
        assert_eq!(found_outer.id, "file.rs::struct::Foo::L10");
    }
}
