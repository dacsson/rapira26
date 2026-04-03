#ifndef RAPIRA_RUNTIME_H
#define RAPIRA_RUNTIME_H

#include "rapobject.h"
#include "rapvalue.h"

// METADATA

// Should be overriden by compiled module
// TODO: this not thread safe
extern char* RAP_curret_module_path;
extern size_t RAP_current_pos_start;
extern size_t RAP_current_pos_end;

// CONSTRUCTORS

RAP_Value RAP_create_null_obj(void);
RAP_Value RAP_create_int_obj(int64_t value);
RAP_Value RAP_create_float_obj(double value);
RAP_Value RAP_create_text_obj(const char *value);
RAP_Value RAP_create_tuple_obj(uint32_t count, RAP_Value *items);
RAP_Value RAP_create_callable_obj(struct RAP_CallFrame *frame_parent,
                                  RAP_FunctionDecl func, RAP_Parameter **params,
                                  uint32_t params_count, bool is_function);
RAP_Parameter *RAP_create_parameter(RAP_ParameterMode mode, const char *name);
RAP_Value RAP_create_logical_obj(bool value);

// OBJECTS UTILITIES

RAP_Value RAP_call_callable_obj(RAP_Value callable, RAP_Value *args,
                                uint32_t arg_count);
RAP_Value RAP_get_tuple_item(RAP_Value tuple, uint32_t index);
RAP_Value RAP_set_tuple_item(RAP_Value tuple, uint32_t index, RAP_Value value);
// Joins two tuples into a new tuple
RAP_Value RAP_append_tuple(RAP_Object *a, RAP_Object *b);
RAP_Value RAP_index_of(RAP_Value needle, RAP_Value haystack);

// SLICE OPERATIONS

RAP_Value RAP_create_slice(RAP_Value parent, int64_t from, int64_t to);
RAP_Value RAP_materialize_slice(RAP_Object *obj);
void RAP_slice_assign(RAP_Value slice, RAP_Value replacement);

// INTEGER OPERATIONS

RAP_Value RAP_integer_less_than(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_greater_than(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_not_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_modulo(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_add(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_subtract(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_multiply(RAP_Value a, RAP_Value b);
RAP_Value RAP_integer_divide(RAP_Value a, RAP_Value b);

// FLOAT OPERATIONS

RAP_Value RAP_float_less_than(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_greater_than(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_not_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_modulo(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_add(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_subtract(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_multiply(RAP_Value a, RAP_Value b);
RAP_Value RAP_float_divide(RAP_Value a, RAP_Value b);

// GENERIC OPERATIONS

RAP_Value RAP_less_than(RAP_Value a, RAP_Value b);
RAP_Value RAP_less_or_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_greater_than(RAP_Value a, RAP_Value b);
RAP_Value RAP_greater_or_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_not_equal(RAP_Value a, RAP_Value b);
RAP_Value RAP_modulo(RAP_Value a, RAP_Value b);
RAP_Value RAP_add(RAP_Value a, RAP_Value b);
RAP_Value RAP_subtract(RAP_Value a, RAP_Value b);
RAP_Value RAP_multiply(RAP_Value a, RAP_Value b);
RAP_Value RAP_divide(RAP_Value a, RAP_Value b);
RAP_Value RAP_length(RAP_Value a);
RAP_Value RAP_negate(RAP_Value a);
RAP_Value RAP_power(RAP_Value a, RAP_Value b);

// FRAME UTILITIES

struct RAP_CallFrame *RAP_create_call_frame(struct RAP_CallFrame *parent);
void RAP_free_call_frame(struct RAP_CallFrame *frame);
// Get/set a variable in the current frame only (свои / implicit locals)
RAP_Value RAP_frame_get(struct RAP_CallFrame *frame, const char *name);
void RAP_frame_set(struct RAP_CallFrame *frame, const char *name,
                   RAP_Value value);
// Get/set by walking up the parent chain (чужие)
RAP_Value RAP_frame_get_foreign(struct RAP_CallFrame *frame, const char *name);
void RAP_frame_set_foreign(struct RAP_CallFrame *frame, const char *name,
                           RAP_Value value);

// EXTRACTORS

// TODO: re-introduce after BigInt implementation
// #define RAP_get_int_val(obj) ((obj)->int_val)
#define RAP_GET_FLOAT_VAL(obj) (RAP_PTR_VALUE(obj)->float_val)
#define RAP_GET_TEXT_VAL(obj) (RAP_PTR_VALUE(obj)->text_val)
#define RAP_GET_TUPLE_VAL(obj) (RAP_PTR_VALUE(obj)->tuple_val)
#define RAP_GET_CALLABLE_VAL(obj) (RAP_PTR_VALUE(obj)->callable_val)
#define RAP_GET_SLICE_VAL(obj) (RAP_PTR_VALUE(obj)->slice_val)

#define RAP_IS_FLOAT(v)                                                        \
  (RAP_IS_PTR(v) && RAP_PTR_VALUE(v)->tag == RAP_OBJECT_TAG_FLOAT)
#define RAP_IS_TEXT(v)                                                         \
  (RAP_IS_PTR(v) && RAP_PTR_VALUE(v)->tag == RAP_OBJECT_TAG_TEXT)
#define RAP_IS_TUPLE(v)                                                        \
  (RAP_IS_PTR(v) && RAP_PTR_VALUE(v)->tag == RAP_OBJECT_TAG_TUPLE)
#define RAP_IS_SLICE(v)                                                        \
  (RAP_IS_PTR(v) && RAP_PTR_VALUE(v)->tag == RAP_OBJECT_TAG_SLICE)
#define RAP_IS_NULL(v)                                                         \
  (RAP_IS_PTR(v) && RAP_PTR_VALUE(v)->tag == RAP_OBJECT_TAG_NULL)

char *RAP_stringify_object(RAP_Value obj);

// BUILT-IN MATH FUNCTIONS

RAP_Value RAP_abs(RAP_Value a);
RAP_Value RAP_sqrt(RAP_Value a);
RAP_Value RAP_floor(RAP_Value a);
RAP_Value RAP_ceil(RAP_Value a);
RAP_Value RAP_round(RAP_Value a);
RAP_Value RAP_min(RAP_Value a, RAP_Value b);
RAP_Value RAP_max(RAP_Value a, RAP_Value b);
RAP_Value RAP_random(RAP_Value a);
RAP_Value RAP_random_int(RAP_Value a);
RAP_Value RAP_sign(RAP_Value a);

// INPUT

/// Read a line from stdin, return as text object.
RAP_Value RAP_input_text(void);
/// Read a line from stdin, parse as int/float/text. Returns typed object.
RAP_Value RAP_input_value(void);

// REFERENCE COUNTING

// RAP_inc_ref takes a RAP_Value, no-op for inline values (SMI, bool, double)
#define RAP_inc_ref(val)                                                       \
  do {                                                                         \
    if (RAP_IS_PTR(val))                                                       \
      RAP_PTR_VALUE(val)->refcount++;                                          \
  } while (0)

void RAP_dec_ref(RAP_Value obj);

void RAP_free_object(RAP_Object *obj);

// ALLOCATION TRACKING (test-only, compile with -DRAP_TEST_LEAKS)
#ifdef RAP_TEST_LEAKS
void RAP_check_leaks(void);
#endif

// Available for users actually
RAP_Value RAP_get_objects_refcount(RAP_Value obj);

void RAP_free_main_frame(struct RAP_CallFrame *frame);

#endif // RAPIRA_RUNTIME_H
