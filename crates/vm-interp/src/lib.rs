#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![feature(explicit_tail_calls)]
#![feature(array_repeat)]

mod frame;
pub mod interpreter;
pub mod object;

#[cfg(test)]
mod interp_tests;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// #define RAP_IS_SMI(value) (((value) & RAP_TAG_MASK) == 0x0)
pub fn isSMI(data: usize) -> bool {
    (data & (RAP_TAG_MASK as usize)) == 0x0
}

// #define RAP_IS_BOOL(value) (((value) & RAP_TAG_MASK) == 0x1)
pub fn isBool(data: usize) -> bool {
    (data & (RAP_TAG_MASK as usize)) == 0x1
}

// #define RAP_IS_PTR(value) (((value) & RAP_TAG_MASK) == 0x3)
pub fn isPtr(data: usize) -> bool {
    (data & (RAP_TAG_MASK as usize)) == 0x3
}
