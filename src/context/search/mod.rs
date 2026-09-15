//! Search intelligence engine: Intent Gating, Dynamic IDF down-weighting, and Role Penalties.

pub mod idf;
pub mod intent;
pub mod scorer;

pub use idf::{
    analyze_query_terms, compute_query_downweight_factor, TermDocFreq,
    UBIQUITOUS_PENALTY_MULTIPLIER, UBIQUITOUS_TECH_KEYWORDS,
};
pub use intent::{
    classify_intent, is_guidance_path, is_test_path, is_utility_path, SearchIntent,
};
pub use scorer::{calculate_hit_score, sort_and_dedup_results, RankedResult};

use crate::context::db::CodeGraphDb;

#[derive(Debug, Clone)]
pub struct SearchExecutionResult {
    pub results: Vec<RankedResult>,
    pub intent: SearchIntent,
    pub source: &'static str,
    pub term_weights: Vec<TermDocFreq>,
    pub ubiquitous_factor: f64,
}

pub const CONVERSATIONAL_STOP_WORDS: &[&str] = &[
    "how", "to", "use", "what", "is", "the", "a", "an", "in", "for",
    "of", "on", "with", "do", "does", "can", "should", "guide", "docs",
    "test", "tests", "find", "search", "get", "check", "view", "run",
];

pub fn strip_conversational_words(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|w| {
            let lower = w.to_lowercase();
            let clean = lower.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            !CONVERSATIONAL_STOP_WORDS.contains(&clean)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn execute_ranked_search<F>(
    db: &CodeGraphDb,
    query: &str,
    intent_override: Option<&str>,
    limit: usize,
    embed_fn: F,
) -> SearchExecutionResult
where
    F: Fn(&str) -> Option<Vec<f32>>,
{
    let intent = match intent_override {
        Some(opt) if !opt.is_empty() && opt != "auto" => SearchIntent::from_str_opt(Some(opt)),
        _ => classify_intent(query),
    };

    let term_weights = analyze_query_terms(&db.conn, query);
    let ubiquitous_factor = compute_query_downweight_factor(&term_weights);

    // 1. Alias cache lookup (Instant, highest confidence)
    if let Ok(aliases) = db.lookup_aliases(query, limit) {
        if !aliases.is_empty() {
            let hits: Vec<RankedResult> = aliases
                .into_iter()
                .map(|a| {
                    let score = calculate_hit_score(
                        &a.resolved_path,
                        a.confidence * 2.0,
                        query,
                        intent,
                        ubiquitous_factor,
                    );
                    RankedResult {
                        path: a.resolved_path,
                        name: a.resolved_symbol.unwrap_or_else(|| "—".to_string()),
                        line: a.resolved_line.unwrap_or(1),
                        end_line: None,
                        score,
                        source: "alias_cache",
                        snippet: None,
                    }
                })
                .collect();

            let sorted = sort_and_dedup_results(hits, limit);
            if !sorted.is_empty() {
                return SearchExecutionResult {
                    results: sorted,
                    intent,
                    source: "alias_cache",
                    term_weights,
                    ubiquitous_factor,
                };
            }
        }
    }

    let mut candidate_hits = Vec::new();
    let mut primary_source = "symbol_fts";

    // 2. Symbols FTS with BM25 ranking
    let sym_results = db.search_symbols(query, limit * 2)
        .or_else(|_| Ok(Vec::new()))
        .and_then(|res| {
            if res.is_empty() {
                let stripped = strip_conversational_words(query);
                if !stripped.is_empty() && stripped != query {
                    return db.search_symbols(&stripped, limit * 2);
                }
            }
            Ok(res)
        });

    if let Ok(syms) = sym_results {
        if !syms.is_empty() {
            for (idx, (path, name, line)) in syms.into_iter().enumerate() {
                let raw_score = 1.0 / (1.0 + idx as f64 * 0.1);
                let score = calculate_hit_score(&path, raw_score, query, intent, ubiquitous_factor);
                candidate_hits.push(RankedResult {
                    path,
                    name,
                    line,
                    end_line: None,
                    score,
                    source: "symbol_fts",
                    snippet: None,
                });
            }
        }
    }

    // 3. Vector Symbol Signatures
    if candidate_hits.is_empty() {
        if let Some(qv) = embed_fn(query) {
            if let Ok(vec_syms) = db.vector_search_symbols(&qv, limit * 2, 0.60) {
                if !vec_syms.is_empty() {
                    primary_source = "symbol_vector";
                    for v in vec_syms {
                        let score = calculate_hit_score(
                            &v.file_path,
                            v.score as f64,
                            query,
                            intent,
                            ubiquitous_factor,
                        );
                        candidate_hits.push(RankedResult {
                            path: v.file_path,
                            name: v.name,
                            line: v.start_line,
                            end_line: None,
                            score,
                            source: "symbol_vector",
                            snippet: None,
                        });
                    }
                }
            }
        }
    }

    // 4. Content FTS5
    if candidate_hits.is_empty() {
        let content_res = db.search_content_fts(query, limit * 2)
            .or_else(|_| Ok(Vec::new()))
            .and_then(|res| {
                if res.is_empty() {
                    let stripped = strip_conversational_words(query);
                    if !stripped.is_empty() && stripped != query {
                        return db.search_content_fts(&stripped, limit * 2);
                    }
                }
                Ok(res)
            });

        if let Ok(content_hits) = content_res {
            if !content_hits.is_empty() {
                primary_source = "content_fts";
                for (idx, (path, start, end, snip)) in content_hits.into_iter().enumerate() {
                    let raw_score = 0.9 / (1.0 + idx as f64 * 0.1);
                    let score = calculate_hit_score(&path, raw_score, query, intent, ubiquitous_factor);
                    candidate_hits.push(RankedResult {
                        path,
                        name: format!("L{}-{}", start, end),
                        line: start,
                        end_line: Some(end),
                        score,
                        source: "content_fts",
                        snippet: Some(snip),
                    });
                }
            }
        }
    }

    // 5. RAG Vector Content Chunks
    if candidate_hits.is_empty() {
        if let Some(qv) = embed_fn(query) {
            if let Ok(chunks) = db.vector_search_chunks(&qv, limit * 2, 0.55) {
                if !chunks.is_empty() {
                    primary_source = "content_vector";
                    for c in chunks {
                        let score = calculate_hit_score(
                            &c.file_path,
                            c.score as f64,
                            query,
                            intent,
                            ubiquitous_factor,
                        );
                        candidate_hits.push(RankedResult {
                            path: c.file_path,
                            name: format!("L{}-{}", c.start_line, c.end_line),
                            line: c.start_line,
                            end_line: Some(c.end_line),
                            score,
                            source: "content_vector",
                            snippet: None,
                        });
                    }
                }
            }
        }
    }

    let sorted = sort_and_dedup_results(candidate_hits, limit);

    SearchExecutionResult {
        results: sorted,
        intent,
        source: primary_source,
        term_weights,
        ubiquitous_factor,
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
