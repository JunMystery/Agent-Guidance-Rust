//! Frontend Single-File Component (Vue, Svelte, Astro, HTML) & CSS AST Extractor.

use super::types::{AstCall, AstLanguage, AstSymbol};
use regex::Regex;
use std::sync::LazyLock;

static COMPONENT_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<([A-Z][A-Za-z0-9_-]+)[^>]*[/]?>"#).unwrap()
});

static SCRIPT_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:export\s+)?(?:async\s+)?function\s+([A-Za-z0-9_]+)\s*\("#).unwrap()
});

static SCRIPT_CONST_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:export\s+)?const\s+([A-Za-z0-9_]+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>"#).unwrap()
});

static SVELTE_PROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"export\s+let\s+([A-Za-z0-9_]+)"#).unwrap()
});

static VUE_DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"const\s+([A-Za-z0-9_]+)\s*=\s*(?:ref|reactive|computed|defineProps|defineEmits)\s*\(?"#).unwrap()
});

static HTML_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<template\s+id=["']([^"']+)["']"#).unwrap()
});

static CSS_RULE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(\.[A-Za-z0-9_-]+)\s*\{"#).unwrap()
});

static CALL_INVOKE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([A-Za-z0-9_$]+)\s*\("#).unwrap()
});

/// Extracts structural symbols from frontend files (components, functions, props, rules).
pub fn extract_symbols(content: &str, lang: AstLanguage) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        match lang {
            AstLanguage::Vue | AstLanguage::Svelte | AstLanguage::Astro => {
                if let Some(cap) = SCRIPT_FN_RE.captures(trimmed) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(build_sym(m.as_str(), "function", line_num, &lines, lang));
                    }
                } else if let Some(cap) = SCRIPT_CONST_FN_RE.captures(trimmed) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(build_sym(m.as_str(), "function", line_num, &lines, lang));
                    }
                } else if lang == AstLanguage::Svelte {
                    if let Some(cap) = SVELTE_PROP_RE.captures(trimmed) {
                        if let Some(m) = cap.get(1) {
                            symbols.push(build_sym(m.as_str(), "property", line_num, &lines, lang));
                        }
                    }
                } else if lang == AstLanguage::Vue {
                    if let Some(cap) = VUE_DEF_RE.captures(trimmed) {
                        if let Some(m) = cap.get(1) {
                            symbols.push(build_sym(m.as_str(), "state", line_num, &lines, lang));
                        }
                    }
                }
            }
            AstLanguage::Html => {
                if let Some(cap) = HTML_TAG_RE.captures(trimmed) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(build_sym(m.as_str(), "template", line_num, &lines, lang));
                    }
                } else if let Some(cap) = SCRIPT_FN_RE.captures(trimmed) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(build_sym(m.as_str(), "function", line_num, &lines, lang));
                    }
                }
            }
            AstLanguage::Css => {
                for cap in CSS_RULE_RE.captures_iter(trimmed) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(build_sym(m.as_str(), "class", line_num, &lines, lang));
                    }
                }
            }
            _ => {}
        }
    }

    symbols
}

/// Extracts child component invocations and script calls from frontend files.
pub fn extract_calls(content: &str, lang: AstLanguage) -> Vec<AstCall> {
    let mut calls = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        // 1. Child component invocations in templates (<Button>, <UserAvatar />)
        if matches!(lang, AstLanguage::Vue | AstLanguage::Svelte | AstLanguage::Astro | AstLanguage::Html) {
            for cap in COMPONENT_TAG_RE.captures_iter(trimmed) {
                if let Some(m) = cap.get(1) {
                    let tag = m.as_str();
                    if tag != "Template" && tag != "Script" && tag != "Style" {
                        calls.push(AstCall {
                            caller_name: None,
                            callee_name: tag.to_string(),
                            line: line_num,
                            receiver: None,
                            namespace: None,
                        });
                    }
                }
            }
        }

        // 2. Script calls inside <script> blocks or JS expressions
        if matches!(lang, AstLanguage::Vue | AstLanguage::Svelte | AstLanguage::Astro) {
            if trimmed.contains('(') && !trimmed.starts_with("//") && !trimmed.starts_with('*') {
                for cap in CALL_INVOKE_RE.captures_iter(trimmed) {
                    if let Some(m) = cap.get(1) {
                        let name = m.as_str();
                        if !is_js_keyword(name) {
                            calls.push(AstCall {
                                caller_name: None,
                                callee_name: name.to_string(),
                                line: line_num,
                                receiver: None,
                                namespace: None,
                            });
                        }
                    }
                }
            }
        }
    }

    calls
}

fn build_sym(name: &str, kind: &str, line: usize, lines: &[&str], lang: AstLanguage) -> AstSymbol {
    let sig = lines.get(line.saturating_sub(1)).unwrap_or(&"").trim().to_string();
    AstSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        start_line: line,
        end_line: line,
        start_byte: 0,
        end_byte: 0,
        signature: sig,
        body_start_line: Some(line),
        body_end_line: Some(line),
        parent_symbol: None,
        package: None,
        language: lang.as_str().to_string(),
    }
}

fn is_js_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "for" | "while" | "switch" | "catch" | "return" | "function" | "import" | "export" | "typeof"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vue_sfc_extraction() {
        let code = r#"
<template>
  <div class="container">
    <Button @click="handleClick" />
    <UserProfile :user="currentUser" />
  </div>
</template>
<script setup>
import Button from './Button.vue';
import UserProfile from './UserProfile.vue';
const count = ref(0);
function handleClick() {
  count.value++;
}
</script>
"#;
        let syms = extract_symbols(code, AstLanguage::Vue);
        assert!(syms.iter().any(|s| s.name == "handleClick" && s.kind == "function"));
        assert!(syms.iter().any(|s| s.name == "count" && s.kind == "state"));

        let calls = extract_calls(code, AstLanguage::Vue);
        assert!(calls.iter().any(|c| c.callee_name == "Button"));
        assert!(calls.iter().any(|c| c.callee_name == "UserProfile"));
    }

    #[test]
    fn test_svelte_and_css_extraction() {
        let svelte = r#"
<script>
  import Header from './Header.svelte';
  export let title;
  function update() { console.log(title); }
</script>
<Header />
"#;
        let svelte_syms = extract_symbols(svelte, AstLanguage::Svelte);
        assert!(svelte_syms.iter().any(|s| s.name == "title" && s.kind == "property"));
        assert!(svelte_syms.iter().any(|s| s.name == "update" && s.kind == "function"));

        let svelte_calls = extract_calls(svelte, AstLanguage::Svelte);
        assert!(svelte_calls.iter().any(|c| c.callee_name == "Header"));

        let css = ".card { padding: 10px; } .btn-primary { color: red; }";
        let css_syms = extract_symbols(css, AstLanguage::Css);
        assert!(css_syms.iter().any(|s| s.name == ".card" && s.kind == "class"));
        assert!(css_syms.iter().any(|s| s.name == ".btn-primary" && s.kind == "class"));
    }
}
