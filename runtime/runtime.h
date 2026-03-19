#ifndef RAPIRA_RUNTIME_H
#define RAPIRA_RUNTIME_H

#include "rapobject.h"

// CONSTRUCTORS

RAP_Object *RAP_create_null_obj(void);
RAP_Object *RAP_create_int_obj(int64_t value);
RAP_Object *RAP_create_float_obj(double value);
RAP_Object *RAP_create_text_obj(const char *value);
RAP_Object *RAP_create_tuple_obj(uint32_t count, RAP_Object **items);
RAP_Object *RAP_create_callable_obj(struct RAP_CallFrame *frame_parent,
                                    RAP_FunctionDecl func,
                                    RAP_Parameter **params,
                                    uint32_t params_count);
RAP_Parameter *RAP_create_parameter(RAP_ParameterMode mode, const char *name);
RAP_Object *RAP_create_logical_obj(bool value);

// OBJECTS UTILITIES

RAP_Object *RAP_call_callable_obj(RAP_Object *callable, RAP_Object **args,
                                  uint32_t arg_count);
RAP_Object *RAP_get_tuple_item(RAP_Object *tuple, uint32_t index);
RAP_Object *RAP_set_tuple_item(RAP_Object *tuple, uint32_t index,
                               RAP_Object *value);

// INTEGER OPERATIONS

RAP_Object *RAP_integer_less_than(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_greater_than(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_not_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_modulo(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_add(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_subtract(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_multiply(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_integer_divide(RAP_Object *a, RAP_Object *b);

// FLOAT OPERATIONS

RAP_Object *RAP_float_less_than(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_greater_than(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_not_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_modulo(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_add(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_subtract(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_multiply(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_float_divide(RAP_Object *a, RAP_Object *b);

// GENERIC OPERATIONS

RAP_Object *RAP_less_than(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_less_or_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_greater_than(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_greater_or_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_not_equal(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_modulo(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_add(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_subtract(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_multiply(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_divide(RAP_Object *a, RAP_Object *b);
RAP_Object *RAP_length(RAP_Object *a);
RAP_Object *RAP_negate(RAP_Object *a);
RAP_Object *RAP_power(RAP_Object *a, RAP_Object *b);

// FRAME UTILITIES

struct RAP_CallFrame *RAP_create_call_frame(struct RAP_CallFrame *parent);
void RAP_add_local(struct RAP_CallFrame *frame, const char *name,
                   RAP_Object *value);
void RAP_free_call_frame(struct RAP_CallFrame *frame);

// EXTRACTORS

#define RAP_get_int_val(obj) ((obj)->int_val)
#define RAP_get_float_val(obj) ((obj)->float_val)
#define RAP_get_text_val(obj) ((obj)->text_val)
#define RAP_get_tuple_val(obj) ((obj)->tuple_val)
#define RAP_get_callable_val(obj) ((obj)->callable_val)

char *RAP_stringify_object(RAP_Object *obj);

#endif // RAPIRA_RUNTIME_H
