use std::{collections::HashMap, convert::TryInto, fmt, future::Future, pin::Pin, rc::Rc};

use actix_utils::future::{ready, Ready};
use actix_web::{
    body::MessageBody,
    cookie::{Cookie, CookieJar, Key},
    dev::{forward_ready, ResponseHead, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderValue, SET_COOKIE},
    HttpResponse,
};
use anyhow::Context;

use crate::{
    config::{
        self, Configuration, CookieConfiguration, CookieContentSecurity, SessionMiddlewareBuilder,
        TtlExtensionPolicy,
    },
    storage::{LoadError, SessionKey, SessionStore},
    Session, SessionStatus,
};

/// A middleware for session management in Actix Web applications.
///
/// [`SessionMiddleware`] takes care of a few jobs:
///
/// - Instructs the session storage backend to create/update/delete/retrieve the state attached to
///   a session according to its status and the operations that have been performed against it;
/// - Set/remove a cookie, on the client side, to enable a user to be consistently associated with
///   the same session across multiple HTTP requests.
///
/// Use [`SessionMiddleware::new`] to initialize the session framework using the default parameters.
/// To create a new instance of [`SessionMiddleware`] you need to provide:
///
/// - an instance of the session storage backend you wish to use (i.e. an implementation of
///   [`SessionStore`]);
/// - a secret key, to sign or encrypt the content of client-side session cookie.
///
/// ```no_run
/// use actix_web::{web, App, HttpServer, HttpResponse, Error};
/// use actix_session::{Session, SessionMiddleware, storage::RedisActorSessionStore};
/// use actix_web::cookie::Key;
///
/// // The secret key would usually be read from a configuration file/environment variables.
/// fn get_secret_key() -> Key {
///     # todo!()
///     // [...]
/// }
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     let secret_key = get_secret_key();
///     let redis_connection_string = "127.0.0.1:6379";
///     HttpServer::new(move ||
///             App::new()
///             // Add session management to your application using Redis for session state storage
///             .wrap(
///                 SessionMiddleware::new(
///                     RedisActorSessionStore::new(redis_connection_string),
///                     secret_key.clone()
///                 )
///             )
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// If you want to customise use [`builder`](Self::builder) instead of [`new`](Self::new):
///
/// ```no_run
/// use actix_web::{App, cookie::{Key, time}, Error, HttpResponse, HttpServer, web};
/// use actix_session::{Session, SessionMiddleware, storage::RedisActorSessionStore};
/// use actix_session::config::PersistentSession;
///
/// // The secret key would usually be read from a configuration file/environment variables.
/// fn get_secret_key() -> Key {
///     # todo!()
///     // [...]
/// }
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     let secret_key = get_secret_key();
///     let redis_connection_string = "127.0.0.1:6379";
///     HttpServer::new(move ||
///             App::new()
///             // Customise session length!
///             .wrap(
///                 SessionMiddleware::builder(
///                     RedisActorSessionStore::new(redis_connection_string),
///                     secret_key.clone()
///                 )
///                 .session_lifecycle(
///                     PersistentSession::default()
///                         .session_ttl(time::Duration::days(5))
///                 )
///                 .build(),
///             )
///             .default_service(web::to(|| HttpResponse::Ok())))
///         .bind(("127.0.0.1", 8080))?
///         .run()
///         .await
/// }
/// ```
///
/// ## How did we choose defaults?
///
/// You should not regret adding `actix-session` to your dependencies and going to production using
/// the default configuration. That is why, when in doubt, we opt to use the most secure option for
/// each configuration parameter.
///
/// We expose knobs to change the default to suit your needs—i.e., if you know what you are doing,
/// we will not stop you. But being a subject-matter expert should not be a requirement to deploy
/// reasonably secure implementation of sessions.
#[derive(Clone)]
pub struct SessionMiddleware<Store: SessionStore> {
    storage_backend: Rc<Store>,
    configuration: Rc<Configuration>,
}

impl<Store: SessionStore> SessionMiddleware<Store> {
    /// Use [`SessionMiddleware::new`] to initialize the session framework using the default
    /// parameters.
    ///
    /// To create a new instance of [`SessionMiddleware`] you need to provide:
    /// - an instance of the session storage backend you wish to use (i.e. an implementation of
    ///   [`SessionStore]);
    /// - a secret key, to sign or encrypt the content of client-side session cookie.
    pub fn new(store: Store, key: Key) -> Self {
        Self::builder(store, key).build()
    }

    /// A fluent API to configure [`SessionMiddleware`].
    ///
    /// It takes as input the two required inputs to create a new instance of [`SessionMiddleware`]:
    /// - an instance of the session storage backend you wish to use (i.e. an implementation of
    ///   [`SessionStore]);
    /// - a secret key, to sign or encrypt the content of client-side session cookie.
    pub fn builder(store: Store, key: Key) -> SessionMiddlewareBuilder<Store> {
        SessionMiddlewareBuilder::new(store, config::default_configuration(key))
    }

    pub(crate) fn from_parts(store: Store, configuration: Configuration) -> Self {
        Self {
            storage_backend: Rc::new(store),
            configuration: Rc::new(configuration),
        }
    }
}

impl<S, B, Store> Transform<S, ServiceRequest> for SessionMiddleware<Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = InnerSessionMiddleware<S, Store>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(InnerSessionMiddleware {
            service: Rc::new(service),
            configuration: Rc::clone(&self.configuration),
            storage_backend: Rc::clone(&self.storage_backend),
        }))
    }
}

/// Short-hand to create an `actix_web::Error` instance that will result in an `Internal Server
/// Error` response while preserving the error root cause (e.g. in logs).
fn e500<E: fmt::Debug + fmt::Display + 'static>(err: E) -> actix_web::Error {
    // We do not use `actix_web::error::ErrorInternalServerError` because we do not want to
    // leak internal implementation details to the caller.
    //
    // `actix_web::error::ErrorInternalServerError` includes the error Display representation
    // as body of the error responses, leading to messages like "There was an issue persisting
    // the session state" reaching API clients. We don't want that, we want opaque 500s.
    actix_web::error::InternalError::from_response(
        err,
        HttpResponse::InternalServerError().finish(),
    )
    .into()
}

#[doc(hidden)]
#[non_exhaustive]
pub struct InnerSessionMiddleware<S, Store: SessionStore + 'static> {
    service: Rc<S>,
    configuration: Rc<Configuration>,
    storage_backend: Rc<Store>,
}

impl<S, B, Store> Service<ServiceRequest> for InnerSessionMiddleware<S, Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let storage_backend = Rc::clone(&self.storage_backend);
        let configuration = Rc::clone(&self.configuration);

        Box::pin(async move {
            let session_key = extract_session_key(&req, &configuration.cookie);
            let (session_key, session_state) =
                load_session_state(session_key, storage_backend.as_ref()).await?;

            Session::set_session(&mut req, session_state);

            let mut res = service.call(req).await?;
            let (status, session_state) = Session::get_changes(&mut res);

            match session_key {
                None => {
                    // we do not create an entry in the session store if there is no state attached
                    // to a fresh session
                    if !session_state.is_empty() {
                        let session_key = storage_backend
                            .save(session_state, &configuration.session.state_ttl)
                            .await
                            .map_err(e500)?;

                        set_session_cookie(
                            res.response_mut().head_mut(),
                            session_key,
                            &configuration.cookie,
                        )
                        .map_err(e500)?;
                    }
                }

                Some(session_key) => {
                    match status {
                        SessionStatus::Changed => {
                            let session_key = storage_backend
                                .update(
                                    session_key,
                                    session_state,
                                    &configuration.session.state_ttl,
                                )
                                .await
                                .map_err(e500)?;

                            set_session_cookie(
                                res.response_mut().head_mut(),
                                session_key,
                                &configuration.cookie,
                            )
                            .map_err(e500)?;
                        }

                        SessionStatus::Purged => {
                            storage_backend.delete(&session_key).await.map_err(e500)?;

                            delete_session_cookie(
                                res.response_mut().head_mut(),
                                &configuration.cookie,
                            )
                            .map_err(e500)?;
                        }

                        SessionStatus::Renewed => {
                            storage_backend.delete(&session_key).await.map_err(e500)?;

                            let session_key = storage_backend
                                .save(session_state, &configuration.session.state_ttl)
                                .await
                                .map_err(e500)?;

                            set_session_cookie(
                                res.response_mut().head_mut(),
                                session_key,
                                &configuration.cookie,
                            )
                            .map_err(e500)?;
                        }

                        SessionStatus::Unchanged => {
                            if matches!(
                                configuration.ttl_extension_policy,
                                TtlExtensionPolicy::OnEveryRequest
                            ) {
                                storage_backend
                                    .update_ttl(&session_key, &configuration.session.state_ttl)
                                    .await
                                    .map_err(e500)?;

                                if configuration.cookie.max_age.is_some() {
                                    set_session_cookie(
                                        res.response_mut().head_mut(),
                                        session_key,
                                        &configuration.cookie,
                                    )
                                    .map_err(e500)?;
                                }
                            }
                        }
                    };
                }
            }

            Ok(res)
        })
    }
}

/// Examines the session cookie attached to the incoming request, if there is one, and tries
/// to extract the session key.
///
/// It returns `None` if there is no session cookie or if the session cookie is considered invalid
/// (e.g., when failing a signature check).
fn extract_session_key(req: &ServiceRequest, config: &CookieConfiguration) -> Option<SessionKey> {
    let cookies = req.cookies().ok()?;
    let session_cookie = cookies
        .iter()
        .find(|&cookie| cookie.name() == config.name)?;

    let mut jar = CookieJar::new();
    jar.add_original(session_cookie.clone());

    let verification_result = match config.content_security {
        CookieContentSecurity::Signed => jar.signed(&config.key).get(&config.name),
        CookieContentSecurity::Private => jar.private(&config.key).get(&config.name),
    };

    if verification_result.is_none() {
        tracing::warn!(
            "The session cookie attached to the incoming request failed to pass cryptographic \
            checks (signature verification/decryption)."
        );
    }

    match verification_result?.value().to_owned().try_into() {
        Ok(session_key) => Some(session_key),
        Err(err) => {
            tracing::warn!(
                error.message = %err,
                error.cause_chain = ?err,
                "Invalid session key, ignoring."
            );

            None
        }
    }
}

async fn load_session_state<Store: SessionStore>(
    session_key: Option<SessionKey>,
    storage_backend: &Store,
) -> Result<(Option<SessionKey>, HashMap<String, String>), actix_web::Error> {
    if let Some(session_key) = session_key {
        match storage_backend.load(&session_key).await {
            Ok(state) => {
                if let Some(state) = state {
                    Ok((Some(session_key), state))
                } else {
                    // We discard the existing session key given that the state attached to it can
                    // no longer be found (e.g. it expired or we suffered some data loss in the
                    // storage). Regenerating the session key will trigger the `save` workflow
                    // instead of the `update` workflow if the session state is modified during the
                    // lifecycle of the current request.

                    tracing::info!(
                        "No session state has been found for a valid session key, creating a new \
                        empty session."
                    );

                    Ok((None, HashMap::new()))
                }
            }

            Err(err) => match err {
                LoadError::Deserialization(err) => {
                    tracing::warn!(
                        error.message = %err,
                        error.cause_chain = ?err,
                        "Invalid session state, creating a new empty session."
                    );

                    Ok((Some(session_key), HashMap::new()))
                }

                LoadError::Other(err) => Err(e500(err)),
            },
        }
    } else {
        Ok((None, HashMap::new()))
    }
}

fn set_session_cookie(
    response: &mut ResponseHead,
    session_key: SessionKey,
    config: &CookieConfiguration,
) -> Result<(), anyhow::Error> {
    let value: String = session_key.into();
    let mut cookie = Cookie::new(config.name.clone(), value);

    cookie.set_secure(config.secure);
    cookie.set_http_only(config.http_only);
    cookie.set_same_site(config.same_site);
    cookie.set_path(config.path.clone());

    if let Some(max_age) = config.max_age {
        cookie.set_max_age(max_age);
    }

    if let Some(ref domain) = config.domain {
        cookie.set_domain(domain.clone());
    }

    let mut jar = CookieJar::new();
    match config.content_security {
        CookieContentSecurity::Signed => jar.signed_mut(&config.key).add(cookie),
        CookieContentSecurity::Private => jar.private_mut(&config.key).add(cookie),
    }

    // set cookie
    let cookie = jar.delta().next().unwrap();
    let val = HeaderValue::from_str(&cookie.encoded().to_string())
        .context("Failed to attach a session cookie to the outgoing response")?;

    response.headers_mut().append(SET_COOKIE, val);

    Ok(())
}

fn delete_session_cookie(
    response: &mut ResponseHead,
    config: &CookieConfiguration,
) -> Result<(), anyhow::Error> {
    let removal_cookie = Cookie::build(config.name.clone(), "")
        .path(config.path.clone())
        .http_only(config.http_only);

    let mut removal_cookie = if let Some(ref domain) = config.domain {
        removal_cookie.domain(domain)
    } else {
        removal_cookie
    }
    .finish();

    removal_cookie.make_removal();

    let val = HeaderValue::from_str(&removal_cookie.to_string())
        .context("Failed to attach a session removal cookie to the outgoing response")?;
    response.headers_mut().append(SET_COOKIE, val);

    Ok(())
}
