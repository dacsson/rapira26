#ifndef RAPIRA_RUNTIME_INTERNAL_H
#define RAPIRA_RUNTIME_INTERNAL_H

#include "runtime.h"
#include "rapobject.h"
#include "rapvalue.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>

// Fatal error - print a message and exit.
void RAP_fatal_error(const char *message);

// ALLOCATION TRACKING (test-only, compile with -DRAP_TEST_LEAKS)
#ifdef RAP_TEST_LEAKS
extern int rap_alloc_count;
#define RAP_TRACK_ALLOC() rap_alloc_count++
#define RAP_TRACK_FREE()  rap_alloc_count--
#else
#define RAP_TRACK_ALLOC()
#define RAP_TRACK_FREE()
#endif

// Get the underlying RAP_Tuple* for both TUPLE and TEXT objects.
struct RAP_Tuple *rap_get_items(RAP_Object *obj);

// Decode UTF-8 string into an array of codepoint values.
// Caller must free *out_codepoints.
size_t rap_utf8_decode_all(const char *s, int64_t **out_codepoints);

// Encode a single codepoint to UTF-8, append to buffer. Returns new length.
size_t rap_utf8_encode_one(int64_t cp, char *buf, size_t pos);

// Helper: dynamically builds a string by appending to a buffer.
char *rap_strbuf_append(char *buf, size_t *len, size_t *cap, const char *append);

#endif // RAPIRA_RUNTIME_INTERNAL_H
