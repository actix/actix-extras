use std::{fmt, path::PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{de, Deserialize};

use crate::{core::Parse, error::AtError};

/// Settings types for Actix Web.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub struct ActixSettings {
    pub hosts: Vec<Address>,
    pub mode: Mode,
    pub enable_compression: bool,
    pub enable_log: bool,
    pub num_workers: NumWorkers,
    pub backlog: Backlog,
    pub max_connections: MaxConnections,
    pub max_connection_rate: MaxConnectionRate,
    pub keep_alive: KeepAlive,
    pub client_timeout: Timeout,
    pub client_shutdown: Timeout,
    pub shutdown_timeout: Timeout,
    pub tls: Tls,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
pub struct Address {
    pub host: String,
    pub port: u16,
}

pub(crate) static ADDR_REGEX: Lazy<Regex> = Lazy::new(|| {
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

pub(crate) static ADDR_LIST_REGEX: Lazy<Regex> = Lazy::new(|| {
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
pub enum Mode {
    #[serde(rename = "development")]
    Development,

    #[serde(rename = "production")]
    Production,
}

impl Parse for Mode {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        match string {
            "development" => Ok(Self::Development),
            "production" => Ok(Self::Production),
            _ => Err(InvalidValue! {
                expected: "\"development\" | \"production\".",
                got: string,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumWorkers {
    Default,
    Manual(usize),
}

impl Parse for NumWorkers {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        match string {
            "default" => Ok(NumWorkers::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(NumWorkers::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "a positive integer",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for NumWorkers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NumWorkersVisitor;

        impl<'de> de::Visitor<'de> for NumWorkersVisitor {
            type Value = NumWorkers;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match NumWorkers::parse(value) {
                    Ok(num_workers) => Ok(num_workers),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(NumWorkersVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Backlog {
    Default,
    Manual(usize),
}

impl Parse for Backlog {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        match string {
            "default" => Ok(Backlog::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(Backlog::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "an integer > 0",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for Backlog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BacklogVisitor;

        impl<'de> de::Visitor<'de> for BacklogVisitor {
            type Value = Backlog;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Backlog::parse(value) {
                    Ok(backlog) => Ok(backlog),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(BacklogVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaxConnections {
    Default,
    Manual(usize),
}

impl Parse for MaxConnections {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        match string {
            "default" => Ok(MaxConnections::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(MaxConnections::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "an integer > 0",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for MaxConnections {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MaxConnectionsVisitor;

        impl<'de> de::Visitor<'de> for MaxConnectionsVisitor {
            type Value = MaxConnections;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match MaxConnections::parse(value) {
                    Ok(max_connections) => Ok(max_connections),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(MaxConnectionsVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaxConnectionRate {
    Default,
    Manual(usize),
}

impl Parse for MaxConnectionRate {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        match string {
            "default" => Ok(MaxConnectionRate::Default),
            string => match string.parse::<usize>() {
                Ok(val) => Ok(MaxConnectionRate::Manual(val)),
                Err(_) => Err(InvalidValue! {
                    expected: "an integer > 0",
                    got: string,
                }),
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for MaxConnectionRate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MaxConnectionRateVisitor;

        impl<'de> de::Visitor<'de> for MaxConnectionRateVisitor {
            type Value = MaxConnectionRate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\" or a string containing an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match MaxConnectionRate::parse(value) {
                    Ok(max_connection_rate) => Ok(max_connection_rate),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(MaxConnectionRateVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeepAlive {
    Default,
    Disabled,
    Os,
    Seconds(usize),
}

impl Parse for KeepAlive {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        pub(crate) static FMT: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\d+ seconds$").expect("Failed to compile regex: FMT"));

        pub(crate) static DIGITS: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\d+").expect("Failed to compile regex: FMT"));

        macro_rules! invalid_value {
            ($got:expr) => {
                Err(InvalidValue! {
                    expected: "a string of the format \"N seconds\" where N is an integer > 0",
                    got: $got,
                })
            };
        }

        let digits_in = |m: regex::Match| &string[m.start()..m.end()];
        match string {
            "default" => Ok(KeepAlive::Default),
            "disabled" => Ok(KeepAlive::Disabled),
            "OS" | "os" => Ok(KeepAlive::Os),
            string if !FMT.is_match(string) => invalid_value!(string),
            string => match DIGITS.find(string) {
                None => invalid_value!(string),
                Some(mat) => match digits_in(mat).parse() {
                    Ok(val) => Ok(KeepAlive::Seconds(val)),
                    Err(_) => invalid_value!(string),
                },
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for KeepAlive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KeepAliveVisitor;

        impl<'de> de::Visitor<'de> for KeepAliveVisitor {
            type Value = KeepAlive;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\", \"disabled\", \"os\", or a string of the format \"N seconds\" where N is an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match KeepAlive::parse(value) {
                    Ok(keep_alive) => Ok(keep_alive),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(KeepAliveVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Timeout {
    Default,
    Milliseconds(usize),
    Seconds(usize),
}

impl Parse for Timeout {
    fn parse(string: &str) -> std::result::Result<Self, AtError> {
        pub static FMT: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"^\d+ (milliseconds|seconds)$").expect("Failed to compile regex: FMT")
        });

        pub static DIGITS: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\d+").expect("Failed to compile regex: DIGITS"));

        pub static UNIT: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(milliseconds|seconds)$").expect("Failed to compile regex: UNIT")
        });

        macro_rules! invalid_value {
            ($got:expr) => {
                Err(InvalidValue! {
                    expected: "a string of the format \"N seconds\" or \"N milliseconds\" where N is an integer > 0",
                    got: $got,
                })
            }
        }
        match string {
            "default" => Ok(Timeout::Default),
            string if !FMT.is_match(string) => invalid_value!(string),
            string => match (DIGITS.find(string), UNIT.find(string)) {
                (None, _) => invalid_value!(string),
                (_, None) => invalid_value!(string),
                (Some(dmatch), Some(umatch)) => {
                    let digits = &string[dmatch.start()..dmatch.end()];
                    let unit = &string[umatch.start()..umatch.end()];
                    match (digits.parse(), unit) {
                        (Ok(v), "milliseconds") => Ok(Timeout::Milliseconds(v)),
                        (Ok(v), "seconds") => Ok(Timeout::Seconds(v)),
                        _ => invalid_value!(string),
                    }
                }
            },
        }
    }
}

impl<'de> serde::Deserialize<'de> for Timeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TimeoutVisitor;

        impl<'de> de::Visitor<'de> for TimeoutVisitor {
            type Value = Timeout;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let msg = "Either \"default\", \"disabled\", \"os\", or a string of the format \"N seconds\" where N is an integer > 0";
                formatter.write_str(msg)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Timeout::parse(value) {
                    Ok(num_workers) => Ok(num_workers),
                    Err(AtError::InvalidValue { expected, got, .. }) => Err(
                        de::Error::invalid_value(de::Unexpected::Str(&got), &expected),
                    ),
                    Err(_) => unreachable!(),
                }
            }
        }

        deserializer.deserialize_string(TimeoutVisitor)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub struct Tls {
    pub enabled: bool,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}
