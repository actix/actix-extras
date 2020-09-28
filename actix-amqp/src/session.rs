use std::collections::VecDeque;
use std::future::Future;

use actix_utils::oneshot;
use bytes::{BufMut, Bytes, BytesMut};
use bytestring::ByteString;
use either::Either;
use futures::future::ok;
use fxhash::FxHashMap;
use slab::Slab;

use amqp_codec::protocol::{
    Accepted, Attach, DeliveryNumber, DeliveryState, Detach, Disposition, Error, Flow, Frame,
    Handle, ReceiverSettleMode, Role, SenderSettleMode, Transfer, TransferBody, TransferNumber,
};
use amqp_codec::AmqpFrame;

use crate::cell::Cell;
use crate::connection::ConnectionController;
use crate::errors::AmqpTransportError;
use crate::rcvlink::{ReceiverLink, ReceiverLinkBuilder, ReceiverLinkInner};
use crate::sndlink::{SenderLink, SenderLinkBuilder, SenderLinkInner};
use crate::{Configuration, DeliveryPromise};

const INITIAL_OUTGOING_ID: TransferNumber = 0;

#[derive(Clone)]
pub struct Session {
    pub(crate) inner: Cell<SessionInner>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.inner.get_mut().drop_session()
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Session").finish()
    }
}

impl Session {
    pub(crate) fn new(inner: Cell<SessionInner>) -> Session {
        Session { inner }
    }

    #[inline]
    /// Get remote connection configuration
    pub fn remote_config(&self) -> &Configuration {
        self.inner.connection.remote_config()
    }

    pub fn close(&self) -> impl Future<Output = Result<(), AmqpTransportError>> {
        ok(())
    }

    pub fn get_sender_link(&self, name: &str) -> Option<&SenderLink> {
        let inner = self.inner.get_ref();

        if let Some(id) = inner.links_by_name.get(name) {
            if let Some(Either::Left(SenderLinkState::Established(ref link))) = inner.links.get(*id)
            {
                return Some(link);
            }
        }
        None
    }

    pub fn get_sender_link_by_handle(&self, hnd: Handle) -> Option<&SenderLink> {
        self.inner.get_ref().get_sender_link_by_handle(hnd)
    }

    pub fn get_receiver_link_by_handle(&self, hnd: Handle) -> Option<&ReceiverLink> {
        self.inner.get_ref().get_receiver_link_by_handle(hnd)
    }

    /// Open sender link
    pub fn build_sender_link<T: Into<String>, U: Into<String>>(
        &mut self,
        name: U,
        address: T,
    ) -> SenderLinkBuilder {
        let name = ByteString::from(name.into());
        let address = ByteString::from(address.into());
        SenderLinkBuilder::new(name, address, self.inner.clone())
    }

    /// Open receiver link
    pub fn build_receiver_link<T: Into<String>, U: Into<String>>(
        &mut self,
        name: U,
        address: T,
    ) -> ReceiverLinkBuilder {
        let name = ByteString::from(name.into());
        let address = ByteString::from(address.into());
        ReceiverLinkBuilder::new(name, address, self.inner.clone())
    }

    /// Detach receiver link
    pub fn detach_receiver_link(
        &mut self,
        handle: Handle,
        error: Option<Error>,
    ) -> impl Future<Output = Result<(), AmqpTransportError>> {
        let (tx, rx) = oneshot::channel();

        self.inner
            .get_mut()
            .detach_receiver_link(handle, false, error, tx);

        async move {
            match rx.await {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(AmqpTransportError::Disconnected),
            }
        }
    }

    pub fn wait_disposition(
        &mut self,
        id: DeliveryNumber,
    ) -> impl Future<Output = Result<Disposition, AmqpTransportError>> {
        self.inner.get_mut().wait_disposition(id)
    }
}

#[derive(Debug)]
enum SenderLinkState {
    Opening(oneshot::Sender<SenderLink>),
    Established(SenderLink),
    Closing(Option<oneshot::Sender<Result<(), AmqpTransportError>>>),
}

#[derive(Debug)]
enum ReceiverLinkState {
    Opening(Option<Cell<ReceiverLinkInner>>),
    OpeningLocal(
        Option<(
            Cell<ReceiverLinkInner>,
            oneshot::Sender<Result<ReceiverLink, AmqpTransportError>>,
        )>,
    ),
    Established(ReceiverLink),
    Closing(Option<oneshot::Sender<Result<(), AmqpTransportError>>>),
}

impl SenderLinkState {
    fn is_opening(&self) -> bool {
        match self {
            SenderLinkState::Opening(_) => true,
            _ => false,
        }
    }
}

impl ReceiverLinkState {
    fn is_opening(&self) -> bool {
        match self {
            ReceiverLinkState::OpeningLocal(_) => true,
            _ => false,
        }
    }
}

pub(crate) struct SessionInner {
    id: usize,
    connection: ConnectionController,
    next_outgoing_id: TransferNumber,
    local: bool,

    remote_channel_id: u16,
    next_incoming_id: TransferNumber,
    remote_outgoing_window: u32,
    remote_incoming_window: u32,

    unsettled_deliveries: FxHashMap<DeliveryNumber, DeliveryPromise>,

    links: Slab<Either<SenderLinkState, ReceiverLinkState>>,
    links_by_name: FxHashMap<ByteString, usize>,
    remote_handles: FxHashMap<Handle, usize>,
    pending_transfers: VecDeque<PendingTransfer>,
    disposition_subscribers: FxHashMap<DeliveryNumber, oneshot::Sender<Disposition>>,
    error: Option<AmqpTransportError>,
}

struct PendingTransfer {
    link_handle: Handle,
    idx: u32,
    body: Option<TransferBody>,
    promise: DeliveryPromise,
    tag: Option<Bytes>,
    settled: Option<bool>,
}

impl SessionInner {
    pub fn new(
        id: usize,
        local: bool,
        connection: ConnectionController,
        remote_channel_id: u16,
        next_incoming_id: DeliveryNumber,
        remote_incoming_window: u32,
        remote_outgoing_window: u32,
    ) -> SessionInner {
        SessionInner {
            id,
            local,
            connection,
            next_incoming_id,
            remote_channel_id,
            remote_incoming_window,
            remote_outgoing_window,
            next_outgoing_id: INITIAL_OUTGOING_ID,
            unsettled_deliveries: FxHashMap::default(),
            links: Slab::new(),
            links_by_name: FxHashMap::default(),
            remote_handles: FxHashMap::default(),
            pending_transfers: VecDeque::new(),
            disposition_subscribers: FxHashMap::default(),
            error: None,
        }
    }

    /// Local channel id
    pub fn id(&self) -> u16 {
        self.id as u16
    }

    /// Set error. New operations will return error.
    pub(crate) fn set_error(&mut self, err: AmqpTransportError) {
        // drop pending transfers
        for tr in self.pending_transfers.drain(..) {
            let _ = tr.promise.send(Err(err.clone()));
        }

        // drop links
        self.links_by_name.clear();
        for (_, st) in self.links.iter_mut() {
            match st {
                Either::Left(SenderLinkState::Opening(_)) => (),
                Either::Left(SenderLinkState::Established(ref mut link)) => {
                    link.inner.get_mut().set_error(err.clone())
                }
                Either::Left(SenderLinkState::Closing(ref mut link)) => {
                    if let Some(tx) = link.take() {
                        let _ = tx.send(Err(err.clone()));
                    }
                }
                _ => (),
            }
        }
        self.links.clear();

        self.error = Some(err);
    }

    fn drop_session(&mut self) {
        self.connection.drop_session_copy(self.id);
    }

    fn wait_disposition(
        &mut self,
        id: DeliveryNumber,
    ) -> impl Future<Output = Result<Disposition, AmqpTransportError>> {
        let (tx, rx) = oneshot::channel();
        self.disposition_subscribers.insert(id, tx);
        async move { rx.await.map_err(|_| AmqpTransportError::Disconnected) }
    }

    /// Register remote sender link
    pub(crate) fn confirm_sender_link(&mut self, cell: Cell<SessionInner>, attach: &Attach) {
        trace!("Remote sender link opened: {:?}", attach.name());
        let entry = self.links.vacant_entry();
        let token = entry.key();
        let delivery_count = attach.initial_delivery_count.unwrap_or(0);

        let mut name = None;
        if let Some(ref source) = attach.source {
            if let Some(ref addr) = source.address {
                name = Some(addr.clone());
                self.links_by_name.insert(addr.clone(), token);
            }
        }

        self.remote_handles.insert(attach.handle(), token);
        let link = Cell::new(SenderLinkInner::new(
            token,
            name.unwrap_or_else(|| ByteString::default()),
            attach.handle(),
            delivery_count,
            cell,
        ));
        entry.insert(Either::Left(SenderLinkState::Established(SenderLink::new(
            link,
        ))));

        let attach = Attach {
            name: attach.name.clone(),
            handle: token as Handle,
            role: Role::Sender,
            snd_settle_mode: SenderSettleMode::Mixed,
            rcv_settle_mode: ReceiverSettleMode::First,
            source: attach.source.clone(),
            target: attach.target.clone(),
            unsettled: None,
            incomplete_unsettled: false,
            initial_delivery_count: Some(delivery_count),
            max_message_size: Some(65536),
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        };
        self.post_frame(attach.into());
    }

    /// Register receiver link
    pub(crate) fn open_receiver_link(
        &mut self,
        cell: Cell<SessionInner>,
        attach: Attach,
    ) -> ReceiverLink {
        let handle = attach.handle();
        let entry = self.links.vacant_entry();
        let token = entry.key();

        let inner = Cell::new(ReceiverLinkInner::new(cell, token as u32, attach));
        entry.insert(Either::Right(ReceiverLinkState::Opening(Some(
            inner.clone(),
        ))));
        self.remote_handles.insert(handle, token);
        ReceiverLink::new(inner)
    }

    pub(crate) fn open_local_receiver_link(
        &mut self,
        cell: Cell<SessionInner>,
        mut frame: Attach,
    ) -> oneshot::Receiver<Result<ReceiverLink, AmqpTransportError>> {
        let (tx, rx) = oneshot::channel();

        let entry = self.links.vacant_entry();
        let token = entry.key();

        let inner = Cell::new(ReceiverLinkInner::new(cell, token as u32, frame.clone()));
        entry.insert(Either::Right(ReceiverLinkState::OpeningLocal(Some((
            inner.clone(),
            tx,
        )))));

        frame.handle = token as Handle;

        self.links_by_name.insert(frame.name.clone(), token);
        self.post_frame(Frame::Attach(frame));
        rx
    }

    pub(crate) fn confirm_receiver_link(&mut self, token: Handle, attach: &Attach) {
        if let Some(Either::Right(link)) = self.links.get_mut(token as usize) {
            match link {
                ReceiverLinkState::Opening(l) => {
                    let attach = Attach {
                        name: attach.name.clone(),
                        handle: token as Handle,
                        role: Role::Receiver,
                        snd_settle_mode: SenderSettleMode::Mixed,
                        rcv_settle_mode: ReceiverSettleMode::First,
                        source: attach.source.clone(),
                        target: attach.target.clone(),
                        unsettled: None,
                        incomplete_unsettled: false,
                        initial_delivery_count: Some(0),
                        max_message_size: Some(65536),
                        offered_capabilities: None,
                        desired_capabilities: None,
                        properties: None,
                    };
                    *link = ReceiverLinkState::Established(ReceiverLink::new(l.take().unwrap()));
                    self.post_frame(attach.into());
                }
                _ => error!("Unexpected receiver link state"),
            }
        }
    }

    /// Close receiver link
    pub(crate) fn detach_receiver_link(
        &mut self,
        id: Handle,
        closed: bool,
        error: Option<Error>,
        tx: oneshot::Sender<Result<(), AmqpTransportError>>,
    ) {
        if let Some(Either::Right(link)) = self.links.get_mut(id as usize) {
            match link {
                ReceiverLinkState::Opening(inner) => {
                    let attach = Attach {
                        name: inner.as_ref().unwrap().get_ref().name().clone(),
                        handle: id as Handle,
                        role: Role::Sender,
                        snd_settle_mode: SenderSettleMode::Mixed,
                        rcv_settle_mode: ReceiverSettleMode::First,
                        source: None,
                        target: None,
                        unsettled: None,
                        incomplete_unsettled: false,
                        initial_delivery_count: None,
                        max_message_size: None,
                        offered_capabilities: None,
                        desired_capabilities: None,
                        properties: None,
                    };
                    let detach = Detach {
                        handle: id,
                        closed,
                        error,
                    };
                    *link = ReceiverLinkState::Closing(Some(tx));
                    self.post_frame(attach.into());
                    self.post_frame(detach.into());
                }
                ReceiverLinkState::Established(_) => {
                    let detach = Detach {
                        handle: id,
                        closed,
                        error,
                    };
                    *link = ReceiverLinkState::Closing(Some(tx));
                    self.post_frame(detach.into());
                }
                ReceiverLinkState::Closing(_) => {
                    let _ = tx.send(Ok(()));
                    error!("Unexpected receiver link state: closing - {}", id);
                }
                ReceiverLinkState::OpeningLocal(_inner) => unimplemented!(),
            }
        } else {
            let _ = tx.send(Ok(()));
            error!("Receiver link does not exist while detaching: {}", id);
        }
    }

    pub(crate) fn detach_sender_link(
        &mut self,
        id: usize,
        closed: bool,
        error: Option<Error>,
        tx: oneshot::Sender<Result<(), AmqpTransportError>>,
    ) {
        if let Some(Either::Left(link)) = self.links.get_mut(id) {
            match link {
                SenderLinkState::Opening(_) => {
                    let detach = Detach {
                        handle: id as u32,
                        closed,
                        error,
                    };
                    *link = SenderLinkState::Closing(Some(tx));
                    self.post_frame(detach.into());
                }
                SenderLinkState::Established(_) => {
                    let detach = Detach {
                        handle: id as u32,
                        closed,
                        error,
                    };
                    *link = SenderLinkState::Closing(Some(tx));
                    self.post_frame(detach.into());
                }
                SenderLinkState::Closing(_) => {
                    let _ = tx.send(Ok(()));
                    error!("Unexpected receiver link state: closing - {}", id);
                }
            }
        } else {
            let _ = tx.send(Ok(()));
            error!("Receiver link does not exist while detaching: {}", id);
        }
    }

    pub(crate) fn get_sender_link_by_handle(&self, hnd: Handle) -> Option<&SenderLink> {
        if let Some(id) = self.remote_handles.get(&hnd) {
            if let Some(Either::Left(SenderLinkState::Established(ref link))) = self.links.get(*id)
            {
                return Some(link);
            }
        }
        None
    }

    pub(crate) fn get_receiver_link_by_handle(&self, hnd: Handle) -> Option<&ReceiverLink> {
        if let Some(id) = self.remote_handles.get(&hnd) {
            if let Some(Either::Right(ReceiverLinkState::Established(ref link))) =
                self.links.get(*id)
            {
                return Some(link);
            }
        }
        None
    }

    pub fn handle_frame(&mut self, frame: Frame) {
        if self.error.is_none() {
            match frame {
                Frame::Flow(flow) => self.apply_flow(&flow),
                Frame::Disposition(disp) => {
                    if let Some(sender) = self.disposition_subscribers.remove(&disp.first) {
                        let _ = sender.send(disp);
                    } else {
                        self.settle_deliveries(disp);
                    }
                }
                Frame::Transfer(transfer) => {
                    let idx = if let Some(idx) = self.remote_handles.get(&transfer.handle()) {
                        *idx
                    } else {
                        error!("Transfer's link {:?} is unknown", transfer.handle());
                        return;
                    };

                    if let Some(link) = self.links.get_mut(idx) {
                        match link {
                            Either::Left(_) => error!("Got trasfer from sender link"),
                            Either::Right(link) => match link {
                                ReceiverLinkState::Opening(_) => {
                                    error!(
                                        "Got transfer for opening link: {} -> {}",
                                        transfer.handle(),
                                        idx
                                    );
                                }
                                ReceiverLinkState::OpeningLocal(_) => {
                                    error!(
                                        "Got transfer for opening link: {} -> {}",
                                        transfer.handle(),
                                        idx
                                    );
                                }
                                ReceiverLinkState::Established(link) => {
                                    // self.outgoing_window -= 1;
                                    let _ = self.next_incoming_id.wrapping_add(1);
                                    link.inner.get_mut().handle_transfer(transfer);
                                }
                                ReceiverLinkState::Closing(_) => (),
                            },
                        }
                    } else {
                        error!(
                            "Remote link handle mapped to non-existing link: {} -> {}",
                            transfer.handle(),
                            idx
                        );
                    }
                }
                Frame::Detach(detach) => {
                    self.handle_detach(&detach);
                }
                frame => error!("Unexpected frame: {:?}", frame),
            }
        }
    }

    /// Handle `Attach` frame. return false if attach frame is remote and can not be handled
    pub fn handle_attach(&mut self, attach: &Attach, cell: Cell<SessionInner>) -> bool {
        let name = attach.name();

        if let Some(index) = self.links_by_name.get(name) {
            match self.links.get_mut(*index) {
                Some(Either::Left(item)) => {
                    if item.is_opening() {
                        trace!(
                            "sender link opened: {:?} {} -> {}",
                            name,
                            index,
                            attach.handle()
                        );

                        self.remote_handles.insert(attach.handle(), *index);
                        let delivery_count = attach.initial_delivery_count.unwrap_or(0);
                        let link = Cell::new(SenderLinkInner::new(
                            *index,
                            name.clone(),
                            attach.handle(),
                            delivery_count,
                            cell,
                        ));
                        let local_sender = std::mem::replace(
                            item,
                            SenderLinkState::Established(SenderLink::new(link.clone())),
                        );

                        if let SenderLinkState::Opening(tx) = local_sender {
                            let _ = tx.send(SenderLink::new(link));
                        }
                    }
                }
                Some(Either::Right(item)) => {
                    if item.is_opening() {
                        trace!(
                            "receiver link opened: {:?} {} -> {}",
                            name,
                            index,
                            attach.handle()
                        );
                        if let ReceiverLinkState::OpeningLocal(opt_item) = item {
                            let (link, tx) = opt_item.take().unwrap();
                            self.remote_handles.insert(attach.handle(), *index);

                            *item = ReceiverLinkState::Established(ReceiverLink::new(link.clone()));
                            let _ = tx.send(Ok(ReceiverLink::new(link)));
                        }
                    }
                }
                _ => {
                    // TODO: error in proto, have to close connection
                }
            }
            true
        } else {
            // cannot handle remote attach
            false
        }
    }

    /// Handle `Detach` frame.
    pub fn handle_detach(&mut self, detach: &Detach) {
        // get local link instance
        let idx = if let Some(idx) = self.remote_handles.get(&detach.handle()) {
            *idx
        } else {
            // should not happen, error
            return;
        };

        let remove = if let Some(link) = self.links.get_mut(idx) {
            match link {
                Either::Left(link) => match link {
                    SenderLinkState::Opening(_) => true,
                    SenderLinkState::Established(link) => {
                        // detach from remote endpoint
                        let detach = Detach {
                            handle: link.inner.get_ref().id(),
                            closed: true,
                            error: detach.error.clone(),
                        };
                        let err = AmqpTransportError::LinkDetached(detach.error.clone());

                        // remove name
                        self.links_by_name.remove(link.inner.name());

                        // drop pending transfers
                        let mut idx = 0;
                        let handle = link.inner.get_ref().remote_handle();
                        while idx < self.pending_transfers.len() {
                            if self.pending_transfers[idx].link_handle == handle {
                                let tr = self.pending_transfers.remove(idx).unwrap();
                                let _ = tr.promise.send(Err(err.clone()));
                            } else {
                                idx += 1;
                            }
                        }

                        // detach snd link
                        link.inner.get_mut().detached(err);
                        self.connection
                            .post_frame(AmqpFrame::new(self.remote_channel_id, detach.into()));
                        true
                    }
                    SenderLinkState::Closing(_) => true,
                },
                Either::Right(link) => match link {
                    ReceiverLinkState::Opening(_) => false,
                    ReceiverLinkState::OpeningLocal(_) => false,
                    ReceiverLinkState::Established(link) => {
                        // detach from remote endpoint
                        let detach = Detach {
                            handle: link.handle(),
                            closed: true,
                            error: None,
                        };

                        // detach rcv link
                        self.connection
                            .post_frame(AmqpFrame::new(self.remote_channel_id, detach.into()));
                        true
                    }
                    ReceiverLinkState::Closing(tx) => {
                        // detach confirmation
                        if let Some(tx) = tx.take() {
                            if let Some(err) = detach.error.clone() {
                                let _ = tx.send(Err(AmqpTransportError::LinkDetached(Some(err))));
                            } else {
                                let _ = tx.send(Ok(()));
                            }
                        }
                        true
                    }
                },
            }
        } else {
            false
        };

        if remove {
            self.links.remove(idx);
            self.remote_handles.remove(&detach.handle());
        }
    }

    fn settle_deliveries(&mut self, disposition: Disposition) {
        trace!("settle delivery: {:#?}", disposition);

        let from = disposition.first;
        let to = disposition.last.unwrap_or(from);

        if from == to {
            let _ = self
                .unsettled_deliveries
                .remove(&from)
                .unwrap()
                .send(Ok(disposition));
        } else {
            for k in from..=to {
                let _ = self
                    .unsettled_deliveries
                    .remove(&k)
                    .unwrap()
                    .send(Ok(disposition.clone()));
            }
        }
    }

    pub(crate) fn apply_flow(&mut self, flow: &Flow) {
        // # AMQP1.0 2.5.6
        self.next_incoming_id = flow.next_outgoing_id();
        self.remote_outgoing_window = flow.outgoing_window();

        self.remote_incoming_window = flow
            .next_incoming_id()
            .unwrap_or(INITIAL_OUTGOING_ID)
            .saturating_add(flow.incoming_window())
            .saturating_sub(self.next_outgoing_id);

        trace!(
            "session received credit. window: {}, pending: {}",
            self.remote_outgoing_window,
            self.pending_transfers.len()
        );

        while let Some(t) = self.pending_transfers.pop_front() {
            self.send_transfer(t.link_handle, t.idx, t.body, t.promise, t.tag, t.settled);
            if self.remote_outgoing_window == 0 {
                break;
            }
        }

        // apply link flow
        if let Some(Either::Left(link)) = flow.handle().and_then(|h| self.links.get_mut(h as usize))
        {
            match link {
                SenderLinkState::Established(ref mut link) => {
                    link.inner.get_mut().apply_flow(&flow);
                }
                _ => warn!("Received flow frame"),
            }
        }
        if flow.echo() {
            self.send_flow();
        }
    }

    fn send_flow(&mut self) {
        let flow = Flow {
            next_incoming_id: if self.local {
                Some(self.next_incoming_id)
            } else {
                None
            },
            incoming_window: std::u32::MAX,
            next_outgoing_id: self.next_outgoing_id,
            outgoing_window: self.remote_incoming_window,
            handle: None,
            delivery_count: None,
            link_credit: None,
            available: None,
            drain: false,
            echo: false,
            properties: None,
        };
        self.post_frame(flow.into());
    }

    pub(crate) fn rcv_link_flow(&mut self, handle: u32, delivery_count: u32, credit: u32) {
        let flow = Flow {
            next_incoming_id: if self.local {
                Some(self.next_incoming_id)
            } else {
                None
            },
            incoming_window: std::u32::MAX,
            next_outgoing_id: self.next_outgoing_id,
            outgoing_window: self.remote_incoming_window,
            handle: Some(handle),
            delivery_count: Some(delivery_count),
            link_credit: Some(credit),
            available: None,
            drain: false,
            echo: false,
            properties: None,
        };
        self.post_frame(flow.into());
    }

    pub fn post_frame(&mut self, frame: Frame) {
        self.connection
            .post_frame(AmqpFrame::new(self.remote_channel_id, frame));
    }

    pub(crate) fn open_sender_link(&mut self, mut frame: Attach) -> oneshot::Receiver<SenderLink> {
        let (tx, rx) = oneshot::channel();

        let entry = self.links.vacant_entry();
        let token = entry.key();
        entry.insert(Either::Left(SenderLinkState::Opening(tx)));

        frame.handle = token as Handle;

        self.links_by_name.insert(frame.name.clone(), token);
        self.post_frame(Frame::Attach(frame));
        rx
    }

    pub fn send_transfer(
        &mut self,
        link_handle: Handle,
        idx: u32,
        body: Option<TransferBody>,
        promise: DeliveryPromise,
        tag: Option<Bytes>,
        settled: Option<bool>,
    ) {
        if self.remote_incoming_window == 0 {
            self.pending_transfers.push_back(PendingTransfer {
                link_handle,
                idx,
                body,
                promise,
                tag,
                settled,
            });
            return;
        }
        let frame = self.prepare_transfer(link_handle, body, promise, tag, settled);
        self.post_frame(frame);
    }

    pub fn prepare_transfer(
        &mut self,
        link_handle: Handle,
        body: Option<TransferBody>,
        promise: DeliveryPromise,
        delivery_tag: Option<Bytes>,
        settled: Option<bool>,
    ) -> Frame {
        let delivery_id = self.next_outgoing_id;

        let tag = if let Some(tag) = delivery_tag {
            tag
        } else {
            let mut buf = BytesMut::new();
            buf.put_u32(delivery_id);
            buf.freeze()
        };

        self.next_outgoing_id += 1;
        self.remote_incoming_window -= 1;

        let message_format = if let Some(ref body) = body {
            body.message_format()
        } else {
            None
        };

        let settled2 = settled.clone().unwrap_or(false);
        let state = if settled2 {
            Some(DeliveryState::Accepted(Accepted {}))
        } else {
            None
        };

        let transfer = Transfer {
            settled,
            message_format,
            handle: link_handle,
            delivery_id: Some(delivery_id),
            delivery_tag: Some(tag),
            more: false,
            rcv_settle_mode: None,
            state, //: Some(DeliveryState::Accepted(Accepted {})),
            resume: false,
            aborted: false,
            batchable: false,
            body: body,
        };
        self.unsettled_deliveries.insert(delivery_id, promise);

        Frame::Transfer(transfer)
    }
}
