#include "../runtime.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

// функ ПРОСТОЕ (N)
RAP_Object *RAP_FUNC_PROSTOE(struct RAP_CallFrame *_frame,
                              RAP_Object **_args, unsigned int _argc) {
  // param: N
  RAP_Object *_local_N = _args[0];

  // если N < 2 то
  RAP_Object *_t0 = RAP_create_int_obj(2);
  RAP_Object *_t1 = RAP_integer_less_than(_local_N, _t0);
  if (_t1->logical_val) {
    // возврат нет
    return RAP_create_logical_obj(false);
  }
  // все

  // для M от 2 до корень(N) + 0.5 цикл
  RAP_Object *_t2 = RAP_create_int_obj(2);
  RAP_Object *_t3 = RAP_create_float_obj(sqrt((double)RAP_get_int_val(_local_N)) + 0.5);
  for (int64_t _iter_M = RAP_get_int_val(_t2); _iter_M <= (int64_t)RAP_get_float_val(_t3); _iter_M++) {
    RAP_Object *_local_M = RAP_create_int_obj(_iter_M);

    // если N /% M = 0 то
    RAP_Object *_t4 = RAP_integer_modulo(_local_N, _local_M);
    RAP_Object *_t5 = RAP_create_int_obj(0);
    RAP_Object *_t6 = RAP_integer_equal(_t4, _t5);
    if (_t6->logical_val) {
      // возврат нет
      return RAP_create_logical_obj(false);
    }
    // все
  }
  // кц

  // возврат да
  return RAP_create_logical_obj(true);
}

/* ── top-level ─────────────────────────────────────────────────────────── */

int main(void) {
  struct RAP_CallFrame _main_frame = {NULL, NULL, 0};

  // функ ПРОСТОЕ -> callable
  RAP_Parameter *_p0 = RAP_create_parameter(RAP_PARAMETER_MODE_IN, "N");
  RAP_Object *_local_PROSTOE =
      RAP_create_callable_obj(&_main_frame, &RAP_FUNC_PROSTOE, &_p0, 1);

  // вывод: ПРОСТОЕ(2003)
  RAP_Object *_t0 = RAP_create_int_obj(2003);
  RAP_Object *_t1 = RAP_call_callable_obj(_local_PROSTOE, &_t0, 1);
  char *_s0 = RAP_stringify_object(_t1);
  printf("%s\n", _s0);

  // вывод: ПРОСТОЕ(2004)
  RAP_Object *_t2 = RAP_create_int_obj(2004);
  RAP_Object *_t3 = RAP_call_callable_obj(_local_PROSTOE, &_t2, 1);
  char *_s1 = RAP_stringify_object(_t3);
  printf("%s\n", _s1);

  return 0;
}
