//! AST walker for intra-procedural parameters, arguments, and def-use tracking.

use std::collections::{HashMap, HashSet};
use tree_sitter::Node;
use super::dataflow_def::{CallArgDetail, FormalParam};
use super::types::AstLanguage;

fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

/// Extracts all function definitions along with their formal parameters and outgoing call arguments.
pub fn extract_dataflow(
    root: Node,
    source: &[u8],
    lang: AstLanguage,
) -> Vec<(String, Vec<FormalParam>, Vec<CallArgDetail>)> {
    let mut results = Vec::new();
    collect_function_dataflow(root, source, lang, &mut results);
    results
}

fn collect_function_dataflow(
    node: Node,
    source: &[u8],
    lang: AstLanguage,
    results: &mut Vec<(String, Vec<FormalParam>, Vec<CallArgDetail>)>,
) {
    let k = node.kind();
    let (is_func, func_name_opt, target_node, body_node) = match lang {
        AstLanguage::Rust => (k == "function_item", node.child_by_field_name("name").map(|n| node_text(&n, source).to_string()), node, node.child_by_field_name("body")),
        AstLanguage::Python => (k == "function_definition", node.child_by_field_name("name").map(|n| node_text(&n, source).to_string()), node, node.child_by_field_name("body")),
        AstLanguage::TypeScript | AstLanguage::Tsx | AstLanguage::JavaScript => {
            if k == "function_declaration" || k == "method_definition" || k == "generator_function_declaration" {
                (true, node.child_by_field_name("name").map(|n| node_text(&n, source).to_string()), node, node.child_by_field_name("body"))
            } else if k == "variable_declarator" {
                let val_opt = node.child_by_field_name("value");
                let is_arrow = val_opt.as_ref().map(|v| v.kind() == "arrow_function" || v.kind() == "function_expression").unwrap_or(false);
                if is_arrow {
                    let v = val_opt.unwrap();
                    let name = node.child_by_field_name("name").map(|n| node_text(&n, source).to_string());
                    let b = v.child_by_field_name("body").or(Some(v));
                    (true, name, v, b)
                } else {
                    (false, None, node, None)
                }
            } else {
                (false, None, node, None)
            }
        }
        AstLanguage::Go => (k == "function_declaration" || k == "method_declaration", node.child_by_field_name("name").map(|n| node_text(&n, source).to_string()), node, node.child_by_field_name("body")),
        _ => (false, None, node, None),
    };

    if is_func {
        if let Some(func_name) = func_name_opt {
            let params = extract_formal_parameters(target_node, source, lang);
            let mut param_names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            let mut alias_map: HashMap<String, String> = HashMap::new();
            let mut call_args = Vec::new();

            if let Some(body) = body_node {
                analyze_body_dataflow(body, source, lang, &mut param_names, &mut alias_map, &mut call_args, true);
            }

            results.push((func_name, params, call_args));
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_function_dataflow(child, source, lang, results);
    }
}

pub fn extract_formal_parameters(func_node: Node, source: &[u8], lang: AstLanguage) -> Vec<FormalParam> {
    let mut params = Vec::new();
    let param_node = func_node.child_by_field_name("parameters");

    let Some(p_node) = param_node else {
        return params;
    };

    let mut cur = p_node.walk();
    let mut idx = 0;
    for ch in p_node.children(&mut cur) {
        let kind = ch.kind();
        if kind == "(" || kind == ")" || kind == "," {
            continue;
        }
        let (name, type_hint) = match lang {
            AstLanguage::Rust => {
                let name = ch.child_by_field_name("pattern")
                    .map(|n| node_text(&n, source))
                    .unwrap_or_else(|| node_text(&ch, source));
                let ty = ch.child_by_field_name("type").map(|t| node_text(&t, source).to_string());
                (name, ty)
            }
            AstLanguage::TypeScript | AstLanguage::Tsx | AstLanguage::JavaScript | AstLanguage::Python | AstLanguage::Go => {
                let name = ch.child_by_field_name("name")
                    .map(|n| node_text(&n, source))
                    .unwrap_or_else(|| node_text(&ch, source));
                let ty = ch.child_by_field_name("type").map(|t| node_text(&t, source).to_string());
                (name, ty)
            }
            _ => (node_text(&ch, source), None),
        };

        let clean_name = name
            .trim()
            .trim_start_matches('&')
            .trim_start_matches("mut ")
            .trim_start_matches('*')
            .trim_start_matches('.')
            .trim();

        if !clean_name.is_empty() && clean_name != "self" && clean_name != "this" {
            params.push(FormalParam {
                name: clean_name.to_string(),
                index: idx,
                type_hint,
            });
            idx += 1;
        }
    }
    params
}

fn analyze_body_dataflow(
    node: Node,
    source: &[u8],
    lang: AstLanguage,
    params: &mut HashSet<String>,
    alias_map: &mut HashMap<String, String>,
    call_args: &mut Vec<CallArgDetail>,
    is_root: bool,
) {
    let k = node.kind();

    // Prevent call-sites in nested functions from leaking into parent scope
    if !is_root {
        let is_nested = match lang {
            AstLanguage::Rust => k == "function_item",
            AstLanguage::Python => k == "function_definition",
            AstLanguage::TypeScript | AstLanguage::Tsx | AstLanguage::JavaScript => {
                k == "function_declaration" || k == "arrow_function" || k == "function_expression"
            }
            AstLanguage::Go => k == "function_declaration",
            _ => false,
        };
        if is_nested {
            return;
        }
    }

    // 1. Def-Use Alias tracking
    match lang {
        AstLanguage::Rust if k == "let_declaration" => {
            if let (Some(pat), Some(val)) = (node.child_by_field_name("pattern"), node.child_by_field_name("value")) {
                track_alias(node_text(&pat, source).trim(), node_text(&val, source).trim(), params, alias_map);
            }
        }
        AstLanguage::TypeScript | AstLanguage::Tsx | AstLanguage::JavaScript if k == "variable_declarator" => {
            if let (Some(name), Some(val)) = (node.child_by_field_name("name"), node.child_by_field_name("value")) {
                track_alias(node_text(&name, source).trim(), node_text(&val, source).trim(), params, alias_map);
            }
        }
        AstLanguage::Python if k == "assignment" => {
            if let (Some(left), Some(right)) = (node.child_by_field_name("left"), node.child_by_field_name("right")) {
                track_alias(node_text(&left, source).trim(), node_text(&right, source).trim(), params, alias_map);
            }
        }
        AstLanguage::Go if k == "short_var_declaration" => {
            if let (Some(left), Some(right)) = (node.child_by_field_name("left"), node.child_by_field_name("right")) {
                track_alias(node_text(&left, source).trim(), node_text(&right, source).trim(), params, alias_map);
            }
        }
        _ => {}
    }

    // 2. Call expression inspection
    let is_call = if lang == AstLanguage::Python { k == "call" } else { k == "call_expression" };
    if is_call {
        if let Some(func_node) = node.child_by_field_name("function") {
            let callee_name = extract_callee_name(func_node, source);
            if !callee_name.is_empty() {
                let line = node.start_position().row + 1;
                extract_arguments_from_call(node, source, &callee_name, line, params, alias_map, call_args);
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        analyze_body_dataflow(child, source, lang, params, alias_map, call_args, false);
    }
}

fn track_alias(var_name: &str, expr: &str, params: &HashSet<String>, alias_map: &mut HashMap<String, String>) {
    let clean = expr.trim().trim_start_matches('&').trim_start_matches('*');
    let base = clean.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or(clean);

    if params.contains(clean) {
        alias_map.insert(var_name.to_string(), clean.to_string());
    } else if let Some(orig) = alias_map.get(clean) {
        alias_map.insert(var_name.to_string(), orig.clone());
    } else if params.contains(base) {
        alias_map.insert(var_name.to_string(), base.to_string());
    } else if let Some(orig) = alias_map.get(base) {
        alias_map.insert(var_name.to_string(), orig.clone());
    }
}

fn extract_callee_name(func_node: Node, source: &[u8]) -> String {
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

fn extract_arguments_from_call(
    call_node: Node,
    source: &[u8],
    callee_name: &str,
    call_line: usize,
    params: &HashSet<String>,
    alias_map: &HashMap<String, String>,
    call_args: &mut Vec<CallArgDetail>,
) {
    let Some(a_node) = call_node.child_by_field_name("arguments") else { return; };
    let mut cur = a_node.walk();
    let mut arg_idx = 0;

    for ch in a_node.children(&mut cur) {
        let kind = ch.kind();
        if kind == "(" || kind == ")" || kind == "," { continue; }

        let expr_text = node_text(&ch, source).trim();
        let (source_var, flow_type) = if params.contains(expr_text) {
            (expr_text.to_string(), "direct_param".to_string())
        } else if let Some(p) = alias_map.get(expr_text) {
            (p.clone(), "intermediate_var".to_string())
        } else if expr_text.contains('.') {
            let base = expr_text.split('.').next().unwrap_or(expr_text);
            let s = if params.contains(base) { base } else { alias_map.get(base).map(|s| s.as_str()).unwrap_or(base) };
            (s.to_string(), "field_access".to_string())
        } else {
            (expr_text.to_string(), "direct_param".to_string())
        };

        if !source_var.is_empty() && !source_var.starts_with('"') && !source_var.starts_with('\'') {
            call_args.push(CallArgDetail {
                callee_name: callee_name.to_string(),
                arg_index: arg_idx,
                source_expr: expr_text.to_string(),
                source_var,
                call_line,
                flow_type,
            });
        }
        arg_idx += 1;
    }
}
