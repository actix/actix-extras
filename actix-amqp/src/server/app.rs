use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_router::{IntoPattern, Router};
use actix_service::{boxed, fn_factory_with_config, IntoServiceFactory, Service, ServiceFactory};
use amqp_codec::protocol::{DeliveryNumber, DeliveryState, Disposition, Error, Rejected, Role};
use futures::future::{err, ok, Either, Ready};
use futures::{Stream, StreamExt};

use crate::cell::Cell;
use crate::rcvlink::ReceiverLink;

use super::errors::LinkError;
use super::link::Link;
use super::message::{Message, Outcome};
use super::State;

type Handle<S> = boxed::BoxServiceFactory<Link<S>, Message<S>, Outcome, Error, Error>;

pub struct App<S = ()>(Vec<(Vec<String>, Handle<S>)>);

impl<S: 'static> App<S> {
    pub fn new() -> App<S> {
        App(Vec::new())
    }

    pub fn service<T, F, U: 'static>(mut self, address: T, service: F) -> Self
    where
        T: IntoPattern,
        F: IntoServiceFactory<U>,
        U: ServiceFactory<Config = Link<S>, Request = Message<S>, Response = Outcome>,
        U::Error: Into<Error>,
        U::InitError: Into<Error>,
    {
        self.0.push((
            address.patterns(),
            boxed::factory(
                service
                    .into_factory()
                    .map_init_err(|e| e.into())
                    .map_err(|e| e.into()),
            ),
        ));

        self
    }

    pub fn finish(
        self,
    ) -> impl ServiceFactory<
        Config = State<S>,
        Request = Link<S>,
        Response = (),
        Error = Error,
        InitError = Error,
    > {
        let mut router = Router::build();
        for (addr, hnd) in self.0 {
            router.path(addr, hnd);
        }
        let router = Cell::new(router.finish());

        fn_factory_with_config(move |_: State<S>| {
            ok(AppService {
                router: router.clone(),
            })
        })
    }
}

struct AppService<S> {
    router: Cell<Router<Handle<S>>>,
}

impl<S: 'static> Service for AppService<S> {
    type Request = Link<S>;
    type Response = ();
    type Error = Error;
    type Future = Either<Ready<Result<(), Error>>, AppServiceResponse<S>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut link: Link<S>) -> Self::Future {
        let path = link
            .frame()
            .target
            .as_ref()
            .and_then(|target| target.address.as_ref().map(|addr| addr.clone()));

        if let Some(path) = path {
            link.path_mut().set(path);
            if let Some((hnd, _info)) = self.router.recognize(link.path_mut()) {
                let fut = hnd.new_service(link.clone());
                Either::Right(AppServiceResponse {
                    link: link.link.clone(),
                    app_state: link.state.clone(),
                    state: AppServiceResponseState::NewService(fut),
                    // has_credit: true,
                })
            } else {
                Either::Left(err(LinkError::force_detach()
                    .description(format!(
                        "Target address is not supported: {}",
                        link.path().get_ref()
                    ))
                    .into()))
            }
        } else {
            Either::Left(err(LinkError::force_detach()
                .description("Target address is required")
                .into()))
        }
    }
}

struct AppServiceResponse<S> {
    link: ReceiverLink,
    app_state: State<S>,
    state: AppServiceResponseState<S>,
    // has_credit: bool,
}

enum AppServiceResponseState<S> {
    Service(boxed::BoxService<Message<S>, Outcome, Error>),
    NewService(
        Pin<Box<dyn Future<Output = Result<boxed::BoxService<Message<S>, Outcome, Error>, Error>>>>,
    ),
}

impl<S> Future for AppServiceResponse<S> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        let mut link = this.link.clone();
        let app_state = this.app_state.clone();

        loop {
            match this.state {
                AppServiceResponseState::Service(ref mut srv) => {
                    // check readiness
                    match srv.poll_ready(cx) {
                        Poll::Ready(Ok(_)) => (),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => {
                            let _ = this.link.close_with_error(
                                LinkError::force_detach()
                                    .description(format!("error: {}", e))
                                    .into(),
                            );
                            return Poll::Ready(Ok(()));
                        }
                    }

                    match Pin::new(&mut link).poll_next(cx) {
                        Poll::Ready(Some(Ok(transfer))) => {
                            // #2.7.5 delivery_id MUST be set. batching is not supported atm
                            if transfer.delivery_id.is_none() {
                                let _ = this.link.close_with_error(
                                    LinkError::force_detach()
                                        .description("delivery_id MUST be set")
                                        .into(),
                                );
                                return Poll::Ready(Ok(()));
                            }
                            if link.credit() == 0 {
                                // self.has_credit = self.link.credit() != 0;
                                link.set_link_credit(50);
                            }

                            let delivery_id = transfer.delivery_id.unwrap();
                            let msg = Message::new(app_state.clone(), transfer, link.clone());

                            let mut fut = srv.call(msg);
                            match Pin::new(&mut fut).poll(cx) {
                                Poll::Ready(Ok(outcome)) => settle(
                                    &mut this.link,
                                    delivery_id,
                                    outcome.into_delivery_state(),
                                ),
                                Poll::Pending => {
                                    actix_rt::spawn(HandleMessage {
                                        fut,
                                        delivery_id,
                                        link: this.link.clone(),
                                    });
                                }
                                Poll::Ready(Err(e)) => settle(
                                    &mut this.link,
                                    delivery_id,
                                    DeliveryState::Rejected(Rejected { error: Some(e) }),
                                ),
                            }
                        }
                        Poll::Ready(None) => return Poll::Ready(Ok(())),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Some(Err(_))) => {
                            let _ = this.link.close_with_error(LinkError::force_detach().into());
                            return Poll::Ready(Ok(()));
                        }
                    }
                }
                AppServiceResponseState::NewService(ref mut fut) => match Pin::new(fut).poll(cx) {
                    Poll::Ready(Ok(srv)) => {
                        this.link.open();
                        this.link.set_link_credit(50);
                        this.state = AppServiceResponseState::Service(srv);
                        continue;
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                },
            }
        }
    }
}

struct HandleMessage {
    link: ReceiverLink,
    delivery_id: DeliveryNumber,
    fut: Pin<Box<dyn Future<Output = Result<Outcome, Error>>>>,
}

impl Future for HandleMessage {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = self.as_mut();

        match Pin::new(&mut this.fut).poll(cx) {
            Poll::Ready(Ok(outcome)) => {
                let delivery_id = this.delivery_id;
                settle(&mut this.link, delivery_id, outcome.into_delivery_state());
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => {
                let delivery_id = this.delivery_id;
                settle(
                    &mut this.link,
                    delivery_id,
                    DeliveryState::Rejected(Rejected { error: Some(e) }),
                );
                Poll::Ready(())
            }
        }
    }
}

fn settle(link: &mut ReceiverLink, id: DeliveryNumber, state: DeliveryState) {
    let disposition = Disposition {
        state: Some(state),
        role: Role::Receiver,
        first: id,
        last: None,
        settled: true,
        batchable: false,
    };
    link.send_disposition(disposition);
}
