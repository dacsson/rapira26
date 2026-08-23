#include "rapvalue.h"
#include "runtime_internal.h"

RAP_Value RAP_create_callable_obj(size_t offset_or_ptr, uint32_t arity,
                                  const RAP_Value *captures,
                                  uint32_t capture_count) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_CALLABLE;
  obj->callable_val = malloc(sizeof(struct RAP_Callable));
  obj->refcount = 1;
  obj->callable_val->offset_or_ptr = offset_or_ptr;
  obj->callable_val->arity = arity;
  obj->callable_val->capture_count = capture_count;

  if (capture_count > 0) {
    obj->callable_val->captures = malloc(capture_count * sizeof(RAP_Value));
    for (uint32_t i = 0; i < capture_count; i++) {
      obj->callable_val->captures[i] = captures[i];
      RAP_inc_ref(captures[i]);
    }
  } else {
    obj->callable_val->captures = NULL;
  }

  return RAP_CREATE_PTR(obj);
}

size_t RAP_get_callable_offset_or_ptr(RAP_Value callable) {
  return RAP_GET_CALLABLE_VAL(callable)->offset_or_ptr;
}

uint32_t RAP_get_callable_arity(RAP_Value callable) {
  return RAP_GET_CALLABLE_VAL(callable)->arity;
}
