#include "runtime_internal.h"

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
  uint32_t count = rap_get_items(parent)->count;
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
  struct RAP_Tuple *parent_items = rap_get_items(parent);
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

  struct RAP_Tuple *parent_data = rap_get_items(parent);
  struct RAP_Tuple *rep_data = rap_get_items(replacement);

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
