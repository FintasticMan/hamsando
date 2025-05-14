use core::{
    hash::Hash,
    ops::Deref,
    str::{self, FromStr},
};

use psl::Psl;
use serde::Deserialize;
use simple_dst::{AllocDst, AllocDstError, Dst};
use thiserror::Error;

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainCreationError {
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
    #[error(transparent)]
    Alloc(#[from] AllocDstError),
}

fn get_domain(s: &str) -> &str {
    if s.ends_with('.') {
        &s[..s.len() - 1]
    } else {
        s
    }
}

/// Returns the indices for the `.`s between the prefix and root, and before the suffix.
fn parse_domain(domain: &str) -> Result<(Option<usize>, usize), DomainCreationError> {
    if domain.is_empty() {
        return Err(DomainCreationError::Empty);
    }

    if domain.len() > MAX_DOMAIN_LEN {
        return Err(DomainCreationError::TooLong(domain.to_string()));
    }

    for label in domain.split('.') {
        if label.is_empty() {
            return Err(DomainCreationError::EmptyLabel(domain.to_string()));
        } else if label.len() > MAX_LABEL_LEN {
            return Err(DomainCreationError::TooLongLabel(
                domain.to_string(),
                label.to_string(),
            ));
        }
    }

    let suffix = match psl::List.suffix(domain.as_bytes()) {
        Some(suffix) => suffix,
        None => return Err(DomainCreationError::MissingSuffix(domain.to_string())),
    };
    if !suffix.is_known() {
        return Err(DomainCreationError::UnknownSuffix(
            domain.to_string(),
            str::from_utf8(suffix.as_bytes()).unwrap().to_string(),
        ));
    }

    let suffix_len = str::from_utf8(suffix.as_bytes()).unwrap().len();
    if domain.len() == suffix_len {
        return Err(DomainCreationError::MissingRoot(domain.to_string()));
    }
    let suffix_separator_index = domain.len() - suffix_len - 1;
    let without_suffix = &domain[..suffix_separator_index];

    Ok((without_suffix.rfind('.'), suffix_separator_index))
}

#[repr(C)]
#[derive(Dst, Debug)]
pub struct Root {
    root_separator_index: Option<usize>,
    suffix_separator_index: usize,
    domain: str,
}

impl Root {
    /// This function will return an error if the input is not just the root
    /// part of a domain.
    pub fn parse<A>(input: &str) -> Result<A, DomainCreationError>
    where
        A: AllocDst<Self>,
    {
        let root = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(root)?;
        if root_separator_index.is_some() {
            return Err(DomainCreationError::HasPrefix(root.to_string()));
        }

        Ok(Self::new_internal(
            root_separator_index,
            suffix_separator_index,
            root,
        )?)
    }

    pub fn as_str(&self) -> &str {
        let offset = self.root_separator_index.map(|i| i + 1).unwrap_or(0);
        &self.domain[offset..]
    }

    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_index + 1..]
    }
}

impl AsRef<str> for Root {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Root {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq for Root {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Root {}

impl PartialOrd for Root {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Root {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for Root {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl FromStr for Box<Root> {
    type Err = DomainCreationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Root::parse(s)
    }
}

impl TryFrom<&str> for Box<Root> {
    type Error = DomainCreationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Root::parse(value)
    }
}

impl<'de> Deserialize<'de> for Box<Root> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let s = <&str>::deserialize(deserializer)?;
        s.parse::<Self>().map_err(D::Error::custom)
    }
}

#[repr(C)]
#[derive(Dst, Debug)]
pub struct Domain {
    root_separator_index: Option<usize>,
    suffix_separator_index: usize,
    domain: str,
}

impl Domain {
    pub fn parse<A>(input: &str) -> Result<A, DomainCreationError>
    where
        A: AllocDst<Self>,
    {
        let domain = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(domain)?;

        Ok(Self::new_internal(
            root_separator_index,
            suffix_separator_index,
            domain,
        )?)
    }

    pub fn as_str(&self) -> &str {
        &self.domain
    }

    pub fn prefix(&self) -> Option<&str> {
        self.root_separator_index.map(|i| &self.domain[..i])
    }

    pub fn root(&self) -> &Root {
        // SAFETY: Domain and Root have the exact same fields in the same order
        // and are both repr(C), meaning that they have the same layout.
        unsafe { &*((&raw const *self) as *const Root) }
    }

    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_index + 1..]
    }
}

impl AsRef<str> for Domain {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Domain {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq for Domain {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Domain {}

impl PartialOrd for Domain {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Domain {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for Domain {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl FromStr for Box<Domain> {
    type Err = DomainCreationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Domain::parse(s)
    }
}

impl TryFrom<&str> for Box<Domain> {
    type Error = DomainCreationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Domain::parse(value)
    }
}

impl<'de> Deserialize<'de> for Box<Domain> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let s = <&str>::deserialize(deserializer)?;
        s.parse::<Self>().map_err(D::Error::custom)
    }
}
