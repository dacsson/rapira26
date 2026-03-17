#include "runtime.h"
#include "rapobject.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

// Fatal error - print a message and exit.
void RAP_fatal_error(const char *message) {
  fprintf(stderr, "Упс, ошибка: %s\n", message);
  exit(1);
}

// CONSTRUCTORS

RAP_Object *RAP_create_null_obj(void) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_NULL;
  return obj;
}

RAP_Object *RAP_create_int_obj(int64_t value) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_INT;
  obj->int_val = value;
  return obj;
}

RAP_Object *RAP_create_float_obj(double value) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_FLOAT;
  obj->float_val = value;
  return obj;
}

RAP_Object *RAP_create_text_obj(const char *value) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_TEXT;
  obj->text_val = malloc(strlen(value) + 1);
  strcpy(obj->text_val, value);
  return obj;
}

RAP_Object *RAP_create_logical_obj(bool value) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_LOGICAL;
  obj->logical_val = value;
  return obj;
}

RAP_Object *RAP_create_tuple_obj(uint32_t count, RAP_Object **items) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_TUPLE;
  obj->tuple_val = malloc(sizeof(struct RAP_Tuple));
  obj->tuple_val->count = count;
  obj->tuple_val->items = malloc(count * sizeof(RAP_Object *));
  for (uint32_t i = 0; i < count; i++) {
    obj->tuple_val->items[i] = items[i];
  }
  return obj;
}

// TUPLE UTILITIES

RAP_Object *RAP_set_tuple_item(RAP_Object *tuple, uint32_t index,
                               RAP_Object *item) {
  tuple->tuple_val->items[index] = item;
  return tuple;
}

RAP_Object *RAP_get_tuple_item(RAP_Object *tuple, uint32_t index) {
  return tuple->tuple_val->items[index];
}

// CALLABLE (FUNC AND PROC) UTILITIES

RAP_Object *RAP_create_callable_obj(struct RAP_CallFrame *frame_parent,
                                    RAP_FunctionDecl func,
                                    RAP_Parameter **params,
                                    uint32_t params_count) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_CALLABLE;
  obj->callable_val = malloc(sizeof(struct RAP_Callable));

  // Setup frame, initially it only has a pointer to the parent frame,
  // so we can walk up the call stack to find the enclosing scope variables
  obj->callable_val->frame = RAP_create_call_frame(frame_parent);

  obj->callable_val->func = func;
  obj->callable_val->params = params;
  obj->callable_val->param_count = params_count;
  return obj;
}

RAP_Parameter *RAP_create_parameter(RAP_ParameterMode mode, const char *name) {
  RAP_Parameter *param = malloc(sizeof(RAP_Parameter));
  param->mode = mode;
  param->name = malloc(strlen(name) + 1);
  strcpy(param->name, name);
  return param;
}

struct RAP_CallFrame *RAP_create_call_frame(struct RAP_CallFrame *parent) {
  struct RAP_CallFrame *frame = malloc(sizeof(struct RAP_CallFrame));
  frame->parent = parent;
  frame->locals = NULL;
  frame->locals_count = 0;
  return frame;
}

void RAP_add_local(struct RAP_CallFrame *frame, const char *name,
                   RAP_Object *value) {
  if (frame->locals == NULL) {
    frame->locals = malloc(sizeof(RAP_Object *));
    frame->locals_count = 1;
  } else {
    frame->locals_count++;
    frame->locals =
        realloc(frame->locals, frame->locals_count * sizeof(RAP_Object *));
  }

  frame->locals[frame->locals_count - 1] = value;
}

RAP_Object *RAP_call_callable_obj(RAP_Object *callable, RAP_Object **args,
                                  uint32_t arg_count) {
  return callable->callable_val->func(callable->callable_val->frame, args,
                                      arg_count);
}

void RAP_free_call_frame(struct RAP_CallFrame *frame) { free(frame); }

/// Helper: dynamically builds a string by appending to a buffer.
static char *strbuf_append(char *buf, size_t *len, size_t *cap,
                           const char *append) {
  size_t append_len = strlen(append);
  while (*len + append_len + 1 > *cap) {
    *cap = (*cap == 0) ? 64 : *cap * 2;
    buf = realloc(buf, *cap);
  }
  memcpy(buf + *len, append, append_len + 1);
  *len += append_len;
  return buf;
}

char *RAP_stringify_object(RAP_Object *obj) {
  switch (obj->tag) {
  case RAP_OBJECT_TAG_NULL: {
    return strdup("пусто");
  }
  case RAP_OBJECT_TAG_LOGICAL: {
    return strdup(obj->logical_val ? "да" : "нет");
  }
  case RAP_OBJECT_TAG_INT: {
    size_t needed_size = snprintf(NULL, 0, "%ld", (long)RAP_get_int_val(obj));
    char *str = malloc(needed_size + 1);
    snprintf(str, needed_size + 1, "%ld", (long)RAP_get_int_val(obj));
    return str;
  }
  case RAP_OBJECT_TAG_FLOAT: {
    size_t needed_size = snprintf(NULL, 0, "%g", RAP_get_float_val(obj));
    char *str = malloc(needed_size + 1);
    snprintf(str, needed_size + 1, "%g", RAP_get_float_val(obj));
    return str;
  }
  case RAP_OBJECT_TAG_TEXT: {
    char *str = malloc(strlen(RAP_get_text_val(obj)) + 1);
    strcpy(str, RAP_get_text_val(obj));
    return str;
  }
  case RAP_OBJECT_TAG_TUPLE: {
    size_t len = 0, cap = 0;
    char *buf = NULL;
    buf = strbuf_append(buf, &len, &cap, "<* ");
    for (uint32_t i = 0; i < obj->tuple_val->count; i++) {
      if (i > 0) {
        buf = strbuf_append(buf, &len, &cap, ", ");
      }
      char *item_str = RAP_stringify_object(obj->tuple_val->items[i]);
      buf = strbuf_append(buf, &len, &cap, item_str);
      free(item_str);
    }
    buf = strbuf_append(buf, &len, &cap, " *>");
    return buf;
  }
  case RAP_OBJECT_TAG_CALLABLE: {
    char *str = malloc(strlen("<callable>") + 1);
    strcpy(str, "<callable>");
    return str;
  }
  default: {
    char *str = malloc(strlen("<unknown>") + 1);
    strcpy(str, "<unknown>");
    return str;
  }
  }
}

// Integer operations

RAP_Object *RAP_integer_less_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) < RAP_get_int_val(b));
}

RAP_Object *RAP_integer_greater_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) > RAP_get_int_val(b));
}

RAP_Object *RAP_integer_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) == RAP_get_int_val(b));
}

RAP_Object *RAP_integer_not_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) != RAP_get_int_val(b));
}

RAP_Object *RAP_integer_modulo(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) % RAP_get_int_val(b));
}

RAP_Object *RAP_integer_add(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) + RAP_get_int_val(b));
}

RAP_Object *RAP_integer_subtract(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) - RAP_get_int_val(b));
}

RAP_Object *RAP_integer_multiply(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) * RAP_get_int_val(b));
}

RAP_Object *RAP_integer_divide(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) / RAP_get_int_val(b));
}

// Float operations

RAP_Object *RAP_float_less_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) < RAP_get_float_val(b));
}

RAP_Object *RAP_float_greater_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) > RAP_get_float_val(b));
}

RAP_Object *RAP_float_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) == RAP_get_float_val(b));
}

RAP_Object *RAP_float_not_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) != RAP_get_float_val(b));
}

RAP_Object *RAP_float_add(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) + RAP_get_float_val(b));
}

RAP_Object *RAP_float_subtract(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) - RAP_get_float_val(b));
}

RAP_Object *RAP_float_multiply(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) * RAP_get_float_val(b));
}

RAP_Object *RAP_float_divide(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) / RAP_get_float_val(b));
}

// Generic operations

RAP_Object *RAP_add(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_add(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_add(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сложения");
}

RAP_Object *RAP_subtract(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_subtract(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_subtract(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для вычитания");
}

RAP_Object *RAP_multiply(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_multiply(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_multiply(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для умножения");
}

RAP_Object *RAP_divide(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_divide(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_divide(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для деления");
}

RAP_Object *RAP_less_than(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_less_than(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_less_than(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_greater_than(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_greater_than(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_greater_than(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_equal(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_equal(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_equal(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_not_equal(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_not_equal(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_not_equal(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_modulo(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_modulo(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для модуля");
}

RAP_Object *RAP_negate(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(-1 * RAP_get_int_val(a));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(-1.0 * RAP_get_float_val(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для отрицания");
}

RAP_Object *RAP_length(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_create_int_obj(RAP_get_tuple_val(a)->count);
  }
  else if (a->tag == RAP_OBJECT_TAG_TEXT) {
    return RAP_create_int_obj(strlen(RAP_get_text_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для длины");
}

RAP_Object *RAP_power(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_float_obj(pow(RAP_get_int_val(a), RAP_get_int_val(b)));
  }
  RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
}
