#include "runtime_internal.h"

// Integer operations

RAP_Object *RAP_integer_less_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) < RAP_get_int_val(b));
}

RAP_Object *RAP_integer_greater_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) > RAP_get_int_val(b));
}

RAP_Object *RAP_integer_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) == RAP_get_int_val(b));
}

RAP_Object *RAP_integer_not_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_int_val(a) != RAP_get_int_val(b));
}

RAP_Object *RAP_integer_modulo(RAP_Object *a, RAP_Object *b) {
  // Spec §2.2.4.2: remainder = A - A//B * B (where // is floor division)
  int64_t av = RAP_get_int_val(a);
  int64_t bv = RAP_get_int_val(b);
  int64_t quotient = (int64_t)floor((double)av / bv);
  return RAP_create_int_obj(av - quotient * bv);
}

RAP_Object *RAP_integer_add(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) + RAP_get_int_val(b));
}

RAP_Object *RAP_integer_subtract(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) - RAP_get_int_val(b));
}

RAP_Object *RAP_integer_multiply(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(RAP_get_int_val(a) * RAP_get_int_val(b));
}

RAP_Object *RAP_integer_divide(RAP_Object *a, RAP_Object *b) {
  return RAP_create_int_obj(floor((float) RAP_get_int_val(a) / RAP_get_int_val(b)));
}

// Float operations

RAP_Object *RAP_float_less_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) < RAP_get_float_val(b));
}

RAP_Object *RAP_float_greater_than(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) > RAP_get_float_val(b));
}

RAP_Object *RAP_float_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) == RAP_get_float_val(b));
}

RAP_Object *RAP_float_not_equal(RAP_Object *a, RAP_Object *b) {
  return RAP_create_logical_obj(RAP_get_float_val(a) != RAP_get_float_val(b));
}

RAP_Object *RAP_float_add(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) + RAP_get_float_val(b));
}

RAP_Object *RAP_float_subtract(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) - RAP_get_float_val(b));
}

RAP_Object *RAP_float_multiply(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) * RAP_get_float_val(b));
}

RAP_Object *RAP_float_divide(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(RAP_get_float_val(a) / RAP_get_float_val(b));
}

RAP_Object *RAP_float_modulo(RAP_Object *a, RAP_Object *b) {
  return RAP_create_float_obj(fmod(RAP_get_float_val(a), RAP_get_float_val(b)));
}

// Generic operations

RAP_Object *RAP_add(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_add(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_add(a, b);
  }
  // TUPLE APPEND
  if (a->tag == RAP_OBJECT_TAG_TUPLE && b->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_append_tuple(a, b);
  }
  // TEXT CONCAT
  if (a->tag == RAP_OBJECT_TAG_TEXT && b->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *at = RAP_get_text_val(a);
    struct RAP_Tuple *bt = RAP_get_text_val(b);
    uint32_t new_count = at->count + bt->count;
    RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
    for (uint32_t i = 0; i < at->count; i++) items[i] = at->items[i];
    for (uint32_t i = 0; i < bt->count; i++) items[at->count + i] = bt->items[i];
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return result;
  }
  RAP_fatal_error("Неподдерживаемые типы для сложения");
}

RAP_Object *RAP_subtract(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_subtract(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_subtract(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для вычитания");
}

RAP_Object *RAP_multiply(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_multiply(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_multiply(a, b);
  }
  // Tuple repeat: tuple * int
  if (a->tag == RAP_OBJECT_TAG_TUPLE && b->tag == RAP_OBJECT_TAG_INT) {
    int64_t n = RAP_get_int_val(b);
    if (n <= 0) return RAP_create_tuple_obj(0, NULL);
    uint32_t src_count = a->tuple_val->count;
    uint32_t new_count = src_count * n;
    RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
    for (int64_t rep = 0; rep < n; rep++) {
      for (uint32_t i = 0; i < src_count; i++) {
        items[rep * src_count + i] = a->tuple_val->items[i];
      }
    }
    RAP_Object *result = RAP_create_tuple_obj(new_count, items);
    free(items);
    return result;
  }
  // Text repeat: text * int or int * text
  RAP_Object *text_obj = NULL;
  int64_t repeat_n = 0;
  if (a->tag == RAP_OBJECT_TAG_TEXT && b->tag == RAP_OBJECT_TAG_INT) {
    text_obj = a; repeat_n = RAP_get_int_val(b);
  } else if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_TEXT) {
    text_obj = b; repeat_n = RAP_get_int_val(a);
  }
  if (text_obj) {
    struct RAP_Tuple *src = RAP_get_text_val(text_obj);
    if (repeat_n <= 0 || src->count == 0) {
      return RAP_create_text_obj("");
    }
    uint32_t new_count = src->count * repeat_n;
    RAP_Object **items = malloc(new_count * sizeof(RAP_Object *));
    for (int64_t rep = 0; rep < repeat_n; rep++) {
      for (uint32_t i = 0; i < src->count; i++) {
        items[rep * src->count + i] = src->items[i];
      }
    }
    RAP_Object *result = malloc(sizeof(RAP_Object));
    result->tag = RAP_OBJECT_TAG_TEXT;
    result->text_val = malloc(sizeof(struct RAP_Tuple));
    result->text_val->count = new_count;
    result->text_val->items = items;
    return result;
  }
  printf("%d %d\n", a->tag, b->tag);
  RAP_fatal_error("Неподдерживаемые типы для умножения");
}

RAP_Object *RAP_divide(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    // Spec §2.2.4.2: int/int returns int if exact, real otherwise
    int64_t av = RAP_get_int_val(a);
    int64_t bv = RAP_get_int_val(b);
    if (bv != 0 && av % bv == 0) {
      return RAP_create_int_obj(av / bv);
    }
    return RAP_create_float_obj((double)av / (double)bv);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    if (b->tag == RAP_OBJECT_TAG_INT) {
      return RAP_create_float_obj(RAP_get_float_val(a) / (double)RAP_get_int_val(b));
    } else if (b->tag == RAP_OBJECT_TAG_FLOAT) {
      return RAP_float_divide(a, b);
    }
  } else if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj((double)RAP_get_int_val(a) / RAP_get_float_val(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для деления");
}

RAP_Object *RAP_less_than(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_less_than(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_less_than(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_less_or_equal(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_logical_obj(a->int_val <= b->int_val);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_logical_obj(a->float_val <= b->float_val);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_greater_than(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_greater_than(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_greater_than(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_greater_or_equal(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_logical_obj(a->int_val >= b->int_val);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_logical_obj(a->float_val >= b->float_val);
  }
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_equal(RAP_Object *a, RAP_Object *b) {
  // Materialize slices before comparing
  a = RAP_materialize_slice(a);
  b = RAP_materialize_slice(b);

  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_equal(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_float_equal(a, b);
  } else if (a->tag == RAP_OBJECT_TAG_TEXT && b->tag == RAP_OBJECT_TAG_TEXT) {
    struct RAP_Tuple *at = RAP_get_text_val(a);
    struct RAP_Tuple *bt = RAP_get_text_val(b);
    if (at->count != bt->count) return RAP_create_logical_obj(false);
    for (uint32_t i = 0; i < at->count; i++) {
      if (RAP_get_int_val(at->items[i]) != RAP_get_int_val(bt->items[i]))
        return RAP_create_logical_obj(false);
    }
    return RAP_create_logical_obj(true);
  } else if (a->tag == RAP_OBJECT_TAG_NULL && b->tag == RAP_OBJECT_TAG_NULL) {
    return RAP_create_logical_obj(true);
  } else if (a->tag == RAP_OBJECT_TAG_LOGICAL && b->tag == RAP_OBJECT_TAG_LOGICAL) {
    return RAP_create_logical_obj(a->logical_val == b->logical_val);
  } else if (a->tag == RAP_OBJECT_TAG_TUPLE && b->tag == RAP_OBJECT_TAG_TUPLE) {
    if (a->tuple_val->count != b->tuple_val->count) {
      return RAP_create_logical_obj(false);
    }
    for (uint32_t i = 0; i < a->tuple_val->count; i++) {
      if (!RAP_equal(a->tuple_val->items[i], b->tuple_val->items[i])->logical_val) {
        return RAP_create_logical_obj(false);
      }
    }
    return RAP_create_logical_obj(true);
  }
  printf("%s %d %d\n", RAP_stringify_object(a), a->tag, b->tag);
  RAP_fatal_error("Неподдерживаемые типы для сравнения");
}

RAP_Object *RAP_not_equal(RAP_Object *a, RAP_Object *b) {
  // Reuse RAP_equal and negate
  return RAP_create_logical_obj(!RAP_equal(a, b)->logical_val);
}

RAP_Object *RAP_modulo(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_integer_modulo(a, b);
  }
  RAP_fatal_error("Неподдерживаемые типы для модуля");
}

RAP_Object *RAP_negate(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(-1 * RAP_get_int_val(a));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(-1.0 * RAP_get_float_val(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для отрицания");
}

RAP_Object *RAP_length(RAP_Object *a) {
  // Text stores one codepoint per item, so count == character length
  if (a->tag == RAP_OBJECT_TAG_TEXT) {
    return RAP_create_int_obj(RAP_get_text_val(a)->count);
  }
  if (a->tag == RAP_OBJECT_TAG_TUPLE) {
    return RAP_create_int_obj(RAP_get_tuple_val(a)->count);
  }
  if (a->tag == RAP_OBJECT_TAG_SLICE) {
    return RAP_create_int_obj(a->slice_val->to - a->slice_val->from);
  }
  RAP_fatal_error("Неподдерживаемые типы для длины");
}

RAP_Object *RAP_power(RAP_Object *a, RAP_Object *b) {
  bool result_is_float = a->tag == RAP_OBJECT_TAG_FLOAT || b->tag == RAP_OBJECT_TAG_FLOAT;
  if (b->tag != RAP_OBJECT_TAG_INT && b->tag != RAP_OBJECT_TAG_FLOAT) {
    RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
  }
  double power_value = b->tag == RAP_OBJECT_TAG_INT ? RAP_get_int_val(b) : RAP_get_float_val(b);
  if (a->tag == RAP_OBJECT_TAG_INT) {
    if (result_is_float) {
      return RAP_create_float_obj(pow(RAP_get_int_val(a), power_value));
    } else {
      return RAP_create_int_obj(pow(RAP_get_int_val(a), power_value));
    }
  }
  else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    if (result_is_float) {
      return RAP_create_float_obj(pow(RAP_get_float_val(a), power_value));
    } else {
      return RAP_create_int_obj(pow(RAP_get_float_val(a), power_value));
    }
  }

  RAP_fatal_error("Неподдерживаемые типы для возведения в степень");
}
