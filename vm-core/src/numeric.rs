//! Defines some numeric constraints for memory reads

/// Convert between a primitive type and an array of bytes in little-endian
/// order.
pub trait LeBytes {
    /// The `[u8; N]` array type holding the serialized bytes for this value.
    type Bytes: AsRef<[u8]> + for<'a> TryFrom<&'a [u8], Error = std::array::TryFromSliceError>;

    fn from_le_bytes(bytes: Self::Bytes) -> Self;
    fn to_le_bytes(self) -> Self::Bytes;
}

macro_rules! impl_le_bytes {
    ($type:ty, $size:literal) => {
        impl LeBytes for $type {
            type Bytes = [u8; $size];

            fn from_le_bytes(bytes: Self::Bytes) -> Self {
                <$type>::from_le_bytes(bytes)
            }

            fn to_le_bytes(self) -> Self::Bytes {
                <$type>::to_le_bytes(self)
            }
        }
    };
}

// Possible byte-size types we are able to read from memory
impl_le_bytes!(i8, 1);
impl_le_bytes!(u8, 1);
impl_le_bytes!(i32, 4);
impl_le_bytes!(u32, 4);
impl_le_bytes!(i64, 8);
impl_le_bytes!(u64, 8);
