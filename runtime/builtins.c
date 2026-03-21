#include "runtime_internal.h"

// BUILT-IN MATH FUNCTIONS

RAP_Object *RAP_abs(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(abs(RAP_get_int_val(a)));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(fabs(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для модуля");
}

RAP_Object *RAP_sqrt(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT || a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(sqrt(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для квадратного корня");
}

RAP_Object *RAP_floor(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_int_obj((int64_t)floor(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления вниз");
}

RAP_Object *RAP_ceil(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(ceil(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления вверх");
}

RAP_Object *RAP_round(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_int_obj((int64_t)round(RAP_get_float_val(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления");
}

RAP_Object *RAP_min(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(RAP_get_int_val(a) < RAP_get_int_val(b) ? RAP_get_int_val(a) : RAP_get_int_val(b));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(RAP_get_float_val(a) < RAP_get_float_val(b) ? RAP_get_float_val(a) : RAP_get_float_val(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для min");
}

RAP_Object *RAP_max(RAP_Object *a, RAP_Object *b) {
  if (a->tag == RAP_OBJECT_TAG_INT && b->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(RAP_get_int_val(a) > RAP_get_int_val(b) ? RAP_get_int_val(a) : RAP_get_int_val(b));
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT && b->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj(RAP_get_float_val(a) > RAP_get_float_val(b) ? RAP_get_float_val(a) : RAP_get_float_val(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для max");
}

RAP_Object *RAP_random(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    return RAP_create_float_obj((double)rand() / RAND_MAX * RAP_get_float_val(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для random");
}

RAP_Object *RAP_random_int(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    return RAP_create_int_obj(rand() % RAP_get_int_val(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для random_int");
}

RAP_Object *RAP_sign(RAP_Object *a) {
  if (a->tag == RAP_OBJECT_TAG_INT) {
    int64_t v = RAP_get_int_val(a);
    return RAP_create_int_obj(v < 0 ? -1 : v > 0 ? 1 : 0);
  } else if (a->tag == RAP_OBJECT_TAG_FLOAT) {
    double v = RAP_get_float_val(a);
    return RAP_create_int_obj(v < 0.0 ? -1 : v > 0.0 ? 1 : 0);
  }
  RAP_fatal_error("Неподдерживаемые типы для sign");
}
