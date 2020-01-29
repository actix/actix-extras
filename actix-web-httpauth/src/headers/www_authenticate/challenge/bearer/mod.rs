//! Challenge for the "Bearer" HTTP Authentication Scheme

mod builder;
mod challenge;
mod errors;

pub use self::builder::BearerBuilder;
pub use self::challenge::Bearer;
pub use self::errors::Error;

#[cfg(test)]
mod tests;
