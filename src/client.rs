use std::net::IpAddr;

use addr::domain;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use url::Url;

use crate::record::{self, Content, Record, Type};
use crate::{ApiError, ClientBuilderError, ClientError, DomainError, Payload};

/// Splits the given domain name into the root and the prefix, if there is one.
///
/// # Errors
/// - `MissingRoot` if the given domain name has no root
fn split_domain<'a>(name: &'a domain::Name) -> Result<(Option<&'a str>, &'a str), DomainError> {
    let root = name
        .root()
        .ok_or_else(|| DomainError::MissingRoot(name.to_string()))?;
    let prefix = name.prefix();

    Ok((prefix, root))
}

/// Builder for a [Client] that handles default values.
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

    /// Sets the API endpoint to the one given.
    ///
    /// The endpoint should have a trailing slash, as per [Url]'s semantics.
    pub fn endpoint(mut self, endpoint: &Url) -> Self {
        self.endpoint = Some(endpoint.clone());
        self
    }

    /// In the case that `endpoint` is the Some variant, sets the API endpoint to it.
    ///
    /// The endpoint should have a trailing slash, as per [Url]'s semantics.
    pub fn endpoint_if_some(mut self, endpoint: Option<&Url>) -> Self {
        if let Some(endpoint) = endpoint {
            self.endpoint = Some(endpoint.clone());
        }
        self
    }

    /// Sets the API key to the one given.
    pub fn apikey(mut self, apikey: &str) -> Self {
        self.apikey = Some(apikey.to_string());
        self
    }

    /// Sets the secret API key to the one given.
    pub fn secretapikey(mut self, secretapikey: &str) -> Self {
        self.secretapikey = Some(secretapikey.to_string());
        self
    }

    /// Builds a [Client] from the builder.
    ///
    /// In the case that no API endpoint is set, the default endpoint of
    /// `https://api.porkbun.com/api/json/v3/` is used.
    ///
    /// # Errors
    /// - `MissingField` if a required field isn't added to the builder.
    /// - `UrlParse` if the default API endpoint fails to parse. This shouldn't happen.
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

        Ok(Client::new(&endpoint, &apikey, &secretapikey))
    }
}

/// API client.
pub struct Client {
    endpoint: Url,
    apikey: String,
    secretapikey: String,
    client: reqwest::blocking::Client,
}

impl Client {
    /// Creates a new Client.
    pub fn new(endpoint: &Url, apikey: &str, secretapikey: &str) -> Self {
        Self {
            endpoint: endpoint.clone(),
            apikey: apikey.to_string(),
            secretapikey: secretapikey.to_string(),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Returns a builder for a Client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Creates a [Url] from the endpoint and the path sections.
    fn build_url(&self, path: &[&str]) -> Result<Url, url::ParseError> {
        path.iter()
            .filter(|p| !p.is_empty())
            .try_fold(self.endpoint.clone(), |acc, p| acc.join(&format!("{p}/")))
    }

    /// Sends a POST request to the given url with the given payload.
    fn send_request<T: for<'de> Deserialize<'de>>(
        &self,
        url: Url,
        payload: Payload,
    ) -> Result<T, ClientError> {
        let resp = self
            .client
            .post(url)
            .json(&JsonValue::from(payload))
            .send()?;
        if resp.status() != StatusCode::OK {
            return Err(ClientError::Porkbun(ApiError::from_response(resp)));
        }
        Ok(resp.json()?)
    }

    /// Returns a payload for sending to the Porkbun API.
    ///
    /// This payload already includes the data necessary for authorization.
    fn payload(&self) -> Payload {
        Payload::new(&self.apikey, &self.secretapikey)
    }

    /// Calls the endpoint that tests if the authorization is correct.
    ///
    /// Also returns the caller's public IP address.
    pub fn test_auth(&self) -> Result<IpAddr, ClientError> {
        let url = self.build_url(&["ping"])?;

        let payload = self.payload();

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            your_ip: IpAddr,
        }

        Ok(self.send_request::<Response>(url, payload)?.your_ip)
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
            .payload()
            .add("type", content.type_as_str())
            .add("content", content.value_to_string())
            .add_if_some("name", prefix)
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio);

        #[derive(Deserialize)]
        struct Response {
            #[serde(deserialize_with = "record::deserialize_to_i64")]
            id: i64,
        }

        Ok(self.send_request::<Response>(url, payload)?.id)
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
            .payload()
            .add("type", content.type_as_str())
            .add("content", content.value_to_string())
            .add_if_some("name", prefix)
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio);

        self.send_request(url, payload)
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
            .payload()
            .add("content", content.value_to_string())
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio);

        self.send_request(url, payload)
    }

    /// Deletes the DNS entry specified by the root of the domain name to be deleted, and its ID.
    ///
    /// # Errors
    ///
    /// Will return a `Domain` error in the case of the `domain` having a prefix.
    pub fn delete_dns(&self, domain: &domain::Name, id: i64) -> Result<(), ClientError> {
        let (prefix, root) = split_domain(domain)?;
        if prefix.is_some() {
            return Err(ClientError::Domain(DomainError::HasPrefix(
                domain.to_string(),
            )));
        }

        let url = self.build_url(&["dns", "delete", root, &id.to_string()])?;

        let payload = self.payload();

        self.send_request(url, payload)
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

        let payload = self.payload();

        self.send_request(url, payload)
    }

    /// Retrieves the DNS entry specified by the root of the domain name, and its ID.
    ///
    /// # Errors
    ///
    /// Will return a `Domain` error in the case of the `domain` having a prefix.
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

        let payload = self.payload();

        #[derive(Deserialize)]
        struct Response {
            records: Vec<Record>,
        }

        Ok(self.send_request::<Response>(url, payload)?.records)
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

        let payload = self.payload();

        #[derive(Deserialize)]
        struct Response {
            records: Vec<Record>,
        }

        Ok(self.send_request::<Response>(url, payload)?.records)
    }
}
