    use super::*;

    #[test]
    fn test_extract_symbols_and_chunks() {
        let code = r#"
pub struct PaymentService {
    api_key: String,
}

impl PaymentService {
    pub fn process_payment(&self, amount: u64) -> bool {
        let timeout = 30;
        true
    }
}
"#;
        let symbols = extract_symbols_from_content("src/payment.rs", code);
        assert!(symbols.iter().any(|s| s.name == "PaymentService" && s.kind == "struct"));
        assert!(symbols.iter().any(|s| s.name == "process_payment" && s.kind == "function"));

        let chunks = chunk_code_content("src/payment.rs", code, 50, 10);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().any(|c| c.text.contains("process_payment")));
    }

    #[test]
    fn test_extract_edges_uses_symbol_id() {
        let code = r#"
pub fn helper() -> bool {
    true
}

pub fn main_runner() {
    helper();
}
"#;
        let symbols = extract_symbols_from_content("src/app.rs", code);
        let edges = extract_edges_from_content("src/app.rs", code, &symbols);
        assert!(!edges.is_empty(), "Should extract at least 1 call edge");
        let edge = &edges[0];
        assert_eq!(edge.edge_type, "calls");
        assert!(edge.source_id.contains("src/app.rs::function::main_runner"), "source_id must be caller symbol ID: {}", edge.source_id);
        assert!(edge.target_id.contains("src/app.rs::function::helper"), "target_id must be callee symbol ID: {}", edge.target_id);
    }
