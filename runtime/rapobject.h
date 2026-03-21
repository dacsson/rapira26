#ifndef RAPIRA_OBJECT_H
#define RAPIRA_OBJECT_H

#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>

/// From spec 2.2.2:
/// Объекты:
/// Пустой
/// Логический
/// Процедура
/// Функция
/// Целый
/// Вещественный
/// Текст
/// Кортеж
typedef enum {
  RAP_OBJECT_TAG_NULL,
  RAP_OBJECT_TAG_LOGICAL,
  RAP_OBJECT_TAG_CALLABLE, // unifies proc and func
  RAP_OBJECT_TAG_INT,
  RAP_OBJECT_TAG_FLOAT,
  RAP_OBJECT_TAG_TEXT,
  RAP_OBJECT_TAG_TUPLE,
  RAP_OBJECT_TAG_SLICE,
} RAP_ObjectTag;

struct RAP_Tuple;
struct RAP_Callable;
/// Each object in the runtime has a tag indicating its type and a union of
/// possible values for that type.
typedef struct {
  RAP_ObjectTag tag;
  union {
    bool logical_val;
    int64_t int_val;
    double float_val;
    struct RAP_Tuple *text_val;
    struct RAP_Tuple *tuple_val;
    struct RAP_Callable *callable_val;
    struct RAP_Slice *slice_val;
  };
} RAP_Object;

/// From spec 2.2.2:
/// Параметры:
/// Входные - ['=>']
/// Возвратные - ['<=']
typedef enum {
  RAP_PARAMETER_MODE_IN,
  RAP_PARAMETER_MODE_OUT,
} RAP_ParameterMode;

typedef struct {
  RAP_ParameterMode mode;
  char *name;
} RAP_Parameter;

/// Named variable slot in a call frame.
struct RAP_FrameSlot {
  const char *name;
  RAP_Object *value;
};

/// Each function keeps some context about 'чужие' and 'свои' scoped variables.
/// Variables have dynamic scope and are looked up in the call frame chain.
struct RAP_CallFrame {
  struct RAP_CallFrame *parent;
  struct RAP_FrameSlot *slots;
  uint32_t slot_count;
};

/// Funcs and procs are treated as objects.
struct RAP_Callable {
  char *name;
  RAP_Object *(*func)(struct RAP_CallFrame *frame, RAP_Object **args,
                      unsigned int arg_count);
  struct RAP_CallFrame *frame;
  RAP_Parameter **params;
  uint32_t param_count;
};

/// Tuple is a untyped list of objects.
struct RAP_Tuple {
  uint32_t count;
  RAP_Object **items;
};

/// Unified function type
typedef RAP_Object *(*RAP_FunctionDecl)(struct RAP_CallFrame *frame,
                                        RAP_Object **args, uint32_t arg_count);

/// Slice: a view into a tuple (or text).
/// Holds a pointer to the parent and 0-based [from, to) bounds.
/// Slices of slices flatten to the root parent (no chaining).
struct RAP_Slice {
  RAP_Object *parent;  // the tuple/text we're viewing
  int64_t from;        // inclusive start
  int64_t to;          // exclusive end
};

#endif // RAPIRA_OBJECT_H
