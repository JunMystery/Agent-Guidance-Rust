//! Hit ranking, role penalties (tests, utilities), and intent score adjustments.

use super::intent::{is_guidance_path, is_test_path, is_utility_path, SearchIntent};

#[derive(Debug, Clone)]
pub struct RankedResult {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub end_line: Option<usize>,
    pub score: f64,
    pub source: &'static str,
    pub snippet: Option<String>,
}

pub fn calculate_hit_score(
    path: &str,
    raw_score: f64,
    query: &str,
    intent: SearchIntent,
    ubiquitous_factor: f64,
) -> f64 {
    let q_lower = query.to_lowercase();
    let q_words: Vec<&str> = q_lower.split_whitespace().collect();

    let mut multiplier = 1.0;

    // 1. Test / Fixture penalty
    let query_explicit_test = q_words.iter().any(|&w| {
        w == "test" || w == "tests" || w == "mock" || w == "spec" || w == "fixture"
    });
    if is_test_path(path) {
        if !query_explicit_test {
            multiplier *= 0.40;
        } else {
            multiplier *= 1.20;
        }
    }

    // 2. Utility / Helpers penalty
    let query_explicit_util = q_words.iter().any(|&w| {
        w == "util" || w == "utils" || w == "helper" || w == "helpers" || w == "common"
    });
    if is_utility_path(path) && !query_explicit_util {
        multiplier *= 0.70;
    }

    // 3. Intent Gating adjustments
    let is_guidance = is_guidance_path(path);
    match intent {
        SearchIntent::Logic => {
            if is_guidance {
                multiplier *= 0.20;
            } else {
                multiplier *= 1.50;
            }
        }
        SearchIntent::Guidance => {
            if is_guidance {
                multiplier *= 2.50;
            } else {
                multiplier *= 0.35;
            }
        }
        SearchIntent::Universal => {
            // Neutral: leave as-is
        }
    }

    // 4. Ubiquitous query down-weighting factor
    multiplier *= ubiquitous_factor;

    (raw_score * multiplier).max(0.0)
}

pub fn sort_and_dedup_results(mut results: Vec<RankedResult>, limit: usize) -> Vec<RankedResult> {
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();

    for r in results {
        let key = (r.path.clone(), r.line, r.name.clone());
        if !seen.contains(&key) {
            seen.insert(key);
            deduped.push(r);
            if deduped.len() >= limit {
                break;
            }
        }
    }

    deduped
}
