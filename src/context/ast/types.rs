//! AST domain types and language recognition for polyglot Tree-Sitter & declarative parsing.

use std::path::Path;

/// Supported languages across 3-tier architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AstLanguage {
    // Tier 1: Tree-Sitter native AST
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,

    // Tier 2: Pure Rust Declarative Polyglot Lexer Registry
    Kotlin,
    Java,
    C,
    Cpp,
    CSharp,
    Swift,
    Php,
    Ruby,
    Dart,
    Scala,
    Zig,
    Elixir,
    Lua,
    Shell,

    // Tier 3: Manifest & Schema IDL Engine
    Gradle,
    Maven,
    CargoManifest,
    NpmManifest,
    Cmake,
    Protobuf,
    Graphql,
    Sql,

    // Tier 4: Frontend & Web Frameworks
    Vue,
    Svelte,
    Astro,
    Html,
    Css,

    // Tier 5: Database & ORM Schemas
    Prisma,

    Unsupported,
}

impl AstLanguage {
    /// Detects AST language from file path or extension.
    pub fn from_path(path: &str) -> Self {
        let p = Path::new(path);
        let filename = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let fname_lower = filename.to_ascii_lowercase();

        // Check exact manifest filenames
        if fname_lower == "cargo.toml" {
            return AstLanguage::CargoManifest;
        } else if fname_lower == "package.json" {
            return AstLanguage::NpmManifest;
        } else if fname_lower == "pom.xml" {
            return AstLanguage::Maven;
        } else if fname_lower == "cmakelists.txt" || fname_lower.ends_with(".cmake") {
            return AstLanguage::Cmake;
        } else if fname_lower == "build.gradle" || fname_lower == "settings.gradle" || fname_lower.ends_with(".gradle.kts") {
            return AstLanguage::Gradle;
        }

        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext.to_ascii_lowercase().as_str() {
            "rs" => AstLanguage::Rust,
            "ts" => AstLanguage::TypeScript,
            "tsx" => AstLanguage::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => AstLanguage::JavaScript,
            "py" | "pyw" => AstLanguage::Python,
            "go" => AstLanguage::Go,
            "kt" | "kts" => AstLanguage::Kotlin,
            "java" => AstLanguage::Java,
            "c" | "h" => AstLanguage::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => AstLanguage::Cpp,
            "cs" => AstLanguage::CSharp,
            "swift" => AstLanguage::Swift,
            "php" => AstLanguage::Php,
            "rb" => AstLanguage::Ruby,
            "dart" => AstLanguage::Dart,
            "scala" => AstLanguage::Scala,
            "zig" => AstLanguage::Zig,
            "ex" | "exs" => AstLanguage::Elixir,
            "lua" => AstLanguage::Lua,
            "sh" | "bash" | "zsh" | "ps1" => AstLanguage::Shell,
            "proto" => AstLanguage::Protobuf,
            "graphql" | "gql" => AstLanguage::Graphql,
            "sql" => AstLanguage::Sql,
            "vue" => AstLanguage::Vue,
            "svelte" => AstLanguage::Svelte,
            "astro" => AstLanguage::Astro,
            "html" | "htm" => AstLanguage::Html,
            "css" | "scss" | "sass" | "less" => AstLanguage::Css,
            "prisma" => AstLanguage::Prisma,
            _ => AstLanguage::Unsupported,
        }
    }

    /// Returns true if this language uses Tier 1 native Tree-Sitter.
    pub fn is_tier1(&self) -> bool {
        matches!(
            self,
            AstLanguage::Rust
                | AstLanguage::TypeScript
                | AstLanguage::Tsx
                | AstLanguage::JavaScript
                | AstLanguage::Python
                | AstLanguage::Go
        )
    }

    /// Returns true if this language uses Tier 2 pure-Rust declarative parsing.
    pub fn is_tier2(&self) -> bool {
        matches!(
            self,
            AstLanguage::Kotlin
                | AstLanguage::Java
                | AstLanguage::C
                | AstLanguage::Cpp
                | AstLanguage::CSharp
                | AstLanguage::Swift
                | AstLanguage::Php
                | AstLanguage::Ruby
                | AstLanguage::Dart
                | AstLanguage::Scala
                | AstLanguage::Zig
                | AstLanguage::Elixir
                | AstLanguage::Lua
                | AstLanguage::Shell
        )
    }

    /// Returns true if this language is a build manifest.
    pub fn is_manifest(&self) -> bool {
        matches!(
            self,
            AstLanguage::Gradle
                | AstLanguage::Maven
                | AstLanguage::CargoManifest
                | AstLanguage::NpmManifest
                | AstLanguage::Cmake
                | AstLanguage::Protobuf
        )
    }

    /// Returns true if this language is a frontend component or stylesheet.
    pub fn is_frontend(&self) -> bool {
        matches!(
            self,
            AstLanguage::Vue
                | AstLanguage::Svelte
                | AstLanguage::Astro
                | AstLanguage::Html
                | AstLanguage::Css
        )
    }

    /// Returns true if this language is a database schema or query file.
    pub fn is_database(&self) -> bool {
        matches!(
            self,
            AstLanguage::Sql
                | AstLanguage::Prisma
                | AstLanguage::Graphql
        )
    }

    /// Returns true if language is supported by any parser tier.
    pub fn is_supported(&self) -> bool {
        !matches!(self, AstLanguage::Unsupported)
    }

    /// Returns static string identifier for language.
    pub fn as_str(&self) -> &'static str {
        match self {
            AstLanguage::Rust => "rust",
            AstLanguage::TypeScript => "typescript",
            AstLanguage::Tsx => "tsx",
            AstLanguage::JavaScript => "javascript",
            AstLanguage::Python => "python",
            AstLanguage::Go => "go",
            AstLanguage::Kotlin => "kotlin",
            AstLanguage::Java => "java",
            AstLanguage::C => "c",
            AstLanguage::Cpp => "cpp",
            AstLanguage::CSharp => "csharp",
            AstLanguage::Swift => "swift",
            AstLanguage::Php => "php",
            AstLanguage::Ruby => "ruby",
            AstLanguage::Dart => "dart",
            AstLanguage::Scala => "scala",
            AstLanguage::Zig => "zig",
            AstLanguage::Elixir => "elixir",
            AstLanguage::Lua => "lua",
            AstLanguage::Shell => "shell",
            AstLanguage::Gradle => "gradle",
            AstLanguage::Maven => "maven",
            AstLanguage::CargoManifest => "cargo",
            AstLanguage::NpmManifest => "npm",
            AstLanguage::Cmake => "cmake",
            AstLanguage::Protobuf => "protobuf",
            AstLanguage::Graphql => "graphql",
            AstLanguage::Sql => "sql",
            AstLanguage::Vue => "vue",
            AstLanguage::Svelte => "svelte",
            AstLanguage::Astro => "astro",
            AstLanguage::Html => "html",
            AstLanguage::Css => "css",
            AstLanguage::Prisma => "prisma",
            AstLanguage::Unsupported => "unsupported",
        }
    }
}

/// A parsed symbol extracted via AST analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: usize, // 1-indexed
    pub end_line: usize,   // 1-indexed
    pub start_byte: usize,
    pub end_byte: usize,
    pub signature: String,
    pub body_start_line: Option<usize>,
    pub body_end_line: Option<usize>,
    pub parent_symbol: Option<String>,
    pub package: Option<String>,
    pub language: String,
}

/// A function/method call detected in AST for call graph building.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstCall {
    pub caller_name: Option<String>,
    pub callee_name: String,
    pub line: usize,
    pub receiver: Option<String>,
    pub namespace: Option<String>,
}
