use std::{env::VarError, io, num::ParseIntError, path::PathBuf, str::ParseBoolError};

use derive_more::{Display, Error};
use toml::de::Error as TomlError;

/// Errors that can be returned from methods in this crate.
#[derive(Debug, Display, Error)]
pub enum Error {
    /// Environment variable does not exists or is invalid.
    #[display(fmt = "Env var error: {_0}")]
    EnvVarError(VarError),

    /// File already exists on disk.
    #[display(fmt = "File exists: {}", "_0.display()")]
    FileExists(#[error(not(source))] PathBuf),

    /// Invalid value.
    #[allow(missing_docs)]
    #[display(fmt = "Expected {expected}, got {got} (@ {file}:{line}:{column})")]
    InvalidValue {
        expected: &'static str,
        got: String,
        file: &'static str,
        line: u32,
        column: u32,
    },

    /// I/O error.
    #[display(fmt = "")]
    IoError(io::Error),

    /// Value is not a boolean.
    #[display(fmt = "Failed to parse boolean: {_0}")]
    ParseBoolError(ParseBoolError),

    /// Value is not an integer.
    #[display(fmt = "Failed to parse integer: {_0}")]
    ParseIntError(ParseIntError),

    /// Value is not an address.
    #[display(fmt = "Failed to parse address: {_0}")]
    ParseAddressError(#[error(not(source))] String),

    /// Error deserializing as TOML.
    #[display(fmt = "TOML error: {_0}")]
    TomlError(TomlError),
}

macro_rules! InvalidValue {
    (expected: $expected:expr, got: $got:expr,) => {
        crate::Error::InvalidValue {
            expected: $expected,
            got: $got.to_string(),
            file: file!(),
            line: line!(),
            column: column!(),
        }
    };
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<ParseBoolError> for Error {
    fn from(err: ParseBoolError) -> Self {
        Self::ParseBoolError(err)
    }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self {
        Self::ParseIntError(err)
    }
}

impl From<TomlError> for Error {
    fn from(err: TomlError) -> Self {
        Self::TomlError(err)
    }
}

impl From<VarError> for Error {
    fn from(err: VarError) -> Self {
        Self::EnvVarError(err)
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::EnvVarError(_) => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),

            Error::FileExists(_) => io::Error::new(io::ErrorKind::AlreadyExists, err.to_string()),

            Error::InvalidValue { .. } => {
                io::Error::new(io::ErrorKind::InvalidInput, err.to_string())
            }

            Error::IoError(io_error) => io_error,

            Error::ParseBoolError(_) => {
                io::Error::new(io::ErrorKind::InvalidInput, err.to_string())
            }

            Error::ParseIntError(_) => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),

            Error::ParseAddressError(_) => {
                io::Error::new(io::ErrorKind::InvalidInput, err.to_string())
            }

            Error::TomlError(_) => io::Error::new(io::ErrorKind::InvalidInput, err.to_string()),
        }
    }
}
