## Description
<!-- Provide a clear and concise summary of the changes introduced in this PR. -->

## Related Issue
<!-- Link any related issues or discussions: e.g. Fixes #123, Resolves #456 -->

## Type of Change
<!-- Please check the options that apply: -->
- [ ] 🐛 Bug fix (non-breaking change fixing an issue)
- [ ] ✨ New feature (non-breaking change adding functionality)
- [ ] ⚡ Performance optimization (latency, memory, token reduction)
- [ ] ♻️ Refactoring / Code quality (no functional change)
- [ ] 📝 Documentation update (README, docs, guides)
- [ ] 🔧 Tooling, CI/CD, or packaging update

## Architectural & Code Quality Checklist
<!-- All items must be checked before merging: -->
- [ ] **300 LOC Hard Ceiling**: All new/modified source files are strictly `< 300 LOC` (aim for `< 150 LOC` per sub-module).
- [ ] **Single Responsibility**: No compound plural junk-drawer files created (`*services.rs`, `*helpers.rs`, `*utils.rs`, etc.).
- [ ] **Stdio Protocol Purity**: No raw `println!` debug statements outputting to `stdout` in server mode (all logging routed to `stderr` via `tracing`).
- [ ] **Error Handling**: No bare `.unwrap()` or `.expect()` in production error paths; errors cleanly propagated via `Result`.
- [ ] **Tests**: Ran and verified with `cargo test --bin agent-guidance`.
- [ ] **Formatting & Linting**: Checked with `cargo fmt --check` and `cargo clippy`.
- [ ] **Documentation**: Updated README or docs under `docs/` if modifying tool surface or configuration.

## Verification & Screenshots
<!-- Describe tests run, output logs, or provide terminal output demonstrating that changes work as expected. -->
```text
cargo test --bin agent-guidance
```
