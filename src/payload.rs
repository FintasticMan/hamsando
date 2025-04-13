use serde_json::Value as JsonValue;

#[derive(Debug)]
pub(crate) struct PayloadBuilder {
    payload: serde_json::Map<String, JsonValue>,
}

impl PayloadBuilder {
    pub(crate) fn new(apikey: &str, secretapikey: &str) -> Self {
        let mut payload = serde_json::Map::new();
        payload["apikey"] = apikey.into();
        payload["secretapikey"] = secretapikey.into();
        Self { payload }
    }

    pub(crate) fn add<T: Into<JsonValue>>(mut self, key: &str, value: T) -> Self {
        self.payload[key] = value.into();
        self
    }

    pub(crate) fn add_if_some<T: Into<JsonValue>>(mut self, key: &str, value: Option<T>) -> Self {
        if let Some(value) = value {
            self.payload[key] = value.into();
        }
        self
    }

    pub(crate) fn build(self) -> JsonValue {
        JsonValue::Object(self.payload)
    }
}
