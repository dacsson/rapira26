#include "rapvalue.h"
#include "runtime.h"
#include "runtime_internal.h"

RAP_Value RAP_create_custom_typed_obj(const char* name, void *value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_VARIANT;
  obj->refcount = 1;
  obj->variant_val = malloc(sizeof(struct RAP_Variant));
  obj->variant_val->name = name;
  obj->variant_val->payload = value;
  return RAP_CREATE_PTR(obj);
}

uint16_t RAP_get_variant_tag(RAP_Value val) {
  if (!RAP_IS_VARIANT(val)) return 0; // TODO

  // Enums discriminant is first 16 bits of payload
  return *(uint16_t*)RAP_PTR_VALUE(val)->variant_val->payload;
}
