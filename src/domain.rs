use core::str;
use std::{fmt::Display, ops::Deref, str::FromStr};

use psl::Psl;
use thiserror::Error as ThisError;

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

#[derive(Debug, ThisError)]
pub enum DomainParseError {
    #[error("domain is empty")]
    Empty,
    #[error("{0}: domain contains an empty label")]
    EmptyLabel(String),
    #[error("{0}: domain has a prefix")]
    HasPrefix(String),
    #[error("{0}: domain has no root")]
    MissingRoot(String),
    /// This case seems to be unreachable with this psl implementation.
    #[error("{0}: domain is missing a suffix")]
    MissingSuffix(String),
    #[error("{0}: domain is too long: {0}")]
    TooLong(String),
    #[error("{0}: domain contains a too-long label: {1}")]
    TooLongLabel(String, String),
    #[error("{0}: domain has an unknown suffix, {1}")]
    UnknownSuffix(String, String),
}

fn get_domain(s: &str) -> &str {
    if s.ends_with('.') {
        &s[..s.len() - 1]
    } else {
        s
    }
}

/// Returns the indices for the `.`s between the prefix and root, and before the suffix.
fn parse_domain(domain: &str) -> Result<(Option<usize>, usize), DomainParseError> {
    if domain.is_empty() {
        return Err(DomainParseError::Empty);
    }

    if domain.len() > MAX_DOMAIN_LEN {
        return Err(DomainParseError::TooLong(domain.to_string()));
    }

    for label in domain.split('.') {
        if label.is_empty() {
            return Err(DomainParseError::EmptyLabel(domain.to_string()));
        } else if label.len() > MAX_LABEL_LEN {
            return Err(DomainParseError::TooLongLabel(
                domain.to_string(),
                label.to_string(),
            ));
        }
    }

    let suffix = match psl::List.suffix(domain.as_bytes()) {
        Some(suffix) => suffix,
        None => return Err(DomainParseError::MissingSuffix(domain.to_string())),
    };
    if !suffix.is_known() {
        return Err(DomainParseError::UnknownSuffix(
            domain.to_string(),
            str::from_utf8(suffix.as_bytes()).unwrap().to_string(),
        ));
    }

    let suffix_len = str::from_utf8(suffix.as_bytes()).unwrap().len();
    if domain.len() == suffix_len {
        return Err(DomainParseError::MissingRoot(domain.to_string()));
    }
    let suffix_separator_index = domain.len() - suffix_len - 1;
    let without_suffix = &domain[..suffix_separator_index];

    Ok((without_suffix.rfind('.'), suffix_separator_index))
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct Root<'a> {
    root: &'a str,
    suffix_separator_index: usize,
}

impl<'a> Root<'a> {
    pub fn parse(input: &'a str) -> Result<Self, DomainParseError> {
        let domain = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(domain)?;
        if root_separator_index.is_some() {
            return Err(DomainParseError::HasPrefix(domain.to_string()));
        }

        Ok(Self {
            root: domain,
            suffix_separator_index,
        })
    }

    pub fn as_str(&self) -> &str {
        self.root
    }

    pub fn suffix(&self) -> &str {
        &self.root[self.suffix_separator_index + 1..]
    }
}

impl<'a> TryFrom<&'a str> for Root<'a> {
    type Error = DomainParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<'a> Display for Root<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root)
    }
}

impl<'a> AsRef<str> for Root<'a> {
    fn as_ref(&self) -> &str {
        self.root
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct OwnedRoot {
    root: String,
    suffix_separator_index: usize,
}

impl OwnedRoot {
    pub fn parse(input: &str) -> Result<Self, DomainParseError> {
        let domain = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(domain)?;
        if root_separator_index.is_some() {
            return Err(DomainParseError::HasPrefix(domain.to_string()));
        }

        Ok(Self {
            root: domain.to_string(),
            suffix_separator_index,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.root
    }

    pub fn suffix(&self) -> &str {
        &self.root[self.suffix_separator_index + 1..]
    }
}

impl FromStr for OwnedRoot {
    type Err = DomainParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for OwnedRoot {
    type Error = DomainParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl Display for OwnedRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root)
    }
}

impl AsRef<str> for OwnedRoot {
    fn as_ref(&self) -> &str {
        &self.root
    }
}

impl Deref for OwnedRoot {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct Domain<'a> {
    domain: &'a str,
    root_separator_index: Option<usize>,
    suffix_separator_index: usize,
}

impl<'a> Domain<'a> {
    pub fn parse(input: &'a str) -> Result<Self, DomainParseError> {
        let domain = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(domain)?;

        Ok(Self {
            domain,
            root_separator_index,
            suffix_separator_index,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.domain
    }

    pub fn prefix(&self) -> Option<&str> {
        self.root_separator_index.map(|i| &self.domain[..i])
    }

    pub fn root(&self) -> Root {
        let offset = self.root_separator_index.map(|i| i + 1).unwrap_or(0);
        let root = &self.domain[offset..];
        let suffix_separator_index = self.suffix_separator_index - offset;
        Root {
            root,
            suffix_separator_index,
        }
    }

    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_index + 1..]
    }
}

impl<'a> TryFrom<&'a str> for Domain<'a> {
    type Error = DomainParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<'a> Display for Domain<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.domain)
    }
}

impl<'a> AsRef<str> for Domain<'a> {
    fn as_ref(&self) -> &str {
        self.domain
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct OwnedDomain {
    domain: String,
    root_separator_index: Option<usize>,
    suffix_separator_index: usize,
}

impl OwnedDomain {
    pub fn parse(input: &str) -> Result<Self, DomainParseError> {
        let domain = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(domain)?;

        Ok(Self {
            domain: domain.to_string(),
            root_separator_index,
            suffix_separator_index,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.domain
    }

    pub fn prefix(&self) -> Option<&str> {
        self.root_separator_index.map(|i| &self.domain[..i])
    }

    pub fn root(&self) -> Root {
        let offset = self.root_separator_index.map(|i| i + 1).unwrap_or(0);
        let root = &self.domain[offset..];
        let suffix_separator_index = self.suffix_separator_index - offset;
        Root {
            root,
            suffix_separator_index,
        }
    }

    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_index + 1..]
    }
}

impl FromStr for OwnedDomain {
    type Err = DomainParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for OwnedDomain {
    type Error = DomainParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl Display for OwnedDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.domain)
    }
}

impl AsRef<str> for OwnedDomain {
    fn as_ref(&self) -> &str {
        &self.domain
    }
}

impl Deref for OwnedDomain {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.domain
    }
}
