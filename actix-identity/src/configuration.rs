//! Configuration options to tune the behaviour of [`IdentityMiddleware`].
use std::time::Duration;

use crate::IdentityMiddleware;

#[derive(Clone, Debug)]
pub(crate) struct Configuration {
    pub(crate) on_logout: LogoutBehaviour,
    pub(crate) login_deadline: Option<Duration>,
    pub(crate) visit_deadline: Option<Duration>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            on_logout: LogoutBehaviour::PurgeSession,
            login_deadline: None,
            visit_deadline: None,
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

    /// Automatically log out users after a certain amount of time has passed since they logged in,
    /// regardless of their activity pattern.
    ///
    /// If set to `None`, login deadline is disabled.
    /// If set to `Some(duration)`, login deadline is enabled and users will be logged out after
    /// `duration` has passed since their login.
    ///
    /// By default, login deadline is disabled.
    pub fn login_deadline(mut self, deadline: Option<Duration>) -> Self {
        self.configuration.login_deadline = deadline;
        self
    }

    /// Automatically log out users after a certain amount of time has passed since their last visit.
    ///
    /// If set to `None`, visit deadline is disabled.
    /// If set to `Some(duration)`, visit deadline is enabled and users will be logged out after
    /// `duration` has passed since their last visit.
    ///
    /// By default, visit deadline is disabled.
    pub fn visit_deadline(mut self, deadline: Option<Duration>) -> Self {
        self.configuration.visit_deadline = deadline;
        self
    }

    /// Finalise the builder and return an [`IdentityMiddleware`] instance.
    pub fn build(self) -> IdentityMiddleware {
        IdentityMiddleware::new(self.configuration)
    }
}
