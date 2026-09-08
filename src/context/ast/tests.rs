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
}
