use std::future::Future;
use std::marker::PhantomData;
use std::num::NonZeroU16;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use actix_ioframe as ioframe;
use actix_service::{boxed, fn_factory_with_config, pipeline, Service, ServiceFactory};
use actix_utils::inflight::InFlightService;
use actix_utils::keepalive::KeepAliveService;
use actix_utils::order::{InOrder, InOrderError};
use actix_utils::time::LowResTimeService;
use futures::future::{join3, ok, Either, FutureExt, LocalBoxFuture, Ready};
use futures::ready;
use mqtt_codec as mqtt;

use crate::cell::Cell;
use crate::error::MqttError;
use crate::publish::Publish;
use crate::sink::MqttSink;
use crate::subs::{Subscribe, SubscribeResult, Unsubscribe};

pub(crate) struct MqttState<St> {
    inner: Cell<MqttStateInner<St>>,
}

struct MqttStateInner<St> {
    pub(crate) st: St,
    pub(crate) sink: MqttSink,
    pub(self) timeout: Duration,
    pub(self) in_flight: usize,
}

impl<St> Clone for MqttState<St> {
    fn clone(&self) -> Self {
        MqttState {
            inner: self.inner.clone(),
        }
    }
}

impl<St> MqttState<St> {
    pub(crate) fn new(st: St, sink: MqttSink, timeout: Duration, in_flight: usize) -> Self {
        MqttState {
            inner: Cell::new(MqttStateInner {
                st,
                sink,
                timeout,
                in_flight,
            }),
        }
    }

    pub(crate) fn sink(&self) -> &MqttSink {
        &self.inner.sink
    }

    pub(crate) fn session(&self) -> &St {
        &self.inner.get_ref().st
    }

    pub(crate) fn session_mut(&mut self) -> &mut St {
        &mut self.inner.get_mut().st
    }
}

// dispatcher factory
pub(crate) fn dispatcher<St, T, E>(
    publish: T,
    subscribe: Rc<
        boxed::BoxServiceFactory<
            St,
            Subscribe<St>,
            SubscribeResult,
            MqttError<E>,
            MqttError<E>,
        >,
    >,
    unsubscribe: Rc<
        boxed::BoxServiceFactory<St, Unsubscribe<St>, (), MqttError<E>, MqttError<E>>,
    >,
) -> impl ServiceFactory<
    Config = MqttState<St>,
    Request = ioframe::Item<MqttState<St>, mqtt::Codec>,
    Response = Option<mqtt::Packet>,
    Error = MqttError<E>,
    InitError = MqttError<E>,
>
where
    E: 'static,
    St: Clone + 'static,
    T: ServiceFactory<
            Config = St,
            Request = Publish<St>,
            Response = (),
            Error = MqttError<E>,
            InitError = MqttError<E>,
        > + 'static,
{
    let time = LowResTimeService::with(Duration::from_secs(1));

    fn_factory_with_config(move |cfg: MqttState<St>| {
        let time = time.clone();
        let state = cfg.session().clone();
        let timeout = cfg.inner.timeout;
        let inflight = cfg.inner.in_flight;

        // create services
        let fut = join3(
            publish.new_service(state.clone()),
            subscribe.new_service(state.clone()),
            unsubscribe.new_service(state.clone()),
        );

        async move {
            let (publish, subscribe, unsubscribe) = fut.await;

            // mqtt dispatcher
            Ok(Dispatcher::new(
                // keep-alive connection
                pipeline(KeepAliveService::new(timeout, time, || {
                    MqttError::KeepAliveTimeout
                }))
                .and_then(
                    // limit number of in-flight messages
                    InFlightService::new(
                        inflight,
                        // mqtt spec requires ack ordering, so enforce response ordering
                        InOrder::service(publish?).map_err(|e| match e {
                            InOrderError::Service(e) => e,
                            InOrderError::Disconnected => MqttError::Disconnected,
                        }),
                    ),
                ),
                subscribe?,
                unsubscribe?,
            ))
        }
    })
}

/// PUBLIS/SUBSCRIBER/UNSUBSCRIBER packets dispatcher
pub(crate) struct Dispatcher<St, T: Service> {
    publish: T,
    subscribe: boxed::BoxService<Subscribe<St>, SubscribeResult, T::Error>,
    unsubscribe: boxed::BoxService<Unsubscribe<St>, (), T::Error>,
}

impl<St, T> Dispatcher<St, T>
where
    T: Service<Request = Publish<St>, Response = ()>,
{
    pub(crate) fn new(
        publish: T,
        subscribe: boxed::BoxService<Subscribe<St>, SubscribeResult, T::Error>,
        unsubscribe: boxed::BoxService<Unsubscribe<St>, (), T::Error>,
    ) -> Self {
        Self {
            publish,
            subscribe,
            unsubscribe,
        }
    }
}

impl<St, T> Service for Dispatcher<St, T>
where
    T: Service<Request = Publish<St>, Response = ()>,
    T::Error: 'static,
{
    type Request = ioframe::Item<MqttState<St>, mqtt::Codec>;
    type Response = Option<mqtt::Packet>;
    type Error = T::Error;
    type Future = Either<
        Either<
            Ready<Result<Self::Response, T::Error>>,
            LocalBoxFuture<'static, Result<Self::Response, T::Error>>,
        >,
        PublishResponse<T::Future, T::Error>,
    >;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        let res1 = self.publish.poll_ready(cx)?;
        let res2 = self.subscribe.poll_ready(cx)?;
        let res3 = self.unsubscribe.poll_ready(cx)?;

        if res1.is_pending() || res2.is_pending() || res3.is_pending() {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn call(&mut self, req: ioframe::Item<MqttState<St>, mqtt::Codec>) -> Self::Future {
        let (mut state, _, packet) = req.into_parts();

        log::trace!("Dispatch packet: {:#?}", packet);
        match packet {
            mqtt::Packet::PingRequest => {
                Either::Left(Either::Left(ok(Some(mqtt::Packet::PingResponse))))
            }
            mqtt::Packet::Disconnect => Either::Left(Either::Left(ok(None))),
            mqtt::Packet::Publish(publish) => {
                let packet_id = publish.packet_id;
                Either::Right(PublishResponse {
                    packet_id,
                    fut: self.publish.call(Publish::new(state, publish)),
                    _t: PhantomData,
                })
            }
            mqtt::Packet::PublishAck { packet_id } => {
                state.inner.get_mut().sink.complete_publish_qos1(packet_id);
                Either::Left(Either::Left(ok(None)))
            }
            mqtt::Packet::Subscribe {
                packet_id,
                topic_filters,
            } => Either::Left(Either::Right(
                SubscribeResponse {
                    packet_id,
                    fut: self.subscribe.call(Subscribe::new(state, topic_filters)),
                }
                .boxed_local(),
            )),
            mqtt::Packet::Unsubscribe {
                packet_id,
                topic_filters,
            } => Either::Left(Either::Right(
                self.unsubscribe
                    .call(Unsubscribe::new(state, topic_filters))
                    .map(move |_| Ok(Some(mqtt::Packet::UnsubscribeAck { packet_id })))
                    .boxed_local(),
            )),
            _ => Either::Left(Either::Left(ok(None))),
        }
    }
}

/// Publish service response future
#[pin_project::pin_project]
pub(crate) struct PublishResponse<T, E> {
    #[pin]
    fut: T,
    packet_id: Option<NonZeroU16>,
    _t: PhantomData<E>,
}

impl<T, E> Future for PublishResponse<T, E>
where
    T: Future<Output = Result<(), E>>,
{
    type Output = Result<Option<mqtt::Packet>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.project();

        ready!(this.fut.poll(cx))?;
        if let Some(packet_id) = this.packet_id {
            Poll::Ready(Ok(Some(mqtt::Packet::PublishAck {
                packet_id: *packet_id,
            })))
        } else {
            Poll::Ready(Ok(None))
        }
    }
}

/// Subscribe service response future
pub(crate) struct SubscribeResponse<E> {
    fut: LocalBoxFuture<'static, Result<SubscribeResult, E>>,
    packet_id: NonZeroU16,
}

impl<E> Future for SubscribeResponse<E> {
    type Output = Result<Option<mqtt::Packet>, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let res = ready!(Pin::new(&mut self.fut).poll(cx))?;
        Poll::Ready(Ok(Some(mqtt::Packet::SubscribeAck {
            status: res.codes,
            packet_id: self.packet_id,
        })))
    }
}
