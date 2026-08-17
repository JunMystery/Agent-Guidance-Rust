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
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("process_payment"));
    }
