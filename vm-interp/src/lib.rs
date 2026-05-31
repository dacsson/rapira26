#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![feature(explicit_tail_calls)]
#![feature(array_repeat)]

use core::ffi::{CStr, c_void};

use crate::object::Object;

mod frame;
pub mod interpreter;
pub mod object;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// #define UNBOX(x) (((aint)(x)) >> 1)
#[inline(always)]
fn rtUnbox(x: i64) -> i64 {
    ((x as i64) >> 1) as i64
}

// #define BOX(x) ((((aint)(x)) << 1) | 1)
#[inline(always)]
fn rtBox(x: i64) -> i64 {
    (((x as i64) << 1) | 1) as i64
}

// #define UNBOXED(x) (((aint)(x)) & 1)
#[inline(always)]
fn isUnboxed(x: i64) -> bool {
    ((x as i64) & 1) == 1
}

/// Create a new lama string.
#[inline(always)]
fn new_string(bytes: &[u8]) -> Result<*mut c_void, core::ffi::FromBytesWithNulError> {
    // unsafe {
    //     let c_string = CStr::from_bytes_with_nul(bytes)?;
    //     let as_ptr = c_string.as_ptr();

    //     let mut slice: [i64; 1] = [as_ptr as i64];

    //     Ok(Bstring(slice.as_mut_ptr()))
    // }

    todo!()
}

/// Create array from given elements.
/// Returns a pointer to *contents* of the array.
/// To retrieve the actual array, use `rtToData`.
#[inline(always)]
fn new_array(elements: &mut [Object]) -> *mut c_void {
    // let ptr = elements.as_mut_ptr() as *mut i64;
    // unsafe {
    //     Barray(
    //         ptr,                          /* [args_1,...,arg_n, tag] */
    //         rtBox(elements.len() as i64), /* n args */
    //     )
    // }
    todo!()
}

/// Remember that arrays store raw values, meaning callee is responsible for unboxing them.
#[inline(always)]
fn get_array_el(arr: &data, index: usize) -> i64 {
    // unsafe {
    //     let contents = arr.contents.as_ptr() as *const crate::object::Object;
    //     let obj = contents.add(index).read();
    //     obj.raw()
    // }

    todo!()
}

/// Create a new closure object
/// Returns a pointer to *contents* of the closure.
/// To retrieve the actual closure, use `rtToData`.
#[inline(always)]
fn new_closure(args: &mut [Object]) -> *mut c_void {
    // let ptr = args.as_mut_ptr() as *mut i64;

    // unsafe {
    //     Bclosure(
    //         ptr,                      /* [args_1,...,arg_n, tag] */
    //         rtBox(args.len() as i64), /* n args */
    //     )
    // }

    todo!()
}

#[inline(always)]
fn get_captured_variable(closure: &data, index: usize) -> i64 {
    // unsafe {
    //     // index + 1 because the first element is the offset
    //     (closure.contents.as_ptr() as *const i64)
    //         .add(index + 1)
    //         .read()
    // }

    todo!()
}

#[inline(always)]
fn set_captured_variable(closure: &mut data, index: usize, value: i64) {
    // unsafe {
    //     // index + 1 because the first element is the offset
    //     (closure.contents.as_ptr() as *mut i64)
    //         .add(index + 1)
    //         .write(value);
    // }

    todo!()
}

/// Callee is responsible for ensuring that index is within bounds.
#[inline(always)]
fn set_array_el(arr: &mut data, index: usize, value: i64) {
    // unsafe {
    //     (arr.contents.as_ptr() as *mut i64).add(index).write(value);
    // }

    todo!()
}

/// Remember that S-expressions store raw values, meaning callee is responsible for unboxing them.
#[inline(always)]
fn get_sexp_el(sexp: &sexp, index: usize) -> i64 {
    // unsafe { (sexp.contents.as_ptr() as *const i64).add(index).read() }

    todo!()
}

/// Callee is responsible for ensuring that index is within bounds.
#[inline(always)]
fn set_sexp_el(sexp: &mut sexp, index: usize, value: i64) {
    // unsafe {
    //     (sexp.contents.as_ptr() as *mut i64).add(index).write(value);
    // }

    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_link_smoke_test() {
        assert_eq!(isUnboxed(0), false);
        assert_eq!(isUnboxed(1), true);
    }
}
