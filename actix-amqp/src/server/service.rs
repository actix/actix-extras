use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, time};

use actix_codec::{AsyncRead, AsyncWrite, Framed};
use actix_service::{boxed, IntoServiceFactory, Service, ServiceFactory};
use amqp_codec::protocol::{Error, ProtocolId};
use amqp_codec::{AmqpCodecError, AmqpFrame, ProtocolIdCodec, ProtocolIdError};
use futures::future::{err, poll_fn, Either};
use futures::{FutureExt, SinkExt, StreamExt};

use crate::cell::Cell;
use crate::connection::{Connection, ConnectionController};
use crate::Configuration;

use super::connect::{Connect, ConnectAck};
use super::control::{ControlFrame, ControlFrameNewService};
use super::dispatcher::Dispatcher;
use super::errors::{LinkError, ServerError};
use super::link::Link;
use super::sasl::Sasl;
use super::State;

/// Amqp connection type
pub type AmqpConnect<Io> = either::Either<Connect<Io>, Sasl<Io>>;

/// Server dispatcher factory
pub struct Server<Io, St, Cn: ServiceFactory> {
    connect: Cn,
    config: Configuration,
    control: Option<ControlFrameNewService<St>>,
    disconnect: Option<Box<dyn Fn(&mut St, Option<&ServerError<Cn::Error>>)>>,
    max_size: usize,
    handshake_timeout: u64,
    _t: PhantomData<(Io, St)>,
}

pub(super) struct ServerInner<St, Cn: ServiceFactory, Pb> {
    connect: Cn,
    publish: Pb,
    config: Configuration,
    control: Option<ControlFrameNewService<St>>,
    disconnect: Option<Box<dyn Fn(&mut St, Option<&ServerError<Cn::Error>>)>>,
    max_size: usize,
    handshake_timeout: u64,
}

impl<Io, St, Cn> Server<Io, St, Cn>
where
    St: 'static,
    Io: AsyncRead + AsyncWrite + 'static,
    Cn: ServiceFactory<Config = (), Request = AmqpConnect<Io>, Response = ConnectAck<Io, St>>
        + 'static,
{
    /// Create server factory and provide connect service
    pub fn new<F>(connect: F) -> Self
    where
        F: IntoServiceFactory<Cn>,
    {
        Self {
            connect: connect.into_factory(),
            config: Configuration::default(),
            control: None,
            disconnect: None,
            max_size: 0,
            handshake_timeout: 0,
            _t: PhantomData,
        }
    }

    /// Provide connection configuration
    pub fn config(mut self, config: Configuration) -> Self {
        self.config = config;
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

    /// Set handshake timeout in millis.
    ///
    /// By default handshake timeuot is disabled.
    pub fn handshake_timeout(mut self, timeout: u64) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    /// Service to call with control frames
    pub fn control<F, S>(self, f: F) -> Self
    where
        F: IntoServiceFactory<S>,
        S: ServiceFactory<Config = (), Request = ControlFrame<St>, Response = (), InitError = ()>
            + 'static,
        S::Error: Into<LinkError>,
    {
        Server {
            connect: self.connect,
            config: self.config,
            disconnect: self.disconnect,
            control: Some(boxed::factory(
                f.into_factory()
                    .map_err(|e| e.into())
                    .map_init_err(|e| e.into()),
            )),
            max_size: self.max_size,
            handshake_timeout: self.handshake_timeout,
            _t: PhantomData,
        }
    }

    /// Callback to execute on disconnect
    ///
    /// Second parameter indicates error occured during disconnect.
    pub fn disconnect<F, Out>(self, disconnect: F) -> Self
    where
        F: Fn(&mut St, Option<&ServerError<Cn::Error>>) -> Out + 'static,
        Out: Future + 'static,
    {
        Server {
            connect: self.connect,
            config: self.config,
            control: self.control,
            disconnect: Some(Box::new(move |st, err| {
                let fut = disconnect(st, err);
                actix_rt::spawn(fut.map(|_| ()));
            })),
            max_size: self.max_size,
            handshake_timeout: self.handshake_timeout,
            _t: PhantomData,
        }
    }

    /// Set service to execute for incoming links and create service factory
    pub fn finish<F, Pb>(
        self,
        service: F,
    ) -> impl ServiceFactory<Config = (), Request = Io, Response = (), Error = ServerError<Cn::Error>>
    where
        F: IntoServiceFactory<Pb>,
        Pb: ServiceFactory<Config = State<St>, Request = Link<St>, Response = ()> + 'static,
        Pb::Error: fmt::Display + Into<Error>,
        Pb::InitError: fmt::Display + Into<Error>,
    {
        ServerImpl {
            inner: Cell::new(ServerInner {
                connect: self.connect,
                config: self.config,
                publish: service.into_factory(),
                control: self.control,
                disconnect: self.disconnect,
                max_size: self.max_size,
                handshake_timeout: self.handshake_timeout,
            }),
            _t: PhantomData,
        }
    }
}

struct ServerImpl<Io, St, Cn: ServiceFactory, Pb> {
    inner: Cell<ServerInner<St, Cn, Pb>>,
    _t: PhantomData<(Io,)>,
}

impl<Io, St, Cn, Pb> ServiceFactory for ServerImpl<Io, St, Cn, Pb>
where
    St: 'static,
    Io: AsyncRead + AsyncWrite + 'static,
    Cn: ServiceFactory<Config = (), Request = AmqpConnect<Io>, Response = ConnectAck<Io, St>>
        + 'static,
    Pb: ServiceFactory<Config = State<St>, Request = Link<St>, Response = ()> + 'static,
    Pb::Error: fmt::Display + Into<Error>,
    Pb::InitError: fmt::Display + Into<Error>,
{
    type Config = ();
    type Request = Io;
    type Response = ();
    type Error = ServerError<Cn::Error>;
    type Service = ServerImplService<Io, St, Cn, Pb>;
    type InitError = Cn::InitError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Service, Cn::InitError>>>>;

    fn new_service(&self, _: ()) -> Self::Future {
        let inner = self.inner.clone();

        Box::pin(async move {
            inner
                .connect
                .new_service(())
                .await
                .map(move |connect| ServerImplService {
                    inner,
                    connect: Cell::new(connect),
                    _t: PhantomData,
                })
        })
    }
}

struct ServerImplService<Io, St, Cn: ServiceFactory, Pb> {
    connect: Cell<Cn::Service>,
    inner: Cell<ServerInner<St, Cn, Pb>>,
    _t: PhantomData<(Io,)>,
}

impl<Io, St, Cn, Pb> Service for ServerImplService<Io, St, Cn, Pb>
where
    St: 'static,
    Io: AsyncRead + AsyncWrite + 'static,
    Cn: ServiceFactory<Config = (), Request = AmqpConnect<Io>, Response = ConnectAck<Io, St>>
        + 'static,
    Pb: ServiceFactory<Config = State<St>, Request = Link<St>, Response = ()> + 'static,
    Pb::Error: fmt::Display + Into<Error>,
    Pb::InitError: fmt::Display + Into<Error>,
{
    type Request = Io;
    type Response = ();
    type Error = ServerError<Cn::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.connect
            .get_mut()
            .poll_ready(cx)
            .map(|res| res.map_err(|e| ServerError::Service(e)))
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let timeout = self.inner.handshake_timeout;
        if timeout == 0 {
            Box::pin(handshake(
                self.inner.max_size,
                self.connect.clone(),
                self.inner.clone(),
                req,
            ))
        } else {
            Box::pin(
                actix_rt::time::timeout(
                    time::Duration::from_millis(timeout),
                    handshake(
                        self.inner.max_size,
                        self.connect.clone(),
                        self.inner.clone(),
                        req,
                    ),
                )
                .map(|res| match res {
                    Ok(res) => res,
                    Err(_) => Err(ServerError::HandshakeTimeout),
                }),
            )
        }
    }
}

async fn handshake<Io, St, Cn: ServiceFactory, Pb>(
    max_size: usize,
    connect: Cell<Cn::Service>,
    inner: Cell<ServerInner<St, Cn, Pb>>,
    io: Io,
) -> Result<(), ServerError<Cn::Error>>
where
    St: 'static,
    Io: AsyncRead + AsyncWrite + 'static,
    Cn: ServiceFactory<Config = (), Request = AmqpConnect<Io>, Response = ConnectAck<Io, St>>,
    Pb: ServiceFactory<Config = State<St>, Request = Link<St>, Response = ()> + 'static,
    Pb::Error: fmt::Display + Into<Error>,
    Pb::InitError: fmt::Display + Into<Error>,
{
    let inner2 = inner.clone();
    let mut framed = Framed::new(io, ProtocolIdCodec);

    let protocol = framed
        .next()
        .await
        .ok_or(ServerError::Disconnected)?
        .map_err(ServerError::Handshake)?;

    let (st, srv, conn) = match protocol {
        // start amqp processing
        ProtocolId::Amqp | ProtocolId::AmqpSasl => {
            framed.send(protocol).await.map_err(ServerError::from)?;

            let cfg = inner.get_ref().config.clone();
            let controller = ConnectionController::new(cfg.clone());

            let ack = connect
                .get_mut()
                .call(if protocol == ProtocolId::Amqp {
                    either::Either::Left(Connect::new(framed, controller))
                } else {
                    either::Either::Right(Sasl::new(framed, controller))
                })
                .await
                .map_err(|e| ServerError::Service(e))?;

            let (st, mut framed, controller) = ack.into_inner();
            let st = State::new(st);
            framed.get_codec_mut().max_size(max_size);

            // confirm Open
            let local = cfg.to_open();
            framed
                .send(AmqpFrame::new(0, local.into()))
                .await
                .map_err(ServerError::from)?;

            let conn = Connection::new_server(framed, controller.0, None);

            // create publish service
            let srv = inner.publish.new_service(st.clone()).await.map_err(|e| {
                error!("Can not construct app service");
                ServerError::ProtocolError(e.into())
            })?;

            (st, srv, conn)
        }
        ProtocolId::AmqpTls => {
            return Err(ServerError::from(ProtocolIdError::Unexpected {
                exp: ProtocolId::Amqp,
                got: ProtocolId::AmqpTls,
            }))
        }
    };

    let mut st2 = st.clone();

    if let Some(ref control_srv) = inner2.control {
        let control = control_srv
            .new_service(())
            .await
            .map_err(|_| ServerError::ControlServiceInit)?;

        let res = Dispatcher::new(conn, st, srv, Some(control))
            .await
            .map_err(ServerError::from);

        if inner2.disconnect.is_some() {
            (*inner2.get_mut().disconnect.as_mut().unwrap())(st2.get_mut(), res.as_ref().err())
        }
        res
    } else {
        let res = Dispatcher::new(conn, st, srv, None)
            .await
            .map_err(ServerError::from);

        if inner2.disconnect.is_some() {
            (*inner2.get_mut().disconnect.as_mut().unwrap())(st2.get_mut(), res.as_ref().err())
        }
        res
    }
}
