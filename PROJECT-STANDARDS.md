# Project-Specific Agent Guidance

This file contains standards, conventions, and rules specific to this project. AI Agents MUST follow the 8 core standards for every single action, in parallel with the rules herein.

> **💡 Instructions:**
> - **Independent operation:** Simply create/edit this file, and the AI Agent will automatically detect it.
> - **Fully customizable:** You can add new fields (e.g., "Reviewer", "Effective Date") if necessary. Just keep it in a bulleted list format.
> - Use this file to define: project architecture, preferred libraries, naming conventions, error handling flows, etc.

---

## 🏗️ Template

*(Copy the block below to add a new standard. You can add/remove any fields according to your needs)*

### [Standard Name / Category]
- **Rule (Required):** [Clear description of the rule the AI must follow]
- **Reason (Required):** [Brief explanation of why this rule exists]
- **Do / Good Example (Optional):** [Example of correct code/behavior]
- **Don't / Bad Example (Optional):** [Example of incorrect code/behavior]
- **Scope (Optional):** [e.g.: Only applies to Frontend code, or only .ts files]
- **Reference (Optional):** [Link or path to documentation, design file, issue...]
- **Exceptions (Optional):** [Cases where this rule does not apply]
- **Terminal Command (Optional):** [Commands to run, e.g.: npm run lint]
- **How to Test (Optional):** [How to know this rule has been followed?]

---

## 📋 Project Standards List (Customize below)

> [!IMPORTANT]
> **GLOBAL ENFORCEMENT**: All rules herein MUST be evaluated and followed for every single coding action, repository lookup, refactoring, or planning phase without exception.

### ♻️ Reusable & Unified Shared Code (DRY Mandate)
- **Rule (Required):** Always check for existing shared utilities, formatters, models, and UI helpers in common/shared directories before writing new code. Never create duplicate implementations of logic that already exists in the project.
- **Reason (Required):** Prevents code fragmentation, eliminates duplicate bugs, reduces bundle size, and maintains a Single Source of Truth (SSOT).
- **Do / Good Example (Optional):** Search via `project_context(operation="search", query="...")` or `project_context(operation="graph_rag")` to find existing utility functions, import them, and use them directly.
- **Don't / Bad Example (Optional):** Writing a custom date formatter, string slugifier, or validation regex inside a feature controller when `shared/utils` already has one.
- **Scope (Optional):** All frontend and backend code across the repository.
- **How to Test (Optional):** Run symbol/AST duplicate check and codebase linting.

### 📏 300 LOC Hard Cap & Refactoring Exemption
- **Rule (Required):** All source code files must remain strictly < 300 LOC (target < 150 LOC per sub-module). If a file exceeds 300 LOC, edits are blocked with error code `300_LOC_CAP_EXCEEDED` unless justification explicitly targets modular decomposition.
- **Reason (Required):** Keeps cognitive complexity low, enhances testability, and prevents monolith accumulation.
- **Bypass Keywords:** Justification must contain one of: `refactor`, `refactoring`, `decompose`, `decomposing`, `extract`, `extracting`, `split`, `splitting`, `tách`, `tách file`.
- **Exemptions:** Markdown (`.md`), documentation, configs (`.toml`, `.json`, `.yaml`, `.yml`), lockfiles (`Cargo.lock`, `package-lock.json`), static assets (`.svg`, `.png`, `.ico`), and test fixtures.
- **How to Test (Optional):** Evaluated automatically via `workflow_gate(action="authorize_edit")`.

### 🧱 Modularity Gate & Single-Responsibility Naming
- **Rule (Required):** New source files must adhere to Single Responsibility Principle (SRP). Compound plural naming and multi-component justifications are strictly prohibited.
- **Compound Plural Prohibition (`COMPOUND_FILE_NAME_PROHIBITED`):** Disallowed compound plural suffix patterns (40 suffixes):
  `modals`, `dialogs`, `drawers`, `forms`, `tables`, `cards`, `panels`, `widgets`, `components`, `views`, `screens`, `pages`, `services`, `handlers`, `controllers`, `managers`, `repositories`, `endpoints`, `routes`, `actions`, `mutations`, `queries`, `reducers`, `helpers`, `utils`, `utilities`, `models`, `entities`, `adapters`, `transformers`, `listeners`, `providers`, `subscribers`, `factories`, `builders`, `validators`, `converters`, `processors`, `resolvers`, `types`.
  Example prohibited: `user_services.rs`, `order_controllers.rs`. Permitted: `user_service.rs` or focused directory `user/service.rs`.
- **Multi-Component Prohibition (`MULTI_COMPONENT_NEW_FILE_PROHIBITED`):** File creation justifications describing multiple disparate responsibilities or containing connector conjunctions (`and`, `both`, `multiple`) will be rejected. Split into discrete sub-modules.
- **How to Test (Optional):** Enforced automatically during `workflow_gate(action="authorize_edit")`.

### ✅ Multi-Lingual & GUI User Approval Detection
- **Rule (Required):** Moving from `Plan` to `Build` stage requires user approval. Approval is detected automatically via chat intent analysis or GUI interaction.
- **Supported Triggers:**
  - English keywords: `approve`, `approved`, `proceed`, `accept`, `accepted`, `looks good`, `lgtm`, `go ahead`.
  - Vietnamese keywords: `đồng ý`, `chấp nhận`, `tiến hành`, `duyệt`.
  - Antigravity / Gemini GUI Artifact: Clicking "Proceed" or "Accept Plan" button on `implementation_plan.md`.
- **Reason (Required):** Guarantees human-in-the-loop governance before destructive or major architectural changes occur.
- **How to Test (Optional):** Call `workflow_gate(action="set_stage", target_stage="Build")`. Reject if approval missing (`STAGE_TRANSITION_BLOCKED`).

### 🎯 Strict Gate on Skill Selection (Option B)
- **Rule (Required):** When skills are proposed or queried via `guidance(operation="search")`, the agent MUST NEVER auto-select skills on behalf of the user. The agent must trigger the IDE tool `ask_question(questions=[{question: "...", options: [...], is_multi_select: true}])` to present the proposed skill options to the user (formatted as `"{name} - {short_desc}"` with short `{short_desc}` <= 38 chars, `is_multi_select: true`). Only after user confirmation can `select_skills(skills=[...], user_confirmed=true)` (or `autonomous=true` for headless subagents) be called.
- **Reason (Required):** Prevents unwanted skill injection, enforces user agency in tailoring specialized workflow guidelines, and avoids token bloat from unneeded skill sets.
- **How to Test (Optional):** Calling `select_skills` with skills but without `user_confirmed=true` returns `USER_CONFIRMATION_REQUIRED` error.
