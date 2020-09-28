use std::convert::TryFrom;
use std::num::NonZeroU16;

use actix_router::Path;
use bytes::Bytes;
use bytestring::ByteString;
use mqtt_codec as mqtt;
use serde::de::DeserializeOwned;
use serde_json::Error as JsonError;

use crate::dispatcher::MqttState;
use crate::sink::MqttSink;

/// Publish message
pub struct Publish<S> {
    publish: mqtt::Publish,
    sink: MqttSink,
    state: MqttState<S>,
    topic: Path<ByteString>,
    query: Option<ByteString>,
}

impl<S> Publish<S> {
    pub(crate) fn new(state: MqttState<S>, publish: mqtt::Publish) -> Self {
        let (topic, query) = if let Some(pos) = publish.topic.find('?') {
            (
                ByteString::try_from(publish.topic.get_ref().slice(0..pos)).unwrap(),
                Some(
                    ByteString::try_from(
                        publish.topic.get_ref().slice(pos + 1..publish.topic.len()),
                    )
                    .unwrap(),
                ),
            )
        } else {
            (publish.topic.clone(), None)
        };
        let topic = Path::new(topic);
        let sink = state.sink().clone();
        Self {
            sink,
            publish,
            state,
            topic,
            query,
        }
    }

    #[inline]
    /// this might be re-delivery of an earlier attempt to send the Packet.
    pub fn dup(&self) -> bool {
        self.publish.dup
    }

    #[inline]
    pub fn retain(&self) -> bool {
        self.publish.retain
    }

    #[inline]
    /// the level of assurance for delivery of an Application Message.
    pub fn qos(&self) -> mqtt::QoS {
        self.publish.qos
    }

    #[inline]
    /// the information channel to which payload data is published.
    pub fn publish_topic(&self) -> &str {
        &self.publish.topic
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
    /// only present in PUBLISH Packets where the QoS level is 1 or 2.
    pub fn id(&self) -> Option<NonZeroU16> {
        self.publish.packet_id
    }

    #[inline]
    pub fn topic(&self) -> &Path<ByteString> {
        &self.topic
    }

    #[inline]
    pub fn topic_mut(&mut self) -> &mut Path<ByteString> {
        &mut self.topic
    }

    #[inline]
    pub fn query(&self) -> &str {
        self.query.as_ref().map(|s| s.as_ref()).unwrap_or("")
    }

    #[inline]
    pub fn packet(&self) -> &mqtt::Publish {
        &self.publish
    }

    #[inline]
    /// the Application Message that is being published.
    pub fn payload(&self) -> &Bytes {
        &self.publish.payload
    }

    /// Extract Bytes from packet payload
    pub fn take_payload(&self) -> Bytes {
        self.publish.payload.clone()
    }

    #[inline]
    /// Mqtt client sink object
    pub fn sink(&self) -> &MqttSink {
        &self.sink
    }

    /// Loads and parse `application/json` encoded body.
    pub fn json<T: DeserializeOwned>(&mut self) -> Result<T, JsonError> {
        serde_json::from_slice(&self.publish.payload)
    }
}

impl<S> std::fmt::Debug for Publish<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.publish.fmt(f)
    }
}
