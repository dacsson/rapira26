#include "rapobject.h"
#include "rapvalue.h"
#include "runtime_internal.h"

// Integer operations

RAP_Value RAP_integer_less_than(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_SMI_VALUE(a) < RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_greater_than(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_SMI_VALUE(a) > RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_equal(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_SMI_VALUE(a) == RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_not_equal(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_SMI_VALUE(a) != RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_modulo(RAP_Value a, RAP_Value b) {
  // Spec §2.2.4.2: remainder = A - A//B * B (where // is floor division)
  int64_t av = RAP_SMI_VALUE(a);
  int64_t bv = RAP_SMI_VALUE(b);
  int64_t quotient = (int64_t)floor((double)av / bv);
  return RAP_create_int_obj(av - quotient * bv);
}

RAP_Value RAP_integer_add(RAP_Value a, RAP_Value b) {
  return RAP_create_int_obj(RAP_SMI_VALUE(a) + RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_subtract(RAP_Value a, RAP_Value b) {
  return RAP_create_int_obj(RAP_SMI_VALUE(a) - RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_multiply(RAP_Value a, RAP_Value b) {
  return RAP_create_int_obj(RAP_SMI_VALUE(a) * RAP_SMI_VALUE(b));
}

RAP_Value RAP_integer_divide(RAP_Value a, RAP_Value b) {
  return RAP_create_int_obj(floor((float)RAP_SMI_VALUE(a) / RAP_SMI_VALUE(b)));
}

// Float operations

RAP_Value RAP_float_less_than(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_DOUBLE_VALUE(a) < RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_greater_than(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_DOUBLE_VALUE(a) > RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_equal(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_DOUBLE_VALUE(a) == RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_not_equal(RAP_Value a, RAP_Value b) {
  return RAP_create_logical_obj(RAP_DOUBLE_VALUE(a) != RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_add(RAP_Value a, RAP_Value b) {
  return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) + RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_subtract(RAP_Value a, RAP_Value b) {
  return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) - RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_multiply(RAP_Value a, RAP_Value b) {
  return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) * RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_divide(RAP_Value a, RAP_Value b) {
  return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) / RAP_DOUBLE_VALUE(b));
}

RAP_Value RAP_float_modulo(RAP_Value a, RAP_Value b) {
  return RAP_create_float_obj(fmod(RAP_DOUBLE_VALUE(a), RAP_DOUBLE_VALUE(b)));
}

// Generic operations

RAP_Value RAP_add(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_add(a, b);
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_float_add(a, b);
  }

  RAP_Object *a_ptr = RAP_PTR_VALUE(a);
  RAP_Object *b_ptr = RAP_PTR_VALUE(b);

  // TUPLE APPEND
  if (a_ptr->tag == RAP_OBJECT_TAG_TUPLE &&
      b_ptr->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_append_tuple(a_ptr, b_ptr);
  }
  // TEXT CONCAT
  if (a_ptr->tag == RAP_OBJECT_TAG_TEXT && b_ptr->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *at = RAP_get_text_val(a_ptr);
    struct RAP_Tuple *bt = RAP_get_text_val(b_ptr);
    uint32_t new_count = at->count + bt->count;
    RAP_Value *items = malloc(new_count * sizeof(RAP_Value));
    for (uint32_t i = 0; i < at->count; i++) {
      items[i] = at->items[i];
      if (RAP_IS_PTR(items[i]))
        RAP_inc_ref(at->items[i]);
    }
    for (uint32_t i = 0; i < bt->count; i++) {
      items[at->count + i] = bt->items[i];
      if (RAP_IS_PTR(items[at->count + i]))
        RAP_inc_ref(bt->items[i]);
    }
    RAP_TRACK_ALLOC();
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    // TODO: reference counting here inserted
    return RAP_CREATE_PTR(result);
  }
  RAP_fatal_error("Неподдерживаемые типы для сложения");
}

RAP_Value RAP_subtract(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_subtract(a, b);
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_float_subtract(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для вычитания");
}

RAP_Value RAP_multiply(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_multiply(a, b);
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_float_multiply(a, b);
  }

  // Tuple repeat: tuple * int
  if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_TUPLE && RAP_IS_SMI(b)) {
    RAP_Object *a_ptr = RAP_PTR_VALUE(a);
    int64_t n = RAP_SMI_VALUE(b);
    if (n <= 0)
      return RAP_create_tuple_obj(0, NULL);
    uint32_t src_count = a_ptr->tuple_val->count;
    uint32_t new_count = src_count * n;
    RAP_Value *items = malloc(new_count * sizeof(RAP_Value));
    for (int64_t rep = 0; rep < n; rep++) {
      for (uint32_t i = 0; i < src_count; i++) {
        items[rep * src_count + i] = a_ptr->tuple_val->items[i];
        RAP_inc_ref(a_ptr->tuple_val->items[i]);
      }
    }
    RAP_Value result = RAP_create_tuple_obj(new_count, items);
    free(items);
    return result;
  }

  // Text repeat: text * int or int * text
  RAP_Object *text_obj = NULL;
  int64_t repeat_n = 0;
  if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_TEXT && RAP_IS_SMI(b)) {
    text_obj = RAP_PTR_VALUE(a);
    repeat_n = RAP_SMI_VALUE(b);
  } else if (RAP_IS_SMI(a) && RAP_IS_PTR(b) && RAP_PTR_VALUE(b)->tag == RAP_OBJECT_TAG_TEXT) {
    text_obj = RAP_PTR_VALUE(b);
    repeat_n = RAP_SMI_VALUE(a);
  }
  if (text_obj) {
    struct RAP_Tuple *src = RAP_get_text_val(text_obj);
    if (repeat_n <= 0 || src->count == 0) {
      return RAP_create_text_obj("");
    }
    uint32_t new_count = src->count * repeat_n;
    RAP_Value *items = malloc(new_count * sizeof(RAP_Value));
    for (int64_t rep = 0; rep < repeat_n; rep++) {
      for (uint32_t i = 0; i < src->count; i++) {
        items[rep * src->count + i] = src->items[i];
        RAP_inc_ref(src->items[i]);
      }
    }
    RAP_TRACK_ALLOC();
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return RAP_CREATE_PTR(result);
  }
  RAP_fatal_error("Неподдерживаемые типы для умножения");
}

RAP_Value RAP_divide(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    // Spec §2.2.4.2: int/int returns int if exact, real otherwise
    int64_t av = RAP_SMI_VALUE(a);
    int64_t bv = RAP_SMI_VALUE(b);
    if (bv != 0 && av % bv == 0) {
      return RAP_create_int_obj(av / bv);
    }
    return RAP_create_float_obj((double)av / (double)bv);
  } else if (RAP_IS_DOUBLE(a)) {
    if (RAP_IS_SMI(b)) {
      return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) /
                                  (double)RAP_SMI_VALUE(b));
    } else if (RAP_IS_DOUBLE(b)) {
      return RAP_float_divide(a, b);
    }
  } else if (RAP_IS_SMI(a) && RAP_IS_DOUBLE(b)) {
    return RAP_create_float_obj((double)RAP_SMI_VALUE(a) / RAP_DOUBLE_VALUE(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для деления");
}

RAP_Value RAP_less_than(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_less_than(a, b);
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_float_less_than(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Value RAP_less_or_equal(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_create_logical_obj(RAP_SMI_VALUE(a) <= RAP_SMI_VALUE(b));
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_create_logical_obj(RAP_DOUBLE_VALUE(a) <= RAP_DOUBLE_VALUE(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Value RAP_greater_than(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_greater_than(a, b);
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_float_greater_than(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Value RAP_greater_or_equal(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_create_logical_obj(RAP_SMI_VALUE(a) >= RAP_SMI_VALUE(b));
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_create_logical_obj(RAP_DOUBLE_VALUE(a) >= RAP_DOUBLE_VALUE(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Value RAP_equal(RAP_Value a, RAP_Value b) {
  // Materialize slices before comparing
  a = RAP_IS_PTR(a) ? RAP_materialize_slice(RAP_PTR_VALUE(a)) : a;
  b = RAP_IS_PTR(b) ? RAP_materialize_slice(RAP_PTR_VALUE(b)) : b;

  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_equal(a, b);
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_float_equal(a, b);
  } else if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_TEXT &&
             RAP_IS_PTR(b) && RAP_PTR_VALUE(b)->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *at = RAP_get_text_val(RAP_PTR_VALUE(a));
    struct RAP_Tuple *bt = RAP_get_text_val(RAP_PTR_VALUE(b));
    if (at->count != bt->count)
      return RAP_create_logical_obj(false);
    for (uint32_t i = 0; i < at->count; i++) {
      if (RAP_SMI_VALUE(at->items[i]) != RAP_SMI_VALUE(bt->items[i]))
        return RAP_create_logical_obj(false);
    }
    return RAP_create_logical_obj(true);
  } else if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_NULL &&
             RAP_IS_PTR(b) && RAP_PTR_VALUE(b)->tag == RAP_OBJECT_TAG_NULL) {
    return RAP_create_logical_obj(true);
  } else if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_LOGICAL &&
             RAP_IS_PTR(b) && RAP_PTR_VALUE(b)->tag == RAP_OBJECT_TAG_LOGICAL) {
    return RAP_create_logical_obj(RAP_PTR_VALUE(a)->logical_val ==
                                  RAP_PTR_VALUE(b)->logical_val);
  } else if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_TUPLE &&
             RAP_IS_PTR(b) && RAP_PTR_VALUE(b)->tag == RAP_OBJECT_TAG_TUPLE) {
    if (RAP_PTR_VALUE(a)->tuple_val->count !=
        RAP_PTR_VALUE(b)->tuple_val->count) {
      return RAP_create_logical_obj(false);
    }
    for (uint32_t i = 0; i < RAP_PTR_VALUE(a)->tuple_val->count; i++) {
      RAP_Value eq = RAP_equal(RAP_PTR_VALUE(a)->tuple_val->items[i],
                               RAP_PTR_VALUE(b)->tuple_val->items[i]);
      if (!RAP_BOOL_VALUE(eq)) {
        return RAP_create_logical_obj(false);
      }
    }
    return RAP_create_logical_obj(true);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Value RAP_not_equal(RAP_Value a, RAP_Value b) {
  // Reuse RAP_equal and negate
  return RAP_create_logical_obj(!RAP_BOOL_VALUE(RAP_equal(a, b)));
}

RAP_Value RAP_modulo(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_integer_modulo(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для модуля");
}

RAP_Value RAP_negate(RAP_Value a) {
  if (RAP_IS_SMI(a)) {
    return RAP_create_int_obj(-1 * RAP_SMI_VALUE(a));
  } else if (RAP_IS_DOUBLE(a)) {
    return RAP_create_float_obj(-1.0 * RAP_DOUBLE_VALUE(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для отрицания");
}

RAP_Value RAP_length(RAP_Value a) {
  // Text stores one codepoint per item, so count == character length
  if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_TEXT) {
    return RAP_create_int_obj(RAP_get_text_val(RAP_PTR_VALUE(a))->count);
  }
  if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_create_int_obj(RAP_get_tuple_val(RAP_PTR_VALUE(a))->count);
  }
  if (RAP_IS_PTR(a) && RAP_PTR_VALUE(a)->tag == RAP_OBJECT_TAG_SLICE) {
    return RAP_create_int_obj(RAP_PTR_VALUE(a)->slice_val->to - RAP_PTR_VALUE(a)->slice_val->from);
  }
  RAP_fatal_error("Неподдерживаемые типы для длины");
}

RAP_Value RAP_power(RAP_Value a, RAP_Value b) {
  bool result_is_float =
      RAP_IS_DOUBLE(a) || RAP_IS_DOUBLE(b);
  if (!RAP_IS_SMI(b) && !RAP_IS_DOUBLE(b)) {
    RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
  }
  double power_value =
      RAP_IS_SMI(b) ? RAP_SMI_VALUE(b) : RAP_DOUBLE_VALUE(b);
  if (RAP_IS_SMI(a)) {
    if (result_is_float) {
      return RAP_create_float_obj(pow(RAP_SMI_VALUE(a), power_value));
    } else {
      return RAP_create_int_obj(pow(RAP_SMI_VALUE(a), power_value));
    }
  } else if (RAP_IS_DOUBLE(a)) {
    if (result_is_float) {
      return RAP_create_float_obj(pow(RAP_DOUBLE_VALUE(a), power_value));
    } else {
      return RAP_create_int_obj(pow(RAP_DOUBLE_VALUE(a), power_value));
    }
  }

  RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
}
