use std::marker::PhantomData;

use bytestring::ByteString;
use mqtt_codec as mqtt;

use crate::dispatcher::MqttState;
use crate::sink::MqttSink;

/// Subscribe message
pub struct Subscribe<S> {
    topics: Vec<(ByteString, mqtt::QoS)>,
    codes: Vec<mqtt::SubscribeReturnCode>,
    state: MqttState<S>,
}

/// Result of a subscribe message
pub struct SubscribeResult {
    pub(crate) codes: Vec<mqtt::SubscribeReturnCode>,
}

impl<S> Subscribe<S> {
    pub(crate) fn new(state: MqttState<S>, topics: Vec<(ByteString, mqtt::QoS)>) -> Self {
        let mut codes = Vec::with_capacity(topics.len());
        (0..topics.len()).for_each(|_| codes.push(mqtt::SubscribeReturnCode::Failure));

        Self {
            topics,
            state,
            codes,
        }
    }

    #[inline]
    /// returns reference to a connection session
    pub fn session(&self) -> &S {
        self.state.session()
    }

    #[inline]
    /// returns mutable reference to a connection session
    pub fn session_mut(&mut self) -> &mut S {
        self.state.session_mut()
    }

    #[inline]
    /// Mqtt client sink object
    pub fn sink(&self) -> MqttSink {
        self.state.sink().clone()
    }

    #[inline]
    /// returns iterator over subscription topics
    pub fn iter_mut(&mut self) -> SubscribeIter<S> {
        SubscribeIter {
            subs: self as *const _ as *mut _,
            entry: 0,
            lt: PhantomData,
        }
    }

    #[inline]
    /// convert subscription to a result
    pub fn into_result(self) -> SubscribeResult {
        SubscribeResult { codes: self.codes }
    }
}

impl<'a, S> IntoIterator for &'a mut Subscribe<S> {
    type Item = Subscription<'a, S>;
    type IntoIter = SubscribeIter<'a, S>;

    fn into_iter(self) -> SubscribeIter<'a, S> {
        self.iter_mut()
    }
}

/// Iterator over subscription topics
pub struct SubscribeIter<'a, S> {
    subs: *mut Subscribe<S>,
    entry: usize,
    lt: PhantomData<&'a mut Subscribe<S>>,
}

impl<'a, S> SubscribeIter<'a, S> {
    fn next_unsafe(&mut self) -> Option<Subscription<'a, S>> {
        let subs = unsafe { &mut *self.subs };

        if self.entry < subs.topics.len() {
            let s = Subscription {
                topic: &subs.topics[self.entry].0,
                qos: subs.topics[self.entry].1,
                state: subs.state.clone(),
                code: &mut subs.codes[self.entry],
            };
            self.entry += 1;
            Some(s)
        } else {
            None
        }
    }
}

impl<'a, S> Iterator for SubscribeIter<'a, S> {
    type Item = Subscription<'a, S>;

    #[inline]
    fn next(&mut self) -> Option<Subscription<'a, S>> {
        self.next_unsafe()
    }
}

/// Subscription topic
pub struct Subscription<'a, S> {
    topic: &'a ByteString,
    state: MqttState<S>,
    qos: mqtt::QoS,
    code: &'a mut mqtt::SubscribeReturnCode,
}

impl<'a, S> Subscription<'a, S> {
    #[inline]
    /// reference to a connection session
    pub fn session(&self) -> &S {
        self.state.session()
    }

    #[inline]
    /// mutable reference to a connection session
    pub fn session_mut(&mut self) -> &mut S {
        self.state.session_mut()
    }

    #[inline]
    /// subscription topic
    pub fn topic(&self) -> &'a ByteString {
        &self.topic
    }

    #[inline]
    /// the level of assurance for delivery of an Application Message.
    pub fn qos(&self) -> mqtt::QoS {
        self.qos
    }

    #[inline]
    /// fail to subscribe to the topic
    pub fn fail(&mut self) {
        *self.code = mqtt::SubscribeReturnCode::Failure
    }

    #[inline]
    /// subscribe to a topic with specific qos
    pub fn subscribe(&mut self, qos: mqtt::QoS) {
        *self.code = mqtt::SubscribeReturnCode::Success(qos)
    }
}

/// Unsubscribe message
pub struct Unsubscribe<S> {
    state: MqttState<S>,
    topics: Vec<ByteString>,
}

impl<S> Unsubscribe<S> {
    pub(crate) fn new(state: MqttState<S>, topics: Vec<ByteString>) -> Self {
        Self { topics, state }
    }

    #[inline]
    /// reference to a connection session
    pub fn session(&self) -> &S {
        self.state.session()
    }

    #[inline]
    /// mutable reference to a connection session
    pub fn session_mut(&mut self) -> &mut S {
        self.state.session_mut()
    }

    #[inline]
    /// Mqtt client sink object
    pub fn sink(&self) -> MqttSink {
        self.state.sink().clone()
    }

    /// returns iterator over unsubscribe topics
    pub fn iter(&self) -> impl Iterator<Item = &ByteString> {
        self.topics.iter()
    }
}
