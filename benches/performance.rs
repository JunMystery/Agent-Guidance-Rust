#![allow(warnings, unused_imports, dead_code)]

pub mod mcp {
    pub mod impact {
        use std::path::Path;
        pub fn ensure_agent_context_gitignored(_: &Path) {}
    }
}

#[path = "../src/context"]
mod context {
    pub mod hnsw;
    pub mod db;
    pub mod scanner;
}

#[path = "../src/optimizer"]
mod optimizer {
    pub mod compressor;
}

use context::db;
use context::scanner;
use optimizer::compressor;

use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

fn measure<F>(name: &str, iterations: usize, mut operation: F)
where
    F: FnMut(),
{
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(operation());
    }
    let elapsed = started.elapsed();
    let per_iteration = elapsed / iterations as u32;
    println!(
        "{name}: iterations={iterations} total_ms={} avg_us={:.3}",
        elapsed.as_millis(),
        per_iteration.as_nanos() as f64 / 1_000.0
    );
}

fn main() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let query = "fn_test OR NOT * NEAR 'quote' database performance";
    let sample_markdown = "# Heading\n\n<!-- comment -->\nSome content with ![badge](https://img.shields.io/badge/test-v1)\n\n\n\nDouble blank lines";

    measure("workspace_scan_depth_2", 20, || {
        black_box(scanner::scan_project(project_root, 2));
    });
    measure("fts_query_sanitization", 10_000, || {
        black_box(db::sanitize_fts5_query(query));
    });
    measure("compress_markdown_speed", 10_000, || {
        black_box(compressor::compress_markdown(sample_markdown));
    });
    let query_vector = vec![0.25_f32; 384];
    let passage_vector = vec![0.5_f32; 384];
    measure("normalized_vector_scoring", 100_000, || {
        black_box(
            query_vector
                .iter()
                .zip(passage_vector.iter())
                .map(|(a, b)| a * b)
                .sum::<f32>(),
        );
    });
}
