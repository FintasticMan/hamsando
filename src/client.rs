use std::net::IpAddr;

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use url::Url;

use crate::domain::{Domain, Root};
use crate::record::{self, Content, Record, Type};
use crate::{ApiError, ClientBuilderError, ClientError, Payload};

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
    pub fn endpoint(mut self, endpoint: Url) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    /// In the case that `endpoint` is the Some variant, sets the API endpoint to it.
    ///
    /// The endpoint should have a trailing slash, as per [Url]'s semantics.
    pub fn endpoint_if_some(mut self, endpoint: Option<Url>) -> Self {
        if let Some(endpoint) = endpoint {
            self.endpoint = Some(endpoint);
        }
        self
    }

    /// Sets the API key to the one given.
    pub fn apikey(mut self, apikey: String) -> Self {
        self.apikey = Some(apikey);
        self
    }

    /// Sets the secret API key to the one given.
    pub fn secretapikey(mut self, secretapikey: String) -> Self {
        self.secretapikey = Some(secretapikey);
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

        Ok(Client::new(endpoint, apikey, secretapikey))
    }
}

/// API client.
pub struct Client {
    endpoint: Url,
    apikey: String,
    secretapikey: String,
    client: reqwest::Client,
}

impl Client {
    /// Creates a new Client.
    pub fn new(endpoint: Url, apikey: String, secretapikey: String) -> Self {
        Self {
            endpoint,
            apikey,
            secretapikey,
            client: reqwest::Client::new(),
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
    async fn send_request<T: for<'de> Deserialize<'de>>(
        &self,
        url: Url,
        payload: Payload,
    ) -> Result<T, ClientError> {
        let resp = self
            .client
            .post(url)
            .json(&JsonValue::from(payload))
            .send()
            .await?;
        if resp.status() != StatusCode::OK {
            return Err(ClientError::Porkbun(ApiError::from_response(resp).await));
        }
        Ok(resp.json().await?)
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
    pub async fn test_auth(&self) -> Result<IpAddr, ClientError> {
        let url = self.build_url(&["ping"])?;

        let payload = self.payload();

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            your_ip: IpAddr,
        }

        Ok(self.send_request::<Response>(url, payload).await?.your_ip)
    }

    pub async fn create_dns(
        &self,
        domain: &Domain,
        content: &Content,
        ttl: Option<i64>,
        prio: Option<i64>,
    ) -> Result<i64, ClientError> {
        let url = self.build_url(&["dns", "create", domain.root()])?;

        let payload = self
            .payload()
            .add("type", content.type_as_str())
            .add("content", content.value_to_string())
            .add_if_some("name", domain.prefix())
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio);

        #[derive(Deserialize)]
        struct Response {
            #[serde(deserialize_with = "record::deserialize_string_or_t")]
            id: i64,
        }

        Ok(self.send_request::<Response>(url, payload).await?.id)
    }

    pub async fn edit_dns(
        &self,
        domain: &Domain,
        id: i64,
        content: &Content,
        ttl: Option<i64>,
        prio: Option<i64>,
    ) -> Result<(), ClientError> {
        let url = self.build_url(&["dns", "edit", domain.root(), &id.to_string()])?;

        let payload = self
            .payload()
            .add("type", content.type_as_str())
            .add("content", content.value_to_string())
            .add_if_some("name", domain.prefix())
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio);

        self.send_request(url, payload).await
    }

    pub async fn edit_dns_by_name_type(
        &self,
        domain: &Domain,
        content: &Content,
        ttl: Option<i64>,
        prio: Option<i64>,
    ) -> Result<(), ClientError> {
        let url = self.build_url(&[
            "dns",
            "editByNameType",
            domain.root(),
            content.type_as_str(),
            domain.prefix().unwrap_or(""),
        ])?;

        let payload = self
            .payload()
            .add("content", content.value_to_string())
            .add_if_some("ttl", ttl)
            .add_if_some("prio", prio);

        self.send_request(url, payload).await
    }

    /// Deletes the DNS entry specified by the root of the domain name to be deleted, and its ID.
    pub async fn delete_dns(&self, root: &Root, id: i64) -> Result<(), ClientError> {
        let url = self.build_url(&["dns", "delete", root, &id.to_string()])?;

        let payload = self.payload();

        self.send_request(url, payload).await
    }

    pub async fn delete_dns_by_name_type(
        &self,
        domain: &Domain,
        type_: &Type,
    ) -> Result<(), ClientError> {
        let url = self.build_url(&[
            "dns",
            "deleteByNameType",
            domain.root(),
            type_.as_str(),
            domain.prefix().unwrap_or(""),
        ])?;

        let payload = self.payload();

        self.send_request(url, payload).await
    }

    /// Retrieves the DNS entry specified by the root of the domain name, and its ID.
    pub async fn retrieve_dns(
        &self,
        root: &Root,
        id: Option<i64>,
    ) -> Result<Vec<Record>, ClientError> {
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

        Ok(self.send_request::<Response>(url, payload).await?.records)
    }

    pub async fn retrieve_dns_by_name_type(
        &self,
        domain: &Domain,
        type_: &Type,
    ) -> Result<Vec<Record>, ClientError> {
        let url = self.build_url(&[
            "dns",
            "retrieveByNameType",
            domain.root(),
            type_.as_str(),
            domain.prefix().unwrap_or(""),
        ])?;

        let payload = self.payload();

        #[derive(Deserialize)]
        struct Response {
            records: Vec<Record>,
        }

        Ok(self.send_request::<Response>(url, payload).await?.records)
    }
}
