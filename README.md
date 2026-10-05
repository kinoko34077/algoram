# Algoram

Algorithm × Program.

A hierarchical meta-development platform for turning existing computational and service capabilities into composable, reusable, swappable blocks.

## Phase 1 prototype

The current implementation starts with the language-neutral Rust Graph Core defined by Issue #4.

```text
Reference/source structure
→ Algoram Graph
→ later importers / renderer / route planner / execution plan
```

Phase 1-A (#5) owns only the Graph Core and versioned JSON persistence. Importers, UI, interoperability execution and runtime planning remain separate follow-up units.

### Verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

See repository Issues for the conception record (#1), integrated concept proposal (#2), roadmap (#3), and v0.1 technical specification (#4).
