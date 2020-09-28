use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use actix_router::{IntoPattern, RouterBuilder};
use actix_service::boxed::{self, BoxService, BoxServiceFactory};
use actix_service::{fn_service, IntoServiceFactory, Service, ServiceFactory};
use futures::future::{join_all, ok, JoinAll, LocalBoxFuture};

use crate::publish::Publish;

type Handler<S, E> = BoxServiceFactory<S, Publish<S>, (), E, E>;
type HandlerService<S, E> = BoxService<Publish<S>, (), E>;

/// Router - structure that follows the builder pattern
/// for building publish packet router instances for mqtt server.
pub struct Router<S, E> {
    router: RouterBuilder<usize>,
    handlers: Vec<Handler<S, E>>,
    default: Handler<S, E>,
}

impl<S, E> Router<S, E>
where
    S: Clone + 'static,
    E: 'static,
{
    /// Create mqtt application.
    ///
    /// **Note** Default service acks all publish packets
    pub fn new() -> Self {
        Router {
            router: actix_router::Router::build(),
            handlers: Vec::new(),
            default: boxed::factory(
                fn_service(|p: Publish<S>| {
                    log::warn!("Unknown topic {:?}", p.publish_topic());
                    ok::<_, E>(())
                })
                .map_init_err(|_| panic!()),
            ),
        }
    }

    /// Configure mqtt resource for a specific topic.
    pub fn resource<T, F, U: 'static>(mut self, address: T, service: F) -> Self
    where
        T: IntoPattern,
        F: IntoServiceFactory<U>,
        U: ServiceFactory<Config = S, Request = Publish<S>, Response = (), Error = E>,
        E: From<U::InitError>,
    {
        self.router.path(address, self.handlers.len());
        self.handlers
            .push(boxed::factory(service.into_factory().map_init_err(E::from)));
        self
    }

    /// Default service to be used if no matching resource could be found.
    pub fn default_resource<F, U: 'static>(mut self, service: F) -> Self
    where
        F: IntoServiceFactory<U>,
        U: ServiceFactory<
            Config = S,
            Request = Publish<S>,
            Response = (),
            Error = E,
            InitError = E,
        >,
    {
        self.default = boxed::factory(service.into_factory());
        self
    }
}

impl<S, E> IntoServiceFactory<RouterFactory<S, E>> for Router<S, E>
where
    S: Clone + 'static,
    E: 'static,
{
    fn into_factory(self) -> RouterFactory<S, E> {
        RouterFactory {
            router: Rc::new(self.router.finish()),
            handlers: self.handlers,
            default: self.default,
        }
    }
}

pub struct RouterFactory<S, E> {
    router: Rc<actix_router::Router<usize>>,
    handlers: Vec<Handler<S, E>>,
    default: Handler<S, E>,
}

impl<S, E> ServiceFactory for RouterFactory<S, E>
where
    S: Clone + 'static,
    E: 'static,
{
    type Config = S;
    type Request = Publish<S>;
    type Response = ();
    type Error = E;
    type InitError = E;
    type Service = RouterService<S, E>;
    type Future = RouterFactoryFut<S, E>;

    fn new_service(&self, session: S) -> Self::Future {
        let fut: Vec<_> = self
            .handlers
            .iter()
            .map(|h| h.new_service(session.clone()))
            .collect();

        RouterFactoryFut {
            router: self.router.clone(),
            handlers: join_all(fut),
            default: Some(either::Either::Left(self.default.new_service(session))),
        }
    }
}

pub struct RouterFactoryFut<S, E> {
    router: Rc<actix_router::Router<usize>>,
    handlers: JoinAll<LocalBoxFuture<'static, Result<HandlerService<S, E>, E>>>,
    default: Option<
        either::Either<
            LocalBoxFuture<'static, Result<HandlerService<S, E>, E>>,
            HandlerService<S, E>,
        >,
    >,
}

impl<S, E> Future for RouterFactoryFut<S, E> {
    type Output = Result<RouterService<S, E>, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let res = match self.default.as_mut().unwrap() {
            either::Either::Left(ref mut fut) => {
                let default = match futures::ready!(Pin::new(fut).poll(cx)) {
                    Ok(default) => default,
                    Err(e) => return Poll::Ready(Err(e)),
                };
                self.default = Some(either::Either::Right(default));
                return self.poll(cx);
            }
            either::Either::Right(_) => futures::ready!(Pin::new(&mut self.handlers).poll(cx)),
        };

        let mut handlers = Vec::new();
        for handler in res {
            match handler {
                Ok(h) => handlers.push(h),
                Err(e) => return Poll::Ready(Err(e)),
            }
        }

        Poll::Ready(Ok(RouterService {
            handlers,
            router: self.router.clone(),
            default: self.default.take().unwrap().right().unwrap(),
        }))
    }
}

pub struct RouterService<S, E> {
    router: Rc<actix_router::Router<usize>>,
    handlers: Vec<BoxService<Publish<S>, (), E>>,
    default: BoxService<Publish<S>, (), E>,
}

impl<S, E> Service for RouterService<S, E>
where
    S: 'static,
    E: 'static,
{
    type Request = Publish<S>;
    type Response = ();
    type Error = E;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        let mut not_ready = false;
        for hnd in &mut self.handlers {
            if let Poll::Pending = hnd.poll_ready(cx)? {
                not_ready = true;
            }
        }

        if not_ready {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn call(&mut self, mut req: Publish<S>) -> Self::Future {
        if let Some((idx, _info)) = self.router.recognize(req.topic_mut()) {
            self.handlers[*idx].call(req)
        } else {
            self.default.call(req)
        }
    }
}
