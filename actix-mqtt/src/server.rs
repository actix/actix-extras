use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use actix_codec::{AsyncRead, AsyncWrite};
use actix_ioframe as ioframe;
use actix_service::{apply, apply_fn, boxed, fn_factory, pipeline_factory, unit_config};
use actix_service::{IntoServiceFactory, Service, ServiceFactory};
use actix_utils::timeout::{Timeout, TimeoutError};
use futures::{FutureExt, SinkExt, StreamExt};
use mqtt_codec as mqtt;

use crate::cell::Cell;
use crate::connect::{Connect, ConnectAck};
use crate::default::{SubsNotImplemented, UnsubsNotImplemented};
use crate::dispatcher::{dispatcher, MqttState};
use crate::error::MqttError;
use crate::publish::Publish;
use crate::sink::MqttSink;
use crate::subs::{Subscribe, SubscribeResult, Unsubscribe};

/// Mqtt Server
pub struct MqttServer<Io, St, C: ServiceFactory, U> {
    connect: C,
    subscribe: boxed::BoxServiceFactory<
        St,
        Subscribe<St>,
        SubscribeResult,
        MqttError<C::Error>,
        MqttError<C::Error>,
    >,
    unsubscribe: boxed::BoxServiceFactory<
        St,
        Unsubscribe<St>,
        (),
        MqttError<C::Error>,
        MqttError<C::Error>,
    >,
    disconnect: U,
    max_size: usize,
    inflight: usize,
    handshake_timeout: u64,
    _t: PhantomData<(Io, St)>,
}

fn default_disconnect<St>(_: St, _: bool) {}

impl<Io, St, C> MqttServer<Io, St, C, ()>
where
    St: 'static,
    C: ServiceFactory<Config = (), Request = Connect<Io>, Response = ConnectAck<Io, St>>
        + 'static,
{
    /// Create server factory and provide connect service
    pub fn new<F>(connect: F) -> MqttServer<Io, St, C, impl Fn(St, bool)>
    where
        F: IntoServiceFactory<C>,
    {
        MqttServer {
            connect: connect.into_factory(),
            subscribe: boxed::factory(
                pipeline_factory(SubsNotImplemented::default())
                    .map_err(MqttError::Service)
                    .map_init_err(MqttError::Service),
            ),
            unsubscribe: boxed::factory(
                pipeline_factory(UnsubsNotImplemented::default())
                    .map_err(MqttError::Service)
                    .map_init_err(MqttError::Service),
            ),
            max_size: 0,
            inflight: 15,
            disconnect: default_disconnect,
            handshake_timeout: 0,
            _t: PhantomData,
        }
    }
}

impl<Io, St, C, U> MqttServer<Io, St, C, U>
where
    St: Clone + 'static,
    U: Fn(St, bool) + 'static,
    C: ServiceFactory<Config = (), Request = Connect<Io>, Response = ConnectAck<Io, St>>
        + 'static,
{
    /// Set handshake timeout in millis.
    ///
    /// Handshake includes `connect` packet and response `connect-ack`.
    /// By default handshake timeuot is disabled.
    pub fn handshake_timeout(mut self, timeout: u64) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    /// Set max inbound frame size.
    ///
    /// If max size is set to `0`, size is unlimited.
    /// By default max size is set to `0`
    pub fn max_size(mut self, size: usize) -> Self {
        self.max_size = size;
        self
    }

    /// Number of in-flight concurrent messages.
    ///
    /// in-flight is set to 15 messages
    pub fn inflight(mut self, val: usize) -> Self {
        self.inflight = val;
        self
    }

    /// Service to execute for subscribe packet
    pub fn subscribe<F, Srv>(mut self, subscribe: F) -> Self
    where
        F: IntoServiceFactory<Srv>,
        Srv: ServiceFactory<Config = St, Request = Subscribe<St>, Response = SubscribeResult>
            + 'static,
        C::Error: From<Srv::Error> + From<Srv::InitError>,
    {
        self.subscribe = boxed::factory(
            subscribe
                .into_factory()
                .map_err(|e| MqttError::Service(e.into()))
                .map_init_err(|e| MqttError::Service(e.into())),
        );
        self
    }

    /// Service to execute for unsubscribe packet
    pub fn unsubscribe<F, Srv>(mut self, unsubscribe: F) -> Self
    where
        F: IntoServiceFactory<Srv>,
        Srv: ServiceFactory<Config = St, Request = Unsubscribe<St>, Response = ()> + 'static,
        C::Error: From<Srv::Error> + From<Srv::InitError>,
    {
        self.unsubscribe = boxed::factory(
            unsubscribe
                .into_factory()
                .map_err(|e| MqttError::Service(e.into()))
                .map_init_err(|e| MqttError::Service(e.into())),
        );
        self
    }

    /// Callback to execute on disconnect
    ///
    /// Second parameter indicates error occured during disconnect.
    pub fn disconnect<F, Out>(self, disconnect: F) -> MqttServer<Io, St, C, impl Fn(St, bool)>
    where
        F: Fn(St, bool) -> Out,
        Out: Future + 'static,
    {
        MqttServer {
            connect: self.connect,
            subscribe: self.subscribe,
            unsubscribe: self.unsubscribe,
            max_size: self.max_size,
            inflight: self.inflight,
            handshake_timeout: self.handshake_timeout,
            disconnect: move |st: St, err| {
                let fut = disconnect(st, err);
                actix_rt::spawn(fut.map(|_| ()));
            },
            _t: PhantomData,
        }
    }

    /// Set service to execute for publish packet and create service factory
    pub fn finish<F, P>(
        self,
        publish: F,
    ) -> impl ServiceFactory<Config = (), Request = Io, Response = (), Error = MqttError<C::Error>>
    where
        Io: AsyncRead + AsyncWrite + 'static,
        F: IntoServiceFactory<P>,
        P: ServiceFactory<Config = St, Request = Publish<St>, Response = ()> + 'static,
        C::Error: From<P::Error> + From<P::InitError>,
    {
        let connect = self.connect;
        let max_size = self.max_size;
        let handshake_timeout = self.handshake_timeout;
        let disconnect = self.disconnect;
        let publish = boxed::factory(
            publish
                .into_factory()
                .map_err(|e| MqttError::Service(e.into()))
                .map_init_err(|e| MqttError::Service(e.into())),
        );

        unit_config(
            ioframe::Builder::new()
                .factory(connect_service_factory(
                    connect,
                    max_size,
                    self.inflight,
                    handshake_timeout,
                ))
                .disconnect(move |cfg, err| disconnect(cfg.session().clone(), err))
                .finish(dispatcher(
                    publish,
                    Rc::new(self.subscribe),
                    Rc::new(self.unsubscribe),
                ))
                .map_err(|e| match e {
                    ioframe::ServiceError::Service(e) => e,
                    ioframe::ServiceError::Encoder(e) => MqttError::Protocol(e),
                    ioframe::ServiceError::Decoder(e) => MqttError::Protocol(e),
                }),
        )
    }
}

fn connect_service_factory<Io, St, C>(
    factory: C,
    max_size: usize,
    inflight: usize,
    handshake_timeout: u64,
) -> impl ServiceFactory<
    Config = (),
    Request = ioframe::Connect<Io, mqtt::Codec>,
    Response = ioframe::ConnectResult<Io, MqttState<St>, mqtt::Codec>,
    Error = MqttError<C::Error>,
>
where
    Io: AsyncRead + AsyncWrite,
    C: ServiceFactory<Config = (), Request = Connect<Io>, Response = ConnectAck<Io, St>>,
{
    apply(
        Timeout::new(Duration::from_millis(handshake_timeout)),
        fn_factory(move || {
            let fut = factory.new_service(());

            async move {
                let service = Cell::new(fut.await?);

                Ok::<_, C::InitError>(apply_fn(
                    service.map_err(MqttError::Service),
                    move |conn: ioframe::Connect<Io, mqtt::Codec>, service| {
                        let mut srv = service.clone();
                        let mut framed = conn.codec(mqtt::Codec::new().max_size(max_size));

                        async move {
                            // read first packet
                            let packet = framed
                                .next()
                                .await
                                .ok_or(MqttError::Disconnected)
                                .and_then(|res| res.map_err(|e| MqttError::Protocol(e)))?;

                            match packet {
                                mqtt::Packet::Connect(connect) => {
                                    let sink = MqttSink::new(framed.sink().clone());

                                    // authenticate mqtt connection
                                    let mut ack = srv
                                        .call(Connect::new(
                                            connect,
                                            framed,
                                            sink.clone(),
                                            inflight,
                                        ))
                                        .await?;

                                    match ack.session {
                                        Some(session) => {
                                            log::trace!(
                                                "Sending: {:#?}",
                                                mqtt::Packet::ConnectAck {
                                                    session_present: ack.session_present,
                                                    return_code:
                                                        mqtt::ConnectCode::ConnectionAccepted,
                                                }
                                            );
                                            ack.io
                                                .send(mqtt::Packet::ConnectAck {
                                                    session_present: ack.session_present,
                                                    return_code:
                                                        mqtt::ConnectCode::ConnectionAccepted,
                                                })
                                                .await?;

                                            Ok(ack.io.state(MqttState::new(
                                                session,
                                                sink,
                                                ack.keep_alive,
                                                ack.inflight,
                                            )))
                                        }
                                        None => {
                                            log::trace!(
                                                "Sending: {:#?}",
                                                mqtt::Packet::ConnectAck {
                                                    session_present: false,
                                                    return_code: ack.return_code,
                                                }
                                            );

                                            ack.io
                                                .send(mqtt::Packet::ConnectAck {
                                                    session_present: false,
                                                    return_code: ack.return_code,
                                                })
                                                .await?;
                                            Err(MqttError::Disconnected)
                                        }
                                    }
                                }
                                packet => {
                                    log::info!(
                                        "MQTT-3.1.0-1: Expected CONNECT packet, received {}",
                                        packet.packet_type()
                                    );
                                    Err(MqttError::Unexpected(
                                        packet,
                                        "MQTT-3.1.0-1: Expected CONNECT packet",
                                    ))
                                }
                            }
                        }
                    },
                ))
            }
        }),
    )
    .map_err(|e| match e {
        TimeoutError::Service(e) => e,
        TimeoutError::Timeout => MqttError::HandshakeTimeout,
    })
}
