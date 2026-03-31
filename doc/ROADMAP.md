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
- **Automatic Reference counting** 

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

## Phase 2 — Modernization

Revisit and extend the language with modern features. The spec will be rewritten to reflect these additions.

### ✓ Step 0 — Indentation-based syntax ✓
TODO: Replace `конец`/`все`/`кц` block terminators with indentation-based scoping (Python-style). Requires reworking the lexer to emit indent/dedent tokens and updating the parser accordingly.
RESULT: Lexer emits `Indent`/`Dedent`/`Newline` tokens; parser uses `parse_block_or_single_statement` for all block constructs. Single-line forms (e.g. `если X то выход`) supported. Multi-line expressions require balanced delimiters `()`, `[]`, `<* *>`. All 13 test files rewritten. Keywords `конец`/`все`/`кц` kept in lexer for error messages but unused by parser.

### ✓ Step 2 — Reference counting ✓
Add `refcount` field to `RAP_Object`. Increment on assignment/parameter pass, decrement on scope exit/reassignment, free at zero. No circular references in Rapira, so refcounting is sufficient.

### ✓ Step 3 — SMI pointer tagging ✓
Replace `RAP_Object*` with a tagged `uintptr_t` (V8-style). Lowest bit distinguishes SMI (Small Integer, bit 0 = 0, value = word >> 1) from heap pointer (bit 0 = 1, pointer = word & ~1). Integers — the most common type in loops, indexing, arithmetic — never touch the heap. Floats, text, tuples, callables remain heap-allocated with a type tag. Gives 63-bit integers, single-instruction type checks (`v & 1`), and free add/subtract without untagging.

### ✓ Step 4 — Simple optimization ✓
- Come up with a number of benchmarks 
- Optimize bottleneckzzz:
  - Frame variable lookup: we can statically analyze when do we actually need to save a local variable in frame, i.e. do it _only if_ there is a function in CFG that uses it in `чужие` block
  - Forbid `чужие` usage from top-level
  - Experiment with mimalloc
- Reference counting (PUSHED TO LATER, too hard m8 :c )
  - idk look at Perceus?
  - escape-analysis?
  - Каунтер надо сделать гораздо меньше, хедер объекта у нас какой-то безумный
  - Фикс залипания счётчика ссылок, переполнения каунтера

### Step 5 - Errors tied to file source instead of C and Semcheck
If an error hapens emit source file line, not C error

### Step 6 - Add new features from big spec and change container syntax
- Source: https://ershov.iis.nsk.su/ru/node/772596

Fetures to add:
- Structs: `<$ имя: "иван", фамилия: "петров" $>`
  - Dont forget about cycle references in refcounting alg.
- Change tuple syntax: `< 1, 2, 3 >`
- Sets: `<* 1, 1+1, <1, 5> *>`
- ИЗ — проверка принадлежности:
```
«м» ИЗ «микроскоп» истинно;
```
- ДЛЯ-ИЗ - foreach:
```
ДЛЯ Буква ИЗ Книга :: ЕСЛИ Буква=«А» ТО Сч+1->Сч ВСЕ ВСЕ;
ВЫВОД: «В тексте Книга », Сч, " букв <А>";
```
- `КОНТРОЛЬ` - asserts
- `::` - consts (Jai inspired):
```
пенделей := 10 \ переменная
пенделей :: 10 \ константа
```

We probably should change symbols `<` to use something more ergonomic for RU keyborads, something like:
- Structs: `(имя: "иван", фамилия: "петров")`
- tuple: `(1, 2, 3)`
- Sets: `(1; 1+1; <1, 5>)`

### Step 7 - Semantic analysis

possible errors to find (at this stage):
- Passing InOut parameters from top-level
- Requesting foreign vars from top-level
- Indent/Dedent issues
- Infinite or zero-effect loop check
- Function call arity
- Tuple bound checks (?)
- Check for existence of requested "чужие" variables

General semantic anal. idea:
- Have N levels of checks/strictness/safety/...
  - 1st if Prototyping, no type checking minimal semcheck
  - 2nd is Refactoring, type checking for TYPED MODULES, check for unknown vars
    - We should make a law, either a function where type hints occur must be typed for all variables, or same but for a whole module
  - 3d is Safety, type checking, check for unknown and unititalized (?) vars, disables refcounting, enables manual memory handling (?)
    - Maybe some sort of safety feature like borrow checking or whatever

### Step 8 — Optional type hints with flow typing
Leverage `тип_*` checks for static type narrowing. When the compiler can prove a variable's type from a guard (`если тип_цел(X) то ...`), emit direct typed operations instead of polymorphic dispatch. Optional type annotations on parameters and variables.

### Step 9 — Module system
Import/export mechanism for splitting programs across files. Spec §1.6 sketches modules and devices — design a modern take that supports namespacing and selective imports.

### Step 10 — OOP / Object system
User-defined object types with fields and methods. Design TBD — could be prototype-based (like Lua) or class-based.

### Step 11 — Build system
Project-level build tool: dependency resolution, multi-file compilation, incremental builds. Replaces manual `cargo run -- file.rap` workflow.

### Step 12 — REPL mode
Interactive line-by-line execution. Compile each input to a shared library, dlopen into a persistent process with a live frame. Accumulate definitions across inputs.
