# Rules Generation

> [!NOTE]
> The rules templates, manifest, and rules-generation scripts are maintained in the **upstream parent repository** and are not shipped in this standalone Rust MCP package distribution. This page documents the upstream workflow for reference.

Agent instruction files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, etc.) are generated from shared sources so the framework can update rules without editing every AI configuration file by hand.

## Upstream Source Files

- `karpathy/principles.md`: source of truth for the 6 Core Principles.
- `rules/agent-manifest.json`: list of supported agents, labels, output paths, templates, and optional frontmatter.
- `rules/templates/`: wrapper content for each instruction-file format.
- `scripts/generate-rules.py`: renders generated files and checks drift.

Do not edit generated instruction files directly when working from the upstream repository. Edit the source files above, then regenerate.

## Skill Installation

The generator does not modify or move `skills/`, `skills/*/SKILL.md`, or `SKILL-REFERENCE.md`. Generated instruction files must continue to reference `SKILL-REFERENCE.md` and `skills/` so task-specific skill loading keeps working.
