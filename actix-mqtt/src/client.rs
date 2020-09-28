use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use actix_codec::{AsyncRead, AsyncWrite};
use actix_ioframe as ioframe;
use actix_service::{boxed, IntoService, IntoServiceFactory, Service, ServiceFactory};
use bytes::Bytes;
use bytestring::ByteString;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::{Sink, SinkExt, Stream, StreamExt};
use mqtt_codec as mqtt;

use crate::cell::Cell;
use crate::default::{SubsNotImplemented, UnsubsNotImplemented};
use crate::dispatcher::{dispatcher, MqttState};
use crate::error::MqttError;
use crate::publish::Publish;
use crate::sink::MqttSink;
use crate::subs::{Subscribe, SubscribeResult, Unsubscribe};

/// Mqtt client
#[derive(Clone)]
pub struct Client<Io, St> {
    client_id: ByteString,
    clean_session: bool,
    protocol: mqtt::Protocol,
    keep_alive: u16,
    last_will: Option<mqtt::LastWill>,
    username: Option<ByteString>,
    password: Option<Bytes>,
    inflight: usize,
    _t: PhantomData<(Io, St)>,
}

impl<Io, St> Client<Io, St>
where
    St: 'static,
{
    /// Create new client and provide client id
    pub fn new(client_id: ByteString) -> Self {
        Client {
            client_id,
            clean_session: true,
            protocol: mqtt::Protocol::default(),
            keep_alive: 30,
            last_will: None,
            username: None,
            password: None,
            inflight: 15,
            _t: PhantomData,
        }
    }

    /// Mqtt protocol version
    pub fn protocol(mut self, val: mqtt::Protocol) -> Self {
        self.protocol = val;
        self
    }

    /// The handling of the Session state.
    pub fn clean_session(mut self, val: bool) -> Self {
        self.clean_session = val;
        self
    }

    /// A time interval measured in seconds.
    ///
    /// keep-alive is set to 30 seconds by default.
    pub fn keep_alive(mut self, val: u16) -> Self {
        self.keep_alive = val;
        self
    }

    /// Will Message be stored on the Server and associated with the Network Connection.
    ///
    /// by default last will value is not set
    pub fn last_will(mut self, val: mqtt::LastWill) -> Self {
        self.last_will = Some(val);
        self
    }

    /// Username can be used by the Server for authentication and authorization.
    pub fn username(mut self, val: ByteString) -> Self {
        self.username = Some(val);
        self
    }

    /// Password can be used by the Server for authentication and authorization.
    pub fn password(mut self, val: Bytes) -> Self {
        self.password = Some(val);
        self
    }

    /// Number of in-flight concurrent messages.
    ///
    /// in-flight is set to 15 messages
    pub fn inflight(mut self, val: usize) -> Self {
        self.inflight = val;
        self
    }

    /// Set state service
    ///
    /// State service verifies connect ack packet and construct connection state.
    pub fn state<C, F>(self, state: F) -> ServiceBuilder<Io, St, C>
    where
        F: IntoService<C>,
        Io: AsyncRead + AsyncWrite,
        C: Service<Request = ConnectAck<Io>, Response = ConnectAckResult<Io, St>>,
        C::Error: 'static,
    {
        ServiceBuilder {
            state: Cell::new(state.into_service()),
            packet: mqtt::Connect {
                client_id: self.client_id,
                clean_session: self.clean_session,
                protocol: self.protocol,
                keep_alive: self.keep_alive,
                last_will: self.last_will,
                username: self.username,
                password: self.password,
            },
            subscribe: Rc::new(boxed::factory(SubsNotImplemented::default())),
            unsubscribe: Rc::new(boxed::factory(UnsubsNotImplemented::default())),
            disconnect: None,
            keep_alive: self.keep_alive.into(),
            inflight: self.inflight,
            _t: PhantomData,
        }
    }
}

pub struct ServiceBuilder<Io, St, C: Service> {
    state: Cell<C>,
    packet: mqtt::Connect,
    subscribe: Rc<
        boxed::BoxServiceFactory<
            St,
            Subscribe<St>,
            SubscribeResult,
            MqttError<C::Error>,
            MqttError<C::Error>,
        >,
    >,
    unsubscribe: Rc<
        boxed::BoxServiceFactory<
            St,
            Unsubscribe<St>,
            (),
            MqttError<C::Error>,
            MqttError<C::Error>,
        >,
    >,
    disconnect: Option<Cell<boxed::BoxService<St, (), MqttError<C::Error>>>>,
    keep_alive: u64,
    inflight: usize,

    _t: PhantomData<(Io, St, C)>,
}

impl<Io, St, C> ServiceBuilder<Io, St, C>
where
    St: Clone + 'static,
    Io: AsyncRead + AsyncWrite + 'static,
    C: Service<Request = ConnectAck<Io>, Response = ConnectAckResult<Io, St>> + 'static,
    C::Error: 'static,
{
    /// Service to execute on disconnect
    pub fn disconnect<UF, U>(mut self, srv: UF) -> Self
    where
        UF: IntoService<U>,
        U: Service<Request = St, Response = (), Error = C::Error> + 'static,
    {
        self.disconnect = Some(Cell::new(boxed::service(
            srv.into_service().map_err(MqttError::Service),
        )));
        self
    }

    pub fn finish<F, T>(
        self,
        service: F,
    ) -> impl Service<Request = Io, Response = (), Error = MqttError<C::Error>>
    where
        F: IntoServiceFactory<T>,
        T: ServiceFactory<
                Config = St,
                Request = Publish<St>,
                Response = (),
                Error = C::Error,
                InitError = C::Error,
            > + 'static,
    {
        ioframe::Builder::new()
            .service(ConnectService {
                connect: self.state,
                packet: self.packet,
                keep_alive: self.keep_alive,
                inflight: self.inflight,
                _t: PhantomData,
            })
            .finish(dispatcher(
                service
                    .into_factory()
                    .map_err(MqttError::Service)
                    .map_init_err(MqttError::Service),
                self.subscribe,
                self.unsubscribe,
            ))
            .map_err(|e| match e {
                ioframe::ServiceError::Service(e) => e,
                ioframe::ServiceError::Encoder(e) => MqttError::Protocol(e),
                ioframe::ServiceError::Decoder(e) => MqttError::Protocol(e),
            })
    }
}

struct ConnectService<Io, St, C> {
    connect: Cell<C>,
    packet: mqtt::Connect,
    keep_alive: u64,
    inflight: usize,
    _t: PhantomData<(Io, St)>,
}

impl<Io, St, C> Service for ConnectService<Io, St, C>
where
    St: 'static,
    Io: AsyncRead + AsyncWrite + 'static,
    C: Service<Request = ConnectAck<Io>, Response = ConnectAckResult<Io, St>> + 'static,
    C::Error: 'static,
{
    type Request = ioframe::Connect<Io, mqtt::Codec>;
    type Response = ioframe::ConnectResult<Io, MqttState<St>, mqtt::Codec>;
    type Error = MqttError<C::Error>;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.connect
            .get_mut()
            .poll_ready(cx)
            .map_err(MqttError::Service)
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let mut srv = self.connect.clone();
        let packet = self.packet.clone();
        let keep_alive = Duration::from_secs(self.keep_alive as u64);
        let inflight = self.inflight;

        // send Connect packet
        async move {
            let mut framed = req.codec(mqtt::Codec::new());
            framed
                .send(mqtt::Packet::Connect(packet))
                .await
                .map_err(MqttError::Protocol)?;

            let packet = framed
                .next()
                .await
                .ok_or(MqttError::Disconnected)
                .and_then(|res| res.map_err(MqttError::Protocol))?;

            match packet {
                mqtt::Packet::ConnectAck {
                    session_present,
                    return_code,
                } => {
                    let sink = MqttSink::new(framed.sink().clone());
                    let ack = ConnectAck {
                        sink,
                        session_present,
                        return_code,
                        keep_alive,
                        inflight,
                        io: framed,
                    };
                    Ok(srv
                        .get_mut()
                        .call(ack)
                        .await
                        .map_err(MqttError::Service)
                        .map(|ack| ack.io.state(ack.state))?)
                }
                p => Err(MqttError::Unexpected(p, "Expected CONNECT-ACK packet")),
            }
        }
        .boxed_local()
    }
}

pub struct ConnectAck<Io> {
    io: ioframe::ConnectResult<Io, (), mqtt::Codec>,
    sink: MqttSink,
    session_present: bool,
    return_code: mqtt::ConnectCode,
    keep_alive: Duration,
    inflight: usize,
}

impl<Io> ConnectAck<Io> {
    #[inline]
    /// Indicates whether there is already stored Session state
    pub fn session_present(&self) -> bool {
        self.session_present
    }

    #[inline]
    /// Connect return code
    pub fn return_code(&self) -> mqtt::ConnectCode {
        self.return_code
    }

    #[inline]
    /// Mqtt client sink object
    pub fn sink(&self) -> &MqttSink {
        &self.sink
    }

    #[inline]
    /// Set connection state and create result object
    pub fn state<St>(self, state: St) -> ConnectAckResult<Io, St> {
        ConnectAckResult {
            io: self.io,
            state: MqttState::new(state, self.sink, self.keep_alive, self.inflight),
        }
    }
}

impl<Io> Stream for ConnectAck<Io>
where
    Io: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<mqtt::Packet, mqtt::ParseError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.io).poll_next(cx)
    }
}

impl<Io> Sink<mqtt::Packet> for ConnectAck<Io>
where
    Io: AsyncRead + AsyncWrite + Unpin,
{
    type Error = mqtt::ParseError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.io).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: mqtt::Packet) -> Result<(), Self::Error> {
        Pin::new(&mut self.io).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.io).poll_close(cx)
    }
}

#[pin_project::pin_project]
pub struct ConnectAckResult<Io, St> {
    state: MqttState<St>,
    io: ioframe::ConnectResult<Io, (), mqtt::Codec>,
}

impl<Io, St> Stream for ConnectAckResult<Io, St>
where
    Io: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<mqtt::Packet, mqtt::ParseError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.io).poll_next(cx)
    }
}

impl<Io, St> Sink<mqtt::Packet> for ConnectAckResult<Io, St>
where
    Io: AsyncRead + AsyncWrite + Unpin,
{
    type Error = mqtt::ParseError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.io).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: mqtt::Packet) -> Result<(), Self::Error> {
        Pin::new(&mut self.io).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.io).poll_close(cx)
    }
}
