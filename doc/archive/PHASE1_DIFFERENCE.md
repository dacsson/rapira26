# Phase 1 — Differences from Spec (Препринт 767)

This document lists intentional deviations from the original specification
during Phase 1 implementation.

## 0-based indexing

The spec uses 1-based indexing throughout: subscript access (`К[1]` is the
first element), slicing (`К[1:3]`), and the `индекс` function returns positions
starting at 1, with 0 meaning "not found".

We use **0-based indexing** instead:

- `К[0]` is the first element of a tuple or text.
- `К[0:2]` is a slice containing the first two elements.
- `индекс(needle, haystack)` returns the 0-based position of the first
  occurrence, or **-1** if not found.

**Rationale:** 0-based indexing is the universal convention in modern languages
and in the C backend we compile to. Keeping 1-based indexing would require
off-by-one adjustments at every subscript, slice, and index operation in
generated code — a source of subtle bugs for no practical benefit.
This will be revisited in Phase 2.

## C backend code generation

The compiler pipeline produces C source code rather than interpreting the AST
directly:

```
Source (.rap) → Lexer → Parser → AST → C Code Generator → .c file → gcc → binary
```

Generated code links against `librapruntime.a` (`runtime/`), which provides:

- **Object system:** all values are `RAP_Object *` with a tagged union
  (`RAP_OBJECT_TAG_INT`, `_FLOAT`, `_TEXT`, `_TUPLE`, `_LOGICAL`, `_NULL`,
  `_CALLABLE`).
- **Polymorphic operations:** `RAP_add`, `RAP_equal`, `RAP_less_than`, etc.
  dispatch on operand tags at runtime.
- **Built-in functions:** `RAP_index_of`, `RAP_abs`, `RAP_sqrt`, `RAP_floor`,
  `RAP_round`, `RAP_sign`, etc.

### Naming conventions in generated C

| Rapira construct       | C name                               |
|------------------------|--------------------------------------|
| Variable `ФРУКТ`       | `_local_FRUKT`  (transliterated)     |
| Function `квадрат`     | `RAP_FUNC_KVADRAT`                   |
| Temp expression result | `_t0`, `_t1`, ...                    |
| String buffer          | `_s0`, `_s1`, ...                    |
| For-loop iterator      | `_iter_I_3` (name + unique id)       |
| For-loop limit         | `_for_limit_3`                       |
| For-loop step          | `_step_3`                            |

The spec does not prescribe an implementation strategy. A C backend was chosen
for Phase 1 because it gives native performance, avoids writing a VM, and makes
debugging generated code straightforward (the .c file is human-readable).
