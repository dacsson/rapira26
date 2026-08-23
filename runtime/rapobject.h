#ifndef RAPIRA_OBJECT_H
#define RAPIRA_OBJECT_H

#include "rapvalue.h"
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Tag identifies the type of a [RAP_Object]
typedef enum {
  RAP_OBJECT_TAG_NULL,
  RAP_OBJECT_TAG_CALLABLE, // unifies proc and func
  RAP_OBJECT_TAG_INT,
  RAP_OBJECT_TAG_FLOAT,
  RAP_OBJECT_TAG_TEXT,
  RAP_OBJECT_TAG_TUPLE,
  RAP_OBJECT_TAG_SLICE,
  RAP_OBJECT_TAG_VARIANT, // of user-defined type
} RAP_ObjectTag;

struct RAP_Tuple;
struct RAP_Callable;

// RAP_Object is specifically a heap allocated entity, encoded as a pointer in
// [RAP_Value]
// Each object in the runtime has a tag indicating its type and a
// union of possible values for that type
typedef struct {
  RAP_ObjectTag tag;
  int refcount;
  union {
    int64_t int_val;
    double float_val;
    struct RAP_Tuple *text_val;
    struct RAP_Tuple *tuple_val;
    struct RAP_Callable *callable_val;
    struct RAP_Slice *slice_val;
    struct RAP_Variant *variant_val;
  };
} RAP_Object;

/// Funcs and procs are treated as objects.
struct RAP_Callable {
  // Opaque implementation entry. The runtime does not interpret this value;
  // the VM currently uses it as a bytecode entry offset.
  size_t offset_or_ptr;
  uint32_t arity;

  // Reserved for lexical captures when lambdas are implemented. Each capture
  // is retained by the callable for its whole lifetime.
  RAP_Value *captures;
  uint32_t capture_count;
};

/// Tuple is a untyped list of objects.
struct RAP_Tuple {
  uint32_t count;
  RAP_Value *items;
};

/// Slice: a view into a tuple (or text).
/// Holds a pointer to the parent and 0-based [from, to) bounds.
/// Slices of slices flatten to the root parent (no chaining).
struct RAP_Slice {
  RAP_Object *parent; // the tuple/text we're viewing
  int64_t from;       // inclusive start
  int64_t to;         // exclusive end
};

/// User-defined variant type
struct RAP_Variant {
  const char* name; // type name
  const char** field_names;
  size_t field_count;
  void* payload;    // variant data
};

#endif // RAPIRA_OBJECT_H
