use serde_json::{Map as JsonMap, Value as JsonValue};

/// Payload to send to the Porkbun API.
#[derive(Debug)]
pub(crate) struct Payload {
    payload: JsonMap<String, JsonValue>,
}

impl Payload {
    /// Creates a new payload, with the given authorization details.
    pub(crate) fn new(apikey: &str, secretapikey: &str) -> Self {
        let mut payload = JsonMap::new();
        payload.insert("apikey".to_string(), apikey.into());
        payload.insert("secretapikey".to_string(), secretapikey.into());
        Self { payload }
    }

    /// Adds the given key-value pair.
    pub(crate) fn add<T: Into<JsonValue>>(mut self, key: &str, value: T) -> Self {
        self.payload.insert(key.to_string(), value.into());
        self
    }

    /// In the case that `value` is some, adds the key-value pair.
    pub(crate) fn add_if_some<T: Into<JsonValue>>(mut self, key: &str, value: Option<T>) -> Self {
        if let Some(value) = value {
            self.payload.insert(key.to_string(), value.into());
        }
        self
    }
}

impl From<Payload> for JsonValue {
    fn from(value: Payload) -> Self {
        JsonValue::Object(value.payload)
    }
}

impl From<Payload> for JsonMap<String, JsonValue> {
    fn from(value: Payload) -> Self {
        value.payload
    }
}
