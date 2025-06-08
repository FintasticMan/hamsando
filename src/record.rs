//! Type-safe DNS record.

use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use serde::Deserialize;
use strum::IntoStaticStr;

use crate::{ContentCreationError, domain::Domain};

/// Possible types a DNS record can have.
#[derive(Debug, Deserialize, PartialEq, Eq, IntoStaticStr)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum Type {
    A,
    Mx,
    Cname,
    Alias,
    Txt,
    Ns,
    Aaaa,
    Srv,
    Tlsa,
    Caa,
    Https,
    Svcb,
}

impl Type {
    /// Gets the string representation of the type.
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

impl From<Content> for Type {
    fn from(value: Content) -> Self {
        match value {
            Content::A(_) => Type::A,
            Content::Mx(_) => Type::Mx,
            Content::Cname(_) => Type::Cname,
            Content::Alias(_) => Type::Alias,
            Content::Txt(_) => Type::Txt,
            Content::Ns(_) => Type::Ns,
            Content::Aaaa(_) => Type::Aaaa,
            Content::Srv(_) => Type::Srv,
            Content::Tlsa(_) => Type::Tlsa,
            Content::Caa(_) => Type::Caa,
            Content::Https(_) => Type::Https,
            Content::Svcb(_) => Type::Svcb,
        }
    }
}

impl From<&Content> for Type {
    fn from(value: &Content) -> Self {
        match value {
            Content::A(_) => Type::A,
            Content::Mx(_) => Type::Mx,
            Content::Cname(_) => Type::Cname,
            Content::Alias(_) => Type::Alias,
            Content::Txt(_) => Type::Txt,
            Content::Ns(_) => Type::Ns,
            Content::Aaaa(_) => Type::Aaaa,
            Content::Srv(_) => Type::Srv,
            Content::Tlsa(_) => Type::Tlsa,
            Content::Caa(_) => Type::Caa,
            Content::Https(_) => Type::Https,
            Content::Svcb(_) => Type::Svcb,
        }
    }
}

/// The content value of a DNS record with type-safe variants for each type.
///
/// Ensures that each DNS record type contains the appropriate value format.
///
/// # Examples
///
/// ```
/// use hamsando::record::Content;
/// use std::net::{IpAddr, Ipv4Addr};
///
/// let ip: IpAddr = "127.0.0.1".parse().unwrap();
/// let content: Content = ip.into();
///
/// assert_eq!(content, Content::A(Ipv4Addr::new(127, 0, 0, 1)));
/// ```
#[derive(Debug, PartialEq, Eq, IntoStaticStr)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Content {
    A(Ipv4Addr),
    Mx(String),
    Cname(String),
    Alias(String),
    Txt(String),
    Ns(String),
    Aaaa(Ipv6Addr),
    Srv(String),
    Tlsa(String),
    Caa(String),
    Https(String),
    Svcb(String),
}

impl Content {
    /// Gets the string representation of the type of the content.
    pub fn type_as_str(&self) -> &'static str {
        self.into()
    }

    /// Converts the value in the content to a string.
    pub fn value_to_string(&self) -> String {
        match self {
            Content::A(addr) => addr.to_string(),
            Content::Mx(value) => value.clone(),
            Content::Cname(value) => value.clone(),
            Content::Alias(value) => value.clone(),
            Content::Txt(value) => value.clone(),
            Content::Ns(value) => value.clone(),
            Content::Aaaa(addr) => addr.to_string(),
            Content::Srv(value) => value.clone(),
            Content::Tlsa(value) => value.clone(),
            Content::Caa(value) => value.clone(),
            Content::Https(value) => value.clone(),
            Content::Svcb(value) => value.clone(),
        }
    }

    /// Creates a `Content` from a [`Type`] and a string.
    pub fn from(type_: &Type, content: &str) -> Result<Content, ContentCreationError> {
        Ok(match type_ {
            Type::A => Content::A(content.parse()?),
            Type::Mx => Content::Mx(content.to_string()),
            Type::Cname => Content::Cname(content.to_string()),
            Type::Alias => Content::Alias(content.to_string()),
            Type::Txt => Content::Txt(content.to_string()),
            Type::Ns => Content::Ns(content.to_string()),
            Type::Aaaa => Content::Aaaa(content.parse()?),
            Type::Srv => Content::Srv(content.to_string()),
            Type::Tlsa => Content::Tlsa(content.to_string()),
            Type::Caa => Content::Caa(content.to_string()),
            Type::Https => Content::Https(content.to_string()),
            Type::Svcb => Content::Svcb(content.to_string()),
        })
    }
}

impl From<IpAddr> for Content {
    fn from(value: IpAddr) -> Self {
        match value {
            IpAddr::V4(addr) => Content::A(addr),
            IpAddr::V6(addr) => Content::Aaaa(addr),
        }
    }
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct ContentDeserializable {
            #[serde(rename = "type")]
            type_: Type,
            content: String,
        }

        ContentDeserializable::deserialize(deserializer)
            .and_then(|c| Content::from(&c.type_, &c.content).map_err(D::Error::custom))
    }
}

/// A DNS record.
#[derive(Debug, Deserialize)]
pub struct Record {
    #[serde(deserialize_with = "deserialize_string_or_t")]
    pub id: i64,
    pub name: Box<Domain>,
    #[serde(flatten)]
    pub content: Content,
    #[serde(deserialize_with = "deserialize_string_or_t")]
    pub ttl: i64,
    #[serde(deserialize_with = "deserialize_option_string_or_t")]
    pub prio: Option<i64>,
    pub notes: Option<String>,
}

/// Helper type for deserializing a string or any T to a T.
#[derive(Deserialize)]
#[serde(untagged)]
enum StringOr<T> {
    String(String),
    T(T),
}

pub(crate) fn deserialize_string_or_t<'de, D, T: FromStr + Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    <T as FromStr>::Err: Error,
{
    use serde::de::Error;

    let string_or_t = StringOr::<T>::deserialize(deserializer)?;
    Ok(match string_or_t {
        StringOr::<T>::String(s) => s.parse().map_err(D::Error::custom)?,
        StringOr::<T>::T(t) => t,
    })
}

pub(crate) fn deserialize_option_string_or_t<'de, D, T: FromStr + Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    <T as FromStr>::Err: Error,
{
    use serde::de::Error;

    let string_or_i64 = Option::<StringOr<T>>::deserialize(deserializer)?;
    Ok(match string_or_i64 {
        Some(StringOr::<T>::String(s)) => Some(s.parse().map_err(D::Error::custom)?),
        Some(StringOr::<T>::T(t)) => Some(t),
        None => None,
    })
}
