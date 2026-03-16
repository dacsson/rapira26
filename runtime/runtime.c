#include "runtime.h"
#include "rapobject.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

RAP_Object *RAP_set_tuple_item(RAP_Object *tuple, uint32_t index,
                               RAP_Object *item) {
  tuple->tuple_val->items[index] = item;
  return tuple;
}

RAP_Object *RAP_get_tuple_item(RAP_Object *tuple, uint32_t index) {
  return tuple->tuple_val->items[index];
}

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

RAP_Object *RAP_create_logical_obj(bool value) {
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_LOGICAL;
  obj->logical_val = value;
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

RAP_Object *RAP_integer_less_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) < RAP_get_int_val(b));
}

RAP_Object *RAP_integer_greater_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) > RAP_get_int_val(b));
}

RAP_Object *RAP_integer_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) == RAP_get_int_val(b));
}

RAP_Object *RAP_integer_modulo(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) % RAP_get_int_val(b));
}

RAP_Object *RAP_call_callable_obj(RAP_Object *callable, RAP_Object **args,
                                  uint32_t arg_count) {
  return callable->callable_val->func(callable->callable_val->frame, args,
                                      arg_count);
}
