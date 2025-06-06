use std::net::AddrParseError;

use reqwest::{Response, StatusCode, blocking::Response as BlockingResponse};
use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Porkbun API error: {status} - {message}")]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    /// Converts the response from a Porkbun API request to an `ApiError`.
    pub(crate) async fn from_response(resp: Response) -> Self {
        #[derive(Deserialize)]
        struct ErrorResp {
            message: String,
        }

        let status = resp.status();
        let text = resp
            .text()
            .await
            .unwrap_or_else(|e| format!("unable to read response body: {e}"));

        let message = serde_json::from_str::<ErrorResp>(&text).map_or_else(
            |e| format!("unable to get error message from {text:?}: {e}"),
            |r| r.message,
        );

        Self { status, message }
    }

    /// Converts the response from a Porkbun API request to an `ApiError`.
    pub(crate) fn from_blocking_response(resp: BlockingResponse) -> Self {
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

#[derive(Error, Debug)]
pub enum ContentCreationError {
    #[error(transparent)]
    AddrParse(#[from] AddrParseError),
}
