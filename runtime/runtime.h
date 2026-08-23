#ifndef RAPIRA_RUNTIME_H
#define RAPIRA_RUNTIME_H

#include "rapobject.h"
#include "rapvalue.h"

// CONSTRUCTORS

RAP_Value RAP_create_null_obj(void);
RAP_Value RAP_create_int_obj(int64_t value);
RAP_Value RAP_create_float_obj(double value);
RAP_Value RAP_create_text_obj(const char *value);
RAP_Value RAP_create_tuple_obj(uint32_t count, RAP_Value *items);
RAP_Value RAP_create_callable_obj(size_t offset_or_ptr, uint32_t arity,
                                  const RAP_Value *captures,
                                  uint32_t capture_count);
RAP_Value RAP_create_logical_obj(bool value);
RAP_Value RAP_create_custom_typed_obj(const char *name,
                                      const char **field_names,
                                      size_t field_count, void *value);

// OBJECTS UTILITIES

RAP_Value RAP_get_tuple_item(RAP_Value tuple, uint32_t index);
RAP_Value RAP_set_tuple_item(RAP_Value tuple, uint32_t index, RAP_Value value);
// Joins two tuples into a new tuple
RAP_Value RAP_append_tuple(RAP_Object *a, RAP_Object *b);
RAP_Value RAP_index_of(RAP_Value needle, RAP_Value haystack);
RAP_Value RAP_get_variant_field(RAP_Value val, const char *field_name);
RAP_Value RAP_get_variant_field_at(RAP_Value val, size_t index);
void RAP_set_variant_field(RAP_Value val, const char *field_name,
                           RAP_Value field_val);
void RAP_set_variant_field_at(RAP_Value val, size_t index, RAP_Value field_val);

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
/// Returns float obj
RAP_Value RAP_divide(RAP_Value a, RAP_Value b);
/// An integer division, returns int object
RAP_Value RAP_floor_divide(RAP_Value a, RAP_Value b);
RAP_Value RAP_length(RAP_Value a);
RAP_Value RAP_negate(RAP_Value a);
RAP_Value RAP_power(RAP_Value a, RAP_Value b);
RAP_Value RAP_and(RAP_Value a, RAP_Value b);
RAP_Value RAP_or(RAP_Value a, RAP_Value b);
RAP_Value RAP_not(RAP_Value a);

// EXTRACTORS

// TODO: re-introduce after BigInt implementation
// #define RAP_get_int_val(obj) ((obj)->int_val)
#define RAP_GET_FLOAT_VAL(obj) (RAP_PTR_VALUE(obj)->float_val)
#define RAP_GET_TEXT_VAL(obj) (RAP_PTR_VALUE(obj)->text_val)
#define RAP_GET_TUPLE_VAL(obj) (RAP_PTR_VALUE(obj)->tuple_val)
#define RAP_GET_CALLABLE_VAL(obj) (RAP_PTR_VALUE(obj)->callable_val)
#define RAP_GET_SLICE_VAL(obj) (RAP_PTR_VALUE(obj)->slice_val)
#define RAP_GET_VARIANT_VAL(obj) (RAP_PTR_VALUE(obj)->variant_val)

bool RAP_IS_FLOAT(RAP_Value v);
bool RAP_IS_TEXT(RAP_Value v);
bool RAP_IS_TUPLE(RAP_Value v);
bool RAP_IS_SLICE(RAP_Value v);
bool RAP_IS_NULL(RAP_Value v);
bool RAP_IS_VARIANT(RAP_Value v);
bool RAP_IS_CALLABLE(RAP_Value v);

size_t RAP_get_callable_offset_or_ptr(RAP_Value callable);
uint32_t RAP_get_callable_arity(RAP_Value callable);

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

// CUSTOM TYPES

uint16_t RAP_get_variant_tag(RAP_Value val);

#define RAP_GET_FIELD(typename, val, field)                                    \
  ((typename *)RAP_PTR_VALUE(val)->variant_val->payload)->field

// REFERENCE COUNTING

// RAP_inc_ref takes a RAP_Value, no-op for inline values (SMI, bool)
void RAP_inc_ref(RAP_Value obj);

void RAP_dec_ref(RAP_Value obj);

void RAP_free_object(RAP_Object *obj);

// ALLOCATION TRACKING (test-only, compile with -DRAP_TEST_LEAKS)
#ifdef RAP_TEST_LEAKS
void RAP_check_leaks(void);
#endif

// Available for users actually
RAP_Value RAP_get_objects_refcount(RAP_Value obj);

const char *RAP_get_type_name(RAP_Value val);

const char *RAP_type_to_string(RAP_ObjectTag tag);

#endif // RAPIRA_RUNTIME_H
