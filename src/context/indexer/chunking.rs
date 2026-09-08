//! Dynamic semantic chunking and boundary windowing for source code.

use super::compute_hash;
use super::parsers::{CodeChunk, ExtractedSymbol};

/// Builds code chunks by aligning with AST symbol boundaries when available,
/// falling back to sliding-window chunking for unparsed content.
pub fn build_semantic_chunks(
    _rel_path: &str,
    content: &str,
    symbols: &[ExtractedSymbol],
    window_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total == 0 {
        return Vec::new();
    }

    let code_symbols: Vec<&ExtractedSymbol> = symbols
        .iter()
        .filter(|s| s.kind != "module")
        .collect();

    if code_symbols.is_empty() {
        return build_sliding_window_chunks(&lines, window_size, overlap);
    }

    let mut chunks = Vec::new();
    let mut sorted_syms = code_symbols;
    sorted_syms.sort_by_key(|s| s.start_line);

    let mut cursor = 1;

    for sym in &sorted_syms {
        if sym.start_line > total {
            continue;
        }
        let sym_start = sym.start_line.max(1);
        let sym_end = sym.end_line.min(total).max(sym_start);

        // 1. Capture gap between previous cursor and current symbol (e.g. imports, headers)
        if sym_start > cursor {
            let gap_slice = &lines[cursor - 1..sym_start - 1];
            let non_empty = gap_slice.iter().filter(|l| !l.trim().is_empty()).count();
            if non_empty >= 3 {
                let text = gap_slice.join("\n");
                chunks.push(CodeChunk {
                    start_line: cursor,
                    end_line: sym_start - 1,
                    hash: compute_hash(&text),
                    text,
                });
            }
        }

        if sym_end < cursor {
            // Already covered by previous symbol chunk
            continue;
        }

        let sym_lines = sym_end - sym_start + 1;

        // 2. Atomic symbol chunk (<= 80 lines)
        if sym_lines <= 80 {
            let chunk_slice = &lines[sym_start - 1..sym_end];
            let non_empty = chunk_slice.iter().filter(|l| !l.trim().is_empty()).count();
            if non_empty >= 1 {
                let text = chunk_slice.join("\n");
                chunks.push(CodeChunk {
                    start_line: sym_start,
                    end_line: sym_end,
                    hash: compute_hash(&text),
                    text,
                });
            }
        } else {
            // 3. Oversized symbol windowing (> 80 lines) with context header prefix
            let mut sub_start = sym_start;
            let mut is_first = true;

            while sub_start <= sym_end {
                let sub_end = (sub_start + window_size - 1).min(sym_end);
                let slice = &lines[sub_start - 1..sub_end];
                let body = slice.join("\n");

                let text = if is_first {
                    body
                } else {
                    format!("// Context: {} {} (continued from L{})\n{}", sym.kind, sym.name, sym_start, body)
                };

                chunks.push(CodeChunk {
                    start_line: sub_start,
                    end_line: sub_end,
                    hash: compute_hash(&text),
                    text,
                });

                is_first = false;
                if sub_end >= sym_end {
                    break;
                }
                let step = if window_size > overlap { window_size - overlap } else { window_size };
                sub_start += step;
            }
        }

        cursor = sym_end + 1;
    }

    // 4. Capture remaining trailing lines (after last symbol)
    if cursor <= total {
        let tail_slice = &lines[cursor - 1..total];
        let non_empty = tail_slice.iter().filter(|l| !l.trim().is_empty()).count();
        if non_empty >= 3 {
            let text = tail_slice.join("\n");
            chunks.push(CodeChunk {
                start_line: cursor,
                end_line: total,
                hash: compute_hash(&text),
                text,
            });
        }
    }

    if chunks.is_empty() {
        build_sliding_window_chunks(&lines, window_size, overlap)
    } else {
        chunks
    }
}

/// Fallback sliding-window chunker for unsupported languages and non-symbol content.
pub fn build_sliding_window_chunks(
    lines: &[&str],
    window_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let total = lines.len();
    if total == 0 {
        return Vec::new();
    }

    let step = if window_size > overlap {
        window_size - overlap
    } else {
        window_size
    };
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < total {
        let end = (start + window_size).min(total);
        let chunk_lines = &lines[start..end];
        let non_empty = chunk_lines.iter().filter(|l| !l.trim().is_empty()).count();

        if non_empty >= 3 {
            let text = chunk_lines.join("\n");
            let hash = compute_hash(&text);
            chunks.push(CodeChunk {
                start_line: start + 1,
                end_line: end,
                hash,
                text,
            });
        }

        start += step;
    }

    chunks
}
