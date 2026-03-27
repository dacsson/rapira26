#include "rapobject.h"
#include "rapvalue.h"
#include "runtime.h"
#include "runtime_internal.h"

// Fatal error - print a message and exit.
void RAP_fatal_error(const char *message) {
  fprintf(stderr, "Упс, ошибка: %s\n", message);
  exit(1);
}

// HELPERS

struct RAP_Tuple *rap_get_items(RAP_Object *obj) {
  if (obj->tag == RAP_OBJECT_TAG_TEXT)
    return obj->text_val;
  return obj->tuple_val;
}

size_t rap_utf8_decode_all(const char *s, int64_t **out_codepoints) {
  size_t count = 0, cap = 16;
  int64_t *cps = malloc(cap * sizeof(int64_t));
  const unsigned char *p = (const unsigned char *)s;
  while (*p) {
    int64_t cp;
    if (*p < 0x80) {
      cp = *p++;
    } else if (*p < 0xE0) {
      cp = (p[0] & 0x1F) << 6 | (p[1] & 0x3F);
      p += 2;
    } else if (*p < 0xF0) {
      cp = (p[0] & 0x0F) << 12 | (p[1] & 0x3F) << 6 | (p[2] & 0x3F);
      p += 3;
    } else {
      cp = (p[0] & 0x07) << 18 | (p[1] & 0x3F) << 12 | (p[2] & 0x3F) << 6 |
           (p[3] & 0x3F);
      p += 4;
    }
    if (count >= cap) {
      cap *= 2;
      cps = realloc(cps, cap * sizeof(int64_t));
    }
    cps[count++] = cp;
  }
  *out_codepoints = cps;
  return count;
}

size_t rap_utf8_encode_one(int64_t cp, char *buf, size_t pos) {
  if (cp < 0x80) {
    buf[pos++] = cp;
  } else if (cp < 0x800) {
    buf[pos++] = 0xC0 | (cp >> 6);
    buf[pos++] = 0x80 | (cp & 0x3F);
  } else if (cp < 0x10000) {
    buf[pos++] = 0xE0 | (cp >> 12);
    buf[pos++] = 0x80 | ((cp >> 6) & 0x3F);
    buf[pos++] = 0x80 | (cp & 0x3F);
  } else {
    buf[pos++] = 0xF0 | (cp >> 18);
    buf[pos++] = 0x80 | ((cp >> 12) & 0x3F);
    buf[pos++] = 0x80 | ((cp >> 6) & 0x3F);
    buf[pos++] = 0x80 | (cp & 0x3F);
  }
  return pos;
}

char *rap_strbuf_append(char *buf, size_t *len, size_t *cap,
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

// ALLOCATION TRACKING (test-only, compile with -DRAP_TEST_LEAKS)

#ifdef RAP_TEST_LEAKS
int rap_alloc_count = 0;

void RAP_check_leaks(void) {
  if (rap_alloc_count != 0) {
    fprintf(stderr, "LEAK: %d object(s) not freed\n", rap_alloc_count);
  }
}
#endif

// CONSTRUCTORS

RAP_Value RAP_create_null_obj(void) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_NULL;
  obj->refcount = 1;
  return RAP_CREATE_PTR(obj);
}

RAP_Value RAP_create_int_obj(int64_t value) {
  // TODO: BigInts check
  if (value > INT32_MAX) {
    // Heap allocate 64 bit ints
    RAP_TRACK_ALLOC();
    RAP_Object *obj = malloc(sizeof(RAP_Object));
    obj->tag = RAP_OBJECT_TAG_INT;
    obj->int_val = value;
    obj->refcount = 1;
    return RAP_CREATE_PTR(obj);
  }
  // SMI tagged
  RAP_Value obj = RAP_CREATE_SMI(value);
  return obj;
}

RAP_Value RAP_create_float_obj(double value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_FLOAT;
  obj->float_val = value;
  obj->refcount = 1;
  return RAP_CREATE_PTR(obj);
}

RAP_Value RAP_create_logical_obj(bool value) { return RAP_CREATE_BOOL(value); }

// STRINGIFY

char *RAP_stringify_object(RAP_Value obj) {
  if (RAP_IS_BOOL(obj)) {
    return strdup(RAP_BOOL_VALUE(obj) ? "да" : "нет");
  } else if (RAP_IS_SMI(obj)) {
    size_t needed_size = snprintf(NULL, 0, "%ld", (long)RAP_SMI_VALUE(obj));
    char *str = malloc(needed_size + 1);
    snprintf(str, needed_size + 1, "%ld", (long)RAP_SMI_VALUE(obj));
    return str;
  }

  RAP_Object *obj_ptr = RAP_PTR_VALUE(obj);

  switch (obj_ptr->tag) {
  case RAP_OBJECT_TAG_NULL: {
    return strdup("пусто");
  }
  case RAP_OBJECT_TAG_LOGICAL: {
    return strdup(obj_ptr->logical_val ? "да" : "нет");
  }
  // TODO: re-introduce when BigInt is supported
  // case RAP_OBJECT_TAG_INT: {
  //   size_t needed_size =
  //       snprintf(NULL, 0, "%ld", (long)RAP_get_int_val(obj_ptr));
  //   char *str = malloc(needed_size + 1);
  //   snprintf(str, needed_size + 1, "%ld", (long)RAP_get_int_val(obj_ptr));
  //   return str;
  // }
  case RAP_OBJECT_TAG_FLOAT: {
    double integral_part;
    double fractional_part = modf(obj_ptr->float_val, &integral_part);
    double abs_fractional_part = fabs(fractional_part);
    bool has_only_zeros = abs_fractional_part < DBL_EPSILON;
    if (has_only_zeros) {
      size_t needed_size =
          snprintf(NULL, 0, "%.1f", obj_ptr->float_val);
      char *str = malloc(needed_size + 1);
      snprintf(str, needed_size + 1, "%.1f", obj_ptr->float_val);
      return str;
    }
    char tmp[64];
    snprintf(tmp, sizeof(tmp), "%.16g", obj_ptr->float_val);
    return strdup(tmp);
  }
  case RAP_OBJECT_TAG_TEXT: {
    uint32_t count = obj_ptr->text_val->count;
    if (count == 0)
      return strdup("");
    char *buf = malloc(count * 4 + 1);
    size_t pos = 0;
    for (uint32_t i = 0; i < count; i++) {
      int64_t cp = RAP_SMI_VALUE(obj_ptr->text_val->items[i]);
      pos = rap_utf8_encode_one(cp, buf, pos);
    }
    buf[pos] = '\0';
    return buf;
  }
  case RAP_OBJECT_TAG_TUPLE: {
    if (obj_ptr->tuple_val->count == 0) {
      return strdup("<* *>");
    }
    size_t len = 0, cap = 0;
    char *buf = NULL;
    buf = rap_strbuf_append(buf, &len, &cap, "<* ");
    for (uint32_t i = 0; i < obj_ptr->tuple_val->count; i++) {
      if (i > 0) {
        buf = rap_strbuf_append(buf, &len, &cap, ", ");
      }
      char *item_str = RAP_stringify_object(obj_ptr->tuple_val->items[i]);
      buf = rap_strbuf_append(buf, &len, &cap, item_str);
      free(item_str);
    }
    buf = rap_strbuf_append(buf, &len, &cap, " *>");
    return buf;
  }
  case RAP_OBJECT_TAG_SLICE: {
    RAP_Value materialized = RAP_materialize_slice(obj_ptr);
    return RAP_stringify_object(materialized);
  }
  case RAP_OBJECT_TAG_CALLABLE: {
    char *name = obj_ptr->callable_val->name;
    return strdup(name ? name : "<callable>");
  }
  default: {
    return strdup("<unknown>");
  }
  }
}

// REFERENCE COUNTING

void inline RAP_dec_ref(RAP_Value obj) {
  if (!RAP_IS_PTR(obj))
    return;

#ifdef RAP_DEBUG_LEAKS
  printf("dec_ref: %p\n - refcount: %d - string: %s\n", obj,
         RAP_PTR_VALUE(obj)->refcount, RAP_stringify_object(obj));
#endif

  RAP_Object *obj_ptr = RAP_PTR_VALUE(obj);
  if (obj_ptr == NULL)
    return;
  obj_ptr->refcount--;
  if (obj_ptr->refcount <= 0)
    RAP_free_object(obj_ptr);
}

// OBJECT DESTRUCTOR

void RAP_free_object(RAP_Object *obj) {
#ifdef RAP_DEBUG_LEAKS
  printf("free_object: %p - refcount: %d - string: %s\n", obj, obj->refcount,
         RAP_stringify_object(RAP_CREATE_PTR(obj)));
#endif

  if (obj == NULL)
    return;
  RAP_TRACK_FREE();

  switch (obj->tag) {
  case RAP_OBJECT_TAG_TEXT: {
    for (uint32_t i = 0; i < obj->text_val->count; i++) {
      RAP_dec_ref(obj->text_val->items[i]);
    }
    free(obj->text_val->items);
    free(obj->text_val);
    break;
  }
  case RAP_OBJECT_TAG_TUPLE: {
    for (uint32_t i = 0; i < obj->tuple_val->count; i++) {
      RAP_dec_ref(obj->tuple_val->items[i]);
    }
    free(obj->tuple_val->items);
    free(obj->tuple_val);
    break;
  }
  // IMPORTANT: slice destructor must not free parent!!!
  case RAP_OBJECT_TAG_SLICE: {
    RAP_dec_ref(RAP_CREATE_PTR(obj->slice_val->parent));
    free(obj->slice_val);
    break;
  }
  case RAP_OBJECT_TAG_CALLABLE: {
    struct RAP_Callable *c = obj->callable_val;
    free(c->name);
    for (uint32_t i = 0; i < c->param_count; i++) {
      free(c->params[i]->name);
      free(c->params[i]);
    }
    free(c->params);
    RAP_free_call_frame(c->frame);
    free(c);
    break;
  }
  default: {
    break;
  }
  }
  free(obj);
}

RAP_Value RAP_get_objects_refcount(RAP_Value obj) {
  if (!RAP_IS_PTR(obj))
    return 0;
  return RAP_CREATE_SMI(RAP_PTR_VALUE(obj)->refcount);
}

void RAP_free_main_frame(struct RAP_CallFrame *frame) {
  if (frame == NULL)
    return;
  for (uint32_t i = 0; i < frame->slot_count; i++) {
    RAP_dec_ref(frame->slots[i].value);
  }
  free(frame->slots);
}
