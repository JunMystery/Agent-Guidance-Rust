#[path = "../src/context/scanner.rs"]
mod scanner;
#[path = "../src/context/db.rs"]
mod db;

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
        "{name}: iterations={iterations} total_ms={} avg_us={}",
        elapsed.as_millis(),
        per_iteration.as_nanos() as f64 / 1_000.0
    );
}

fn main() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let query = "fn_test OR NOT * NEAR 'quote' database performance";

    measure("workspace_scan_depth_2", 20, || {
        black_box(scanner::scan_project(project_root, 2));
    });
    measure("fts_query_sanitization", 10_000, || {
        black_box(db::sanitize_fts5_query(query));
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
