# Rapira26 — Roadmap to Phase 1 Completion

## Context

The codegen (`src/codegen.rs`) can now generate working C from `prime.rap` — functions, loops, conditionals, output all work. But many spec features are still missing or stubbed. This plan sequences the remaining work toward a complete Phase 1 (faithful Препринт 767 implementation).

## Current State

**Working:** literals, all operators (int-only), assignment, output, conditionals, selection, all loop types, function/procedure defs, callable objects, tuple construction/indexing, 4 builtins (корень, абс, целый, длин), return/break.

**Broken/Missing:** mixed-type arithmetic, slices, input (ввод), scoping (чужие/свои), in-out params on subscripts, type-check builtins, text/tuple concat & repeat, several math builtins, text escaping.

## Proposed Work Sequence

### Step 1 — Runtime polymorphic arithmetic
**Files:** `runtime/runtime.c`, `runtime/runtime.h`, `src/codegen.rs`

Add runtime helpers `RAP_add`, `RAP_subtract`, `RAP_multiply`, `RAP_divide`, `RAP_power`, `RAP_less_than`, `RAP_greater_than`, `RAP_equal`, `RAP_less_or_equal`, `RAP_greater_or_equal`, `RAP_not_equal` that dispatch on tag (int+int→int, int+float→float, text+text→concat). Update codegen to call these instead of inlining int-only versions.

**Why first:** Unblocks almost every test file. The current int-only arithmetic is wrong for any program using floats or text concat.

### Step 2 — Slices (read + write)
**Files:** `runtime/runtime.c`, `runtime/runtime.h`, `src/codegen.rs`

Add `RAP_slice_get(obj, from, to)` and `RAP_slice_set(obj, from, to, value)` to runtime. Works on text (substring) and tuples (subtuple). Fill in `Expr::Slice` and `LValue::Slice` in codegen.

**Why:** Used heavily in text/tuple test files (03, 04, 12).

### Step 3 — Text & tuple operations
**Files:** `runtime/runtime.c`, `runtime/runtime.h`

This mostly falls out of Step 1 (concat via `+`) and Step 2 (slices), but also needs:
- Text repetition (`text * n`) — add to `RAP_multiply`
- Tuple repetition (`tuple * n`) — same
- `индекс(haystack, needle)` builtin — add to codegen's `try_emit_builtin`
- Text comparison (<, >, =) — add to polymorphic comparisons

### Step 4 — Input (ввод)
**Files:** `runtime/runtime.c`, `runtime/runtime.h`, `src/codegen.rs`

Add `RAP_input_text()` → reads a line from stdin, returns text object. Add `RAP_input_value()` → reads and parses a number/etc. Codegen emits calls for `Statement::Input`.

### Step 5 — Dynamic scoping (чужие/свои)
**Files:** `runtime/runtime.c`, `runtime/runtime.h`, `src/codegen.rs`

**Spec rule (confirmed):** Without `чужие` declaration, any variable name inside a proc/func is a *fresh local* defaulting to `пусто`. Only names listed in `чужие:` walk the frame chain. This means we use the **hybrid approach**:

- **Non-чужие names** stay as fast C locals (`_local_X`), initialized to `NULL` (пусто)
- **`чужие` names** use runtime frame lookup: `RAP_frame_get(_frame, "X")` / `RAP_frame_set(_frame, "X", val)`
- **`свои` names** are just explicitly local (same as default behavior, documents intent)
- The **enclosing scope** must `RAP_frame_set` its variables into its frame so that called functions' `чужие` lookups can find them

Runtime additions needed:
- `RAP_frame_get(frame, name)` — walk parent chain, find by string key, return value
- `RAP_frame_set(frame, name, value)` — walk parent chain, find by string key, set value
- Change `RAP_CallFrame.locals` from array to name→value map (or parallel name array)

Codegen changes:
- For each `чужие` name in a func/proc: emit reads/writes via `RAP_frame_get/set` instead of `_local_*`
- Before calling a func/proc, the caller must store its variables into its frame so they're discoverable

### Step 6 — Remaining builtins & type checks
**Files:** `src/codegen.rs`

- Type-check functions: `тип_пуст`, `тип_лог`, `тип_цел`, `тип_вещ`, `тип_текст`, `тип_корт`, `тип_проц`, `тип_функ` — trivial, just check `obj->tag`
- Math: `знак` (sign), `окрч` (round) — trivial with C math.h
- String: proper escape handling in text literals, `нс` newline constant

### Step 7 — In-out parameters on subscripts
**Files:** `src/codegen.rs`, possibly `runtime/`

`вызов SWAP(<=A[1], <=A[2])` — needs pass-by-reference for tuple elements. Requires either pointer-to-slot semantics or post-call writeback.

### Step 8 — End-to-end test harness
**Files:** `tests/codegen_tests.rs` (new)

Run codegen on each `.rap` file → compile with gcc → execute → compare output to expected. This closes the loop and catches regressions.

## Verification

After each step:
1. `cargo build && cargo test` — Rust side compiles
2. Generate C from test `.rap` files → compile with gcc → run
3. After Step 8: automated E2E tests cover everything

## Key Files
- `src/codegen.rs` — C code emitter (main work area)
- `runtime/runtime.c` + `runtime/runtime.h` — C runtime library
- `src/ast.rs` — AST definitions (complete, read-only)
- `tests/examples/*.rap` — 12 test programs covering full spec
