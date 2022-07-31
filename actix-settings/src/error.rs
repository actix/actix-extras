use std::{env::VarError, io, num::ParseIntError, path::PathBuf, str::ParseBoolError};

use toml::de::Error as TomlError;

/// Convenience type alias for `Result<T, AtError>`.
pub type AtResult<T> = std::result::Result<T, AtError>;

/// Errors that can be returned from methods in this crate.
#[derive(Debug, Clone)]
pub enum AtError {
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
        crate::AtError::InvalidValue {
            expected: $expected,
            got: $got.to_string(),
            file: file!(),
            line: line!(),
            column: column!(),
        }
    };
}

impl From<io::Error> for AtError {
    fn from(err: io::Error) -> Self {
        Self::IoError(ioe::IoError::from(err))
    }
}

impl From<ioe::IoError> for AtError {
    fn from(err: ioe::IoError) -> Self {
        Self::IoError(err)
    }
}

impl From<ParseBoolError> for AtError {
    fn from(err: ParseBoolError) -> Self {
        Self::ParseBoolError(err)
    }
}

impl From<ParseIntError> for AtError {
    fn from(err: ParseIntError) -> Self {
        Self::ParseIntError(err)
    }
}

impl From<TomlError> for AtError {
    fn from(err: TomlError) -> Self {
        Self::TomlError(err)
    }
}

impl From<VarError> for AtError {
    fn from(err: VarError) -> Self {
        Self::EnvVarError(err)
    }
}

impl From<AtError> for io::Error {
    fn from(err: AtError) -> Self {
        match err {
            AtError::EnvVarError(var_error) => {
                let msg = format!("Env var error: {}", var_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            AtError::FileExists(path_buf) => {
                let msg = format!("File exists: {}", path_buf.display());
                io::Error::new(io::ErrorKind::AlreadyExists, msg)
            }

            AtError::InvalidValue {
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

            AtError::IoError(io_error) => io_error.into(),

            AtError::ParseBoolError(parse_bool_error) => {
                let msg = format!("Failed to parse boolean: {}", parse_bool_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            AtError::ParseIntError(parse_int_error) => {
                let msg = format!("Failed to parse integer: {}", parse_int_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            AtError::ParseAddressError(string) => {
                let msg = format!("Failed to parse address: {}", string);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }

            AtError::TomlError(toml_error) => {
                let msg = format!("TOML error: {}", toml_error);
                io::Error::new(io::ErrorKind::InvalidInput, msg)
            }
        }
    }
}
