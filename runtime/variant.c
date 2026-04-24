#include "rapvalue.h"
#include "runtime.h"
#include "runtime_internal.h"

RAP_Value RAP_create_custom_typed_obj(const char *name,
                                      const char **field_names,
                                      size_t field_count, void *value) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_VARIANT;
  obj->refcount = 1;
  obj->variant_val = malloc(sizeof(struct RAP_Variant));
  obj->variant_val->name = name;
  obj->variant_val->field_names = field_names;
  obj->variant_val->field_count = field_count;
  size_t payload_size = sizeof(uint16_t) + field_count * sizeof(RAP_Value);
  obj->variant_val->payload = malloc(payload_size);
  memcpy(obj->variant_val->payload, value, payload_size);
  return RAP_CREATE_PTR(obj);
}

uint16_t RAP_get_variant_tag(RAP_Value val) {
  if (!RAP_IS_VARIANT(val))
    RAP_fatal_error("Объект не может быть образцом для сравнения");

  // Enums discriminant is first 16 bits of payload
  return *(uint16_t *)RAP_PTR_VALUE(val)->variant_val->payload;
}

RAP_Value RAP_get_variant_field(RAP_Value val, const char *field_name) {
  if (!RAP_IS_VARIANT(val))
    RAP_fatal_error("Не существует такого поля у объекта");

  // Calculate the offset of the field in the payload
  // 16 bits for the tag, followed by the field values
  // Each field is RAP_Value which is sizeof(uintptr_t)
  size_t field_offset = 2;

  struct RAP_Variant *variant = RAP_PTR_VALUE(val)->variant_val;
  void *payload = variant->payload;
  for (size_t i = 0; i < variant->field_count; i++) {
    if (strcmp(variant->field_names[i], field_name) == 0) {
      RAP_Value *field = (RAP_Value *)(payload + field_offset);
      return *field;
    }
    field_offset += sizeof(uintptr_t);
  }
  RAP_fatal_error("Не существует такого поля у объекта");
}

void RAP_set_variant_field(RAP_Value val, const char *field_name,
                           RAP_Value field_val) {
  if (!RAP_IS_VARIANT(val))
    RAP_fatal_error("Не существует такого поля у объекта");

  struct RAP_Variant *variant = RAP_PTR_VALUE(val)->variant_val;
  void *payload = variant->payload;
  size_t field_offset = 2;
  for (size_t i = 0; i < variant->field_count; i++) {
    if (strcmp(variant->field_names[i], field_name) == 0) {
      RAP_Value *field = (RAP_Value *)(payload + field_offset);
      *field = field_val;
      return;
    }
    field_offset += sizeof(uintptr_t);
  }

  RAP_fatal_error("Не существует такого поля у объекта");
}
