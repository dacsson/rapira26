# Adopting LaMa bytecode for Rapira26

This document describes the translation process from LaMa dependencies to adopt LaMa bytecode and VM for Rapira26 language.

## Overview

The `lamarik` LaMa VM interpreter was written by me with the only code dependency being the LaMa runtime. Obviously the bytecode itself is the exact bytecode used by LaMa. Why we need this:
1. Current C backend of Rapira26 is not suitable for a dynamic language, involves a lot of text manipulation in codegen and is not that interesting 
2. We need a dynamic language VM that is easy to integrate with Rapira26, and whats either then the one you have written yourself?

## Runtime 

Runtime needs to be swapped from LaMa to Rapira26. The open question is a garbage collector.

## Bytecode 

1. Get rid of S-expressions 
2. Get rid of unused opcodes in LaMa itsef (indirect stores etc.)
3. Introduce opcodes for type declaration (sum types)
4. Introduce opcodes for pattern matching on sum type variants

## Strategy

1. Decide what opcodes need to be removed
- Candidates:
  - `Instruction::SEXP`
  - `Instruction::TAG`
2. Write a backend for Rapira26 that translates to LaMa bytecode 
- No full coverage is needed
- Do not yet translate type declarations
3. Replace runtime
4. Decide what opcodes need to be added
5. Patch the backend
6. Remove or archive the old C backend

## TODO

- [x] Create simple bytefile writer API
- [x] Boot up simple `вывод: 2` program
  - [x] Create e2e run pipeline
- [x] Disable optimizations (deframe pass)
- [ ] Implement basic arithemtic operations (Int + Float)
  - [ ] Comment out support for other runtime constructrs
  - [ ] Rewrite runtime support for arithmetic operations
  - [ ] Run `examples/02_arithmetic.rap`
