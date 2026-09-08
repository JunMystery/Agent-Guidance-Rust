//! AST tree-walking and node extraction for symbols and function calls.

use tree_sitter::Node;
use super::types::{AstCall, AstLanguage, AstSymbol};

/// Extracts all top-level and nested symbols from an AST root.
pub fn extract_symbols(root: Node, source: &[u8], lang: AstLanguage) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    collect_symbols(root, source, lang, None, &mut symbols);
    symbols
}

/// Extracts all call expressions with caller-callee relationships from an AST root.
pub fn extract_calls(root: Node, source: &[u8], lang: AstLanguage) -> Vec<AstCall> {
    let mut calls = Vec::new();
    collect_calls(root, source, lang, None, &mut calls);
    calls
}

fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

fn collect_symbols(
    node: Node,
    source: &[u8],
    lang: AstLanguage,
    parent_sym: Option<&str>,
    symbols: &mut Vec<AstSymbol>,
) {
    if let Some((name, kind, body_node)) = match_symbol_node(node, source, lang, parent_sym) {
        let start_pos = node.start_position();
        let end_pos = node.end_position();
        let sig_end = body_node
            .map(|b| b.start_byte())
            .unwrap_or_else(|| node.end_byte());
        let sig = std::str::from_utf8(&source[node.start_byte()..sig_end])
            .unwrap_or("")
            .trim()
            .to_string();

        let sym = AstSymbol {
            name: name.clone(),
            kind,
            start_line: start_pos.row + 1,
            end_line: end_pos.row + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            signature: sig,
            body_start_line: body_node.map(|b| b.start_position().row + 1),
            body_end_line: body_node.map(|b| b.end_position().row + 1),
        };
        symbols.push(sym);

        // Recurse into children with this symbol as parent
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_symbols(child, source, lang, Some(&name), symbols);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, source, lang, parent_sym, symbols);
    }
}

fn match_symbol_node<'a>(
    node: Node<'a>,
    source: &'a [u8],
    lang: AstLanguage,
    _parent_sym: Option<&str>,
) -> Option<(String, String, Option<Node<'a>>)> {
    let k = node.kind();
    match lang {
        AstLanguage::Rust => match k {
            "function_item" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "function".to_string(), node.child_by_field_name("body")))
            }
            "struct_item" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "struct".to_string(), node.child_by_field_name("body")))
            }
            "enum_item" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "enum".to_string(), node.child_by_field_name("body")))
            }
            "trait_item" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "trait".to_string(), node.child_by_field_name("body")))
            }
            "impl_item" => {
                let type_node = node.child_by_field_name("type")?;
                Some((node_text(&type_node, source).to_string(), "impl".to_string(), node.child_by_field_name("body")))
            }
            _ => None,
        },
        AstLanguage::Python => match k {
            "function_definition" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "function".to_string(), node.child_by_field_name("body")))
            }
            "class_definition" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "class".to_string(), node.child_by_field_name("body")))
            }
            _ => None,
        },
        AstLanguage::TypeScript | AstLanguage::Tsx | AstLanguage::JavaScript => match k {
            "function_declaration" | "generator_function_declaration" | "method_definition" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "function".to_string(), node.child_by_field_name("body")))
            }
            "class_declaration" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "class".to_string(), node.child_by_field_name("body")))
            }
            "interface_declaration" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "interface".to_string(), node.child_by_field_name("body")))
            }
            "type_alias_declaration" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "type_alias".to_string(), None))
            }
            _ => None,
        },
        AstLanguage::Go => match k {
            "function_declaration" | "method_declaration" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "function".to_string(), node.child_by_field_name("body")))
            }
            "type_spec" => {
                let name = node.child_by_field_name("name").map(|n| node_text(&n, source))?;
                Some((name.to_string(), "type".to_string(), None))
            }
            _ => None,
        },
        AstLanguage::Unsupported => None,
    }
}

fn collect_calls(
    node: Node,
    source: &[u8],
    lang: AstLanguage,
    current_caller: Option<&str>,
    calls: &mut Vec<AstCall>,
) {
    let k = node.kind();
    let caller = match k {
        "function_item" | "function_definition" | "function_declaration"
        | "method_definition" | "method_declaration" => {
            node.child_by_field_name("name").map(|n| node_text(&n, source))
        }
        _ => None,
    };
    let active_caller = caller.or(current_caller);

    let is_call = match lang {
        AstLanguage::Python => k == "call",
        _ => k == "call_expression",
    };

    if is_call {
        if let Some(func_node) = node.child_by_field_name("function") {
            let callee_name = extract_callee_name(func_node, source);
            if !callee_name.is_empty() {
                calls.push(AstCall {
                    caller_name: active_caller.map(|s| s.to_string()),
                    callee_name,
                    line: node.start_position().row + 1,
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, lang, active_caller, calls);
    }
}

fn extract_callee_name(func_node: Node, source: &[u8]) -> String {
    // If it's a field / attribute access (e.g. obj.foo() or obj->foo()), extract property name
    if let Some(field) = func_node.child_by_field_name("field") {
        return node_text(&field, source).to_string();
    }
    if let Some(attr) = func_node.child_by_field_name("attribute") {
        return node_text(&attr, source).to_string();
    }
    if let Some(prop) = func_node.child_by_field_name("property") {
        return node_text(&prop, source).to_string();
    }
    node_text(&func_node, source).to_string()
}
