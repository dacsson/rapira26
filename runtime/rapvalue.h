#ifndef RAPVALUE_H
#define RAPVALUE_H

#include <stdint.h>
#include <string.h>

// Tagged pointer, representation on 64-bit systems:
// ```
//             |----- 32 bits -----|----- 32 bits -----|
// Pointer:    |________________address______________11|
// Smi:        |____int32_value____|0000000000000000000|
// Boolean:    |______bool_value___|0000000000000000001|
// ```
//
// First and second least significant bit distinguishes between Pointer, SMI,
// and Boolean values.
// - For reference: https://v8.dev/blog/pointer-compression
typedef uintptr_t RAP_Value;

// Two least significant bits are used as a tag:
// 00 = SMI, 01 = boolean, 11 = pointer
#define RAP_TAG_MASK 0x3

// Checks if RAP_Value is a 32-bit integer
#define RAP_IS_SMI(value) (((value) & RAP_TAG_MASK) == 0x0)
// Checks if RAP_Value is a boolean
#define RAP_IS_BOOL(value) (((value) & RAP_TAG_MASK) == 0x1)
// Checks if RAP_Value is a pointer
#define RAP_IS_PTR(value) (((value) & RAP_TAG_MASK) == 0x3)

// Get 32-bit integer value from RAP_Value
#define RAP_SMI_VALUE(value) ((int32_t)((value) >> 32))
// Get pointer value from RAP_Value
#define RAP_PTR_VALUE(value)                                                   \
  ((RAP_Object *)((value) & ~(uintptr_t)RAP_TAG_MASK))
// Get boolean value from RAP_Value
#define RAP_BOOL_VALUE(value) (((value) >> 2) & 1)

// Create 32-bit integer RAP_Value, `uintptr_t` will be just casted to int
#define RAP_CREATE_SMI(value) ((RAP_Value)((uintptr_t)(int32_t)(value) << 32))
// Create boolean RAP_Value, `uintptr_t` will be just casted
#define RAP_CREATE_BOOL(value)                                                 \
  ((RAP_Value)(((uintptr_t)(!!(value)) << 2) | 0x1))
// Create pointer RAP_Value
#define RAP_CREATE_PTR(ptr) ((RAP_Value)((uintptr_t)(ptr) | 0x3))

#endif // RAPVALUE_H
