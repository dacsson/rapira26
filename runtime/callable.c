#include "rapvalue.h"
#include "runtime_internal.h"

RAP_Value RAP_create_callable_obj(struct RAP_CallFrame *frame_parent,
                                    RAP_FunctionDecl func,
                                    RAP_Parameter **params,
                                    uint32_t params_count,
                                    bool is_function) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_CALLABLE;
  obj->callable_val = malloc(sizeof(struct RAP_Callable));
  obj->refcount = 1;

  // Setup frame, initially it only has a pointer to the parent frame,
  // so we can walk up the call stack to find the enclosing scope variables
  obj->callable_val->frame = RAP_create_call_frame(frame_parent);

  obj->callable_val->name = NULL;
  obj->callable_val->func = func;
  obj->callable_val->param_count = params_count;
  obj->callable_val->is_function = is_function;

  // Copy the params array — callers pass stack pointers (e.g. &_p0)
  if (params_count > 0) {
    obj->callable_val->params = malloc(params_count * sizeof(RAP_Parameter *));
    memcpy(obj->callable_val->params, params, params_count * sizeof(RAP_Parameter *));
  } else {
    obj->callable_val->params = NULL;
  }

  return RAP_CREATE_PTR(obj);
}

RAP_Parameter *RAP_create_parameter(RAP_ParameterMode mode, const char *name) {
  RAP_Parameter *param = malloc(sizeof(RAP_Parameter));
  param->mode = mode;
  param->name = malloc(strlen(name) + 1);
  strcpy(param->name, name);
  return param;
}

RAP_Value RAP_call_callable_obj(RAP_Value callable, RAP_Value *args,
                                  uint32_t arg_count) {
  if (!RAP_IS_PTR(callable)) {
    RAP_fatal_error("Ожидал указатель на функцию или процедуру\n");
  }

  // Create a per-call frame with the callable's frame as parent,
  // so чужие lookups can walk up to the enclosing scope.
  struct RAP_CallFrame *call_frame =
      RAP_create_call_frame(RAP_PTR_VALUE(callable)->callable_val->frame);
  RAP_Value result =
      RAP_PTR_VALUE(callable)->callable_val->func(call_frame, args, arg_count);
  // Keep return value alive across frame cleanup
  // if (RAP_IS_PTR(result))
  //   RAP_inc_ref(result);
  RAP_free_call_frame(call_frame);
  return result;
}

// FRAME UTILITIES

struct RAP_CallFrame *RAP_create_call_frame(struct RAP_CallFrame *parent) {
  struct RAP_CallFrame *frame = malloc(sizeof(struct RAP_CallFrame));
  frame->parent = parent;
  frame->slots = NULL;
  frame->slot_count = 0;
  return frame;
}

void RAP_free_call_frame(struct RAP_CallFrame *frame) {
  if (frame == NULL) return;
  for (uint32_t i = 0; i < frame->slot_count; i++) {
#ifdef RAP_DEBUG_LEAKS
    printf(" - slot %u: %s\n", i, frame->slots[i].name);
    printf("   - value: %s\n", RAP_stringify_object(frame->slots[i].value));
    printf("   - refcount: %d\n", RAP_IS_PTR(frame->slots[i].value) ? RAP_PTR_VALUE(frame->slots[i].value)->refcount : 0);
    printf("   - is ptr: %s\n", RAP_IS_PTR(frame->slots[i].value) ? "yes" : "no");
    printf("   - ptr: %p\n", RAP_IS_PTR(frame->slots[i].value) ? RAP_PTR_VALUE(frame->slots[i].value) : NULL);
#endif
    RAP_dec_ref(frame->slots[i].value);
  }
  free(frame->slots);
  free(frame);
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
RAP_Value RAP_frame_get(struct RAP_CallFrame *frame, const char *name) {
  int idx = frame_find_slot(frame, name);
  if (idx >= 0) {
    // Inc ref new value
    RAP_inc_ref(frame->slots[idx].value);
    return frame->slots[idx].value;
  }
  return RAP_create_null_obj();
}

// Set variable in current frame (creates slot if not found).
void RAP_frame_set(struct RAP_CallFrame *frame, const char *name, RAP_Value value) {
  int idx = frame_find_slot(frame, name);
  if (idx >= 0) {
    // Dec ref previous value
    RAP_dec_ref(frame->slots[idx].value);
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
RAP_Value RAP_frame_get_foreign(struct RAP_CallFrame *frame, const char *name) {
  struct RAP_CallFrame *current = frame->parent;
  while (current) {
    int idx = frame_find_slot(current, name);
    if (idx >= 0) {
      // Inc ref new value
      RAP_inc_ref(current->slots[idx].value);
      return current->slots[idx].value;
    }
    current = current->parent;
  }
  return RAP_create_null_obj();
}

// Set variable by walking up the parent chain (чужие).
// If found in a parent, updates it there. Otherwise creates in the immediate parent.
void RAP_frame_set_foreign(struct RAP_CallFrame *frame, const char *name, RAP_Value value) {
  struct RAP_CallFrame *current = frame->parent;
  while (current) {
    int idx = frame_find_slot(current, name);
    if (idx >= 0) {
      // Dec ref previous value
      RAP_dec_ref(current->slots[idx].value);
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
