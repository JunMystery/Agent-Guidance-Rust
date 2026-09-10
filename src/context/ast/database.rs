//! Database Schema & IDL AST Extractor for SQL, Prisma, and GraphQL.

use super::types::{AstCall, AstLanguage, AstSymbol};
use regex::Regex;
use std::sync::LazyLock;

// SQL Regexes
static SQL_TABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)create\s+(?:table|view|index|procedure|function|trigger)\s+(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_"]+)"#).unwrap()
});

static SQL_REFERENCES_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)references\s+([a-zA-Z0-9_"]+)\s*\("#).unwrap()
});

static SQL_JOIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:join|from)\s+([a-zA-Z0-9_"]+)"#).unwrap()
});

// Prisma Regexes
static PRISMA_MODEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(model|enum|datasource|generator)\s+([A-Za-z0-9_]+)\s*\{"#).unwrap()
});

static PRISMA_FIELD_TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*[a-zA-Z0-9_]+\s+([A-Z][A-Za-z0-9_]+)(?:\[\])?\s*"#).unwrap()
});

// GraphQL Regexes
static GQL_TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(type|interface|enum|union|input)\s+([A-Za-z0-9_]+)"#).unwrap()
});

static GQL_FIELD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#":\s*\[?([A-Z][A-Za-z0-9_]+)!?\]?!"#).unwrap()
});

/// Extracts database schema symbols (tables, views, models, GraphQL types).
pub fn extract_symbols(content: &str, lang: AstLanguage) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    match lang {
        AstLanguage::Sql => {
            for (idx, line) in lines.iter().enumerate() {
                let line_num = idx + 1;
                if let Some(cap) = SQL_TABLE_RE.captures(line) {
                    if let Some(m) = cap.get(1) {
                        let name = clean_ident(m.as_str());
                        let kind = if line.to_ascii_lowercase().contains("view") {
                            "view"
                        } else if line.to_ascii_lowercase().contains("index") {
                            "index"
                        } else if line.to_ascii_lowercase().contains("procedure") || line.to_ascii_lowercase().contains("function") {
                            "function"
                        } else if line.to_ascii_lowercase().contains("trigger") {
                            "trigger"
                        } else {
                            "table"
                        };
                        symbols.push(build_sym(&name, kind, line_num, &lines, lang));
                    }
                }
            }
        }
        AstLanguage::Prisma => {
            for cap in PRISMA_MODEL_RE.captures_iter(content) {
                if let (Some(kind_m), Some(name_m)) = (cap.get(1), cap.get(2)) {
                    let line = line_number(content, name_m.start());
                    symbols.push(build_sym(name_m.as_str(), kind_m.as_str(), line, &lines, lang));
                }
            }
        }
        AstLanguage::Graphql => {
            for cap in GQL_TYPE_RE.captures_iter(content) {
                if let (Some(kind_m), Some(name_m)) = (cap.get(1), cap.get(2)) {
                    let line = line_number(content, name_m.start());
                    symbols.push(build_sym(name_m.as_str(), kind_m.as_str(), line, &lines, lang));
                }
            }
        }
        _ => {}
    }

    symbols
}

/// Extracts cross-table relationships, Prisma model relations, and GraphQL field types.
pub fn extract_calls(content: &str, lang: AstLanguage) -> Vec<AstCall> {
    let mut calls = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    match lang {
        AstLanguage::Sql => {
            for (idx, line) in lines.iter().enumerate() {
                let line_num = idx + 1;
                for cap in SQL_REFERENCES_RE.captures_iter(line) {
                    if let Some(m) = cap.get(1) {
                        calls.push(AstCall {
                            caller_name: None,
                            callee_name: clean_ident(m.as_str()),
                            line: line_num,
                            receiver: None,
                            namespace: None,
                        });
                    }
                }
            }
        }
        AstLanguage::Prisma => {
            for (idx, line) in lines.iter().enumerate() {
                let line_num = idx + 1;
                let trimmed = line.trim();
                if let Some(cap) = PRISMA_FIELD_TYPE_RE.captures(trimmed) {
                    if let Some(m) = cap.get(1) {
                        let ty = m.as_str();
                        if !is_prisma_scalar(ty) {
                            calls.push(AstCall {
                                caller_name: None,
                                callee_name: ty.to_string(),
                                line: line_num,
                                receiver: None,
                                namespace: None,
                            });
                        }
                    }
                }
            }
        }
        AstLanguage::Graphql => {
            for (idx, line) in lines.iter().enumerate() {
                let line_num = idx + 1;
                for cap in GQL_FIELD_RE.captures_iter(line) {
                    if let Some(m) = cap.get(1) {
                        let ty = m.as_str();
                        if !is_graphql_scalar(ty) {
                            calls.push(AstCall {
                                caller_name: None,
                                callee_name: ty.to_string(),
                                line: line_num,
                                receiver: None,
                                namespace: None,
                            });
                        }
                    }
                }
            }
        }
        _ => {}
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

fn clean_ident(s: &str) -> String {
    s.trim_matches('"').trim_matches('`').trim().to_string()
}

fn line_number(content: &str, byte_idx: usize) -> usize {
    content[..byte_idx].chars().filter(|&c| c == '\n').count() + 1
}

fn is_prisma_scalar(name: &str) -> bool {
    matches!(
        name,
        "String" | "Boolean" | "Int" | "BigInt" | "Float" | "Decimal" | "DateTime" | "Json" | "Bytes"
    )
}

fn is_graphql_scalar(name: &str) -> bool {
    matches!(name, "String" | "Int" | "Float" | "Boolean" | "ID")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_tables_and_references() {
        let sql = r#"
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL
);

CREATE TABLE posts (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    title TEXT NOT NULL
);
"#;
        let syms = extract_symbols(sql, AstLanguage::Sql);
        assert!(syms.iter().any(|s| s.name == "users" && s.kind == "table"));
        assert!(syms.iter().any(|s| s.name == "posts" && s.kind == "table"));

        let calls = extract_calls(sql, AstLanguage::Sql);
        assert!(calls.iter().any(|c| c.callee_name == "users"));
    }

    #[test]
    fn test_prisma_and_graphql_extraction() {
        let prisma = r#"
datasource db {
  provider = "postgresql"
}

model User {
  id    Int     @id @default(autoincrement())
  posts Post[]
}

model Post {
  id       Int  @id
  authorId Int
  author   User @relation(fields: [authorId], references: [id])
}
"#;
        let p_syms = extract_symbols(prisma, AstLanguage::Prisma);
        assert!(p_syms.iter().any(|s| s.name == "User" && s.kind == "model"));
        assert!(p_syms.iter().any(|s| s.name == "Post" && s.kind == "model"));

        let p_calls = extract_calls(prisma, AstLanguage::Prisma);
        assert!(p_calls.iter().any(|c| c.callee_name == "Post"));
        assert!(p_calls.iter().any(|c| c.callee_name == "User"));

        let gql = r#"
type User {
  id: ID!
  posts: [Post!]!
}

type Post {
  id: ID!
  title: String!
}
"#;
        let g_syms = extract_symbols(gql, AstLanguage::Graphql);
        assert!(g_syms.iter().any(|s| s.name == "User" && s.kind == "type"));
        assert!(g_syms.iter().any(|s| s.name == "Post" && s.kind == "type"));

        let g_calls = extract_calls(gql, AstLanguage::Graphql);
        assert!(g_calls.iter().any(|c| c.callee_name == "Post"));
    }
}
