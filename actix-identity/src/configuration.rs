//! Configuration options to tune the behaviour of [`IdentityMiddleware`].
use crate::IdentityMiddleware;

#[derive(Clone, Debug)]
pub(crate) struct Configuration {
    pub(crate) on_logout: LogoutBehaviour,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            on_logout: LogoutBehaviour::PurgeSession,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug)]
/// `LogoutBehaviour` controls what actions are going to be performed when [`Identity::logout`]
/// is invoked.
pub enum LogoutBehaviour {
    /// When [`Identity::logout`] is called, purge the current session.
    ///
    /// This behaviour might be desirable when you have stored additional information
    /// in the session state that are tied to the user's identity and should not be
    /// retained after logout.
    PurgeSession,
    /// When [`Identity::logout`] is called, remove the identity information from
    /// the current session state. The session itself is not destroyed.
    ///
    /// This behaviour might be desirable when you have stored information in the session state
    /// that is not tied to the user's identity and should be retained after logout.
    DeleteIdentityKeys,
}

/// A fluent builder to construct an [`IdentityMiddleware`] instance with custom configuration
/// parameters.
#[derive(Clone, Debug)]
pub struct IdentityMiddlewareBuilder {
    configuration: Configuration,
}

impl IdentityMiddlewareBuilder {
    pub(crate) fn new() -> Self {
        Self {
            configuration: Configuration::default(),
        }
    }

    /// Determine how [`Identity::logout`] affects the current session.
    ///
    /// By default, the current session is purged ([`LogoutBehaviour::PurgeSession`]).
    pub fn logout_behaviour(mut self, logout_behaviour: LogoutBehaviour) -> Self {
        self.configuration.on_logout = logout_behaviour;
        self
    }

    /// Finalise the builder and return an [`IdentityMiddleware`] instance.
    pub fn build(self) -> IdentityMiddleware {
        IdentityMiddleware::new(self.configuration)
    }
}
