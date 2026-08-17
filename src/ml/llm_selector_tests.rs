    use super::*;
    use crate::catalog::store::SkillSource;

    #[test]
    fn test_keyword_fallback() {
        let selector = LLMSelector::new();
        let candidates = vec![
            (
                0.5,
                SkillItem {
                    name: "rust-testing".to_string(),
                    relative_path: "rust-testing/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "Testing in Rust.".to_string(),
                },
            ),
            (
                0.8,
                SkillItem {
                    name: "rust-async".to_string(),
                    relative_path: "rust-async/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "Async programming in Rust.".to_string(),
                },
            ),
        ];
        let ranked = selector.keyword_fallback("rust async programming", candidates, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].1.name, "rust-async");
    }

    #[test]
    fn test_language_aware_filtering() {
        use crate::catalog::language_detector::ProjectLanguageProfile;

        let selector = LLMSelector::new();
        let candidates = vec![
            (
                0.8,
                SkillItem {
                    name: "python-fastapi-guide".to_string(),
                    relative_path: "skills/python-fastapi/SKILL.md".to_string(),
                    content: "FastAPI guidelines".to_string(),
                    source: SkillSource::Embedded,
                },
            ),
            (
                0.8,
                SkillItem {
                    name: "rust-best-practices".to_string(),
                    relative_path: "skills/rust-best-practices/SKILL.md".to_string(),
                    content: "Rust coding guidelines".to_string(),
                    source: SkillSource::Embedded,
                },
            ),
        ];

        let mut rust_profile = ProjectLanguageProfile::default();
        rust_profile.primary_languages.insert("rust".to_string());

        let results = selector.rerank("optimize code", candidates, &rust_profile, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.name, "rust-best-practices");
    }
