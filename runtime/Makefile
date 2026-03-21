CC = gcc
BUILD_DIR = build
LIB_DIR = lib
SRCS = core.c text.c tuple.c callable.c arithmetic.c builtins.c
OBJS = $(SRCS:%.c=$(BUILD_DIR)/%.o)
HEADERS = rapobject.h runtime.h runtime_internal.h

all: $(LIB_DIR)/librapruntime.a

$(LIB_DIR)/librapruntime.a: $(OBJS) | $(LIB_DIR)
	ar rcs $@ $(OBJS)

$(BUILD_DIR)/%.o: %.c $(HEADERS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(LIB_DIR):
	mkdir -p $(LIB_DIR)

clean:
	rm -rf $(BUILD_DIR) $(LIB_DIR)
