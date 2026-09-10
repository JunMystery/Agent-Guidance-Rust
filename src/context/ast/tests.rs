//! Tests for AST parsing, symbol extraction, call detection, and skeleton generation.

#[cfg(test)]
mod tests {
    use crate::context::ast::{AstEngine, AstLanguage};

    #[test]
    fn test_ast_rust_symbol_and_call_extraction() {
        let code = r#"
pub struct Service {
    name: String,
}

impl Service {
    pub fn process(&self) -> bool {
        self.validate();
        true
    }

    fn validate(&self) {
        println!("validating");
    }
}
"#;
        let symbols = AstEngine::extract_symbols(code, AstLanguage::Rust);
        assert!(symbols.iter().any(|s| s.name == "Service" && s.kind == "struct"));
        assert!(symbols.iter().any(|s| s.name == "process" && s.kind == "function"));
        assert!(symbols.iter().any(|s| s.name == "validate" && s.kind == "function"));

        let calls = AstEngine::extract_calls(code, AstLanguage::Rust);
        assert!(calls.iter().any(|c| c.callee_name == "validate"));
    }

    #[test]
    fn test_ast_python_symbol_and_call_extraction() {
        let code = r#"
class TaskHandler:
    def execute(self):
        self.cleanup()
        return True

    def cleanup(self):
        pass
"#;
        let symbols = AstEngine::extract_symbols(code, AstLanguage::Python);
        assert!(symbols.iter().any(|s| s.name == "TaskHandler" && s.kind == "class"));
        assert!(symbols.iter().any(|s| s.name == "execute" && s.kind == "function"));
        assert!(symbols.iter().any(|s| s.name == "cleanup" && s.kind == "function"));

        let calls = AstEngine::extract_calls(code, AstLanguage::Python);
        assert!(calls.iter().any(|c| c.callee_name == "cleanup"));
    }

    #[test]
    fn test_ast_typescript_symbol_extraction() {
        let code = r#"
export interface UserConfig {
    timeout: number;
}

export class Client {
    async fetch(url: string): Promise<string> {
        return this.send(url);
    }

    private send(url: string): string {
        return url;
    }
}
"#;
        let symbols = AstEngine::extract_symbols(code, AstLanguage::TypeScript);
        assert!(symbols.iter().any(|s| s.name == "UserConfig" && s.kind == "interface"));
        assert!(symbols.iter().any(|s| s.name == "Client" && s.kind == "class"));
        assert!(symbols.iter().any(|s| s.name == "fetch" && s.kind == "function"));
        assert!(symbols.iter().any(|s| s.name == "send" && s.kind == "function"));

        let calls = AstEngine::extract_calls(code, AstLanguage::TypeScript);
        assert!(calls.iter().any(|c| c.callee_name == "send"));
    }

    #[test]
    fn test_ast_go_symbol_and_call_extraction() {
        let code = r#"
package main

type Server struct {
    port int
}

func (s *Server) Start() {
    s.log()
}

func (s *Server) log() {
    println("ready")
}
"#;
        let symbols = AstEngine::extract_symbols(code, AstLanguage::Go);
        assert!(symbols.iter().any(|s| s.name == "Server" && s.kind == "type"));
        assert!(symbols.iter().any(|s| s.name == "Start" && s.kind == "function"));
        assert!(symbols.iter().any(|s| s.name == "log" && s.kind == "function"));

        let calls = AstEngine::extract_calls(code, AstLanguage::Go);
        assert!(calls.iter().any(|c| c.callee_name == "log"));
    }

    #[test]
    fn test_ast_find_symbol_exact_boundary() {
        let code = r#"
fn first() {}

fn target_function() -> i32 {
    let x = 10;
    x * 2
}

fn last() {}
"#;
        let sym = AstEngine::find_symbol(code, AstLanguage::Rust, "target_function");
        assert!(sym.is_some());
        let s = sym.unwrap();
        assert_eq!(s.start_line, 4);
        assert_eq!(s.end_line, 7);
    }

    #[test]
    fn test_ast_skeleton_generation() {
        let rs_code = r#"
pub struct Data {
    id: u64,
}

impl Data {
    pub fn process(&self) -> bool {
        let x = 1;
        let y = 2;
        x + y > 0
    }
}
"#;
        let skeleton = AstEngine::generate_skeleton(rs_code, AstLanguage::Rust, "test.rs").unwrap();
        assert!(skeleton.contains("pub fn process(&self) -> bool { /* L8-11: 4 lines */ }"));
        assert!(skeleton.contains("pub struct Data {"));

        let py_code = r#"
class Worker:
    def run(self):
        step_one()
        step_two()
        return True
"#;
        let py_skeleton = AstEngine::generate_skeleton(py_code, AstLanguage::Python, "test.py").unwrap();
        assert!(py_skeleton.contains("def run(self):"));
        assert!(py_skeleton.contains("... /* L4-6: 3 lines */"));
    }

    #[test]
    fn test_ast_kotlin_symbol_and_call_extraction() {
        let code = r#"
package com.example.app

enum class ScreenTransition {
    SLIDE, FADE
}

data class User(val id: String)

class HomeViewModel {
    fun loadData() {
        fetchUser()
    }

    private fun fetchUser() {}
}
"#;
        let symbols = AstEngine::extract_symbols(code, AstLanguage::Kotlin);
        assert!(symbols.iter().any(|s| s.name == "ScreenTransition" && s.kind == "enum"));
        assert!(!symbols.iter().any(|s| s.name == "class"));
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == "class"));
        assert!(symbols.iter().any(|s| s.name == "loadData" && s.kind == "function"));

        let calls = AstEngine::extract_calls(code, AstLanguage::Kotlin);
        assert!(calls.iter().any(|c| c.callee_name == "fetchUser"));
    }

    #[test]
    fn test_ast_java_and_csharp_and_shell() {
        let java_code = r#"
package com.service;

public class OrderProcessor {
    public void processOrder() {
        validate();
    }
}
"#;
        let j_symbols = AstEngine::extract_symbols(java_code, AstLanguage::Java);
        assert!(j_symbols.iter().any(|s| s.name == "OrderProcessor"));
        assert!(j_symbols.iter().any(|s| s.name == "processOrder"));

        let cs_code = r#"
namespace Core.Services {
    public class Worker {
        public void DoWork() {
            Helper.Run();
        }
    }
}
"#;
        let cs_symbols = AstEngine::extract_symbols(cs_code, AstLanguage::CSharp);
        assert!(cs_symbols.iter().any(|s| s.name == "Worker"));
        assert!(cs_symbols.iter().any(|s| s.name == "DoWork"));
        let cs_calls = AstEngine::extract_calls(cs_code, AstLanguage::CSharp);
        assert!(cs_calls.iter().any(|c| c.callee_name == "Run" && c.receiver.as_deref() == Some("Helper")));

        let sh_code = r#"
source ./common.sh

deploy_service() {
    echo "deploying"
}
"#;
        let sh_symbols = AstEngine::extract_symbols(sh_code, AstLanguage::Shell);
        assert!(sh_symbols.iter().any(|s| s.name == "deploy_service"));
        let sh_calls = AstEngine::extract_calls(sh_code, AstLanguage::Shell);
        assert!(sh_calls.iter().any(|c| c.receiver.as_deref() == Some("source")));
    }

    #[test]
    fn test_ast_ruby_block_and_zig_and_manifests() {
        let rb = "class Service\n  def first\n    call_a()\n  end\n  def second\n    call_b()\n  end\nend\n";
        let rb_calls = AstEngine::extract_calls(rb, AstLanguage::Ruby);
        let first_call = rb_calls.iter().find(|c| c.callee_name == "call_a").unwrap();
        let second_call = rb_calls.iter().find(|c| c.callee_name == "call_b").unwrap();
        assert_eq!(first_call.caller_name.as_deref(), Some("first"));
        assert_eq!(second_call.caller_name.as_deref(), Some("second"));

        let zig = "pub const Point = struct {\n    pub fn add(a: i32, b: i32) i32 {\n        return a + b;\n    }\n};\n";
        let zig_syms = AstEngine::extract_symbols(zig, AstLanguage::Zig);
        assert!(zig_syms.iter().any(|s| s.name == "Point" && s.kind == "struct"));
        assert!(zig_syms.iter().any(|s| s.name == "add" && s.kind == "function"));

        let c_proto = "int calculate_tax(int amount);\nvoid render(void) {}\n";
        let c_syms = AstEngine::extract_symbols(c_proto, AstLanguage::C);
        assert!(c_syms.iter().any(|s| s.name == "calculate_tax" && s.kind == "function"));
        assert!(c_syms.iter().any(|s| s.name == "render" && s.kind == "function"));

        let cargo = "[package]\nname = \"my_pkg\"\n[dependencies]\nserde = \"1.0\"\nrusqlite = \"0.31\"\n";
        let cargo_calls = AstEngine::extract_calls(cargo, AstLanguage::CargoManifest);
        assert!(cargo_calls.iter().any(|c| c.callee_name == "serde"));
        assert!(cargo_calls.iter().any(|c| c.callee_name == "rusqlite"));

        let npm = "{\"name\": \"web\", \"dependencies\": {\"react\": \"^18.0.0\"}}";
        let npm_calls = AstEngine::extract_calls(npm, AstLanguage::NpmManifest);
        assert!(npm_calls.iter().any(|c| c.callee_name == "react"));
    }
}
