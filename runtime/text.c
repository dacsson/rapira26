#include "runtime_internal.h"

RAP_Object *RAP_create_text_obj(const char *value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_TEXT;
  obj->text_val = malloc(sizeof(struct RAP_Tuple));
  obj->refcount = 1;
  // Decode UTF-8 into codepoints — one codepoint per tuple item
  int64_t *codepoints;
  size_t count = rap_utf8_decode_all(value, &codepoints);
  obj->text_val->count = count;
  obj->text_val->items = malloc(count * sizeof(RAP_Object *));
  for (size_t i = 0; i < count; i++) {
    obj->text_val->items[i] = RAP_create_int_obj(codepoints[i]);
  }
  free(codepoints);
  return obj;
}
