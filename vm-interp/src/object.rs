//! Interpreter Objet type description

use crate::{RAP_Value, isSMI};

/// A wrapper around RAP_VALUE defined in runtime
///
/// See runtime/rapvalue.h for detailed description
#[derive(Clone, Copy)]
pub struct Object(RAP_Value);

impl Object {
    /// Wrap a raw [`RAP_Value`] (e.g. a result handed back by the runtime).
    pub fn new(data: usize) -> Self {
        Object(data)
    }

    /// Empty placeholder slot (raw 0). Used for globals and uninitialized
    /// frame metadata, which is read back via [`raw`](Self::raw).
    pub fn new_empty() -> Self {
        Object(0)
    }

    /// Store an integer verbatim, without any tagging.
    ///
    /// Frame metadata (arg/local counts, return ip, frame pointers) is read
    /// back via [`raw`](Self::raw) in `frame.rs`, so these internal values must
    /// round-trip as plain machine integers — not tagged SMIs.
    pub fn new_unboxed(value: i64) -> Self {
        Object(value as usize)
    }

    /// Box a user integer as a Rapira SMI.
    ///
    /// Mirrors `RAP_CREATE_SMI` from `runtime/rapvalue.h`.
    pub fn new_boxed(value: i64) -> Self {
        Object((value << 32) as usize)
    }

    /// Create a new boolean object.
    ///
    /// Mirrors `RAP_CREATE_BOOL` from `runtime/rapvalue.h`.
    pub fn new_bool(value: bool) -> Self {
        Object((value as usize) << 32 | 2)
    }

    /// Extract the integer from a SMI. Mirrors `RAP_SMI_VALUE`.
    pub fn unbox(&self) -> i64 {
        (self.0 as i64) >> 32
    }

    /// Retrieve objects raw underlying value, without any translation
    pub fn raw(&self) -> usize {
        self.0
    }

    /// Data is stored as tagged ptr, hence we need to translate back to pointer
    /// iff it was created from pointer
    pub fn as_ptr<T>(&self) -> Option<*const T> {
        if isSMI(self.0) {
            None
        } else {
            Some(self.0 as *const T)
        }
    }

    /// [`as_ptr`]
    pub fn as_ptr_mut<T>(&self) -> Option<*mut T> {
        if isSMI(self.0) {
            None
        } else {
            Some(self.0 as *mut T)
        }
    }

    /// [`as_ptr`]
    pub fn as_ptr_mut_unchecked<T>(&self) -> *mut T {
        self.0 as *mut T
    }
}

/// Errors raised while constructing or coercing [`Object`]s.
#[derive(Debug, PartialEq, Eq)]
pub enum ObjectError {
    InvalidPointer,
}

impl core::fmt::Display for ObjectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ObjectError::InvalidPointer => write!(f, "invalid object pointer"),
        }
    }
}
