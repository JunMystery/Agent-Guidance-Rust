//! AST walker for extracting traits, interfaces, and implementation bindings.

use tree_sitter::Node;
use super::trait_def::{TraitBinding, TraitSpec};
use super::types::AstLanguage;

fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

/// Extract trait definitions and implementation bindings from an AST root.
pub fn extract_trait_bindings(root: Node, source: &[u8], lang: AstLanguage) -> (Vec<TraitSpec>, Vec<TraitBinding>) {
    let mut specs = Vec::new();
    let mut bindings = Vec::new();
    collect_traits(root, source, lang, &mut specs, &mut bindings);
    (specs, bindings)
}

fn collect_traits(
    node: Node,
    source: &[u8],
    lang: AstLanguage,
    specs: &mut Vec<TraitSpec>,
    bindings: &mut Vec<TraitBinding>,
) {
    let k = node.kind();
    match lang {
        AstLanguage::Rust => match k {
            "trait_item" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(&name_node, source).to_string();
                    let methods = extract_methods_by_field(node, "body", source, &["function_signature_item", "function_item"]);
                    specs.push(TraitSpec {
                        name,
                        method_signatures: methods,
                        line: node.start_position().row + 1,
                        kind: "trait".to_string(),
                    });
                }
            }
            "impl_item" => {
                if let (Some(trait_node), Some(type_node)) = (
                    node.child_by_field_name("trait"),
                    node.child_by_field_name("type"),
                ) {
                    let trait_name = clean_rust_type(node_text(&trait_node, source));
                    let implementor_name = clean_rust_type(node_text(&type_node, source));
                    let methods = extract_methods_by_field(node, "body", source, &["function_item"]);
                    bindings.push(TraitBinding {
                        trait_name,
                        implementor_name,
                        line: node.start_position().row + 1,
                        methods,
                        language: "rust".to_string(),
                        is_inferred: false,
                    });
                }
            }
            _ => {}
        },
        AstLanguage::TypeScript | AstLanguage::Tsx | AstLanguage::JavaScript => match k {
            "interface_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(&name_node, source).to_string();
                    let methods = extract_methods_by_field(node, "body", source, &["method_signature", "property_signature"]);
                    specs.push(TraitSpec {
                        name,
                        method_signatures: methods,
                        line: node.start_position().row + 1,
                        kind: "interface".to_string(),
                    });
                }
            }
            "class_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let class_name = node_text(&name_node, source).to_string();
                    let methods = extract_methods_by_field(node, "body", source, &["method_definition"]);
                    let interfaces = extract_ts_class_interfaces(node, source);
                    for iface in interfaces {
                        bindings.push(TraitBinding {
                            trait_name: iface,
                            implementor_name: class_name.clone(),
                            line: node.start_position().row + 1,
                            methods: methods.clone(),
                            language: "typescript".to_string(),
                            is_inferred: false,
                        });
                    }
                }
            }
            _ => {}
        },
        AstLanguage::Python if k == "class_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let class_name = node_text(&name_node, source).to_string();
                let methods = extract_methods_by_field(node, "body", source, &["function_definition"]);
                let superclasses = extract_py_superclasses(node, source);
                for sc in superclasses {
                    bindings.push(TraitBinding {
                        trait_name: sc,
                        implementor_name: class_name.clone(),
                        line: node.start_position().row + 1,
                        methods: methods.clone(),
                        language: "python".to_string(),
                        is_inferred: false,
                    });
                }
            }
        }
        AstLanguage::Go => match k {
            "type_spec" => {
                if let (Some(name_node), Some(type_node)) = (
                    node.child_by_field_name("name"),
                    node.child_by_field_name("type"),
                ) {
                    if type_node.kind() == "interface_type" {
                        let iface_name = node_text(&name_node, source).to_string();
                        let methods = extract_direct_child_methods(type_node, source, &["method_elem"]);
                        specs.push(TraitSpec {
                            name: iface_name,
                            method_signatures: methods,
                            line: node.start_position().row + 1,
                            kind: "interface".to_string(),
                        });
                    }
                }
            }
            "var_declaration" => {
                if let Some(binding) = extract_go_type_assertion_binding(node, source) {
                    bindings.push(binding);
                }
            }
            _ => {}
        },
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_traits(child, source, lang, specs, bindings);
    }
}

fn clean_rust_type(raw: &str) -> String {
    raw.split('<').next().unwrap_or(raw).trim().to_string()
}

fn extract_methods_by_field(node: Node, field: &str, source: &[u8], kinds: &[&str]) -> Vec<String> {
    node.child_by_field_name(field)
        .map(|b| extract_direct_child_methods(b, source, kinds))
        .unwrap_or_default()
}

fn extract_direct_child_methods(parent: Node, source: &[u8], kinds: &[&str]) -> Vec<String> {
    let mut methods = Vec::new();
    let mut cur = parent.walk();
    for ch in parent.children(&mut cur) {
        if kinds.contains(&ch.kind()) {
            if let Some(n) = ch.child_by_field_name("name") {
                methods.push(node_text(&n, source).to_string());
            }
        }
    }
    methods
}

fn extract_ts_class_interfaces(node: Node, source: &[u8]) -> Vec<String> {
    let mut ifaces = Vec::new();
    let mut cur = node.walk();
    for ch in node.children(&mut cur) {
        if ch.kind() == "class_heritage" {
            let mut hcur = ch.walk();
            for clause in ch.children(&mut hcur) {
                if clause.kind() == "implements_clause" || clause.kind() == "extends_clause" {
                    let mut icur = clause.walk();
                    for item in clause.children(&mut icur) {
                        if item.kind() == "type_identifier" || item.kind() == "identifier" {
                            ifaces.push(node_text(&item, source).to_string());
                        }
                    }
                }
            }
        }
    }
    ifaces
}

fn extract_py_superclasses(node: Node, source: &[u8]) -> Vec<String> {
    let mut supers = Vec::new();
    if let Some(args) = node.child_by_field_name("superclasses") {
        let mut cur = args.walk();
        for ch in args.children(&mut cur) {
            let k = ch.kind();
            if k == "identifier" || k == "attribute" {
                let name = node_text(&ch, source);
                if !name.is_empty() && name != "object" {
                    supers.push(name.to_string());
                }
            }
        }
    }
    supers
}

fn extract_go_type_assertion_binding(node: Node, source: &[u8]) -> Option<TraitBinding> {
    let text = node_text(&node, source);
    if text.starts_with("var _ ") && text.contains('=') {
        let parts: Vec<&str> = text.split('=').collect();
        if parts.len() == 2 {
            let lhs = parts[0].trim().strip_prefix("var _ ")?.trim();
            let rhs = parts[1].trim().trim_end_matches(';').trim();
            let iface_name = lhs.split_whitespace().next()?.to_string();
            let struct_name = rhs.trim_start_matches("(*").trim_start_matches('&')
                .split(')').next()?.split('{').next()?.trim().to_string();
            if !iface_name.is_empty() && !struct_name.is_empty() {
                return Some(TraitBinding {
                    trait_name: iface_name,
                    implementor_name: struct_name,
                    line: node.start_position().row + 1,
                    methods: Vec::new(),
                    language: "go".to_string(),
                    is_inferred: false,
                });
            }
        }
    }
    None
}
