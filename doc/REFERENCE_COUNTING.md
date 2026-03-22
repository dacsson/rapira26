# Reference Counting

## Overview

Every `RAP_Object` has an `atomic_int refcount` field. When refcount reaches 0, the object is freed immediately. No circular references are possible in Rapira (no mutable object graphs), so simple refcounting is sufficient.

## Rules

### Constructors set refcount = 1

Every `RAP_create_*_obj()` call returns an object with `refcount = 1`. The caller "owns" this reference.

### Incref when sharing a reference

When an existing object gains an additional owner:
- **Assignment from a name:** `КОПИЯ := ЧИСЛА` — the object now has two owners
- **Parameter entry:** function increfs each parameter on entry (takes ownership of the borrowed reference)
- **Copying into a new container:** `RAP_append_tuple`, `RAP_multiply` (repeat), `RAP_create_slice` all incref elements/parents they reference

### Decref when a reference is lost

- **Reassignment:** `X := новое_значение` — decref old value of X before overwriting
- **Frame cleanup:** `RAP_free_call_frame` decrefs all slot values when the frame is destroyed
- **Container element overwrite:** `RAP_set_tuple_item` decrefs the old element
- **Frame slot overwrite:** `RAP_frame_set` decrefs the old value when an existing slot is updated

### Free when refcount hits 0

`RAP_free_object` switches on tag to clean up inner allocations:

| Tag | Cleanup |
|-----|---------|
| `INT`, `FLOAT`, `LOGICAL`, `NULL` | Nothing — value is inline |
| `TUPLE` | Decref each element, free items array, free tuple struct |
| `TEXT` | Decref each codepoint, free items array, free text struct |
| `SLICE` | Decref parent, free slice struct |
| `CALLABLE` | Free name, params, frame, callable struct |

Then `free(obj)` for the object itself.

### NULL (пусто) is safe

`RAP_dec_ref(NULL)` and `RAP_inc_ref(NULL)` are no-ops. No special casing needed at call sites.

## Ownership Model

### Fresh objects (refcount = 1)

Created by constructors. The first assignment takes ownership — no incref needed:
```
ЧИСЛА := <* 1, 2, 3 *>   // tuple born with refcount=1, owned by ЧИСЛА
```

### Shared references (incref)

When assigning from an existing variable, incref because two owners now exist:
```
КОПИЯ := ЧИСЛА            // incref → refcount=2
```

### Parameters are borrowed → incref on entry

Function parameters come from `_args[]` which the caller owns. The function increfs each parameter to take ownership, so reassignment and scope exit can decref safely:
```c
RAP_Object *_local_N = _args[0];
RAP_inc_ref(_local_N);     // function now owns its reference
```

### Per-call frames

`RAP_call_callable_obj` creates a fresh child frame for each call. After the function returns, it increfs the return value (to keep it alive), then frees the frame (which decrefs all locals). The callable's own frame stays alive as the parent.

### Loop variables

For-loop counter variables (`_local_I`) are C-block-scoped inside the loop body. They are added to `declared_vars` on loop entry and removed on loop exit, so they don't appear in function-level cleanup.

### Slices keep parents alive

`RAP_create_slice` increfs the parent. The slice's destructor decrefs it. This prevents use-after-free when the original variable is reassigned:
```
T := <* 1, 2, 3, 4, 5 *>
S := T[1:3]               // slice increfs T → refcount=2
T := пусто                 // decref → refcount=1 (slice keeps it alive)
```

## Where incref happens in runtime functions

Any runtime function that copies an object pointer into a new container must incref it:

- `RAP_append_tuple` — increfs each element from both source tuples
- `RAP_multiply` (tuple repeat) — increfs each element for every copy
- `RAP_multiply` (text repeat) — increfs each codepoint for every copy
- `RAP_create_slice` — increfs the parent
- `RAP_materialize_slice` — copies elements without incref (fresh tuple takes ownership, slice still holds parent ref)
