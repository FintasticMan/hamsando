pub mod record;

use std::net::IpAddr;

use addr::domain;
use reqwest::{blocking::Response, StatusCode};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use thiserror::Error as ThisError;
use url::Url;

use record::{Content, Record, Type};

#[derive(ThisError, Debug)]
pub enum DomainError {
    #[error("domain {0:?} has a prefix")]
    HasPrefix(String),
    #[error("domain {0:?} doesn't have a root")]
    MissingRoot(String),
}

#[derive(ThisError, Debug)]
#[error("Porkbun API error: {status} - {message}")]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn from_response(resp: Response) -> Self {
        #[derive(Deserialize)]
        struct ErrorResp {
            message: String,
        }

        let status = resp.status();
        let text = resp
            .text()
            .unwrap_or_else(|e| format!("unable to read response body: {e}"));

        let message = serde_json::from_str::<ErrorResp>(&text).map_or_else(
            |e| format!("unable to get error message from {text:?}: {e}"),
            |r| r.message,
        );

        Self { status, message }
    }
}

#[derive(ThisError, Debug)]
pub enum ClientError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Porkbun(#[from] ApiError),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
}

#[derive(ThisError, Debug)]
pub enum ClientBuilderError {
    #[error("missing field: {0}")]
    MissingField(String),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
}

pub struct ClientBuilder {
    endpoint: Option<Url>,
    apikey: Option<String>,
    secretapikey: Option<String>,
}

impl ClientBuilder {
    fn new() -> Self {
        Self {
            endpoint: None,
            apikey: None,
            secretapikey: None,
        }
    }

    pub fn endpoint(mut self, endpoint: &Url) -> Self {
        self.endpoint = Some(endpoint.clone());
        self
    }

    pub fn endpoint_if_some(mut self, endpoint: Option<&Url>) -> Self {
        if let Some(endpoint) = endpoint {
            self.endpoint = Some(endpoint.clone());
        }
        self
    }
    pub fn apikey(mut self, apikey: &str) -> Self {
        self.apikey = Some(apikey.to_string());
        self
    }

    pub fn secretapikey(mut self, secretapikey: &str) -> Self {
        self.secretapikey = Some(secretapikey.to_string());
        self
    }

    pub fn build(self) -> Result<Client, ClientBuilderError> {
        let endpoint = match self.endpoint {
            Some(endpoint) => endpoint,
            None => "https://api.porkbun.com/api/json/v3/".parse()?,
        };
        let apikey = self
            .apikey
            .ok_or_else(|| ClientBuilderError::MissingField("apikey".to_string()))?;
        let secretapikey = self
            .secretapikey
            .ok_or_else(|| ClientBuilderError::MissingField("secretapikey".to_string()))?;

        Ok(Client {
            endpoint,
            apikey,
            secretapikey,
            client: reqwest::blocking::Client::new(),
        })
    }
}

#[derive(Debug)]
struct PayloadBuilder {
    payload: serde_json::Map<String, JsonValue>,
}

impl PayloadBuilder {
    fn new(apikey: &str, secretapikey: &str) -> Self {
        let mut payload = serde_json::Map::new();
        payload["apikey"] = apikey.into();
        payload["secretapikey"] = secretapikey.into();
        Self { payload }
    }

    fn add<T: Into<JsonValue>>(mut self, key: &str, value: T) -> Self {
        self.payload[key] = value.into();
        self
    }

    fn add_if_some<T: Into<JsonValue>>(mut self, key: &str, value: Option<T>) -> Self {
        if let Some(value) = value {
            self.payload[key] = value.into();
        }
        self
    }

    fn build(self) -> JsonValue {
        JsonValue::Object(self.payload)
    }
}

fn split_domain<'a>(name: &'a domain::Name) -> Result<(Option<&'a str>, &'a str), DomainError> {
    let root = name
        .root()
        .ok_or_else(|| DomainError::MissingRoot(name.to_string()))?;
    let prefix = name.prefix();

    Ok((prefix, root))
}

pub struct Client {
    endpoint: Url,
    apikey: String,
    secretapikey: String,
    client: reqwest::blocking::Client,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    fn build_url(&self, path: &[&str]) -> Result<Url, url::ParseError> {
        path.iter()
            .filter(|p| !p.is_empty())
            .try_fold(self.endpoint.clone(), |acc, p| acc.join(&format!("{p}/")))
    }

    fn send_request<T: for<'de> Deserialize<'de>>(
        &self,
        url: Url,
        payload: &JsonValue,
    ) -> Result<T, ClientError> {
        let resp = self.client.post(url).json(payload).send()?;
        if resp.status() != StatusCode::OK {
            return Err(ClientError::Porkbun(ApiError::from_response(resp)));
        }
        Ok(resp.json()?)
    }

    fn payload_builder(&self) -> PayloadBuilder {
        PayloadBuilder::new(&self.apikey, &self.secretapikey)
    }

    pub fn test_auth(&self) -> Result<IpAddr, ClientError> {
        let url = self.build_url(&["ping"])?;

        let payload = self.payload_builder().build();

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            your_ip: IpAddr,
        }

        Ok(self.send_request::<Response>(url, &payload)?.your_ip)
    }

    pub fn create_dns(
        &self,
        domain: &domain::Name,
        content: &Content,
        ttl: Option<i64>,
        prio: Option<i64>,
    ) -> Result<i64, ClientError> {
        let (prefix, root) = split_domain(domain)?;
        let url = self.build_url(&["dns", "create", root])?;

        let payload = self
            .payload_builder()
            .add("type", content.type_as_str())
            .add("content", content.value_to_string())
            .add_if_some("name", prefix)
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio)
            .build();

        #[derive(Deserialize)]
        struct Response {
            #[serde(deserialize_with = "record::deserialize_to_i64")]
            id: i64,
        }

        Ok(self.send_request::<Response>(url, &payload)?.id)
    }

    pub fn edit_dns(
        &self,
        domain: &domain::Name,
        id: i64,
        content: &Content,
        ttl: Option<i64>,
        prio: Option<i64>,
    ) -> Result<(), ClientError> {
        let (prefix, root) = split_domain(domain)?;
        let url = self.build_url(&["dns", "edit", root, &id.to_string()])?;

        let payload = self
            .payload_builder()
            .add("type", content.type_as_str())
            .add("content", content.value_to_string())
            .add_if_some("name", prefix)
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio)
            .build();

        self.send_request(url, &payload)
    }

    pub fn edit_dns_by_name_type(
        &self,
        domain: &domain::Name,
        content: &Content,
        ttl: Option<i64>,
        prio: Option<i64>,
    ) -> Result<(), ClientError> {
        let (prefix, root) = split_domain(domain)?;
        let url = self.build_url(&[
            "dns",
            "editByNameType",
            root,
            content.type_as_str(),
            prefix.unwrap_or(""),
        ])?;

        let payload = self
            .payload_builder()
            .add("content", content.value_to_string())
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio)
            .build();

        self.send_request(url, &payload)
    }

    pub fn delete_dns(&self, domain: &domain::Name, id: i64) -> Result<(), ClientError> {
        let (prefix, root) = split_domain(domain)?;
        if prefix.is_some() {
            return Err(ClientError::Domain(DomainError::HasPrefix(
                domain.to_string(),
            )));
        }

        let url = self.build_url(&["dns", "delete", root, &id.to_string()])?;

        let payload = self.payload_builder().build();

        self.send_request(url, &payload)
    }

    pub fn delete_dns_by_name_type(
        &self,
        domain: &domain::Name,
        type_: &Type,
    ) -> Result<(), ClientError> {
        let (prefix, root) = split_domain(domain)?;
        let url = self.build_url(&[
            "dns",
            "deleteByNameType",
            root,
            type_.as_str(),
            prefix.unwrap_or(""),
        ])?;

        let payload = self.payload_builder().build();

        self.send_request(url, &payload)
    }

    pub fn retrieve_dns(
        &self,
        domain: &domain::Name,
        id: Option<i64>,
    ) -> Result<Vec<Record>, ClientError> {
        let (prefix, root) = split_domain(domain)?;
        if prefix.is_some() {
            return Err(ClientError::Domain(DomainError::HasPrefix(
                domain.to_string(),
            )));
        }

        let url = self.build_url(&[
            "dns",
            "retrieve",
            root,
            &id.map_or_else(|| "".to_string(), |id| id.to_string()),
        ])?;

        let payload = self.payload_builder().build();

        #[derive(Deserialize)]
        struct Response {
            records: Vec<Record>,
        }

        Ok(self.send_request::<Response>(url, &payload)?.records)
    }

    pub fn retrieve_dns_by_name_type(
        &self,
        domain: &domain::Name,
        type_: &Type,
    ) -> Result<Vec<Record>, ClientError> {
        let (prefix, root) = split_domain(domain)?;
        let url = self.build_url(&[
            "dns",
            "retrieveByNameType",
            root,
            type_.as_str(),
            prefix.unwrap_or(""),
        ])?;

        let payload = self.payload_builder().build();

        #[derive(Deserialize)]
        struct Response {
            records: Vec<Record>,
        }

        Ok(self.send_request::<Response>(url, &payload)?.records)
    }
}
