use std::{path::PathBuf, str::FromStr};

use crate::Error;

/// A specialized `FromStr` trait that returns [`Error`] errors
pub trait Parse: Sized {
    /// Parse `Self` from `string`.
    fn parse(string: &str) -> Result<Self, Error>;
}

impl Parse for bool {
    fn parse(string: &str) -> Result<Self, Error> {
        Self::from_str(string).map_err(Error::from)
    }
}

macro_rules! impl_parse_for_int_type {
    ($($int_type:ty),+ $(,)?) => {
        $(
            impl Parse for $int_type {
                fn parse(string: &str) -> Result<Self, Error> {
                    Self::from_str(string).map_err(Error::from)
                }
            }
        )+
    }
}
impl_parse_for_int_type![i8, i16, i32, i64, i128, u8, u16, u32, u64, u128];

impl Parse for String {
    fn parse(string: &str) -> Result<Self, Error> {
        Ok(string.to_string())
    }
}

impl Parse for PathBuf {
    fn parse(string: &str) -> Result<Self, Error> {
        Ok(PathBuf::from(string))
    }
}
