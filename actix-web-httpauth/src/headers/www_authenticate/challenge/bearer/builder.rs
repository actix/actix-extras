use std::borrow::Cow;

use super::{Bearer, Error};

/// Builder for the [`Bearer`] challenge.
///
/// It is up to implementor to fill all required fields, neither this `Builder` nor [`Bearer`]
/// provide any validation.
#[derive(Debug, Default)]
pub struct BearerBuilder(Bearer);

impl BearerBuilder {
    /// Provides the `scope` attribute, as defined in [RFC 6749 §3.3].
    ///
    /// [RFC 6749 §3.3]: https://tools.ietf.org/html/rfc6749#section-3.3
    pub fn scope<T>(mut self, value: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.scope = Some(value.into());
        self
    }

    /// Provides the `realm` attribute, as defined in [RFC 2617].
    ///
    /// [RFC 2617]: https://tools.ietf.org/html/rfc2617
    pub fn realm<T>(mut self, value: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.realm = Some(value.into());
        self
    }

    /// Provides the `error` attribute, as defined in [RFC 6750, Section 3.1].
    ///
    /// [RFC 6750 §3.1]: https://tools.ietf.org/html/rfc6750#section-3.1
    pub fn error(mut self, value: Error) -> Self {
        self.0.error = Some(value);
        self
    }

    /// Provides the `error_description` attribute, as defined in [RFC 6750, Section 3].
    ///
    /// [RFC 6750 §3]: https://tools.ietf.org/html/rfc6750#section-3
    pub fn error_description<T>(mut self, value: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.error_description = Some(value.into());
        self
    }

    /// Provides the `error_uri` attribute, as defined in [RFC 6750 §3].
    ///
    /// It is up to implementor to provide properly-formed absolute URI.
    ///
    /// [RFC 6750 §3](https://tools.ietf.org/html/rfc6750#section-3)
    pub fn error_uri<T>(mut self, value: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.error_uri = Some(value.into());
        self
    }

    /// Consumes the builder and returns built `Bearer` instance.
    pub fn finish(self) -> Bearer {
        self.0
    }
}
