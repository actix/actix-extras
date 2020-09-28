use actix_codec::{AsyncRead, AsyncWrite, Framed};
use actix_connect::{Connect as TcpConnect, Connection as TcpConnection};
use actix_service::{apply_fn, pipeline, IntoService, Service};
use actix_utils::time::LowResTimeService;
use bytestring::ByteString;
use either::Either;
use futures::future::{ok, Future};
use futures::{FutureExt, Sink, SinkExt, Stream, StreamExt};
use http::Uri;

use amqp_codec::protocol::{Frame, ProtocolId, SaslCode, SaslFrameBody, SaslInit};
use amqp_codec::types::Symbol;
use amqp_codec::{AmqpCodec, AmqpFrame, ProtocolIdCodec, SaslFrame};

use crate::connection::Connection;
use crate::service::ProtocolNegotiation;

use super::Configuration;
pub use crate::errors::SaslConnectError;

#[derive(Debug)]
/// Sasl connect request
pub struct SaslConnect {
    pub uri: Uri,
    pub config: Configuration,
    pub auth: SaslAuth,
    pub time: Option<LowResTimeService>,
}

#[derive(Debug)]
/// Sasl authentication parameters
pub struct SaslAuth {
    pub authz_id: String,
    pub authn_id: String,
    pub password: String,
}

/// Create service that connects to amqp server and authenticate itself via sasl.
/// This service uses supplied connector service. Service resolves to
/// a `Connection<_>` instance.
pub fn connect_service<T, Io>(
    connector: T,
) -> impl Service<
    Request = SaslConnect,
    Response = Connection<Io>,
    Error = either::Either<SaslConnectError, T::Error>,
>
where
    T: Service<Request = TcpConnect<Uri>, Response = TcpConnection<Uri, Io>>,
    T::Error: 'static,
    Io: AsyncRead + AsyncWrite + 'static,
{
    pipeline(|connect: SaslConnect| {
        let SaslConnect {
            uri,
            config,
            auth,
            time,
        } = connect;
        ok::<_, either::Either<SaslConnectError, T::Error>>((uri, config, auth, time))
    })
    // connect to host
    .and_then(apply_fn(
        connector.map_err(|e| either::Right(e)),
        |(uri, config, auth, time): (Uri, Configuration, _, _), srv| {
            let fut = srv.call(uri.clone().into());
            async move {
                fut.await.map(|stream| {
                    let (io, _) = stream.into_parts();
                    (io, uri, config, auth, time)
                })
            }
        },
    ))
    // sasl protocol negotiation
    .and_then(apply_fn(
        ProtocolNegotiation::new(ProtocolId::AmqpSasl)
            .map_err(|e| Either::Left(SaslConnectError::from(e))),
        |(io, uri, config, auth, time): (Io, _, _, _, _), srv| {
            let framed = Framed::new(io, ProtocolIdCodec);
            let fut = srv.call(framed);
            async move {
                fut.await
                    .map(move |framed| (framed, uri, config, auth, time))
            }
        },
    ))
    // sasl auth
    .and_then(apply_fn(
        sasl_connect.into_service().map_err(Either::Left),
        |(framed, uri, config, auth, time): (_, Uri, _, _, _), srv| {
            let fut = srv.call((framed, uri.clone(), auth));
            async move { fut.await.map(move |framed| (uri, config, framed, time)) }
        },
    ))
    // re-negotiate amqp protocol negotiation
    .and_then(apply_fn(
        ProtocolNegotiation::new(ProtocolId::Amqp)
            .map_err(|e| Either::Left(SaslConnectError::from(e))),
        |(uri, config, framed, time): (_, _, Framed<Io, ProtocolIdCodec>, _), srv| {
            let fut = srv.call(framed);
            async move { fut.await.map(move |framed| (uri, config, framed, time)) }
        },
    ))
    // open connection
    .and_then(
        |(uri, mut config, framed, time): (Uri, Configuration, Framed<Io, ProtocolIdCodec>, _)| {
            async move {
                let mut framed = framed.into_framed(AmqpCodec::<AmqpFrame>::new());
                if let Some(hostname) = uri.host() {
                    config.hostname(hostname);
                }
                let open = config.to_open();
                trace!("Open connection: {:?}", open);
                framed
                    .send(AmqpFrame::new(0, Frame::Open(open)))
                    .await
                    .map_err(|e| Either::Left(SaslConnectError::from(e)))
                    .map(move |_| (config, framed, time))
            }
        },
    )
    // read open frame
    .and_then(
        move |(config, mut framed, time): (Configuration, Framed<_, AmqpCodec<AmqpFrame>>, _)| {
            async move {
                let frame = framed
                    .next()
                    .await
                    .ok_or(Either::Left(SaslConnectError::Disconnected))?
                    .map_err(|e| Either::Left(SaslConnectError::from(e)))?;

                if let Frame::Open(open) = frame.performative() {
                    trace!("Open confirmed: {:?}", open);
                    Ok(Connection::new(framed, config, open.into(), time))
                } else {
                    Err(Either::Left(SaslConnectError::ExpectedOpenFrame))
                }
            }
        },
    )
}

async fn sasl_connect<Io: AsyncRead + AsyncWrite>(
    (framed, uri, auth): (Framed<Io, ProtocolIdCodec>, Uri, SaslAuth),
) -> Result<Framed<Io, ProtocolIdCodec>, SaslConnectError> {
    let mut sasl_io = framed.into_framed(AmqpCodec::<SaslFrame>::new());

    // processing sasl-mechanisms
    let _ = sasl_io
        .next()
        .await
        .ok_or(SaslConnectError::Disconnected)?
        .map_err(SaslConnectError::from)?;

    let initial_response =
        SaslInit::prepare_response(&auth.authz_id, &auth.authn_id, &auth.password);

    let hostname = uri.host().map(|host| ByteString::from(host));

    let sasl_init = SaslInit {
        hostname,
        mechanism: Symbol::from("PLAIN"),
        initial_response: Some(initial_response),
    };

    sasl_io
        .send(sasl_init.into())
        .await
        .map_err(SaslConnectError::from)?;

    // processing sasl-outcome
    let sasl_frame = sasl_io
        .next()
        .await
        .ok_or(SaslConnectError::Disconnected)?
        .map_err(SaslConnectError::from)?;

    if let SaslFrame {
        body: SaslFrameBody::SaslOutcome(outcome),
    } = sasl_frame
    {
        if outcome.code() != SaslCode::Ok {
            return Err(SaslConnectError::Sasl(outcome.code()));
        }
    } else {
        return Err(SaslConnectError::Disconnected);
    }
    Ok(sasl_io.into_framed(ProtocolIdCodec))
}
