#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![feature(explicit_tail_calls)]

use core::ffi::{CStr, c_char, c_void};

use crate::object::Object;

mod frame;
pub mod interpreter;
pub mod object;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

const CONS_TAG_HASH: i64 = 1697575;
const NIL_TAG_HASH: i64 = 115865;

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

// #  define DATA_HEADER_SZ (sizeof(auint) + sizeof(ptrt))
const DATA_HEADER_SZ: usize = core::mem::size_of::<auint>() + core::mem::size_of::<ptrt>();

// define TO_DATA(x) ((data *)((char *)(x)-DATA_HEADER_SZ))
#[inline(always)]
fn rtToData(x: *mut c_void) -> *mut data {
    unsafe { (x as *mut u8).offset(-(DATA_HEADER_SZ as isize)) as *mut data }
}

// #define TO_SEXP(x) ((sexp *)((char *)(x)-DATA_HEADER_SZ))
#[inline(always)]
fn rtToSexp(x: *mut c_void) -> *mut sexp {
    unsafe { (x as *mut u8).offset(-(DATA_HEADER_SZ as isize)) as *mut sexp }
}

// #define LEN_MASK (UINT64_MAX^7)
const LEN_MASK: u64 = u64::MAX ^ 7;

// #define LEN(x) (ptrt)(((ptrt)x & LEN_MASK) >> 3)
#[inline(always)]
fn rtLen(x: u64) -> ptrt {
    ((x & LEN_MASK) >> 3) as ptrt
}

// #define TAG(x) (x & 7)
#[inline(always)]
fn rtTag(x: u64) -> i32 {
    (x & 7) as i32
}

/// Create a new S-expression with the given tag and arguments.
/// Returns a pointer to *contents* of the S-expression.
/// To retrieve the actual S-expression, use `rtToSexp`.
#[inline(always)]
fn new_sexp(tag: &CStr, args: &mut [Object]) -> *mut c_void {
    let ptr = args.as_mut_ptr() as *mut i64;

    unsafe {
        let tag_hash = if tag.to_bytes() == "cons".as_bytes() {
            CONS_TAG_HASH
        } else if tag.to_bytes() == "nil".as_bytes() {
            NIL_TAG_HASH
        } else {
            // WARNING: We are responsible for ensuring lifetime of the CStr
            //          but because LtagHash doesn't write to the CStr, i think it's safe to cast here
            LtagHash(tag.as_ptr() as *mut c_char)
        };

        if let Some(last) = args.last_mut() {
            *last = Object::new_unboxed(tag_hash);
        }

        Bsexp(
            ptr,                      /* [args_1,...,arg_n, tag] */
            rtBox(args.len() as i64), /* n args */
        )
    }
}

/// Create a new lama string.
#[inline(always)]
fn new_string(bytes: &[u8]) -> Result<*mut c_void, core::ffi::FromBytesWithNulError> {
    unsafe {
        let c_string = CStr::from_bytes_with_nul(bytes)?;
        let as_ptr = c_string.as_ptr();

        let mut slice: [i64; 1] = [as_ptr as i64];

        Ok(Bstring(slice.as_mut_ptr()))
    }
}

/// Create array from given elements.
/// Returns a pointer to *contents* of the array.
/// To retrieve the actual array, use `rtToData`.
#[inline(always)]
fn new_array(elements: &mut [Object]) -> *mut c_void {
    let ptr = elements.as_mut_ptr() as *mut i64;
    unsafe {
        Barray(
            ptr,                          /* [args_1,...,arg_n, tag] */
            rtBox(elements.len() as i64), /* n args */
        )
    }
}

/// Remember that arrays store raw values, meaning callee is responsible for unboxing them.
#[inline(always)]
fn get_array_el(arr: &data, index: usize) -> i64 {
    unsafe {
        let contents = arr.contents.as_ptr() as *const crate::object::Object;
        let obj = contents.add(index).read();
        obj.raw()
    }
}

/// Create a new closure object
/// Returns a pointer to *contents* of the closure.
/// To retrieve the actual closure, use `rtToData`.
#[inline(always)]
fn new_closure(args: &mut [Object]) -> *mut c_void {
    let ptr = args.as_mut_ptr() as *mut i64;

    unsafe {
        Bclosure(
            ptr,                      /* [args_1,...,arg_n, tag] */
            rtBox(args.len() as i64), /* n args */
        )
    }
}

#[inline(always)]
fn get_captured_variable(closure: &data, index: usize) -> i64 {
    unsafe {
        // index + 1 because the first element is the offset
        (closure.contents.as_ptr() as *const i64)
            .add(index + 1)
            .read()
    }
}

#[inline(always)]
fn set_captured_variable(closure: &mut data, index: usize, value: i64) {
    unsafe {
        // index + 1 because the first element is the offset
        (closure.contents.as_ptr() as *mut i64)
            .add(index + 1)
            .write(value);
    }
}

/// Callee is responsible for ensuring that index is within bounds.
#[inline(always)]
fn set_array_el(arr: &mut data, index: usize, value: i64) {
    unsafe {
        (arr.contents.as_ptr() as *mut i64).add(index).write(value);
    }
}

/// Remember that S-expressions store raw values, meaning callee is responsible for unboxing them.
#[inline(always)]
fn get_sexp_el(sexp: &sexp, index: usize) -> i64 {
    unsafe { (sexp.contents.as_ptr() as *const i64).add(index).read() }
}

/// Callee is responsible for ensuring that index is within bounds.
#[inline(always)]
fn set_sexp_el(sexp: &mut sexp, index: usize, value: i64) {
    unsafe {
        (sexp.contents.as_ptr() as *mut i64).add(index).write(value);
    }
}

/// Given a c_void pointer to arbitrary data (of Lama aggregate type),
/// returns the tag of the data.
fn get_data_tag(ptr: *mut c_void) -> i32 {
    unsafe {
        let data = rtToData(ptr) as *const data;
        let header = (*data).data_header;
        rtTag(header as u64)
    }
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
