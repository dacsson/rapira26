#set page(paper: "a4", margin: (x: 2cm, y: 2.5cm))
#set text(font: "New Computer Modern", size: 11pt)
#set heading(numbering: "1.")
#set table(inset: 6pt, stroke: 0.5pt)

#align(center)[
  #text(size: 20pt, weight: "bold")[rapira26 VM — Bytecode Reference]
  #v(0.4em)
  #text(size: 12pt, fill: luma(80))[Source of truth: `vm-core/src/decoder.rs`]
]

#v(1.2em)
#line(length: 100%, stroke: 0.5pt)
#v(1em)

= Encoding Conventions

Each instruction begins with a single *opcode byte*. The upper nibble (bits 7–4)
identifies the instruction group; the lower nibble (bits 3–0) selects the variant.
Multi-byte operands immediately follow the opcode and are always encoded as
little-endian 32-bit signed integers (`i32le`, 4 bytes), unless otherwise noted.

#v(0.5em)
*Notation used below*
#table(
  columns: (auto, 1fr),
  [*`i32le`*], [4-byte little-endian signed integer operand],
  [*`f64le`*], [8-byte little-endian IEEE 754 double],
  [*`u8`*], [1-byte unsigned integer],
  [*`—`*], [no operands (opcode byte only)],
)

= Instruction Reference

== Group `0x0_` — Miscellaneous / Binary Operations

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x00`], [`NOP`], [—], [No operation.],
  [`0x01`], [`BINOP ADD`], [—], [Pop two integers, push their sum.],
  [`0x02`], [`BINOP SUB`], [—], [Pop two integers, push their difference (TOS-1 − TOS).],
  [`0x03`], [`BINOP MUL`], [—], [Pop two integers, push their product.],
  [`0x04`],
  [`BINOP DIV`],
  [—],
  [Pop two numbers and divide TOS-1 by TOS; exact integer division stays integer, otherwise push a real.],

  [`0x05`], [`BINOP MOD`], [—], [Pop two integers, push remainder.],
  [`0x06`], [`BINOP LT`], [—], [Push 1 if TOS-1 < TOS, else 0.],
  [`0x07`], [`BINOP LEQ`], [—], [Push 1 if TOS-1 ≤ TOS, else 0.],
  [`0x08`], [`BINOP GT`], [—], [Push 1 if TOS-1 > TOS, else 0.],
  [`0x09`], [`BINOP GEQ`], [—], [Push 1 if TOS-1 ≥ TOS, else 0.],
  [`0x0a`], [`BINOP EQ`], [—], [Push 1 if TOS-1 == TOS, else 0.],
  [`0x0b`], [`BINOP NEQ`], [—], [Push 1 if TOS-1 ≠ TOS, else 0.],
  [`0x0c`], [`BINOP AND`], [—], [Push 1 if both TOS-1 and TOS are non-zero.],
  [`0x0d`], [`BINOP OR`], [—], [Push 1 if either TOS-1 or TOS is non-zero.],
  [`0x0e`], [`BINOP IDIV`], [—], [Pop two integers and push their quotient rounded down.],
  [`0x0f`], [`BINOP POW`], [—], [Pop base and exponent and push base raised to exponent.],
)

The `BINOP` lower nibble encodes the operator (`0x01`–`0x0f`).

== Group `0x1_` — Stack / Constants / Control

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x10`], [`CONST`], [`i32le value`], [Push immediate integer `value` onto the stack.],
  [`0x11`], [`STRING`], [`i32le index`], [Push the `index`-th string from the string table.],
  [`0x12`], [`TUPLE`], [`i32le n`], [Pop `n` values from the stack, pack them into a tuple, push the tuple.],
  [`0x13`], [`STS`], [—], [Store to slice: pop slice, then pop value and assign into the slice.],
  [`0x14`], [`STA`], [—], [Store to aggregate: pop aggregate, pop index, pop value; assign `value` at `index` into `aggregate`.],
  [`0x15`], [`JMP`], [`i32le offset`], [Unconditional jump to `offset` (absolute position in code section).],
  [`0x16`], [`END`], [—], [End of procedure: return top-of-stack value to caller.],
  [`0x17`],
  [`SLICE`],
  [`u8 bounds`],
  [Push a slice (view) of an aggregate. `bounds` is a bitmask: bit 0 set means `from` is on the stack, bit 1 set means `to` is on the stack. Thus `0` = `[:]`, `1` = `[from:]`, `2` = `[:to]`, `3` = `[from:to]`.],

  [`0x18`], [`DROP`], [—], [Discard (pop) the top value from the stack.],
  [`0x19`], [`DUP`], [—], [Duplicate the top value of the stack.],
  [`0x1a`], [`SWAP`], [—], [Swap the two topmost values on the stack.],
  [`0x1b`], [`ELEM`], [—], [Pop aggregate and index, push the element at that index.],
  [`0x1c`],
  [`CONSTF`],
  [`f64le value`],
  [Push immediate 64-bit floating-point `value` (little-endian IEEE 754) onto the stack.],
)

Opcodes `0x1d`–`0x1f` are unused.

== Group `0x2_` — Load

Load a value onto the stack from a variable slot. The lower nibble encodes the
scope (*value relation*).

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x20`], [`LOAD global`], [`i32le index`], [Push global variable at slot `index`.],
  [`0x21`], [`LOAD local`], [`i32le index`], [Push local variable at slot `index`.],
  [`0x22`], [`LOAD arg`], [`i32le index`], [Push function argument at position `index`.],
  [`0x23`], [`LOAD capture`], [`i32le index`], [Push captured (closure) variable at slot `index`.],
)

Lower nibble: `0x0` = Global, `0x1` = Local, `0x2` = Arg, `0x3` = Capture.
Opcodes `0x24`–`0x2f` are unused.

== Group `0x3_` — Unary Operations

Unary operations on the top-of-stack value.

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x30`], [`UNARY Negate`], [—], [Pop a value, push its arithmetic negation (−TOS).],
  [`0x31`], [`UNARY Not`], [—], [Pop a value, push 1 if TOS == 0 else 0 (logical not).],
)

Lower nibble: `0x0` = Negate, `0x1` = Not.
Opcodes `0x32`–`0x3f` are unused.

== Group `0x4_` — Store

Store the top-of-stack value into a variable slot. Lower nibble encodes scope,
same as `LOAD`.

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x40`], [`STORE global`], [`i32le index`], [Pop TOS and store into global variable at slot `index`.],
  [`0x41`], [`STORE local`], [`i32le index`], [Pop TOS and store into local variable at slot `index`.],
  [`0x42`], [`STORE arg`], [`i32le index`], [Pop TOS and store into function argument slot `index`.],
  [`0x43`], [`STORE capture`], [`i32le index`], [Pop TOS and store into captured variable at slot `index`.],
)

Opcodes `0x44`–`0x4f` are unused.

== Group `0x5_` — Control Flow / Procedure Management

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x50`], [`CJMP iszero`], [`i32le offset`], [Jump to `offset` if TOS == 0; pop TOS.],
  [`0x51`], [`CJMP isnonzero`], [`i32le offset`], [Jump to `offset` if TOS ≠ 0; pop TOS.],
  [`0x52`],
  [`BEGIN`],
  [`i32le args`, `i32le locals`],
  [Start of procedure with `args` arguments and `locals` local variables. Initialises locals to empty. Cannot use captured variables.],

  [`0x53`],
  [`CBEGIN`],
  [`i32le args`, `i32le locals`],
  [Start of closure body with `args` arguments and `locals` locals. May reference captured variables.],

  [`0x54`],
  [`CLOSURE`],
  [`i32le offset`, `i32le arity`],
  [Construct a closure whose bytecode starts at `offset`. `arity` is the number of captured variables. The captured variable descriptors follow in a variable-length encoding (not yet implemented).],

  [`0x55`],
  [`CALLC`],
  [`i32le arity`],
  [Call the closure on TOS-(`arity`) with `arity` arguments; push the return value.],

  [`0x56`],
  [`CALL`],
  [`i32le offset`, `i32le n`],
  [Call the function whose bytecode starts at `offset` with `n` arguments; push the return value.],

  [`0x58`], [`ARRAY`], [`i32le n`], [Test whether TOS is an array of exactly `n` elements.],
  [`0x5a`],
  [`LINE`],
  [`i32le n`],
  [Annotation: following bytecode corresponds to source line `n`. Used for diagnostics only.],
)

Opcodes `0x57`, `0x59`, `0x5b`–`0x5f` are unused.

== Group `0x6_` — (unused)

All opcodes `0x60`–`0x6f` are unused.

== Group `0x7_` — Builtin Calls / Bool / Null

#table(
  columns: (auto, auto, auto, 1fr),
  table.header([*Opcode*], [*Mnemonic*], [*Operands*], [*Description*]),

  [`0x70`], [`CALLBUILTIN Lread`], [—], [Call built-in `Lread`: read an integer from stdin, push the result.],
  [`0x71`],
  [`CALLBUILTIN Lwrite`],
  [`i32le n`],
  [Call built-in `Lwrite`: pop TOS and write it to stdout. `n` encodes flags (e.g. newline bit at `1 << 30`). Always reads 4 operand bytes in the decoder even when `n == 0`.],

  [`0x72`], [`CALLBUILTIN Llength`], [—], [Call built-in `Llength`: pop string/array, push its length.],
  [`0x73`],
  [`CALLBUILTIN Lstring`],
  [—],
  [Call built-in `Lstring`: load string from string table (string index is on the stack).],

  [`0x74`],
  [`CALLBUILTIN Barray`],
  [`i32le n`],
  [Call built-in `Barray`: construct an array from the top `n` stack values.],

  [`0x75`], [`BOOL`], [`u8`], [Push a boolean value onto the stack. Operand: non-zero = #true, zero = #false.],
  [`0x76`], [`NULL`], [—], [Push the null value.],
  [`0x77`], [`CALLBUILTIN Abs`], [`i32le n`], [Pop one number and push its absolute value.],
  [`0x78`], [`CALLBUILTIN Sign`], [`i32le n`], [Pop one number and push -1, 0, or 1 according to its sign.],
  [`0x79`], [`CALLBUILTIN Sqrt`], [`i32le n`], [Pop one number and push its square root.],
  [`0x7a`], [`CALLBUILTIN Floor`], [`i32le n`], [Pop one real number and push its floor as an integer.],
  [`0x7b`], [`CALLBUILTIN Round`], [`i32le n`], [Pop one real number and push the nearest integer.],
  [`0x7c`], [`CALLBUILTIN Index`], [`i32le n`], [Pop needle and haystack and push the index of the first match.],
)

Opcodes `0x7d`–`0x7f` are unused.

= Summary Table

All defined opcode bytes at a glance. "Operand bytes" is the total number of
additional bytes the instruction consumes beyond the opcode byte itself.

#table(
  columns: (auto, auto, auto),
  table.header([*Hex*], [*Mnemonic*], [*Operand bytes*]),

  [`0x00`], [`NOP`], [`0`],
  [`0x01`], [`BINOP ADD`], [`0`],
  [`0x02`], [`BINOP SUB`], [`0`],
  [`0x03`], [`BINOP MUL`], [`0`],
  [`0x04`], [`BINOP DIV`], [`0`],
  [`0x05`], [`BINOP MOD`], [`0`],
  [`0x06`], [`BINOP LT`], [`0`],
  [`0x07`], [`BINOP LEQ`], [`0`],
  [`0x08`], [`BINOP GT`], [`0`],
  [`0x09`], [`BINOP GEQ`], [`0`],
  [`0x0a`], [`BINOP EQ`], [`0`],
  [`0x0b`], [`BINOP NEQ`], [`0`],
  [`0x0c`], [`BINOP AND`], [`0`],
  [`0x0d`], [`BINOP OR`], [`0`],
  [`0x0e`], [`BINOP IDIV`], [`0`],
  [`0x0f`], [`BINOP POW`], [`0`],
  [`0x10`], [`CONST`], [`4`],
  [`0x11`], [`STRING`], [`4`],
  [`0x12`], [`TUPLE`], [`4`],
  [`0x13`], [`STS`], [`0`],
  [`0x14`], [`STA`], [`0`],
  [`0x15`], [`JMP`], [`4`],
  [`0x16`], [`END`], [`0`],
  [`0x17`], [`SLICE`], [`1`],
  [`0x18`], [`DROP`], [`0`],
  [`0x19`], [`DUP`], [`0`],
  [`0x1a`], [`SWAP`], [`0`],
  [`0x1b`], [`ELEM`], [`0`],
  [`0x1c`], [`CONSTF`], [`8`],
  [`0x20`], [`LOAD global`], [`4`],
  [`0x21`], [`LOAD local`], [`4`],
  [`0x22`], [`LOAD arg`], [`4`],
  [`0x23`], [`LOAD capture`], [`4`],
  [`0x30`], [`UNARY Negate`], [`0`],
  [`0x31`], [`UNARY Not`], [`0`],
  [`0x40`], [`STORE global`], [`4`],
  [`0x41`], [`STORE local`], [`4`],
  [`0x42`], [`STORE arg`], [`4`],
  [`0x43`], [`STORE capture`], [`4`],
  [`0x50`], [`CJMP iszero`], [`4`],
  [`0x51`], [`CJMP isnonzero`], [`4`],
  [`0x52`], [`BEGIN`], [`8`],
  [`0x53`], [`CBEGIN`], [`8`],
  [`0x54`], [`CLOSURE`], [`8`],
  [`0x55`], [`CALLC`], [`4`],
  [`0x56`], [`CALL`], [`8`],
  [`0x58`], [`ARRAY`], [`4`],
  [`0x5a`], [`LINE`], [`4`],
  [`0x70`], [`CALLBUILTIN Lread`], [`0`],
  [`0x71`], [`CALLBUILTIN Lwrite`], [`4`],
  [`0x72`], [`CALLBUILTIN Llength`], [`0`],
  [`0x73`], [`CALLBUILTIN Lstring`], [`0`],
  [`0x74`], [`CALLBUILTIN Barray`], [`4`],
  [`0x75`], [`BOOL`], [`1`],
  [`0x76`], [`NULL`], [`0`],
  [`0x77`], [`CALLBUILTIN Abs`], [`4`],
  [`0x78`], [`CALLBUILTIN Sign`], [`4`],
  [`0x79`], [`CALLBUILTIN Sqrt`], [`4`],
  [`0x7a`], [`CALLBUILTIN Floor`], [`4`],
  [`0x7b`], [`CALLBUILTIN Round`], [`4`],
  [`0x7c`], [`CALLBUILTIN Index`], [`4`],
)