#include <stddef.h>

// Host-provided module metadata for running under the VM interpreter.
//
// In the (deprecated) C backend these are emitted by the generated module:
// the current source path and the byte span of the instruction being executed,
// used by RAP_fatal_error to render diagnostics. Under the VM there is no
// generated module, so we supply defaults here.
// TODO: have the interpreter update these from bytecode line/span info.
//
// The diagnostic renderer `runtime_error_description` is NOT stubbed here: it
// is provided by the `raperr` crate, linked by the binary (see
// vm-interp/build.rs), same as the C backend.
char *RAP_curret_module_path = "<vm>";
size_t RAP_current_pos_start = 0;
size_t RAP_current_pos_end = 0;
