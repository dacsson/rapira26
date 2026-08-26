#ifndef RAPVALUE_H
#define RAPVALUE_H

#include <stdint.h>
#include <string.h>
#include <stdbool.h>

// Tagged pointer representation on 64-bit systems:
// ```
//             |----- 32 bits -----|----- 32 bits -----|
// Pointer:    |________________address______________11|
// Smi:        |____int32_value____|0000000000000000000|
// Boolean:    |______bool_value___|0000000000000000001|
// ```
//
// The first two least significant bits distinguish pointers, SMIs, and
// booleans. On 32-bit targets (including wasm32), an SMI occupies the upper
// 30 bits instead. Keeping the tag in the low bits makes heap pointers and
// booleans ABI-compatible across both layouts.
// - For reference: https://v8.dev/blog/pointer-compression
typedef uintptr_t RAP_Value;

// Two least significant bits are used as a tag:
// 00 = SMI, 01 = boolean, 11 = pointer
#define RAP_TAG_MASK 0x3

// Checks if RAP_Value is a 32-bit integer
bool RAP_IS_SMI(RAP_Value value);
// Checks if RAP_Value is a boolean
bool RAP_IS_BOOL(RAP_Value value);
// Checks if RAP_Value is a pointer
bool RAP_IS_PTR(RAP_Value value);

#if UINTPTR_MAX == UINT64_MAX
#define RAP_SMI_BITS 32
#define RAP_SMI_MIN INT32_MIN
#define RAP_SMI_MAX INT32_MAX

static inline int32_t RAP_SMI_VALUE(RAP_Value value) {
  return (int32_t)(value >> 32);
}

#define RAP_CREATE_SMI(value)                                                  \
  ((RAP_Value)((uint64_t)(uint32_t)(int32_t)(value) << 32))

#elif UINTPTR_MAX == UINT32_MAX
// FIXME: remove when tagger ptr compression lands
#define RAP_SMI_BITS 30
#define RAP_SMI_MIN (-536870912)
#define RAP_SMI_MAX 536870911

static inline int32_t RAP_SMI_VALUE(RAP_Value value) {
  // The payload is always divisible by four, so signed division recovers
  return (int32_t)(uint32_t)value / 4;
}

#define RAP_CREATE_SMI(value)                                                  \
  ((RAP_Value)((uint32_t)(int32_t)(value) << 2))
#else
#error "Rapira requires 32-bit or 64-bit uintptr_t"
#endif

#define RAP_SMI_FITS(value)                                                    \
  ((int64_t)(value) >= (int64_t)RAP_SMI_MIN &&                                 \
   (int64_t)(value) <= (int64_t)RAP_SMI_MAX)

// Get pointer value from RAP_Value
#define RAP_PTR_VALUE(value)                                                   \
  ((RAP_Object *)((value) & ~(uintptr_t)RAP_TAG_MASK))
// Get boolean value from RAP_Value
#define RAP_BOOL_VALUE(value) (((value) >> 2) & 1)

// Create boolean RAP_Value, `uintptr_t` will be just casted
#define RAP_CREATE_BOOL(value)                                                 \
  ((RAP_Value)(((uintptr_t)(!!(value)) << 2) | 0x1))
// Create pointer RAP_Value
#define RAP_CREATE_PTR(ptr) ((RAP_Value)((uintptr_t)(ptr) | 0x3))

#endif // RAPVALUE_H
