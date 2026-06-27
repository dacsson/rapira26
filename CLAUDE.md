# rapira26

An implementation of the Soviet **Rapira** programming language, based on the specification:
> "Язык программирования Рапира" / Препринт № 767

## Project Status

- **Phase 1 — Spec implementation: ✓ complete.** A faithful implementation of the
  Препринт 767 spec, originally targeting a C code generator.
- **Phase 2 — Modernization: in progress.** The language is being extended with
  modern features (indentation-based syntax, reference counting, SMI pointer
  tagging, structs, new container syntax, …) and, most significantly, the backend
  was switched from C codegen to a **bytecode VM**. See `doc/ROADMAP.md` for the
  authoritative, up-to-date task list.

> The original C backend is **deprecated**. `compiler-core/src/codegen/cgen.rs` is
> commented out and selecting the `C` target panics. Do not extend it — all new
> work targets RBC (see below).

## Architecture

The project is a Cargo **workspace** (`members = ["crates/*"]`, plus
`runtime/raperr`). The pipeline is:

```
Source (.рап/.rap)
  → Lexer → Parser → AST            (compiler-core)
  → AST optimization passes         (compiler-core/opt, e.g. DeframePass)
  → module dependency graph         (compiler-core/module)
  → RBC bytecode → bytefile         (compiler-core/codegen/bcgen)
  → VM interpreter                  (vm-interp, linking the C runtime)
```

**RBC = Rapira Bytecode.** It is a reworked/adopted version of the
[LaMa VM bytecode](https://github.com/PLTools/Lama); see `doc/VM_ADOPTION.md` for
the rationale and translation strategy.

### Crates

| Crate | Role |
|-------|------|
| `crates/cli` | The `рапик` binary. Drives the full pipeline: parse → optimize → dependency graph → codegen → run. CLI flags are in **Russian** (`--дамп_аст`, `--дамп_код`, `--запуск`, `--бэкенд`, …). |
| `crates/compiler-core` | Frontend + backend: `lexer`, `parser`, `ast`, `module` (multi-module dependency graph), `opt` (AST passes), `codegen` (`bcgen` for RBC; `cgen` archived), `pretty` (diagnostics). |
| `crates/vm-core` | Bytecode/bytefile format: `bytecode`, `bytefile`, `decoder`, `numeric`. Adopted LaMa bytecode. |
| `crates/vm-interp` | The bytecode interpreter and `object` model. Links the C runtime (`librapruntime.a`) and generates FFI bindings via `bindgen` in `build.rs`. **Requires a nightly toolchain** (`#![feature(explicit_tail_calls, array_repeat)]`). |
| `runtime/` | The C runtime (`core.c`, `text.c`, `tuple.c`, `arithmetic.c`, `builtins.c`, `io.c`, …) built into `runtime/lib/librapruntime.a` via `runtime/Makefile`. |
| `runtime/raperr` | Rust crate for pretty runtime-error diagnostics (`annotate-snippets`). Its objects are merged into `librapruntime.a` by the runtime Makefile; the `cli` force-links it so the `runtime_error_description` C symbol is present. |

### Runtime model

- **SMI pointer tagging** (V8-style): values are tagged `usize` words. Small
  integers live inline (no heap); floats, text, tuples, callables, structs are
  heap pointers with a type tag. See `RAP_IS_SMI`/`RAP_IS_PTR` helpers.
- **Reference counting** for heap objects.
- Runtime errors are reported against the **Rapira source line**, not C.

## Build, Test, Run

The C runtime must be built **before** the Rust crates, because `vm-interp`'s
`build.rs` links `runtime/lib/librapruntime.a`:

```bash
make -C runtime          # build librapruntime.a (run after changing runtime/*.c)
cargo build              # nightly toolchain required (vm-interp uses feature gates)
cargo test               # unit tests + crates/cli e2e tests
cargo +nightly run -- <file.рап>  # compile to RBC and run on the VM
```

E2E tests (`crates/cli/tests/e2e.rs`) run example programs through the `рапик`
binary and compare stdout. Expected output is embedded in source files as
`\ => ...` comments; stdin as `\ <= ...` comments. Example programs live in
`examples/` (and `examples/bad/` for expected-failure cases).

Useful CLI flags: `--дамп_аст` (print AST), `--дамп_код` (print generated
bytecode / keep the bytefile), `--дамп_граф_модулей` (dump module graph),
`--дамп_опт_дебаг` (optimization-pass debug), `--бэкенд RBC|C` (default `RBC`).

## Code Style

- **Rust idioms**: prefer `match`, `Option`, `Result` over imperative null-checks or panics.
- **Variable names**: descriptive, self-documenting snake_case even if lengthy. Prefer `token_start_position` over `pos` or `p`.
- **Readability over cleverness**: this codebase will be revisited and extended — prioritize clarity.
- **No premature abstraction**: build for what's needed now; refactor when patterns emerge naturally.
- **Comments**: explain *why*, not *what*. Use comments for non-obvious decisions, spec references (e.g. `// Препринт §3.2`), and deviations from the spec.

## Spec Fidelity

Language semantics follow Препринт № 767. Intentional deviations (e.g. 0-based
indexing, indentation-based block syntax, the VM backend) are documented in
`doc/ROADMAP.md` and `doc/archive/PHASE1_DIFFERENCE.md`. Document any spec
ambiguity as a comment or TODO — do not silently deviate.
