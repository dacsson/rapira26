# Rapira26 contributor guide

## Project at a glance

Rapira26 is a modern implementation of the Soviet Rapira language. The active
compiler target is **RBC (Rapira Bytecode)**, executed by the in-process VM.
The former C code generator is retired: `--бэкенд C` deliberately panics. Do
not add features to a C backend.

The repository is a Rust workspace on the **nightly** toolchain (pinned by
`rust-toolchain.toml`) plus a small C runtime. The main executable is
`рапик`.

The current development focus is the VM backend and language modernization.
Recent work added user-defined algebraic types and variants; their VM design is
documented in `doc/USER_TYPES_IN_VM.md`. The broad, partly historical roadmap
is in `doc/ROADMAP.md`, so confirm an item against the code and tests before
treating it as current.

## Architecture

```text
.rap / .рап source
  -> lexer, parser, AST and module graph       crates/compiler-core
  -> RBC bytefile generation                  crates/compiler-core/src/codegen/bcgen.rs
  -> bytecode/bytefile definitions            crates/vm-core
  -> interpreter and C-runtime FFI            crates/vm-interp
```

Key locations:

- `crates/cli` — the `рапик` command, module resolution, and end-to-end tests.
- `crates/compiler-core` — lexer, parser, AST, diagnostics, module graph,
  optional AST passes, and RBC lowering.
- `crates/vm-core` — instruction set, bytefile format, decoder, numeric helpers.
- `crates/vm-interp` — stack interpreter, frames, object bindings, verifier.
- `runtime` — C value/object implementation, reference counting, arithmetic,
  I/O, tuples, and variants. `make -C runtime` builds
  `runtime/lib/librapruntime.a`.
- `runtime/raperr` — Rust runtime-error formatter. It is a standalone crate,
  force-linked by `crates/cli`; it is not a workspace member.
- `std` — standard-library Rapira modules.
- `examples` — executable language samples; `examples/bad` contains invalid
  programs.
- `editors/vscode-rapira` — separately packaged VS Code language extension.

## C runtime and VM boundary

`runtime/rapvalue.h` defines the ABI value type: `RAP_Value` is a tagged
`uintptr_t`. Its two low bits distinguish immediate 32-bit SMIs (`00`),
immediate booleans (`01`), and heap-object pointers (`11`). Do not reinterpret
values manually; use `RAP_IS_*`, `RAP_*_VALUE`, and `RAP_CREATE_*` helpers so
the C runtime and Rust `Object` wrapper stay in sync.

Heap objects (`RAP_Object`) have a tag and a reference count. Null, floats,
text, tuples, slices, callables, and user-defined variants are heap allocated.
The runtime owns recursive destruction in `RAP_free_object`; cycles are not
collected, so introducing a new reference edge requires an explicit ownership
decision.

Ownership rules for runtime and interpreter changes:

- `RAP_inc_ref` and `RAP_dec_ref` are no-ops for immediate values. Call them
  only when an owning reference is copied or consumed; `vm-interp` uses
  `inc_ref_if_ptr` and `dec_ref_if_ptr` for this purpose.
- Constructors return an owned reference. Tuple constructors retain their
  elements, while `RAP_create_custom_typed_obj` transfers ownership of the
  supplied variant fields. Tuple/variant setters retain the replacement and
  release the previous field. Getters that expose an existing heap element must
  be checked for their exact ownership contract before pushing the result onto
  the VM stack.
- Slices retain their root parent and flatten nested slices. Their destructor
  releases that parent; do not independently free it through a slice.
- Variants store a `u16` schema tag followed by `RAP_Value` fields. The bytefile
  schema supplies type and field-name storage, which the interpreter keeps
  alive for its lifetime. Update both schema handling and runtime destruction
  when changing this representation.

`vm-interp/build.rs` uses bindgen on `runtime/runtime.h` and links
`runtime/lib/librapruntime.a`. After changing public C declarations or runtime
headers, rebuild the archive and Rust crates. Runtime failures call
`RAP_fatal_error`, which prints an error and exits the process; do not expect a
recoverable Rust error from such an FFI call.

## Build and verification

Build the C archive before building a fresh Rust target or after changing
`runtime/*.c` or its headers:

```bash
make -C runtime
cargo build
cargo test
```

Run a program through the VM:

```bash
cargo run --bin рапик -- --запуск examples/01_output_and_literals.rap
```

Useful focused checks:

```bash
cargo test -p compiler-core
cargo test -p vm-core
cargo test -p vm-interp
cargo test -p cli
```

The CLI integration tests are in `crates/cli/tests/e2e.rs`. They execute the
programs in `examples`; expected stdout and stdin are encoded in `\\ =>` and
`\\ <=` comments inside those files. Test module resolution with `RAPIRA_STD`
or the CLI's `--путь-стд`; additional module roots use `RAPIRA_PATH` or
`--путь-модулей`.

## Implementation conventions

- Preserve the bytecode pipeline and make instruction-set changes consistently
  across `vm-core`, bytefile encoding/decoding, `vm-interp`, and `bcgen`.
- Runtime values are tagged machine words: small integers are immediate SMIs;
  other values are reference-counted heap objects. Account for ownership when
  adding interpreter instructions or runtime helpers.
- Keep source-facing diagnostics in Rapira terms and tied to source spans.
- The language uses Russian keywords and supports `.rap` and `.рап` files.
  Keep CLI flags and user-facing diagnostics Russian unless the surrounding
  interface establishes another convention.
- Prefer clear idiomatic Rust (`Result`, `Option`, exhaustive `match`) and
  comments that explain non-obvious invariants or language/spec decisions.
- Generated and profiling artefacts (for example `*.rbc`, `perf.data`,
  `flamegraph.svg`, `massif.out.*`, and `runtime/build`) should not be edited as
  source changes.

## Code style

- Use idiomatic Rust: prefer `match`, `Option`, and `Result` to imperative
  null-style checks or avoidable panics.
- Choose descriptive, self-documenting `snake_case` names, even when they are
  long. Prefer `token_start_position` to abbreviations such as `pos` or `p`.
- Prioritize readability over cleverness. Do not introduce abstractions until a
  concrete repeated pattern justifies them.
- Write comments for _why_, not a restatement of _what_ the code does.
