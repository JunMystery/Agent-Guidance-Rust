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
                    content: "Rust coding guidelines to optimize code and performance.".to_string(),
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

    #[test]
    fn test_generic_maintenance_task_yields_zero_skills() {
        let selector = LLMSelector::new();
        let candidates = vec![
            (
                0.4,
                SkillItem {
                    name: "prisma-patterns".to_string(),
                    relative_path: "prisma-patterns/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "---\nname: prisma-patterns\ndescription: Prisma ORM patterns\n---\nPrisma ORM guidelines".to_string(),
                },
            ),
            (
                0.4,
                SkillItem {
                    name: "hermes-imports".to_string(),
                    relative_path: "hermes-imports/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "---\nname: hermes-imports\ndescription: Hermes operator workflows\n---\nHermes workflows".to_string(),
                },
            ),
        ];

        // All keywords (bump, version, update, release, changelog) are stopped -> returns 0 skills
        let results = selector.keyword_fallback("bump version to 1.6.2 and update changelog", candidates, 5);
        assert!(results.is_empty(), "Generic maintenance task must not return irrelevant skills");
    }

    #[test]
    fn test_domain_task_preserves_relevant_skill() {
        let selector = LLMSelector::new();
        let candidates = vec![
            (
                0.6,
                SkillItem {
                    name: "git-advanced-workflows".to_string(),
                    relative_path: "git-advanced-workflows/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "---\nname: git-advanced-workflows\ndescription: Git workflow and branching\nactions: [rebase, merge, branch]\n---\nGit branching and rebase guidelines".to_string(),
                },
            ),
        ];

        let results = selector.keyword_fallback("git rebase branch workflow", candidates, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.name, "git-advanced-workflows");
    }
