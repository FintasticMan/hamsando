use reqwest::{blocking::Response, StatusCode};
use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("domain {0:?} has a prefix")]
    HasPrefix(String),
    #[error("domain {0:?} doesn't have a root")]
    MissingRoot(String),
}

#[derive(Error, Debug)]
#[error("Porkbun API error: {status} - {message}")]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub(crate) fn from_response(resp: Response) -> Self {
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

#[derive(Error, Debug)]
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

#[derive(Error, Debug)]
pub enum ClientBuilderError {
    #[error("missing field: {0}")]
    MissingField(String),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
}
