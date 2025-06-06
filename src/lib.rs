//! # Simple and type-safe client for the Porkbun API.
//!
//! Implements an easy-to-use client for interfacing with the [Porkbun API].
//! Ensures that correct values are supplied using the Rust type system.
//!
//! ## Examples
//!
//! See [hamsando-ddns] for an implementation of a dynamic DNS program using this crate.
//!
//! ```
//! use hamsando::{domain::Domain, blocking::Client};
//!
//! let client = Client::builder()
//!     .apikey("<APIKEY>".to_string())
//!     .secretapikey("<SECRETAPIKEY>".to_string())
//!     .build()
//!     .unwrap();
//!
//! let my_ip = client.test_auth().unwrap();
//!
//! let domain: Box<Domain> = "example.com".parse().unwrap();
//! let record_id = client.create_dns(&domain, &my_ip.into(), None, None).unwrap();
//! ```
//!
//! [Porkbun API]: https://porkbun.com/api/json/v3/documentation
//! [hamsando-ddns]: https://github.com/FintasticMan/hamsando-ddns

pub mod blocking;
mod client;
pub mod domain;
mod errors;
mod payload;
pub mod record;

pub use client::*;
pub use errors::*;
pub(crate) use payload::*;
