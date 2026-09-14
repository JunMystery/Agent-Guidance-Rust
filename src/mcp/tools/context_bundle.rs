use serde_json::Value;
use std::path::Path;

use crate::context::graph_rag::build_subgraph_bundle;

pub(crate) fn handle_subgraph_bundle(
    arguments: &Value,
    proj_path: &Path,
    target: &str,
) -> String {
    let clean_target = target.trim();
    if clean_target.is_empty() {
        return "Error: target_symbol or query parameter is required for subgraph_bundle operation. Example: project_context(operation=\"subgraph_bundle\", target_symbol=\"my_function\")".to_string();
    }

    let budget = arguments
        .get("loc_budget")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(250);

    let bundle = match build_subgraph_bundle(proj_path, clean_target, budget) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return format!(
                "# Multi-File Subgraph Bundle: `{}`\n\nNo symbol named `{}` found in project code graph. Run `project_context(operation=\"search\", query=\"{}\")` to discover symbols.",
                clean_target, clean_target, clean_target
            );
        }
        Err(e) => {
            return format!("Failed to generate subgraph bundle for '{}': {}", clean_target, e);
        }
    };

    let mut out = Vec::new();
    out.push(format!(
        "# 📦 Multi-File Subgraph Bundle: `{}`\n- **Symbol Kind**: `{}` | **File**: `{}:L{}-L{}`\n- **Blast Radius**: **{}** (Score: {:.1}/10) | **Packed LOC**: {}/{} lines",
        bundle.target.name,
        bundle.target.kind,
        bundle.target.file_path,
        bundle.target.start_line,
        bundle.target.end_line,
        bundle.risk_level,
        bundle.blast_radius_score,
        bundle.total_loc,
        bundle.loc_budget
    ));

    // 1. Target Implementation
    out.push("\n## 🎯 Target Implementation".to_string());
    if let Some(ref snip) = bundle.target_snippet {
        out.push(format!(
            "```\n// [{}:L{}-L{}]\n{}\n```",
            snip.file_path,
            snip.start_line,
            snip.end_line,
            snip.format_with_line_numbers()
        ));
    } else {
        out.push(format!(
            "*(Target implementation body could not be extracted from `{}`)*",
            bundle.target.file_path
        ));
    }

    // 2. Immediate Inbound Callers
    out.push(format!(
        "\n## ⬆️ Immediate Callers (1-Hop Inbound: {})",
        bundle.callers.len()
    ));
    if bundle.callers.is_empty() {
        out.push("*(No direct incoming callers found in current AST graph — isolated entrypoint/leaf)*".to_string());
    } else {
        for (i, c) in bundle.callers.iter().enumerate() {
            out.push(format!(
                "### {}. `{}` in `{}:L{}` [{}, weight: {:.1}]",
                i + 1,
                c.symbol_name,
                c.file_path,
                c.call_line,
                c.edge_type,
                c.weight
            ));
            if let Some(ref snip) = c.snippet {
                out.push(format!("```\n{}\n```", snip.format_with_line_numbers()));
            } else {
                out.push("*(Call-site snippet omitted to conserve LOC budget)*".to_string());
            }
        }
    }

    // 3. Immediate Outbound Dependencies
    out.push(format!(
        "\n## ⬇️ Immediate Dependencies (1-Hop Outbound: {})",
        bundle.callees.len()
    ));
    if bundle.callees.is_empty() {
        out.push("*(No outgoing dependencies recorded for this symbol)*".to_string());
    } else {
        for (i, c) in bundle.callees.iter().enumerate() {
            out.push(format!(
                "### {}. `{}` in `{}:L{}` [{}, weight: {:.1}]",
                i + 1,
                c.symbol_name,
                c.file_path,
                c.start_line,
                c.edge_type,
                c.weight
            ));
            if let Some(ref snip) = c.snippet {
                out.push(format!("```\n{}\n```", snip.format_with_line_numbers()));
            } else {
                out.push("*(Callee definition snippet omitted to conserve LOC budget)*".to_string());
            }
        }
    }

    out.join("\n")
}
