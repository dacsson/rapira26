# Rapira26 — Roadmap

## Phase 1 — Spec Implementation ✓

A complete, faithful implementation of the Rapira spec (Препринт 767) with C as the compilation backend.

### What's Working

- **All data types:** null, logical, integer, float, text (as codepoint tuples), tuples, callables, slices
- **All operators:** arithmetic (+, -, *, /, //, /%, **), comparison (>, <, >=, <=, =, /=), logical (и, или, не), length (#), polymorphic dispatch (int/float/text/tuple)
- **Text operations:** concat, repeat, subscript, slice, substring search (индекс)
- **Tuple operations:** construct, concat, repeat, subscript, slice, element assignment, index_of
- **Statements:** assignment, output (вывод/вывод бпс), input (ввод/ввод текста), conditionals (если/иначе), selection (выбор — both value-match and condition-list forms), all loop types (для/повтор/пока/цикл, кц по post-condition), break (выход), return (возврат)
- **Procedures & functions:** definitions, calls (explicit вызов and implicit), input params (=>), in-out params (<=), callable objects, recursion
- **Dynamic scoping:** чужие (foreign names via frame chain walk), свои (explicit locals)
- **Builtins:** корень, абс, целый, длин, знак, целч, окрч, дсч, цсч, индекс, пс, пи
- **Type checks:** тип_пуст, тип_лог, тип_цел, тип_вещ, тип_текст, тип_корт, тип_проц, тип_функ
- **Text escaping:** `""` → literal `"`

### Intentional Deviations

See `doc/PHASE1_DIFFERENCE.md`:
- 0-based indexing (spec uses 1-based)
- C backend code generation (spec doesn't prescribe implementation)

### Test Coverage

13 E2E test files (`tests/examples/01–13_*.rap`) covering all features above, plus lexer/parser unit tests. All passing.

### Runtime Architecture

Runtime split into modules (`runtime/`):
| File | Contents |
|------|----------|
| `core.c` | Fatal error, UTF-8 helpers, constructors, stringify |
| `text.c` | Text constructor (UTF-8 → codepoint tuple) |
| `tuple.c` | Tuple constructor, get/set item, append, index_of, slices |
| `callable.c` | Callable/parameter creation, frame utilities, call dispatch |
| `arithmetic.c` | Integer/float/generic operations |
| `builtins.c` | Built-in math functions |
| `io.c` | Input (ввод/ввод текста) |

## Phase 1 — Remaining Work

- **REPL mode:** Interactive line-by-line execution (see TODO in `src/main.rs`)

## Phase 2 — Modernization

After Phase 1 is complete, revisit and extend the language with modern features. The spec will be rewritten to reflect these additions.
