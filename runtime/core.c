#include "runtime.h"
#include "runtime_internal.h"

// Fatal error - print a message and exit.
void RAP_fatal_error(const char *message) {
  fprintf(stderr, "Упс, ошибка: %s\n", message);
  exit(1);
}

// HELPERS

struct RAP_Tuple *rap_get_items(RAP_Object *obj) {
  if (obj->tag == RAP_OBJECT_TAG_TEXT) return obj->text_val;
  return obj->tuple_val;
}

size_t rap_utf8_decode_all(const char *s, int64_t **out_codepoints) {
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

size_t rap_utf8_encode_one(int64_t cp, char *buf, size_t pos) {
  if (cp < 0x80)        { buf[pos++] = cp; }
  else if (cp < 0x800)  { buf[pos++] = 0xC0 | (cp >> 6);  buf[pos++] = 0x80 | (cp & 0x3F); }
  else if (cp < 0x10000){ buf[pos++] = 0xE0 | (cp >> 12); buf[pos++] = 0x80 | ((cp >> 6) & 0x3F); buf[pos++] = 0x80 | (cp & 0x3F); }
  else                  { buf[pos++] = 0xF0 | (cp >> 18); buf[pos++] = 0x80 | ((cp >> 12) & 0x3F); buf[pos++] = 0x80 | ((cp >> 6) & 0x3F); buf[pos++] = 0x80 | (cp & 0x3F); }
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

RAP_Object *RAP_create_null_obj(void) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_NULL;
  obj->refcount = 1;
  return obj;
}

RAP_Object *RAP_create_int_obj(int64_t value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_INT;
  obj->int_val = value;
  obj->refcount = 1;
  return obj;
}

RAP_Object *RAP_create_float_obj(double value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_FLOAT;
  obj->float_val = value;
  obj->refcount = 1;
  return obj;
}

RAP_Object *RAP_create_logical_obj(bool value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_LOGICAL;
  obj->logical_val = value;
  obj->refcount = 1;
  return obj;
}

// STRINGIFY

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
    char tmp[64];
    snprintf(tmp, sizeof(tmp), "%.16g", RAP_get_float_val(obj));
    return strdup(tmp);
  }
  case RAP_OBJECT_TAG_TEXT: {
    uint32_t count = RAP_get_text_val(obj)->count;
    if (count == 0) return strdup("");
    char *buf = malloc(count * 4 + 1);
    size_t pos = 0;
    for (uint32_t i = 0; i < count; i++) {
      int64_t cp = RAP_get_int_val(RAP_get_text_val(obj)->items[i]);
      pos = rap_utf8_encode_one(cp, buf, pos);
    }
    buf[pos] = '\0';
    return buf;
  }
  case RAP_OBJECT_TAG_TUPLE: {
    if (obj->tuple_val->count == 0) {
      return strdup("<* *>");
    }
    size_t len = 0, cap = 0;
    char *buf = NULL;
    buf = rap_strbuf_append(buf, &len, &cap, "<* ");
    for (uint32_t i = 0; i < obj->tuple_val->count; i++) {
      if (i > 0) {
        buf = rap_strbuf_append(buf, &len, &cap, ", ");
      }
      char *item_str = RAP_stringify_object(obj->tuple_val->items[i]);
      buf = rap_strbuf_append(buf, &len, &cap, item_str);
      free(item_str);
    }
    buf = rap_strbuf_append(buf, &len, &cap, " *>");
    return buf;
  }
  case RAP_OBJECT_TAG_SLICE: {
    RAP_Object *materialized = RAP_materialize_slice(obj);
    return RAP_stringify_object(materialized);
  }
  case RAP_OBJECT_TAG_CALLABLE: {
    char *name = RAP_get_callable_val(obj)->name;
    return strdup(name ? name : "<callable>");
  }
  default: {
    return strdup("<unknown>");
  }
  }
}

// REFERENCE COUNTING

void RAP_dec_ref(RAP_Object *obj) {
  if (obj == NULL) return;
  obj->refcount--;
  if (obj->refcount <= 0) RAP_free_object(obj);
}

// OBJECT DESTRUCTOR

void RAP_free_object(RAP_Object *obj) {
  if (obj == NULL) return;
  RAP_TRACK_FREE();

  switch (obj->tag) {
    case RAP_OBJECT_TAG_TEXT: {
      for (uint32_t i = 0; i < obj->text_val->count; i++) {
        RAP_dec_ref(obj->text_val->items[i]);
      }
      free(RAP_get_text_val(obj)->items);
      free(RAP_get_text_val(obj));
      break;
    }
    case RAP_OBJECT_TAG_TUPLE: {
      for (uint32_t i = 0; i < obj->tuple_val->count; i++) {
        RAP_dec_ref(obj->tuple_val->items[i]);
      }
      free(RAP_get_tuple_val(obj)->items);
      free(RAP_get_tuple_val(obj));
      break;
    }
    // IMPORTANT: slice destructor must not free parent!!!
    case RAP_OBJECT_TAG_SLICE: {
      RAP_dec_ref(RAP_get_slice_val(obj)->parent);
      free(obj->slice_val);
      break;
    }
    case RAP_OBJECT_TAG_CALLABLE: {
      free(RAP_get_callable_val(obj)->name);
      free(RAP_get_callable_val(obj)->params);
      free(RAP_get_callable_val(obj)->frame);
      free(RAP_get_callable_val(obj));
      break;
    }
    default: {
      break;
    }
  }
  free(obj);
}
