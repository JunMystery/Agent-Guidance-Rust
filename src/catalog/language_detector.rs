use std::collections::HashSet;
use crate::context::scanner::FileEntry;

#[derive(Debug, Clone, Default)]
pub struct ProjectLanguageProfile {
    pub primary_languages: HashSet<String>,
    pub secondary_tech: HashSet<String>,
}

impl ProjectLanguageProfile {
    pub fn is_empty(&self) -> bool {
        self.primary_languages.is_empty() && self.secondary_tech.is_empty()
    }
}

pub fn detect_language_profile(files: &[FileEntry], task_prompt: &str) -> ProjectLanguageProfile {
    let mut ext_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for file in files {
        let path = file.path.to_lowercase();
        if let Some(ext) = path.split('.').last() {
            *ext_counts.entry(ext.to_string()).or_insert(0) += 1;
        }
        if path.ends_with("dockerfile") || path.contains("docker") {
            *ext_counts.entry("docker".to_string()).or_insert(0) += 1;
        }
    }

    let mut profile = ProjectLanguageProfile::default();

    // Map extensions to primary programming languages
    for (ext, _count) in &ext_counts {
        match ext.as_str() {
            "rs" => { profile.primary_languages.insert("rust".to_string()); },
            "py" | "pyw" => { profile.primary_languages.insert("python".to_string()); },
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => { profile.primary_languages.insert("javascript".to_string()); profile.primary_languages.insert("typescript".to_string()); },
            "go" => { profile.primary_languages.insert("go".to_string()); },
            "java" => { profile.primary_languages.insert("java".to_string()); },
            "cpp" | "cxx" | "cc" | "c" | "h" | "hpp" => { profile.primary_languages.insert("c++".to_string()); profile.primary_languages.insert("c".to_string()); },
            "rb" => { profile.primary_languages.insert("ruby".to_string()); },
            "php" => { profile.primary_languages.insert("php".to_string()); },
            "cs" => { profile.primary_languages.insert("c#".to_string()); },
            "kt" | "kts" => { profile.primary_languages.insert("kotlin".to_string()); },
            "swift" => { profile.primary_languages.insert("swift".to_string()); },
            // Secondary tech & domain languages (even if 1-2 files exist)
            "sql" => { profile.secondary_tech.insert("sql".to_string()); profile.secondary_tech.insert("database".to_string()); },
            "html" | "css" | "scss" | "vue" | "svelte" => { profile.secondary_tech.insert("web".to_string()); profile.secondary_tech.insert("frontend".to_string()); },
            "docker" | "dockerfile" => { profile.secondary_tech.insert("docker".to_string()); profile.secondary_tech.insert("devops".to_string()); },
            "sh" | "bash" | "zsh" => { profile.secondary_tech.insert("bash".to_string()); profile.secondary_tech.insert("shell".to_string()); },
            _ => {}
        }
    }

    // Explicit User Mentions in prompt override/add to profile
    let prompt_lower = task_prompt.to_lowercase();
    let known_tech = vec![
        ("rust", "rust"), ("python", "python"), ("javascript", "javascript"), ("typescript", "typescript"),
        ("golang", "go"), ("go", "go"), ("java", "java"), ("c++", "c++"), ("c#", "c#"),
        ("sql", "sql"), ("database", "database"), ("docker", "docker"), ("frontend", "frontend"),
        ("html", "web"), ("css", "web"), ("bash", "bash")
    ];

    for (keyword, tech) in known_tech {
        if prompt_lower.contains(keyword) {
            profile.primary_languages.insert(tech.to_string());
            profile.secondary_tech.insert(tech.to_string());
        }
    }

    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_profile() {
        let files = vec![
            FileEntry { path: "src/main.rs".to_string(), file_type: "file".to_string(), size_bytes: 100 },
            FileEntry { path: "src/lib.rs".to_string(), file_type: "file".to_string(), size_bytes: 100 },
            FileEntry { path: "migrations/schema.sql".to_string(), file_type: "file".to_string(), size_bytes: 100 },
            FileEntry { path: "Dockerfile".to_string(), file_type: "file".to_string(), size_bytes: 100 },
        ];

        let profile = detect_language_profile(&files, "refactor database schema");
        assert!(profile.primary_languages.contains("rust"));
        assert!(profile.secondary_tech.contains("sql"));
        assert!(profile.secondary_tech.contains("database"));
        assert!(profile.secondary_tech.contains("docker"));

        let py_profile = detect_language_profile(&files, "write a python script");
        assert!(py_profile.primary_languages.contains("python"));
    }
}

