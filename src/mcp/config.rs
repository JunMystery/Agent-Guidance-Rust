use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

const SERVER_ID: &str = "agent-guidance";
const OLD_SERVER_ID: &str = "agent-guidance-mcp";

const AGENT_GUIDANCE_TAG_START: &str = "<!-- agent-guidance:start -->";
const AGENT_GUIDANCE_TAG_END: &str = "<!-- agent-guidance:end -->";
const AGENT_GUIDANCE_SKILL_TAG_START: &str = "<!-- agent-guidance-skill:start -->";
const AGENT_GUIDANCE_SKILL_TAG_END: &str = "<!-- agent-guidance-skill:end -->";

const AGENT_RULES_BLOCK: &str = r#"
<!-- agent-guidance:start -->
## Agent Guidance MCP — Tool Selection Priority

| You need to... | Use THIS tool first | Why |
|---|---|---|
| Start any task or phase | `task_pipeline(task="...")` | Recommendations + tree + code search + UI in ONE call |
| Check coding standards / skills | `guidance(operation="search", query="...")` | No other tool provides standards or skill lookup |
| Read a file | `project_context(operation="read", relative_path="...")` | Token-capped at 300 lines — prevents context blowout |
| Search codebase text | `project_context(operation="search", query="...")` | Ranked, bounded results. Fallback when codegraph unavailable |
| Understand code structure | `project_context(operation="structure", relative_path="...")` | Hierarchical view of classes, methods, functions in a file |
| Extract symbols | `project_context(operation="symbols", relative_path="...")` | Flat list of classes, functions, methods with signatures |
| Find symbol references | `project_context(operation="references", query="...")` | Locate all usages of a symbol across the codebase |
| Get UI/design guidance | `ui_ux(operation="search", query="...")` | Style, colors, typography, charts, slides |
| Persist/recover session | `session_continuity(operation="save"/"load"/"clear")` | State recovery / task checklist continuity |
| Browse project tree | `project_context(operation="tree")` | Optimized directory tree view |

### Nine Mandatory Rules

1. **Context First**: Call `task_pipeline` or `project_context` BEFORE any file read or code change.
2. **Standards Check**: Use `guidance(operation="search")` BEFORE implementing or answering any prompt.
3. **Token Budget**: Prefer MCP tools over raw file reads — built-in limits prevent context blowout.
4. **No Direct FS**: Never manually read/search files when MCP tools do it with optimization.
5. **Ground & Plan**: Verify files/functions/symbols via search BEFORE proposing changes. Never guess.
6. **300 LOC Cap**: Split files exceeding 300 lines of code. No monolithic files.
7. **Intent Gate**: Classify request type (trivial/explicit/exploratory/open-ended/ambiguous) before acting. If ambiguous, clarify first.
8. **Delegation Before Action**: Decompose multi-step tasks and delegate to specialized subagents. Never implement directly when delegation is possible.
9. **Per-Phase Reset**: For EACH new work phase (plan → implement → test → debug → review → refactor), re-call `task_pipeline` with that phase's goal. Do NOT carry old context across phases. A new phase is a new task.

**CRITICAL: All 9 rules apply to EVERY action without exception — planning, implementation, testing, debugging, reviewing, refactoring, or any other work. There is no action type exempt from these rules.**
<!-- agent-guidance:end -->
"#;

const ENFORCER_SKILL_CONTENT: &str = r#"<!-- agent-guidance-skill:start -->
---
name: agent-guidance
description: Core system standards check and token-optimized codebase context retrieval. Run this skill before performing any tool execution or codebase changes.
---

## When to use me
- Run this skill at the beginning of EVERY task, repository lookup, or codebase refactoring.
- Run this skill to check project conventions and avoid raw file reading/search operations.
- Re-run this skill at EACH phase transition (plan → implement → test → review).

## How to use me
You must invoke the `agent-guidance` tools in this priority order:
1. Call `task_pipeline(task="...")` at the start of any task or phase to retrieve workspace context, tree, and recommendations.
2. Call `guidance(operation="search", query="...")` before implementing coding standards.
3. Call `project_context(operation="read", relative_path="...")` instead of standard file reads (capped at 300 lines).
4. Call `project_context(operation="search", query="...")` instead of standard file search.

## Critical Behavioral Rules
- When unsure about anything, ASK! DO NOT GUESS.
- Propose an implementation plan before making any big or complex changes.
- For each new work phase, re-call `task_pipeline` with the phase goal. Do not carry old context.
<!-- agent-guidance-skill:end -->
"#;

pub fn run_setup(binary_path: &Path) -> Result<()> {
    info!("Configuring MCP clients with binary at {:?}", binary_path);
    let bin_str = binary_path.to_string_lossy().to_string();

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home dir"))?;

    let (claude_path, code_path, cursor_path) = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let app_path = PathBuf::from(appdata);
        (
            app_path.join("Claude").join("claude_desktop_config.json"),
            app_path.join("Code").join("User").join("globalStorage"),
            app_path.join("Cursor").join("User").join("globalStorage"),
        )
    } else if cfg!(target_os = "macos") {
        (
            home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json"),
            home.join("Library").join("Application Support").join("Code").join("User").join("globalStorage"),
            home.join("Library").join("Application Support").join("Cursor").join("User").join("globalStorage"),
        )
    } else {
        (
            home.join(".config").join("Claude").join("claude_desktop_config.json"),
            home.join(".config").join("Code").join("User").join("globalStorage"),
            home.join(".config").join("Cursor").join("User").join("globalStorage"),
        )
    };

    let targets = vec![
        ("Claude Desktop", claude_path, true, "mcpServers"),
        ("Gemini MCP config", home.join(".gemini").join("config").join("mcp_config.json"), true, "mcpServers"),
        ("Antigravity MCP config", home.join(".gemini").join("antigravity").join("mcp_config.json"), true, "mcpServers"),
        ("Cursor Native", home.join(".cursor").join("mcp.json"), true, "mcpServers"),
        ("VS Code Native", code_path.parent().unwrap_or(&code_path).join("mcp.json"), true, "servers"),
        ("Continue.dev", home.join(".continue").join("mcpServers").join("config.json"), true, "mcpServers"),
    ];

    for (name, path, force, key) in targets {
        if force || path.parent().map(|p| p.exists()).unwrap_or(false) {
            merge_mcp_config(&path, SERVER_ID, &bin_str, key)?;
            info!("Successfully configured {}", name);
        }
    }

    // Configure IDE Extensions (Cline / Roo-Code)
    let extensions = vec![
        ("VS Code Cline", code_path.join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")),
        ("VS Code Roo-Code", code_path.join("roovet.roo-cline").join("settings").join("cline_mcp_settings.json")),
        ("Cursor Cline", cursor_path.join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")),
        ("Cursor Roo-Code", cursor_path.join("roovet.roo-cline").join("settings").join("cline_mcp_settings.json")),
    ];

    for (name, path) in extensions {
        if path.parent().and_then(|p| p.parent()).map(|p| p.exists()).unwrap_or(false) {
            merge_mcp_config(&path, SERVER_ID, &bin_str, "mcpServers")?;
            info!("Successfully configured {}", name);
        }
    }

    // Configure OpenCode / OMO
    let opencode_path = if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("opencode").join("opencode.json")
    } else {
        home.join(".config").join("opencode").join("opencode.json")
    };
    configure_opencode(&opencode_path, &bin_str)?;

    // Configure Global Rules & Native Agent Skills
    configure_global_rules(&home)?;
    configure_skills_enforcer(&home)?;

    Ok(())
}

fn merge_mcp_config(config_path: &Path, server_id: &str, bin_path: &str, key: &str) -> Result<()> {
    let mut root: Value = if config_path.exists() {
        let content = fs::read_to_string(config_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    let obj = root.as_object_mut().unwrap();
    if !obj.contains_key(key) || !obj[key].is_object() {
        obj.insert(key.to_string(), json!({}));
    }

    let servers_map = obj.get_mut(key).unwrap().as_object_mut().unwrap();
    // Remove old legacy Python registration if present
    servers_map.remove(OLD_SERVER_ID);

    servers_map.insert(
        server_id.to_string(),
        json!({
            "command": bin_path,
            "args": []
        }),
    );

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(config_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

fn configure_opencode(opencode_path: &Path, bin_path: &str) -> Result<()> {
    let mut root: Value = if opencode_path.exists() {
        let content = fs::read_to_string(opencode_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    let obj = root.as_object_mut().unwrap();
    if !obj.contains_key("mcp") || !obj["mcp"].is_object() {
        obj.insert("mcp".to_string(), json!({}));
    }

    let mcp_map = obj.get_mut("mcp").unwrap().as_object_mut().unwrap();
    mcp_map.remove(OLD_SERVER_ID);
    mcp_map.insert(
        SERVER_ID.to_string(),
        json!({
            "type": "local",
            "command": [bin_path],
            "enabled": true,
            "environment": {}
        }),
    );

    let instructions = obj.entry("instructions").or_insert_with(|| json!([]));
    if let Some(arr) = instructions.as_array_mut() {
        if !arr.contains(&json!("AGENTS.md")) {
            arr.push(json!("AGENTS.md"));
        }
    }

    if let Some(parent) = opencode_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(opencode_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

fn configure_global_rules(home: &Path) -> Result<()> {
    let targets = vec![
        ("Gemini/Antigravity", home.join(".gemini").join("config").join("AGENTS.md")),
        ("OpenCode", home.join(".config").join("opencode").join("AGENTS.md")),
        ("Claude Code Compatibility", home.join(".claude").join("CLAUDE.md")),
    ];

    for (_name, path) in targets {
        let mut content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        content = replace_or_append_tagged_section(&content, AGENT_GUIDANCE_TAG_START, AGENT_GUIDANCE_TAG_END, AGENT_RULES_BLOCK.trim());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
    }

    Ok(())
}

fn configure_skills_enforcer(home: &Path) -> Result<()> {
    let global_targets = vec![
        ("Claude Code Global", home.join(".claude").join("skills").join("agent-guidance").join("SKILL.md")),
        ("OpenCode Global", home.join(".config").join("opencode").join("skills").join("agent-guidance").join("SKILL.md")),
        ("Cline/Roo-Code Global", home.join(".agents").join("skills").join("agent-guidance").join("SKILL.md")),
    ];

    for (_name, path) in global_targets {
        let mut content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        content = replace_or_append_tagged_section(&content, AGENT_GUIDANCE_SKILL_TAG_START, AGENT_GUIDANCE_SKILL_TAG_END, ENFORCER_SKILL_CONTENT.trim());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
    }

    Ok(())
}

fn replace_or_append_tagged_section(content: &str, start_tag: &str, end_tag: &str, new_section: &str) -> String {
    if let (Some(start_idx), Some(end_idx)) = (content.find(start_tag), content.find(end_tag)) {
        let before = content[..start_idx].trim_end();
        let after = content[end_idx + end_tag.len()..].trim_start();
        format!("{}\n{}\n{}", before, new_section.trim(), after)
    } else {
        if content.trim().is_empty() {
            new_section.trim().to_string()
        } else {
            format!("{}\n\n{}", content.trim(), new_section.trim())
        }
    }
}
