CC = gcc
BUILD_DIR = build
LIB_DIR = lib
SRCS = core.c text.c tuple.c callable.c arithmetic.c builtins.c io.c variant.c gc_stub.c vm_host_stub.c
OBJS = $(SRCS:%.c=$(BUILD_DIR)/%.o)
HEADERS = rapobject.h runtime.h runtime_internal.h rapvalue.h
DEFAULT_CFLAGS = "-Lraperr/target/release -Lraperr -Iraperr/include"
CFLAGS = -fPIC

all: $(LIB_DIR)/librapruntime.a

# librapruntime.a leaves `runtime_error_description` (used by RAP_fatal_error)
# undefined; it is resolved at final-binary link time by the `raperr` crate.
# The C backend links raperr's staticlib; the VM consumes raperr as a cargo
# rlib dependency (see cli/Cargo.toml) so the Rust std is shared rather than
# duplicated.
$(LIB_DIR)/librapruntime.a: $(OBJS) | $(LIB_DIR)
	ar rcs $@ $(OBJS)

$(BUILD_DIR)/%.o: %.c $(HEADERS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(DEFAULT_CFLAGS) -c $< -o $@

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(LIB_DIR):
	mkdir -p $(LIB_DIR)

clean:
	rm -rf $(BUILD_DIR) $(LIB_DIR)
