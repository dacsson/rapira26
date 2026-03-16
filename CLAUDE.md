# rapira26

An implementation of the Soviet **Rapira** programming language, based on the specification:
> "Язык программирования Рапира" / Препринт № 767

## Project Roadmap

1. **Phase 1 — Spec implementation**: A complete, faithful implementation of the Rapira spec with C as the compilation backend (not a tree-walking interpreter — the pipeline ends in generated C code).
2. **Phase 2 — Modernization**: After Phase 1 is complete, revisit and extend the language with modern features. The spec will be rewritten to reflect these additions.

## Architecture

The pipeline is:
```
Source (.rap) → Lexer → Parser → AST → C Code Generator → C file
```

Keep each stage in its own module. The AST is the central data structure — design it to be extensible for Phase 2.

## Code Style

- **Rust idioms**: prefer `match`, `Option`, `Result` over imperative null-checks or panics.
- **Variable names**: descriptive, self-documenting snake_case even if lengthy. Prefer `token_start_position` over `pos` or `p`.
- **Readability over cleverness**: this codebase will be revisited and extended — prioritize clarity.
- **No premature abstraction**: build for the current spec; refactor when patterns emerge naturally.
- **Comments**: explain *why*, not *what*. Use comments for non-obvious decisions, spec references (e.g. `// Prepint §3.2`), and deviations from the spec.

## Build & Test

```bash
cargo build
cargo test
cargo run -- <source_file>
```

## Spec Fidelity

During Phase 1, all language semantics must match the Препринт № 767 spec exactly. Document any ambiguity in the spec as a comment or TODO. Do not silently deviate.
