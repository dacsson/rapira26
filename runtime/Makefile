CC = gcc

all: runtime.o
	ar rcs librapruntime.a runtime.o

runtime.o: runtime.c runtime.h rapobject.h
	$(CC) $(CFLAGS) -c runtime.c -o runtime.o

clean:
	rm -f runtime.o librapruntime.a
