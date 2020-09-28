use std::collections::VecDeque;
use std::future::Future;

use actix_utils::oneshot;
use amqp_codec::protocol::{
    Attach, DeliveryNumber, DeliveryState, Disposition, Error, Flow, ReceiverSettleMode, Role,
    SenderSettleMode, SequenceNo, Target, TerminusDurability, TerminusExpiryPolicy, TransferBody,
};
use bytes::Bytes;
use bytestring::ByteString;
use futures::future::{ok, Either};

use crate::cell::Cell;
use crate::errors::AmqpTransportError;
use crate::session::{Session, SessionInner};
use crate::{Delivery, DeliveryPromise, Handle};

#[derive(Clone)]
pub struct SenderLink {
    pub(crate) inner: Cell<SenderLinkInner>,
}

impl std::fmt::Debug for SenderLink {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_tuple("SenderLink")
            .field(&std::ops::Deref::deref(&self.inner.get_ref().name))
            .finish()
    }
}

pub(crate) struct SenderLinkInner {
    id: usize,
    idx: u32,
    name: ByteString,
    session: Session,
    remote_handle: Handle,
    delivery_count: SequenceNo,
    link_credit: u32,
    pending_transfers: VecDeque<PendingTransfer>,
    error: Option<AmqpTransportError>,
    closed: bool,
}

struct PendingTransfer {
    idx: u32,
    tag: Option<Bytes>,
    body: Option<TransferBody>,
    promise: DeliveryPromise,
    settle: Option<bool>,
}

impl SenderLink {
    pub(crate) fn new(inner: Cell<SenderLinkInner>) -> SenderLink {
        SenderLink { inner }
    }

    pub fn id(&self) -> u32 {
        self.inner.id as u32
    }

    pub fn name(&self) -> &ByteString {
        &self.inner.name
    }

    pub fn remote_handle(&self) -> Handle {
        self.inner.remote_handle
    }

    pub fn session(&self) -> &Session {
        &self.inner.get_ref().session
    }

    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.inner.get_mut().session
    }

    pub fn send<T>(&self, body: T) -> impl Future<Output = Result<Disposition, AmqpTransportError>>
    where
        T: Into<TransferBody>,
    {
        self.inner.get_mut().send(body, None)
    }

    pub fn send_with_tag<T>(
        &self,
        body: T,
        tag: Bytes,
    ) -> impl Future<Output = Result<Disposition, AmqpTransportError>>
    where
        T: Into<TransferBody>,
    {
        self.inner.get_mut().send(body, Some(tag))
    }

    pub fn settle_message(&self, id: DeliveryNumber, state: DeliveryState) {
        self.inner.get_mut().settle_message(id, state)
    }

    pub fn close(&self) -> impl Future<Output = Result<(), AmqpTransportError>> {
        self.inner.get_mut().close(None)
    }

    pub fn close_with_error(
        &self,
        error: Error,
    ) -> impl Future<Output = Result<(), AmqpTransportError>> {
        self.inner.get_mut().close(Some(error))
    }
}

impl SenderLinkInner {
    pub(crate) fn new(
        id: usize,
        name: ByteString,
        handle: Handle,
        delivery_count: SequenceNo,
        session: Cell<SessionInner>,
    ) -> SenderLinkInner {
        SenderLinkInner {
            id,
            name,
            delivery_count,
            idx: 0,
            session: Session::new(session),
            remote_handle: handle,
            link_credit: 0,
            pending_transfers: VecDeque::new(),
            error: None,
            closed: false,
        }
    }

    pub fn id(&self) -> u32 {
        self.id as u32
    }

    pub fn remote_handle(&self) -> Handle {
        self.remote_handle
    }

    pub(crate) fn name(&self) -> &ByteString {
        &self.name
    }

    pub(crate) fn detached(&mut self, err: AmqpTransportError) {
        // drop pending transfers
        for tr in self.pending_transfers.drain(..) {
            let _ = tr.promise.send(Err(err.clone()));
        }

        self.error = Some(err);
    }

    pub(crate) fn close(
        &self,
        error: Option<Error>,
    ) -> impl Future<Output = Result<(), AmqpTransportError>> {
        if self.closed {
            Either::Left(ok(()))
        } else {
            let (tx, rx) = oneshot::channel();

            self.session
                .inner
                .get_mut()
                .detach_sender_link(self.id, true, error, tx);

            Either::Right(async move {
                match rx.await {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(AmqpTransportError::Disconnected),
                }
            })
        }
    }

    pub(crate) fn set_error(&mut self, err: AmqpTransportError) {
        // drop pending transfers
        for tr in self.pending_transfers.drain(..) {
            let _ = tr.promise.send(Err(err.clone()));
        }

        self.error = Some(err);
    }

    pub fn apply_flow(&mut self, flow: &Flow) {
        // #2.7.6
        if let Some(credit) = flow.link_credit() {
            let delta = flow
                .delivery_count
                .unwrap_or(0)
                .saturating_add(credit)
                .saturating_sub(self.delivery_count);

            let session = self.session.inner.get_mut();

            // credit became available => drain pending_transfers
            self.link_credit += delta;
            while self.link_credit > 0 {
                if let Some(transfer) = self.pending_transfers.pop_front() {
                    self.link_credit -= 1;
                    let _ = self.delivery_count.saturating_add(1);
                    session.send_transfer(
                        self.remote_handle,
                        transfer.idx,
                        transfer.body,
                        transfer.promise,
                        transfer.tag,
                        transfer.settle,
                    );
                } else {
                    break;
                }
            }
        }

        if flow.echo() {
            // todo: send flow
        }
    }

    pub fn send<T: Into<TransferBody>>(&mut self, body: T, tag: Option<Bytes>) -> Delivery {
        if let Some(ref err) = self.error {
            Delivery::Resolved(Err(err.clone()))
        } else {
            let body = body.into();
            let (delivery_tx, delivery_rx) = oneshot::channel();
            if self.link_credit == 0 {
                self.pending_transfers.push_back(PendingTransfer {
                    tag,
                    settle: Some(false),
                    body: Some(body),
                    idx: self.idx,
                    promise: delivery_tx,
                });
            } else {
                let session = self.session.inner.get_mut();
                self.link_credit -= 1;
                let _ = self.delivery_count.saturating_add(1);
                session.send_transfer(
                    self.remote_handle,
                    self.idx,
                    Some(body),
                    delivery_tx,
                    tag,
                    Some(false),
                );
            }
            let _ = self.idx.saturating_add(1);
            Delivery::Pending(delivery_rx)
        }
    }

    pub fn settle_message(&mut self, id: DeliveryNumber, state: DeliveryState) {
        let _ = self.delivery_count.saturating_add(1);

        let disp = Disposition {
            role: Role::Sender,
            first: id,
            last: None,
            settled: true,
            state: Some(state),
            batchable: false,
        };
        self.session.inner.get_mut().post_frame(disp.into());
    }
}

pub struct SenderLinkBuilder {
    frame: Attach,
    session: Cell<SessionInner>,
}

impl SenderLinkBuilder {
    pub(crate) fn new(name: ByteString, address: ByteString, session: Cell<SessionInner>) -> Self {
        let target = Target {
            address: Some(address),
            durable: TerminusDurability::None,
            expiry_policy: TerminusExpiryPolicy::SessionEnd,
            timeout: 0,
            dynamic: false,
            dynamic_node_properties: None,
            capabilities: None,
        };
        let frame = Attach {
            name,
            handle: 0 as Handle,
            role: Role::Sender,
            snd_settle_mode: SenderSettleMode::Mixed,
            rcv_settle_mode: ReceiverSettleMode::First,
            source: None,
            target: Some(target),
            unsettled: None,
            incomplete_unsettled: false,
            initial_delivery_count: None,
            max_message_size: Some(65536 * 4),
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        };

        SenderLinkBuilder { frame, session }
    }

    pub fn max_message_size(mut self, size: u64) -> Self {
        self.frame.max_message_size = Some(size);
        self
    }

    pub fn with_frame<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut Attach),
    {
        f(&mut self.frame);
        self
    }

    pub async fn open(self) -> Result<SenderLink, AmqpTransportError> {
        self.session
            .get_mut()
            .open_sender_link(self.frame)
            .await
            .map_err(|_e| AmqpTransportError::Disconnected)
    }
}
