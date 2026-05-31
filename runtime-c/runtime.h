#ifndef __LAMA_RUNTIME__
#define __LAMA_RUNTIME__

#include "runtime_common.h"
#include <assert.h>
#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <regex.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>

typedef struct {
  char *contents;
  aint   ptr;
  aint   len;
} StringBuf;

extern size_t __gc_stack_top, __gc_stack_bottom;
extern StringBuf stringBuf;

#define WORD_SIZE (CHAR_BIT * sizeof(ptrt))

_Noreturn void failure (char *s, ...);

// Builders
void *Bsexp (aint* args, aint bn);
void *Barray (aint* args, aint bn);
void *Bstring (aint* args/*void *p*/);

// Builtin
void *Lstring (aint* args /* void *p */);
aint Llength (void *p);
aint Lread ();
aint Lwrite (aint n);
void *Bclosure (aint* args, aint bn);

aint Bstring_patt (void *x, void *y);
aint Barray_patt (void *d, aint n);
aint Bclosure_tag_patt (void *x);
aint Bboxed_patt (void *x);
aint Bunboxed_patt (void *x);
aint Barray_tag_patt (void *x);
aint Bstring_tag_patt (void *x);
aint Bsexp_tag_patt (void *x);

char *de_hash (aint n);
aint LtagHash (char *s);

// bool isUnboxed(aint v) {
//     return UNBOXED(v);
// }

// aint rtBox(aint v) {
//     return BOX(v);
// }

// aint rtUnbox(aint v) {
//     return UNBOX(v);
// }

// #define TO_DATA(x) ((data *)((char *)(x)-DATA_HEADER_SZ))
// data* rtToData(void* ptr) {
//     return TO_DATA(ptr);
// }

// #define TO_SEXP(x) ((sexp *)((char *)(x)-DATA_HEADER_SZ))
// sexp* rtToSexp(void* ptr) {
//     return TO_SEXP(ptr);
// }

// #define LEN(x) (ptrt)(((ptrt)x & LEN_MASK) >> 3)
// int rtLen(auint ptr) {
//     return LEN(ptr);
// }

// #define TAG(x) (x & 7)
// int rtTag(auint ptr) {
//     return TAG(ptr);
// }

// aint rtSexpEl(sexp* sexp) {
//     return ((aint *)sexp->contents)[0];
// }

void printValue (void *p);
void createStringBuf ();

#endif
