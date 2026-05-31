UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Linux)
    CC=clang
else ifeq ($(UNAME_S),Darwin)
    CC=clang
    ARCH = -arch x86_64
endif

DISABLE_WARNINGS=-Wno-shift-negative-value
COMMON_FLAGS=$(DISABLE_WARNINGS) -O3 -fstack-protector-all $(ARCH) --std=c11
PROD_FLAGS=$(COMMON_FLAGS) -DLAMA_ENV
TEST_FLAGS=$(COMMON_FLAGS) -DDEBUG_VERSION
UNIT_TESTS_FLAGS=$(TEST_FLAGS)
INVARIANTS_CHECK_FLAGS=$(TEST_FLAGS) -DFULL_INVARIANT_CHECKS
CFLAGS=-flto

all: gc.o runtime.o printf.o
	ar rc liblamart.a runtime.o gc.o printf.o

gc.o: gc.c gc.h
	$(CC) $(PROD_FLAGS) $(CFLAGS) -c gc.c -o gc.o -fPIC

runtime.o: runtime.c runtime.h
	$(CC) $(PROD_FLAGS) $(CFLAGS) -c runtime.c -o runtime.o -fPIC

printf.o: printf.S
	$(CC) $(PROD_FLAGS) $(CFLAGS) -Wa,--noexecstack -x assembler-with-cpp -c -g printf.S -o printf.o -fPIC

clean:
	$(RM) *.a *.o *~ negative_scenarios/*.err
