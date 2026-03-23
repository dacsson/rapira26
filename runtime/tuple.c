#include "rapvalue.h"
#include "runtime.h"
#include "runtime_internal.h"

RAP_Value RAP_create_tuple_obj(uint32_t count, RAP_Value *items) {
  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_TUPLE;
  obj->tuple_val = malloc(sizeof(struct RAP_Tuple));
  obj->tuple_val->count = count;
  obj->refcount = 1;
  obj->tuple_val->items = malloc(count * sizeof(RAP_Value));
  for (uint32_t i = 0; i < count; i++) {
    obj->tuple_val->items[i] = items[i];
  }
  return RAP_CREATE_PTR(obj);
}

RAP_Value RAP_set_tuple_item(RAP_Value container, uint32_t index,
                               RAP_Value item) {
  if (!RAP_IS_PTR(container)) {
    RAP_fatal_error("Первый аргумент должен быть указателем на объект");
  }

  if (RAP_PTR_VALUE(container)->tag != RAP_OBJECT_TAG_TUPLE && RAP_PTR_VALUE(container)->tag != RAP_OBJECT_TAG_TEXT) {
    RAP_fatal_error("Объект не является кортежем или текстом");
  }

  if (RAP_PTR_VALUE(container)->tag == RAP_OBJECT_TAG_TEXT) {
    // When assigning to a text element, unwrap single-char TEXT to its codepoint int
    if (RAP_IS_PTR(item) && RAP_PTR_VALUE(item)->tag == RAP_OBJECT_TAG_TEXT && RAP_get_text_val(RAP_PTR_VALUE(item))->count == 1) {
      item = RAP_get_text_val(RAP_PTR_VALUE(item))->items[0];
    }
    RAP_dec_ref(RAP_PTR_VALUE(container)->text_val->items[index]);
    RAP_PTR_VALUE(container)->text_val->items[index] = item;
  } else {
    RAP_dec_ref(RAP_PTR_VALUE(container)->tuple_val->items[index]);
    RAP_PTR_VALUE(container)->tuple_val->items[index] = item;
  }
  return container;
}

RAP_Value RAP_get_tuple_item(RAP_Value container, uint32_t index) {
  if (!RAP_IS_PTR(container)) {
    RAP_fatal_error("Первый аргумент должен быть указателем на объект");
  }
  if (RAP_PTR_VALUE(container)->tag != RAP_OBJECT_TAG_TUPLE && RAP_PTR_VALUE(container)->tag != RAP_OBJECT_TAG_TEXT) {
    RAP_fatal_error("Объект не является кортежем или текстом");
  }

  if (RAP_PTR_VALUE(container)->tag == RAP_OBJECT_TAG_TEXT) {
    // Return a single-character TEXT wrapping the codepoint
    RAP_TRACK_ALLOC();
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = 1;
    result->text_val->items = malloc(sizeof(RAP_Object *));
    result->text_val->items[0] = RAP_get_text_val(RAP_PTR_VALUE(container))->items[index];
    return RAP_CREATE_PTR(result);
  }
  return RAP_PTR_VALUE(container)->tuple_val->items[index];
}

RAP_Value RAP_append_tuple(RAP_Object *a, RAP_Object *b) {
  if (a->tag != RAP_OBJECT_TAG_TUPLE || b->tag != RAP_OBJECT_TAG_TUPLE) {
    RAP_fatal_error("Оба объекта должны быть кортежами");
  }

  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_TUPLE;
  obj->tuple_val = malloc(sizeof(struct RAP_Tuple));
  obj->tuple_val->count = a->tuple_val->count + b->tuple_val->count;
  obj->tuple_val->items = malloc(obj->tuple_val->count * sizeof(RAP_Object *));
  for (uint32_t i = 0; i < a->tuple_val->count; i++) {
    obj->tuple_val->items[i] = a->tuple_val->items[i];
    RAP_inc_ref(a->tuple_val->items[i]);
  }
  for (uint32_t i = 0; i < b->tuple_val->count; i++) {
    obj->tuple_val->items[a->tuple_val->count + i] = b->tuple_val->items[i];
    RAP_inc_ref(b->tuple_val->items[i]);
  }
  obj->refcount = 1;
  return RAP_CREATE_PTR(obj);
}

// индекс(needle, haystack) — search for element in tuple or substring in text.
// Returns 0-based position, or -1 if not found.
// Spec uses 1-based and returns 0 for not found; we deviate intentionally (see PHASE1_DIFFERENCE.md).
RAP_Value RAP_index_of(RAP_Value needle, RAP_Value haystack) {
  // Default tuple search
  if (RAP_IS_PTR(haystack) && RAP_PTR_VALUE(haystack)->tag == RAP_OBJECT_TAG_TUPLE) {
    for (uint32_t i = 0; i < RAP_PTR_VALUE(haystack)->tuple_val->count; i++) {
      if (RAP_BOOL_VALUE(RAP_equal(needle, RAP_PTR_VALUE(haystack)->tuple_val->items[i]))) {
        return RAP_create_int_obj(i);
      }
    }
    return RAP_create_int_obj(-1);
  }
  // Special case for strings
  if (RAP_IS_PTR(haystack) && RAP_PTR_VALUE(haystack)->tag == RAP_OBJECT_TAG_TEXT && RAP_IS_PTR(needle) && RAP_PTR_VALUE(needle)->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *h = RAP_get_text_val(RAP_PTR_VALUE(haystack));
    struct RAP_Tuple *n = RAP_get_text_val(RAP_PTR_VALUE(needle));
    if (n->count == 0) return RAP_create_int_obj(0);
    if (n->count > h->count) return RAP_create_int_obj(-1);
    for (uint32_t i = 0; i <= h->count - n->count; i++) {
      bool match = true;
      for (uint32_t j = 0; j < n->count; j++) {
        if (RAP_SMI_VALUE(h->items[i + j]) != RAP_SMI_VALUE(n->items[j])) {
          match = false;
          break;
        }
      }
      if (match) return RAP_create_int_obj(i);
    }
    return RAP_create_int_obj(-1);
  }
  // Materialize slices and recurse
  if (RAP_IS_PTR(haystack) && RAP_PTR_VALUE(haystack)->tag == RAP_OBJECT_TAG_SLICE || RAP_IS_PTR(needle) && RAP_PTR_VALUE(needle)->tag == RAP_OBJECT_TAG_SLICE) {
    return RAP_index_of(RAP_materialize_slice(RAP_PTR_VALUE(needle)), RAP_materialize_slice(RAP_PTR_VALUE(haystack)));
  }
  RAP_fatal_error("Неподдерживаемые типы для индекс()");
}

// SLICE OPERATIONS

RAP_Value RAP_create_slice(RAP_Value parent, int64_t from, int64_t to) {
  if (!RAP_IS_PTR(parent)) {
    RAP_fatal_error("Неподдерживаемый тип для среза");
  }

  // Flatten: if parent is already a slice, resolve to the root parent
  if (RAP_IS_PTR(parent) && RAP_PTR_VALUE(parent)->tag == RAP_OBJECT_TAG_SLICE) {
    from += RAP_PTR_VALUE(parent)->slice_val->from;
    to += RAP_PTR_VALUE(parent)->slice_val->from;
    parent = RAP_CREATE_PTR(RAP_PTR_VALUE(parent)->slice_val->parent);
  }

  // Clamp bounds
  uint32_t count = rap_get_items(RAP_PTR_VALUE(parent))->count;
  if (from < 0) from = 0;
  if (to > count) to = count;
  if (from > to) from = to;

  // For slices that are actually just a single item, expand so we include just the item itself
  if (from == to && to < count) to += 1;

  RAP_TRACK_ALLOC();
  RAP_Object *obj = malloc(sizeof(RAP_Object));
  obj->tag = RAP_OBJECT_TAG_SLICE;
  obj->refcount = 1;
  obj->slice_val = malloc(sizeof(struct RAP_Slice));
  obj->slice_val->parent = RAP_PTR_VALUE(parent);
  obj->slice_val->from = from;
  obj->slice_val->to = to;
  RAP_inc_ref(parent);  // slice keeps parent alive
  return RAP_CREATE_PTR(obj);
}

// Turn a slice into a real tuple/text (copy). If not a slice, return as-is.
RAP_Value RAP_materialize_slice(RAP_Object *obj) {
  if (obj->tag != RAP_OBJECT_TAG_SLICE) return RAP_CREATE_PTR(obj);

  RAP_Object *parent = obj->slice_val->parent;
  int64_t from = obj->slice_val->from;
  int64_t to = obj->slice_val->to;
  uint32_t new_count = (from < to) ? (to - from) : 0;
  struct RAP_Tuple *parent_items = rap_get_items(parent);
  bool is_text = (parent->tag == RAP_OBJECT_TAG_TEXT);

  if (new_count == 0) {
    if (is_text) return RAP_create_text_obj("");
    return RAP_create_tuple_obj(0, NULL);
  }

  RAP_Value *items = malloc(new_count * sizeof(RAP_Value));
  for (uint32_t i = 0; i < new_count; i++) {
    items[i] = parent_items->items[from + i];
  }

  if (is_text) {
    RAP_TRACK_ALLOC();
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return RAP_CREATE_PTR(result);
  }
  RAP_Value result = RAP_create_tuple_obj(new_count, items);
  free(items);
  return result;
}

// Replace parent[from:to] with replacement items.
// Modifies the parent in-place.
void RAP_slice_assign(RAP_Value slice, RAP_Value replacement) {
  if (!RAP_IS_PTR(slice) || !RAP_IS_PTR(replacement)) {
    RAP_fatal_error("Присваивание среза не-значением");
  }

  if (RAP_PTR_VALUE(slice)->tag != RAP_OBJECT_TAG_SLICE) {
    RAP_fatal_error("Присваивание среза не-срезу");
  }
  RAP_Object *parent = RAP_PTR_VALUE(slice)->slice_val->parent;
  int64_t from = RAP_PTR_VALUE(slice)->slice_val->from;
  int64_t to = RAP_PTR_VALUE(slice)->slice_val->to;

  // Materialize replacement if it's a slice
  replacement = RAP_materialize_slice(RAP_PTR_VALUE(replacement));

  struct RAP_Tuple *parent_data = rap_get_items(parent);
  struct RAP_Tuple *rep_data = rap_get_items(RAP_PTR_VALUE(replacement));

  uint32_t old_count = parent_data->count;
  uint32_t removed = (from < to) ? (to - from) : 0;
  uint32_t rep_count = rep_data->count;
  uint32_t new_count = old_count - removed + rep_count;

  RAP_Value *items = malloc(new_count * sizeof(RAP_Value));
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
