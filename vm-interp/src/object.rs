//! Interpreter Objet type description

use crate::{get_obj_header_ptr, get_type_header_ptr, isUnboxed, lama_type, rtBox, rtUnbox};
// use std::fmt::{Debug, Display, Formatter};
// use std::os::raw::c_void;
use core::ffi::{CStr, c_void};
use core::fmt::{Debug, Display, Formatter};

/// An element of operand stack in interpreter:
/// - Pointers (should be) are boxed due to alignment
/// - Other objects get boxed on creation and unboxed on usage
#[derive(Debug, Clone, Copy)]
pub struct Object {
    data: i64,
}

#[derive(Debug, PartialEq)]
pub enum ObjectError {
    CreatingUnboxedObjectWithBoxedValue,
    CreatingBoxedObjectWithAlreadyBoxedValue,
}

impl Display for ObjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            ObjectError::CreatingUnboxedObjectWithBoxedValue => {
                write!(f, "Creating unboxed object with boxed value")
            }
            ObjectError::CreatingBoxedObjectWithAlreadyBoxedValue => {
                write!(f, "Creating boxed object with already boxed value")
            }
        }
    }
}

impl core::error::Error for ObjectError {}

impl Object {
    fn new(data: i64) -> Self {
        Object { data }
    }

    /// Create object via boxing provided value
    pub fn new_boxed(value: i64) -> Self {
        unsafe { Object::new(rtBox(value)) }
    }

    /// Create object via raw provided value
    pub fn new_unboxed(value: i64) -> Self {
        Object::new(value)
    }

    /// Creates a new empty unboxed object with a default value of 0
    pub fn new_empty() -> Self {
        Object::new(0)
    }

    /// Retrieve objects inner value, returns unboxed value if it was boxed
    // pub fn unwrap(&self) -> i64 {
    //     if isUnboxed(self.data) {
    //         self.data
    //     } else {
    //         unsafe { rtUnbox(self.data) }
    //     }
    // }

    pub fn unbox(&self) -> i64 {
        unsafe { rtUnbox(self.data) }
    }

    // pub fn unwrap_boxed(&self) -> i64 {
    //     unsafe { rtUnbox(self.data) }
    // }

    /// Retrieve objects raw underlying value, without any translation
    pub fn raw(&self) -> i64 {
        self.data
    }

    /// Data is stored as raw i64, hence we need to translate back to pointer
    /// iff it was created from pointer
    pub fn as_ptr<T>(&self) -> Option<*const T> {
        if isUnboxed(self.data) {
            None
        } else {
            Some(self.data as *const T)
        }
    }

    /// [`as_ptr`]
    pub fn as_ptr_mut<T>(&self) -> Option<*mut T> {
        if isUnboxed(self.data) {
            None
        } else {
            Some(self.data as *mut T)
        }
    }

    /// [`as_ptr`]
    pub fn as_ptr_mut_unchecked<T>(&self) -> *mut T {
        self.data as *mut T
    }

    /// Get lama type of object
    pub fn lama_type(&mut self) -> Option<lama_type> {
        unsafe {
            if isUnboxed(self.data) {
                None
            } else if let Some(as_ptr) = self.as_ptr_mut::<c_void>() {
                let header_ptr = get_obj_header_ptr(as_ptr);
                Some(get_type_header_ptr(header_ptr))
            } else {
                None
            }
        }
    }
}

/// Create unboxed object from string pointer.
/// Pointers come boxed by default due to alignment requirements.
impl TryFrom<*const i8> for Object {
    type Error = ();

    fn try_from(ptr: *const i8) -> Result<Self, Self::Error> {
        Ok(Object::new(ptr.addr() as i64))
    }
}

/// Create unboxed object from pointer to aggreggate type contents.
/// Pointers come boxed by default due to alignment requirements.
impl TryFrom<*mut c_void> for Object {
    type Error = ();

    fn try_from(ptr: *mut c_void) -> Result<Self, Self::Error> {
        Ok(Object::new(ptr.addr() as i64))
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        if isUnboxed(self.data) {
            write!(f, "Unboxed({})", self.data)
        } else {
            let unboxed = unsafe { rtUnbox(self.data) };
            write!(f, "Boxed({}) | {}", self.data, unboxed)
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::rtToData;
//     use std::ffi::{CStr, CString};

//     /// Test creation of objects and that runtime will
//     /// detect them either boxed or unboxed properly
//     #[test]
//     fn test_creation() {
//         let obj1 = Object::new_boxed(1);
//         let obj2 = Object::new_unboxed(1);
//         let obj3 = Object::new_empty();
//         let obj4 = Object::new_boxed(2);

//         unsafe {
//             if let Object::Boxed(v) = obj1 {
//                 assert_eq!(v, rtBox(1));
//             }
//             assert_eq!(obj1.unwrap(), rtUnbox(rtBox(1)));
//             if let Object::Unboxed(v) = obj2 {
//                 assert_eq!(v, 1);
//             }
//             assert_eq!(obj2.unwrap(), 1);
//             assert_eq!(obj3.unwrap(), 0);
//             if let Object::Boxed(v) = obj4 {
//                 assert_eq!(v, rtBox(2));
//             }
//             assert_eq!(obj4.unwrap(), rtUnbox(rtBox(2)));
//         }
//     }

//     #[test]
//     fn test_create_from_string() -> Result<(), Box<dyn std::error::Error>> {
//         let c_string = CString::new("main")?;
//         let raw_ptr: *const i8 = c_string.into_raw();
//         let obj = Object::try_from(raw_ptr).map_err(|_| "Error at Object::try_from(raw_ptr)")?;

//         unsafe {
//             let as_ptr = obj.as_ptr_mut().ok_or("Failed to get pointer")?;
//             let contents = (*rtToData(as_ptr)).contents.as_ptr();
//             let c_string_again = CStr::from_ptr(contents);

//             assert_eq!(*c_string_again, CString::new("main")?);
//         }

//         Ok(())
//     }
// }
