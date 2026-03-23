#include "runtime_internal.h"

// BUILT-IN MATH FUNCTIONS

RAP_Value RAP_abs(RAP_Value a) {
  if (RAP_IS_SMI(a)) {
    return RAP_create_int_obj(abs(RAP_SMI_VALUE(a)));
  } else if (RAP_IS_DOUBLE(a)) {
    return RAP_create_float_obj(fabs(RAP_DOUBLE_VALUE(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для модуля");
}

RAP_Value RAP_sqrt(RAP_Value a) {
  if (RAP_IS_SMI(a) || RAP_IS_DOUBLE(a)) {
    return RAP_create_float_obj(sqrt(RAP_DOUBLE_VALUE(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для квадратного корня");
}

RAP_Value RAP_floor(RAP_Value a) {
  if (RAP_IS_DOUBLE(a)) {
    return RAP_create_int_obj((int64_t)floor(RAP_DOUBLE_VALUE(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления вниз");
}

RAP_Value RAP_ceil(RAP_Value a) {
  if (RAP_IS_DOUBLE(a)) {
    return RAP_create_float_obj(ceil(RAP_DOUBLE_VALUE(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления вверх");
}

RAP_Value RAP_round(RAP_Value a) {
  if (RAP_IS_DOUBLE(a)) {
    return RAP_create_int_obj((int64_t)round(RAP_DOUBLE_VALUE(a)));
  }
  RAP_fatal_error("Неподдерживаемые типы для округления");
}

RAP_Value RAP_min(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_create_int_obj(RAP_SMI_VALUE(a) < RAP_SMI_VALUE(b) ? RAP_SMI_VALUE(a) : RAP_SMI_VALUE(b));
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) < RAP_DOUBLE_VALUE(b) ? RAP_DOUBLE_VALUE(a) : RAP_DOUBLE_VALUE(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для min");
}

RAP_Value RAP_max(RAP_Value a, RAP_Value b) {
  if (RAP_IS_SMI(a) && RAP_IS_SMI(b)) {
    return RAP_create_int_obj(RAP_SMI_VALUE(a) > RAP_SMI_VALUE(b) ? RAP_SMI_VALUE(a) : RAP_SMI_VALUE(b));
  } else if (RAP_IS_DOUBLE(a) && RAP_IS_DOUBLE(b)) {
    return RAP_create_float_obj(RAP_DOUBLE_VALUE(a) > RAP_DOUBLE_VALUE(b) ? RAP_DOUBLE_VALUE(a) : RAP_DOUBLE_VALUE(b));
  }
  RAP_fatal_error("Неподдерживаемые типы для max");
}

RAP_Value RAP_random(RAP_Value a) {
  if (RAP_IS_DOUBLE(a)) {
    return RAP_create_float_obj((double)rand() / RAND_MAX * RAP_DOUBLE_VALUE(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для random");
}

RAP_Value RAP_random_int(RAP_Value a) {
  if (RAP_IS_SMI(a)) {
    return RAP_create_int_obj(rand() % RAP_SMI_VALUE(a));
  }
  RAP_fatal_error("Неподдерживаемые типы для random_int");
}

RAP_Value RAP_sign(RAP_Value a) {
  if (RAP_IS_SMI(a)) {
    int64_t v = RAP_SMI_VALUE(a);
    return RAP_create_int_obj(v < 0 ? -1 : v > 0 ? 1 : 0);
  } else if (RAP_IS_DOUBLE(a)) {
    double v = RAP_DOUBLE_VALUE(a);
    return RAP_create_int_obj(v < 0.0 ? -1 : v > 0.0 ? 1 : 0);
  }
  RAP_fatal_error("Неподдерживаемые типы для sign");
}
