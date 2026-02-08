use std::collections::HashMap;

use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::{Map, Value};

use super::interface::SessionState;

const SESSION_STATE_FORMAT_VERSION: u8 = 1;

#[derive(Debug)]
struct StoredSessionStateRef<'a> {
    state: &'a SessionState,
}

impl Serialize for StoredSessionStateRef<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("v", &SESSION_STATE_FORMAT_VERSION)?;
        map.serialize_entry("state", self.state)?;
        map.end()
    }
}

pub(crate) fn serialize_session_state(
    session_state: &SessionState,
) -> Result<String, anyhow::Error> {
    let stored = StoredSessionStateRef {
        state: session_state,
    };

    serde_json::to_string(&stored).map_err(anyhow::Error::new)
}

pub(crate) fn deserialize_session_state(value: &str) -> Result<SessionState, anyhow::Error> {
    let value = serde_json::from_str::<Value>(value)?;

    let Value::Object(mut obj) = value else {
        anyhow::bail!("Session state is not a JSON object");
    };

    // Preferred, versioned format (introduced to support future format changes and unambiguous
    // migrations).
    if matches!(obj.get("state"), Some(Value::Object(_))) {
        if let Some(Value::Number(v)) = obj.get("v") {
            let v = v
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Invalid session state format version"))?;
            let v = u8::try_from(v)
                .map_err(|_| anyhow::anyhow!("Invalid session state format version"))?;
            anyhow::ensure!(
                v == SESSION_STATE_FORMAT_VERSION,
                "Unsupported session state format version: {}",
                v
            );

            let Some(Value::Object(state)) = obj.remove("state") else {
                unreachable!("`state` was checked to be an object above");
            };
            return Ok(state);
        }
    }

    // Legacy format (<= actix-session@0.11): the state was persisted as a JSON object where each
    // value is a string containing the JSON representation of the actual value.
    if obj.values().all(Value::is_string) {
        let legacy: HashMap<String, String> = serde_json::from_value(Value::Object(obj))?;
        let mut migrated: Map<String, Value> = Map::new();
        for (key, json_encoded) in legacy {
            migrated.insert(key, serde_json::from_str::<Value>(&json_encoded)?);
        }

        return Ok(migrated);
    }

    // Unversioned modern format; kept as a fallback for robustness.
    Ok(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_versioned_format() {
        let mut state = Map::new();
        state.insert("s".into(), Value::from("hello"));
        state.insert("n".into(), Value::from(42));
        state.insert("b".into(), Value::from(true));
        state.insert("null".into(), Value::Null);
        state.insert("obj".into(), serde_json::json!({"a": 1}));
        state.insert("arr".into(), serde_json::json!([1, 2, 3]));

        let encoded = serialize_session_state(&state).unwrap();
        let decoded = deserialize_session_state(&encoded).unwrap();

        assert_eq!(decoded, state);

        // Ensure strings are not double-serialized in the stored payload.
        let stored: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(stored["state"]["s"], Value::from("hello"));
    }

    #[test]
    fn legacy_format_is_migrated() {
        // This matches the old persisted format:
        // - outer JSON is a map of strings
        // - each string contains JSON of the actual value
        let legacy = serde_json::json!({
            "string": "\"hello\"",
            "num": "1",
            "bool": "true",
            "obj": "{\"a\":1}",
            "arr": "[1,2,3]"
        })
        .to_string();

        let decoded = deserialize_session_state(&legacy).unwrap();

        assert_eq!(decoded.get("string"), Some(&Value::from("hello")));
        assert_eq!(decoded.get("num"), Some(&Value::from(1)));
        assert_eq!(decoded.get("bool"), Some(&Value::from(true)));
        assert_eq!(decoded.get("obj"), Some(&serde_json::json!({"a": 1})));
        assert_eq!(decoded.get("arr"), Some(&serde_json::json!([1, 2, 3])));
    }
}
