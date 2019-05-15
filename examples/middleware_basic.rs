use std::borrow::Cow;
use std::io;

use actix_service::{Service, Transform};
use actix_web::{dev, web, App, Error, HttpRequest, HttpServer};
use futures::future::{self, Either, FutureResult};
use futures::Poll;

use actix_web_httpauth::extractors::basic::{BasicAuth, Config};
use actix_web_httpauth::extractors::AuthenticationError;

struct Auth(Config);

impl<S, B> Transform<S> for Auth
where
    S: Service<Request = dev::ServiceRequest, Response = dev::ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = dev::ServiceRequest;
    type Response = dev::ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddleware<S>;
    type InitError = ();
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(AuthMiddleware {
            service,
            auth: self.0.clone(),
        })
    }
}

struct AuthMiddleware<S> {
    service: S,
    auth: Config,
}

impl<S> AuthMiddleware<S> {
    fn valid_user(credentials: &BasicAuth) -> bool {
        let user_id = credentials.user_id();
        let password = credentials.password();

        user_id == "Alladin" && password == Some(&Cow::Borrowed("open sesame"))
    }
}

impl<S, B> Service for AuthMiddleware<S>
where
    S: Service<Request = dev::ServiceRequest, Response = dev::ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = dev::ServiceRequest;
    type Response = dev::ServiceResponse<B>;
    type Error = Error;
    type Future = Either<S::Future, FutureResult<Self::Response, Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, mut req: dev::ServiceRequest) -> Self::Future {
        let auth = BasicAuth::from_service_request(&mut req, &self.auth);

        match auth {
            Ok(ref credentials) if Self::valid_user(credentials) => Either::A(self.service.call(req)),
            Ok(..) => {
                let challenge = self.auth.as_ref().clone();
                let error = AuthenticationError::new(challenge);
                Either::B(future::err(Self::Error::from(error)))
            }
            Err(e) => Either::B(future::err(e.into())),
        }
    }
}

fn index(_req: HttpRequest) -> String {
    "Hello, authorized user!".to_string()
}

fn main() -> io::Result<()> {
    HttpServer::new(|| {
        let config = Config::default().realm("WallyWorld");

        App::new().wrap(Auth(config)).service(web::resource("/").to(index))
    })
    .bind("127.0.0.1:8088")?
    .run()
}
