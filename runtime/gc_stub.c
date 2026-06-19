#include <stddef.h>

// GC stub for the VM interpreter (adopted from LaMa).
//
// Rapira's runtime is refcounting-based and does not perform conservative
// stack scanning, so garbage collection is a no-op here. These symbols exist
// only because the interpreter still tracks its operand-stack top through
// __gc_stack_bottom; see vm-interp/src/interpreter.rs.
size_t __gc_stack_top = 0;
size_t __gc_stack_bottom = 0;

void __gc_init(void) {}
