use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

use crate::{AtError, Parse};

static ADDR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        \[                     # opening square bracket
        (\s)*                  # optional whitespace
            "(?P<host>[^"]+)"  # host name (string)
            ,                  # separating comma
            (\s)*              # optional whitespace
            (?P<port>\d+)      # port number (integer)
        (\s)*                  # optional whitespace
        \]                     # closing square bracket
    "#,
    )
    .expect("Failed to compile regex: ADDR_REGEX")
});

static ADDR_LIST_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        \[                           # opening square bracket (list)
        (\s)*                        # optional whitespace
            (?P<elements>(
                \[".*", (\s)* \d+\]  # element
                (,)?                 # element separator
                (\s)*                # optional whitespace
            )*)
        (\s)*                        # optional whitespace
        \]                           # closing square bracket (list)
    "#,
    )
    .expect("Failed to compile regex: ADDRS_REGEX")
});

/// A host/port pair for the server to bind to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Address {
    /// Host part of address.
    pub host: String,

    /// Port part of address.
    pub port: u16,
}

impl Parse for Address {
    fn parse(string: &str) -> Result<Self, AtError> {
        let mut items = string
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',');

        let parse_error = || AtError::ParseAddressError(string.to_string());

        if !ADDR_REGEX.is_match(string) {
            return Err(parse_error());
        }

        Ok(Self {
            host: items.next().ok_or_else(parse_error)?.trim().to_string(),
            port: items.next().ok_or_else(parse_error)?.trim().parse()?,
        })
    }
}

impl Parse for Vec<Address> {
    fn parse(string: &str) -> Result<Self, AtError> {
        let parse_error = || AtError::ParseAddressError(string.to_string());

        if !ADDR_LIST_REGEX.is_match(string) {
            return Err(parse_error());
        }

        let mut addrs = vec![];

        for list_caps in ADDR_LIST_REGEX.captures_iter(string) {
            let elements = &list_caps["elements"].trim();
            for elt_caps in ADDR_REGEX.captures_iter(elements) {
                addrs.push(Address {
                    host: elt_caps["host"].to_string(),
                    port: elt_caps["port"].parse()?,
                });
            }
        }

        Ok(addrs)
    }
}
