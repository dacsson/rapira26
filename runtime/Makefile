CC = gcc
AR = ar
BUILD_DIR = build
LIB_DIR = lib
WASI_CC ?= clang
WASI_AR ?= llvm-ar
WASI_TARGET ?= wasm32-wasip1
# Set this to the wasi-sdk sysroot, for example
# /opt/wasi-sdk/share/wasi-sysroot.
WASI_SYSROOT ?=
WASM_BUILD_DIR = build-wasm
WASM_LIB_DIR = lib-wasm
SRCS = core.c text.c tuple.c callable.c arithmetic.c builtins.c io.c variant.c
OBJS = $(SRCS:%.c=$(BUILD_DIR)/%.o)
WASM_OBJS = $(SRCS:%.c=$(WASM_BUILD_DIR)/%.o)
HEADERS = rapobject.h runtime.h runtime_internal.h rapvalue.h
DEFAULT_CFLAGS = "-Lraperr/target/release -Lraperr -Iraperr/include"
CFLAGS = -fPIC -O3
WASM_CFLAGS = -O3

all: $(LIB_DIR)/librapruntime.a

$(LIB_DIR)/librapruntime.a: $(OBJS) | $(LIB_DIR)
	$(AR) rcs $@ $(OBJS)

$(BUILD_DIR)/%.o: %.c $(HEADERS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(DEFAULT_CFLAGS) -c $< -o $@

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(LIB_DIR):
	mkdir -p $(LIB_DIR)

# Build a wasm32 static archive for Rust's wasm32-wasip1 target. wasi-sdk's
# headers make uintptr_t 32-bit, selecting the 30-bit SMI layout.
wasm: $(WASM_LIB_DIR)/librapruntime.a

$(WASM_LIB_DIR)/librapruntime.a: $(WASM_OBJS) | $(WASM_LIB_DIR)
	$(WASI_AR) rcs $@ $(WASM_OBJS)

$(WASM_BUILD_DIR)/%.o: %.c $(HEADERS) | $(WASM_BUILD_DIR)
	@test -n "$(WASI_SYSROOT)" || (echo "set WASI_SYSROOT to the wasi-sdk sysroot" >&2; exit 1)
	$(WASI_CC) --target=$(WASI_TARGET) --sysroot=$(WASI_SYSROOT) $(WASM_CFLAGS) -c $< -o $@

$(WASM_BUILD_DIR):
	mkdir -p $(WASM_BUILD_DIR)

$(WASM_LIB_DIR):
	mkdir -p $(WASM_LIB_DIR)

clean:
	rm -rf $(BUILD_DIR) $(LIB_DIR) $(WASM_BUILD_DIR) $(WASM_LIB_DIR)
