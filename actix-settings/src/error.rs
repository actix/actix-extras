use std::{env::VarError, io, num::ParseIntError, path::PathBuf, str::ParseBoolError};

use toml::de::Error as TomlError;

/// Errors that can be returned from methods in this crate.
#[derive(Debug, Clone)]
pub enum Error {
    /// Environment variable does not exists or is invalid.
    EnvVarError(VarError),

    /// File already exists on disk.
    FileExists(PathBuf),

    /// Invalid value.
    #[allow(missing_docs)]
    InvalidValue {
        expected: &'static str,
        got: String,
        file: &'static str,
        line: u32,
        column: u32,
    },

    /// I/O error.
    IoError(ioe::IoError),

    /// Value is not a boolean.
    ParseBoolError(ParseBoolError),

    /// Value is not an integer.
    ParseIntError(ParseIntError),

    /// Value is not an address.
    ParseAddressError(String),

    /// Error deserializing as TOML.
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
        Self::IoError(ioe::IoError::from(err))
    }
}

impl From<ioe::IoError> for Error {
    fn from(err: ioe::IoError) -> Self {
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
            Error::EnvVarError(var_error) => {
                let msg = format!("Env var error: {}", var_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            Error::FileExists(path_buf) => {
                let msg = format!("File exists: {}", path_buf.display());
                io::Error::new(io::ErrorKind::AlreadyExists, msg)
            }

            Error::InvalidValue {
                expected,
                ref got,
                file,
                line,
                column,
            } => {
                let msg = format!(
                    "Expected {}, got {}  (@ {}:{}:{})",
                    expected, got, file, line, column
                );
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            Error::IoError(io_error) => io_error.into(),

            Error::ParseBoolError(parse_bool_error) => {
                let msg = format!("Failed to parse boolean: {}", parse_bool_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            Error::ParseIntError(parse_int_error) => {
                let msg = format!("Failed to parse integer: {}", parse_int_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            Error::ParseAddressError(string) => {
                let msg = format!("Failed to parse address: {}", string);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            Error::TomlError(toml_error) => {
                let msg = format!("TOML error: {}", toml_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }
        }
    }
}
