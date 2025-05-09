use core::{
    ops::Deref,
    str::{self, FromStr},
};

use psl::Psl;
use simple_dst::{AllocDst, AllocDstError, Dst};
use thiserror::Error;

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

#[derive(Debug, Error)]
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
    suffix_separator_index: usize,
    root: str,
}

impl Root {
    pub fn parse<A>(input: &str) -> Result<A, DomainCreationError>
    where
        A: AllocDst<Self>,
    {
        let root = get_domain(input);

        let (root_separator_index, suffix_separator_index) = parse_domain(root)?;
        if root_separator_index.is_some() {
            return Err(DomainCreationError::HasPrefix(root.to_string()));
        }

        Ok(Self::alloc(suffix_separator_index, root)?)
    }

    pub fn as_str(&self) -> &str {
        &self.root
    }

    pub fn suffix(&self) -> &str {
        &self.root[self.suffix_separator_index + 1..]
    }
}

impl AsRef<str> for Root {
    fn as_ref(&self) -> &str {
        &self.root
    }
}

impl Deref for Root {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.root
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

        Ok(Self::alloc(
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

    pub fn root(&self) -> &str {
        let offset = self.root_separator_index.map(|i| i + 1).unwrap_or(0);
        &self.domain[offset..]
    }

    pub fn alloc_root<A>(&self) -> Result<A, AllocDstError>
    where
        A: AllocDst<Root>,
    {
        let offset = self.root_separator_index.map(|i| i + 1).unwrap_or(0);
        let root = &self.domain[offset..];
        let suffix_separator_index = self.suffix_separator_index - offset;
        Root::alloc(suffix_separator_index, root)
    }

    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_index + 1..]
    }
}

impl AsRef<str> for Domain {
    fn as_ref(&self) -> &str {
        &self.domain
    }
}

impl Deref for Domain {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.domain
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
