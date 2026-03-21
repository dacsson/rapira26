#include "runtime.h"
#include "rapobject.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>

// Fatal error - print a message and exit.
void RAP_fatal_error(const char *message) {
  fprintf(stderr, "Упс, ошибка: %s\n", message);
  exit(1);
}

// HELPERS

// Get the underlying RAP_Tuple* for both TUPLE and TEXT objects.
static struct RAP_Tuple *get_items(RAP_Object *obj) {
  if (obj->tag == RAP_OBJECT_TAG_TEXT) return obj->text_val;
  return obj->tuple_val;
}


// Decode UTF-8 string into an array of codepoint values.
// Caller must free *out_codepoints.
static size_t utf8_decode_all(const char *s, int64_t **out_codepoints) {
  size_t count = 0, cap = 16;
  int64_t *cps = malloc(cap * sizeof(int64_t));
  const unsigned char *p = (const unsigned char *)s;
  while (*p) {
    int64_t cp;
    if (*p < 0x80)      { cp = *p++; }
    else if (*p < 0xE0) { cp = (p[0] & 0x1F) << 6  | (p[1] & 0x3F); p += 2; }
    else if (*p < 0xF0) { cp = (p[0] & 0x0F) << 12 | (p[1] & 0x3F) << 6  | (p[2] & 0x3F); p += 3; }
    else                { cp = (p[0] & 0x07) << 18 | (p[1] & 0x3F) << 12 | (p[2] & 0x3F) << 6 | (p[3] & 0x3F); p += 4; }
    if (count >= cap) { cap *= 2; cps = realloc(cps, cap * sizeof(int64_t)); }
    cps[count++] = cp;
  }
  *out_codepoints = cps;
  return count;
}

// Encode a single codepoint to UTF-8, append to buffer. Returns new length.
static size_t utf8_encode_one(int64_t cp, char *buf, size_t pos) {
  if (cp < 0x80)        { buf[pos++] = cp; }
  else if (cp < 0x800)  { buf[pos++] = 0xC0 | (cp >> 6);  buf[pos++] = 0x80 | (cp & 0x3F); }
  else if (cp < 0x10000){ buf[pos++] = 0xE0 | (cp >> 12); buf[pos++] = 0x80 | ((cp >> 6) & 0x3F); buf[pos++] = 0x80 | (cp & 0x3F); }
  else                  { buf[pos++] = 0xF0 | (cp >> 18); buf[pos++] = 0x80 | ((cp >> 12) & 0x3F); buf[pos++] = 0x80 | ((cp >> 6) & 0x3F); buf[pos++] = 0x80 | (cp & 0x3F); }
  return pos;
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
  obj->text_val = malloc(sizeof(struct RAP_Tuple));
  // Decode UTF-8 into codepoints — one codepoint per tuple item
  int64_t *codepoints;
  size_t count = utf8_decode_all(value, &codepoints);
  obj->text_val->count = count;
  obj->text_val->items = malloc(count * sizeof(RAP_Object *));
  for (size_t i = 0; i < count; i++) {
    obj->text_val->items[i] = RAP_create_int_obj(codepoints[i]);
  }
  free(codepoints);
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

RAP_Object *RAP_set_tuple_item(RAP_Object *container, uint32_t index,
                               RAP_Object *item) {
  if (container->tag != RAP_OBJECT_TAG_TUPLE && container->tag != RAP_OBJECT_TAG_TEXT) {
    RAP_fatal_error("Объект не является кортежем или текстом");
    return container;
  }

  if (container->tag == RAP_OBJECT_TAG_TEXT) {
    // When assigning to a text element, unwrap single-char TEXT to its codepoint int
    if (item->tag == RAP_OBJECT_TAG_TEXT && RAP_get_text_val(item)->count == 1) {
      item = RAP_get_text_val(item)->items[0];
    }
    container->text_val->items[index] = item;
  } else {
    container->tuple_val->items[index] = item;
  }
  return container;
}

RAP_Object *RAP_get_tuple_item(RAP_Object *container, uint32_t index) {
  if (container->tag != RAP_OBJECT_TAG_TUPLE && container->tag != RAP_OBJECT_TAG_TEXT) {
    RAP_fatal_error("Объект не является кортежем или текстом");
    return container;
  }

  if (container->tag == RAP_OBJECT_TAG_TEXT) {
    // Return a single-character TEXT wrapping the codepoint
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = 1;
    result->text_val->items = malloc(sizeof(RAP_Object *));
    result->text_val->items[0] = RAP_get_text_val(container)->items[index];
    return result;
  }
  return container->tuple_val->items[index];
}

RAP_Object *RAP_append_tuple(RAP_Object *a, RAP_Object *b) {
  if (a->tag != RAP_OBJECT_TAG_TUPLE || b->tag != RAP_OBJECT_TAG_TUPLE) {
    RAP_fatal_error("Оба объекта должны быть кортежами");
    return a;
  }

  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_TUPLE;
  obj->tuple_val = malloc(sizeof(struct RAP_Tuple));
  obj->tuple_val->count = a->tuple_val->count + b->tuple_val->count;
  obj->tuple_val->items = malloc(obj->tuple_val->count * sizeof(RAP_Object *));
  for (uint32_t i = 0; i < a->tuple_val->count; i++) {
    obj->tuple_val->items[i] = a->tuple_val->items[i];
  }
  for (uint32_t i = 0; i < b->tuple_val->count; i++) {
    obj->tuple_val->items[a->tuple_val->count + i] = b->tuple_val->items[i];
  }
  return obj;
}

// индекс(needle, haystack) — search for element in tuple or substring in text.
// Returns 0-based position, or -1 if not found.
// Spec uses 1-based and returns 0 for not found; we deviate intentionally (see PHASE1_DIFFERENCE.md).
RAP_Object *RAP_index_of(RAP_Object *needle, RAP_Object *haystack) {
  // Default tuple search
  if (haystack->tag == RAP_OBJECT_TAG_TUPLE) {
    for (uint32_t i = 0; i < haystack->tuple_val->count; i++) {
      if (RAP_equal(needle, haystack->tuple_val->items[i])->logical_val) {
        return RAP_create_int_obj(i);
      }
    }
    return RAP_create_int_obj(-1);
  }
  // Special case for strings
  if (haystack->tag == RAP_OBJECT_TAG_TEXT && needle->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *h = RAP_get_text_val(haystack);
    struct RAP_Tuple *n = RAP_get_text_val(needle);
    if (n->count == 0) return RAP_create_int_obj(0);
    if (n->count > h->count) return RAP_create_int_obj(-1);
    for (uint32_t i = 0; i <= h->count - n->count; i++) {
      bool match = true;
      for (uint32_t j = 0; j < n->count; j++) {
        if (RAP_get_int_val(h->items[i + j]) != RAP_get_int_val(n->items[j])) {
          match = false;
          break;
        }
      }
      if (match) return RAP_create_int_obj(i);
    }
    return RAP_create_int_obj(-1);
  }
  // Materialize slices and recurse
  if (haystack->tag == RAP_OBJECT_TAG_SLICE || needle->tag == RAP_OBJECT_TAG_SLICE) {
    return RAP_index_of(RAP_materialize_slice(needle), RAP_materialize_slice(haystack));
  }
  printf("%s %d %d\n", RAP_stringify_object(needle), needle->tag, haystack->tag);
  RAP_fatal_error("Неподдерживаемые типы для индекс()");
}


// SLICE OPERATIONS

RAP_Object *RAP_create_slice(RAP_Object *parent, int64_t from, int64_t to) {
  // Flatten: if parent is already a slice, resolve to the root parent
  if (parent->tag == RAP_OBJECT_TAG_SLICE) {
    from += parent->slice_val->from;
    to += parent->slice_val->from;
    parent = parent->slice_val->parent;
  }

  // Clamp bounds
  uint32_t count = get_items(parent)->count;
  if (from < 0) from = 0;
  if (to > count) to = count;
  if (from > to) from = to;

  // For slices that are actually just a single item, expand so we include just the item itself
  if (from == to) to += 1;

  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_SLICE;
  obj->slice_val = malloc(sizeof(struct RAP_Slice));
  obj->slice_val->parent = parent;
  obj->slice_val->from = from;
  obj->slice_val->to = to;
  return obj;
}

// Turn a slice into a real tuple/text (copy). If not a slice, return as-is.
RAP_Object *RAP_materialize_slice(RAP_Object *obj) {
  if (obj->tag != RAP_OBJECT_TAG_SLICE) return obj;

  RAP_Object *parent = obj->slice_val->parent;
  int64_t from = obj->slice_val->from;
  int64_t to = obj->slice_val->to;
  uint32_t new_count = (from < to) ? (to - from) : 0;
  struct RAP_Tuple *parent_items = get_items(parent);
  bool is_text = (parent->tag == RAP_OBJECT_TAG_TEXT);

  if (new_count == 0) {
    if (is_text) return RAP_create_text_obj("");
    return RAP_create_tuple_obj(0, NULL);
  }

  RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
  for (uint32_t i = 0; i < new_count; i++) {
    items[i] = parent_items->items[from + i];
  }

  if (is_text) {
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return result;
  }
  RAP_Object *result = RAP_create_tuple_obj(new_count, items);
  free(items);
  return result;
}

// Replace parent[from:to] with replacement items.
// Modifies the parent in-place.
void RAP_slice_assign(RAP_Object *slice, RAP_Object *replacement) {
  if (slice->tag != RAP_OBJECT_TAG_SLICE) {
    RAP_fatal_error("Присваивание среза не-срезу");
  }
  RAP_Object *parent = slice->slice_val->parent;
  int64_t from = slice->slice_val->from;
  int64_t to = slice->slice_val->to;

  // Materialize replacement if it's a slice
  replacement = RAP_materialize_slice(replacement);

  struct RAP_Tuple *parent_data = get_items(parent);
  struct RAP_Tuple *rep_data = get_items(replacement);

  uint32_t old_count = parent_data->count;
  uint32_t removed = (from < to) ? (to - from) : 0;
  uint32_t rep_count = rep_data->count;
  uint32_t new_count = old_count - removed + rep_count;

  RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
  for (uint32_t i = 0; i < (uint32_t)from; i++) {
    items[i] = parent_data->items[i];
  }
  for (uint32_t i = 0; i < rep_count; i++) {
    items[from + i] = rep_data->items[i];
  }
  for (uint32_t i = (uint32_t)to; i < old_count; i++) {
    items[from + rep_count + (i - to)] = parent_data->items[i];
  }

  free(parent_data->items);
  parent_data->items = items;
  parent_data->count = new_count;
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
  frame->slots = NULL;
  frame->slot_count = 0;
  return frame;
}

// Find slot index by name in a single frame. Returns -1 if not found.
static int frame_find_slot(struct RAP_CallFrame *frame, const char *name) {
  for (uint32_t i = 0; i < frame->slot_count; i++) {
    if (strcmp(frame->slots[i].name, name) == 0) return (int)i;
  }
  return -1;
}

// Get variable from current frame only (свои / implicit locals).
// Returns пусто if not found.
RAP_Object *RAP_frame_get(struct RAP_CallFrame *frame, const char *name) {
  int idx = frame_find_slot(frame, name);
  if (idx >= 0) return frame->slots[idx].value;
  return RAP_create_null_obj();
}

// Set variable in current frame (creates slot if not found).
void RAP_frame_set(struct RAP_CallFrame *frame, const char *name, RAP_Object *value) {
  int idx = frame_find_slot(frame, name);
  if (idx >= 0) {
    frame->slots[idx].value = value;
    return;
  }
  frame->slot_count++;
  frame->slots = realloc(frame->slots, frame->slot_count * sizeof(struct RAP_FrameSlot));
  frame->slots[frame->slot_count - 1].name = name;
  frame->slots[frame->slot_count - 1].value = value;
}

// Get variable by walking up the parent chain (чужие).
// Returns пусто if not found in any frame.
RAP_Object *RAP_frame_get_foreign(struct RAP_CallFrame *frame, const char *name) {
  struct RAP_CallFrame *current = frame->parent;
  while (current) {
    int idx = frame_find_slot(current, name);
    if (idx >= 0) return current->slots[idx].value;
    current = current->parent;
  }
  return RAP_create_null_obj();
}

// Set variable by walking up the parent chain (чужие).
// If found in a parent, updates it there. Otherwise creates in the immediate parent.
void RAP_frame_set_foreign(struct RAP_CallFrame *frame, const char *name, RAP_Object *value) {
  struct RAP_CallFrame *current = frame->parent;
  while (current) {
    int idx = frame_find_slot(current, name);
    if (idx >= 0) {
      current->slots[idx].value = value;
      return;
    }
    current = current->parent;
  }
  // Not found anywhere — create in immediate parent
  if (frame->parent) {
    RAP_frame_set(frame->parent, name, value);
  }
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
    // If the float value has only zeros after the decimal point, print it WITH zero after dot
    double integral_part;
    double fractional_part = modf(RAP_get_float_val(obj), &integral_part);
    double abs_fractional_part = fabs(fractional_part);
    bool has_only_zeros = abs_fractional_part < DBL_EPSILON;
    if (has_only_zeros) {
      size_t needed_size = snprintf(NULL, 0, "%.1f", RAP_get_float_val(obj));
      char *str = malloc(needed_size + 1);
      snprintf(str, needed_size + 1, "%.1f", RAP_get_float_val(obj));
      return str;
    }

    // Use full double precision (16 significant digits)
    char tmp[64];
    snprintf(tmp, sizeof(tmp), "%.16g", RAP_get_float_val(obj));
    return strdup(tmp);
  }
  case RAP_OBJECT_TAG_TEXT: {
    uint32_t count = RAP_get_text_val(obj)->count;
    if (count == 0) return strdup("");
    // Each codepoint needs up to 4 bytes in UTF-8
    char *buf = malloc(count * 4 + 1);
    size_t pos = 0;
    for (uint32_t i = 0; i < count; i++) {
      int64_t cp = RAP_get_int_val(RAP_get_text_val(obj)->items[i]);
      pos = utf8_encode_one(cp, buf, pos);
    }
    buf[pos] = '\0';
    return buf;
  }
  case RAP_OBJECT_TAG_TUPLE: {
    // Empty tuple if count is 0
    if (obj->tuple_val->count == 0) {
      return strdup("<* *>");
    }

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
  case RAP_OBJECT_TAG_SLICE: {
    // Materialize and stringify
    RAP_Object *materialized = RAP_materialize_slice(obj);
    return RAP_stringify_object(materialized);
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
  // Spec §2.2.4.2: remainder = A - A//B * B (where // is floor division)
  int64_t av = RAP_get_int_val(a);
  int64_t bv = RAP_get_int_val(b);
  int64_t quotient = (int64_t)floor((double)av / bv);
  return RAP_create_int_obj(av - quotient * bv);
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
  return RAP_create_int_obj(floor((float) RAP_get_int_val(a) / RAP_get_int_val(b)));
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
  // TUPLE APPEND
  if (a->tag == RAP_OBJECT_TAG_TUPLE && b->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_append_tuple(a, b);
  }
  // TEXT CONCAT
  if (a->tag == RAP_OBJECT_TAG_TEXT && b->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *at = RAP_get_text_val(a);
    struct RAP_Tuple *bt = RAP_get_text_val(b);
    uint32_t new_count = at->count + bt->count;
    RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
    for (uint32_t i = 0; i < at->count; i++) items[i] = at->items[i];
    for (uint32_t i = 0; i < bt->count; i++) items[at->count + i] = bt->items[i];
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return result;
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
  // Tuple repeat: tuple * int
  if (a->tag == RAP_OBJECT_TAG_TUPLE && b->tag == RAP_OBJECT_TAG_INT) {
    int64_t n = RAP_get_int_val(b);
    if (n <= 0) return RAP_create_tuple_obj(0, NULL);
    uint32_t src_count = a->tuple_val->count;
    uint32_t new_count = src_count * n;
    RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
    for (int64_t rep = 0; rep < n; rep++) {
      for (uint32_t i = 0; i < src_count; i++) {
        items[rep * src_count + i] = a->tuple_val->items[i];
      }
    }
    RAP_Object *result = RAP_create_tuple_obj(new_count, items);
    free(items);
    return result;
  }
  // Text repeat: text * int or int * text
  RAP_Object *text_obj = NULL;
  int64_t repeat_n = 0;
  if (a->tag == RAP_OBJECT_TAG_TEXT && b->tag == RAP_OBJECT_TAG_INT) {
    text_obj = a; repeat_n = RAP_get_int_val(b);
  } else if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_TEXT) {
    text_obj = b; repeat_n = RAP_get_int_val(a);
  }
  if (text_obj) {
    struct RAP_Tuple *src = RAP_get_text_val(text_obj);
    if (repeat_n <= 0 || src->count == 0) {
      return RAP_create_text_obj("");
    }
    uint32_t new_count = src->count * repeat_n;
    RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
    for (int64_t rep = 0; rep < repeat_n; rep++) {
      for (uint32_t i = 0; i < src->count; i++) {
        items[rep * src->count + i] = src->items[i];
      }
    }
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return result;
  }
  printf("%d %d\n", a->tag, b->tag);
  RAP_fatal_error("Неподдерживаемые типы для умножения");
}

RAP_Object *RAP_divide(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    // Spec §2.2.4.2: int/int returns int if exact, real otherwise
    int64_t av = RAP_get_int_val(a);
    int64_t bv = RAP_get_int_val(b);
    if (bv != 0 && av % bv == 0) {
      return RAP_create_int_obj(av / bv);
    }
    return RAP_create_float_obj((double)av / (double)bv);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    if (b->tag == RAP_OBJECT_TAG_INT) {
      return RAP_create_float_obj(RAP_get_float_val(a) / (double)RAP_get_int_val(b));
    } else if (b->tag == RAP_OBJECT_TAG_FLOAT) {
      return RAP_float_divide(a, b);
    }
  } else if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj((double)RAP_get_int_val(a) / RAP_get_float_val(b));
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

RAP_Object *RAP_less_or_equal(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_logical_obj(a->int_val <= b->int_val);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_logical_obj(a->float_val <= b->float_val);
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

RAP_Object *RAP_greater_or_equal(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_logical_obj(a->int_val >= b->int_val);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_logical_obj(a->float_val >= b->float_val);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_equal(RAP_Object *a, RAP_Object *b) {
  // Materialize slices before comparing
  a = RAP_materialize_slice(a);
  b = RAP_materialize_slice(b);

  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_equal(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_equal(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_TEXT && b->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *at = RAP_get_text_val(a);
    struct RAP_Tuple *bt = RAP_get_text_val(b);
    if (at->count != bt->count) return RAP_create_logical_obj(false);
    for (uint32_t i = 0; i < at->count; i++) {
      if (RAP_get_int_val(at->items[i]) != RAP_get_int_val(bt->items[i]))
        return RAP_create_logical_obj(false);
    }
    return RAP_create_logical_obj(true);
  } else if (a->tag == RAP_OBJECT_TAG_NULL && b->tag == RAP_OBJECT_TAG_NULL) {
    return RAP_create_logical_obj(true);
  } else if (a->tag == RAP_OBJECT_TAG_LOGICAL && b->tag == RAP_OBJECT_TAG_LOGICAL) {
    return RAP_create_logical_obj(a->logical_val == b->logical_val);
  } else if (a->tag == RAP_OBJECT_TAG_TUPLE && b->tag == RAP_OBJECT_TAG_TUPLE) {
    if (a->tuple_val->count != b->tuple_val->count) {
      return RAP_create_logical_obj(false);
    }
    for (uint32_t i = 0; i < a->tuple_val->count; i++) {
      if (!RAP_equal(a->tuple_val->items[i], b->tuple_val->items[i])->logical_val) {
        return RAP_create_logical_obj(false);
      }
    }
    return RAP_create_logical_obj(true);
  }
  printf("%s %d %d\n", RAP_stringify_object(a), a->tag, b->tag);
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_not_equal(RAP_Object *a, RAP_Object *b) {
  // Reuse RAP_equal and negate
  return RAP_create_logical_obj(!RAP_equal(a, b)->logical_val);
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
  // Text stores one codepoint per item, so count == character length
  if (a->tag == RAP_OBJECT_TAG_TEXT) {
    return RAP_create_int_obj(RAP_get_text_val(a)->count);
  }
  if (a->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_create_int_obj(RAP_get_tuple_val(a)->count);
  }
  if (a->tag == RAP_OBJECT_TAG_SLICE) {
    return RAP_create_int_obj(a->slice_val->to - a->slice_val->from);
  }
  RAP_fatal_error("Неподдерживаемые типы для длины");
}

RAP_Object *RAP_power(RAP_Object *a, RAP_Object *b) {
  bool result_is_float = a->tag == RAP_OBJECT_TAG_FLOAT || b->tag == RAP_OBJECT_TAG_FLOAT;
  if (b->tag != RAP_OBJECT_TAG_INT && b->tag != RAP_OBJECT_TAG_FLOAT) {
    RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
  }
  double power_value = b->tag == RAP_OBJECT_TAG_INT ? RAP_get_int_val(b) : RAP_get_float_val(b);
  if (a->tag == RAP_OBJECT_TAG_INT) {
    if (result_is_float) {
      return RAP_create_float_obj(pow(RAP_get_int_val(a), power_value));
    } else {
      return RAP_create_int_obj(pow(RAP_get_int_val(a), power_value));
    }
  }
  else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    if (result_is_float) {
      return RAP_create_float_obj(pow(RAP_get_float_val(a), power_value));
    } else {
      return RAP_create_int_obj(pow(RAP_get_float_val(a), power_value));
    }
  }

  RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
}

// BUILT-IN MATH FUNCTIONS

RAP_Object *RAP_abs(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(abs(RAP_get_int_val(a)));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(fabs(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для модуля");
}

RAP_Object *RAP_sqrt(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT || a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(sqrt(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для квадратного корня");
}

RAP_Object *RAP_floor(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_int_obj((int64_t)floor(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления вниз");
}

RAP_Object *RAP_ceil(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(ceil(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления вверх");
}

RAP_Object *RAP_round(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_int_obj((int64_t)round(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления");
}

RAP_Object *RAP_min(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(RAP_get_int_val(a) < RAP_get_int_val(b) ? RAP_get_int_val(a) : RAP_get_int_val(b));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(RAP_get_float_val(a) < RAP_get_float_val(b) ? RAP_get_float_val(a) : RAP_get_float_val(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для min");
}

RAP_Object *RAP_max(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(RAP_get_int_val(a) > RAP_get_int_val(b) ? RAP_get_int_val(a) : RAP_get_int_val(b));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(RAP_get_float_val(a) > RAP_get_float_val(b) ? RAP_get_float_val(a) : RAP_get_float_val(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для max");
}

RAP_Object *RAP_random(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj((double)rand() / RAND_MAX * RAP_get_float_val(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для random");
}

RAP_Object *RAP_random_int(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(rand() % RAP_get_int_val(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для random_int");
}

RAP_Object *RAP_sign(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    int64_t v = RAP_get_int_val(a);
    return RAP_create_int_obj(v < 0 ? -1 : v > 0 ? 1 : 0);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    double v = RAP_get_float_val(a);
    return RAP_create_int_obj(v < 0.0 ? -1 : v > 0.0 ? 1 : 0);
  }
  RAP_fatal_error("Неподдерживаемые типы для sign");
}
