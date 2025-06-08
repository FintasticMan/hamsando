#[cfg(test)]
mod tests;

use std::{
    alloc::LayoutError,
    borrow::Borrow,
    cmp,
    fmt::{self, Display},
    hash::{self, Hash},
    ops::Deref,
    ptr::slice_from_raw_parts,
    str::{self, FromStr},
};

use serde::{Deserialize, Serialize};
use simple_dst::{AllocDst, CloneToUninit, Dst, ToOwned};
use thiserror::Error;

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

/// Errors representing invalid or malformed domain strings.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum DomainParseError {
    /// The domain is empty.
    #[error("domain is empty")]
    Empty,
    /// The domain contains an empty label.
    #[error("{domain}: domain contains an empty label")]
    EmptyLabel { domain: String },
    /// The domain has a prefix when it shouldn't ([`Root`]).
    #[error("{domain}: domain has a prefix: {prefix}")]
    HasPrefix { domain: String, prefix: String },
    /// The domain has no root.
    #[error("{domain}: domain has no root")]
    MissingRoot { domain: String },
    /// The domain is missing a suffix.
    ///
    /// This case seems to be unreachable with this psl implementation.
    #[error("{domain}: domain is missing a suffix")]
    MissingSuffix { domain: String },
    /// The domain is too long.
    #[error("{domain}: domain is too long")]
    TooLong { domain: String },
    /// The domain contains a too-long label.
    #[error("{domain}: domain contains a too-long label: {label}")]
    TooLongLabel { domain: String, label: String },
    /// The domain has an unknown suffix.
    #[error("{domain}: domain has an unknown suffix: {suffix}")]
    UnknownSuffix { domain: String, suffix: String },
}

/// Errors that can occur when creating a domain instance.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum DomainCreateError {
    /// The domain string failed to parse.
    #[error(transparent)]
    Parse(#[from] DomainParseError),
    /// Failure to calculate the layout of the domain type.
    ///
    /// This could happen if the length of the input string is almost [`isize::MAX`].
    #[error(transparent)]
    Layout(#[from] LayoutError),
}

/// Gets the not-fully-qualified part of the given domain.
fn get_not_fqdn(s: &str) -> &str {
    if s.ends_with('.') {
        &s[..s.len() - 1]
    } else {
        s
    }
}

/// Returns the indices for the `.`s between the prefix and root, and before the suffix.
fn parse_domain(domain: &str) -> Result<(Option<usize>, usize), DomainParseError> {
    let not_fqdn = get_not_fqdn(domain);

    if not_fqdn.is_empty() {
        return Err(DomainParseError::Empty);
    }

    if not_fqdn.len() > MAX_DOMAIN_LEN {
        return Err(DomainParseError::TooLong {
            domain: domain.to_string(),
        });
    }

    for label in not_fqdn.split('.') {
        if label.is_empty() {
            return Err(DomainParseError::EmptyLabel {
                domain: domain.to_string(),
            });
        } else if label.len() > MAX_LABEL_LEN {
            return Err(DomainParseError::TooLongLabel {
                domain: domain.to_string(),
                label: label.to_string(),
            });
        }
    }

    let suffix =
        psl::suffix(not_fqdn.as_bytes()).ok_or_else(|| DomainParseError::MissingSuffix {
            domain: domain.to_string(),
        })?;
    let suffix_str = str::from_utf8(suffix.as_bytes())
        .expect("psl crate returned invalid UTF-8 when slicing domain suffix");
    if !suffix.is_known() {
        return Err(DomainParseError::UnknownSuffix {
            domain: domain.to_string(),
            suffix: suffix_str.to_string(),
        });
    }

    let suffix_len = suffix_str.len();
    if not_fqdn.len() == suffix_len {
        return Err(DomainParseError::MissingRoot {
            domain: domain.to_string(),
        });
    }
    let suffix_separator_idx = not_fqdn.len() - suffix_len - 1;
    let without_suffix = &not_fqdn[..suffix_separator_idx];

    Ok((without_suffix.rfind('.'), suffix_separator_idx))
}

/// The root part of a domain name.
// LAYOUT: This struct must have the same layout as [`Domain`] so that it can be used to
// create a [`Root`] without re-allocating.
#[repr(C)]
#[derive(Debug, Dst, CloneToUninit, ToOwned)]
#[dst(new_unchecked_vis = pub)]
pub struct Root {
    root_separator_idx: Option<usize>,
    suffix_separator_idx: usize,
    domain: str,
}

impl Root {
    /// Parses a string and creates an owned Root.
    ///
    /// # Errors
    ///
    /// Will return an error in case the domain is invalid, contains a prefix, or if an
    /// error occured during allocation.
    pub fn parse<A>(input: &str) -> Result<A, DomainCreateError>
    where
        A: AllocDst<Self>,
    {
        let (root_separator_idx, suffix_separator_idx) = parse_domain(input)?;
        if let Some(root_separator_idx) = root_separator_idx {
            return Err(DomainCreateError::Parse(DomainParseError::HasPrefix {
                domain: input.to_string(),
                prefix: input[..root_separator_idx].to_string(),
            }));
        }

        Ok(unsafe { Self::new_unchecked(root_separator_idx, suffix_separator_idx, input) }?)
    }

    /// Returns a string representing the domain.
    pub fn as_str(&self) -> &str {
        let offset = self.root_separator_idx.map(|i| i + 1).unwrap_or(0);
        &self.domain[offset..]
    }

    /// Returns the suffix (TLD) of the domain.
    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_idx + 1..]
    }

    /// Returns whether the domain is fully-qualified (i.e. ends in a `.`).
    pub fn is_fqdn(&self) -> bool {
        self.as_str().ends_with('.')
    }

    // Returns a Root representing the not-fully-qualified part.
    pub fn not_fqdn(&self) -> &Self {
        if !self.is_fqdn() {
            return self;
        }

        // SAFETY: the pointer metadata for the Root type is just the length of the
        // string in it, so creating a new pointer with the metadata of the length minus
        // one is safe. Lowering the length by one doesn't influence the stored indices.
        unsafe {
            let ptr = (&raw const *self).cast::<()>();
            // FUTURE: switch to using ptr_from_raw_parts when it has stabilised.
            &*(slice_from_raw_parts(ptr, self.len() - 1) as *const Self)
        }
    }
}

impl AsRef<str> for Root {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Root {
    fn borrow(&self) -> &str {
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
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Root {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for Root {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Display for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl FromStr for Box<Root> {
    type Err = DomainCreateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Root::parse(s)
    }
}

impl TryFrom<&str> for Box<Root> {
    type Error = DomainCreateError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Root::parse(value)
    }
}

impl<'de> Deserialize<'de> for Box<Root> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Visitor;

        struct RootVisitor;

        impl<'de> Visitor<'de> for RootVisitor {
            type Value = Box<Root>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing a domain root")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Root::parse(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(RootVisitor)
    }
}

impl<'de> Serialize for Box<Root> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// A domain name.
// LAYOUT: This struct must have the same layout as [`Root`] so that it can be used to
// create a [`Root`] without re-allocating.
#[repr(C)]
#[derive(Debug, Dst, CloneToUninit, ToOwned)]
#[dst(new_unchecked_vis = pub)]
pub struct Domain {
    root_separator_idx: Option<usize>,
    suffix_separator_idx: usize,
    domain: str,
}

impl Domain {
    /// Parses a string and creates an owned Domain.
    ///
    /// # Errors
    ///
    /// Will return an error in case the domain is invalid or if an error occured during
    /// allocation.
    pub fn parse<A>(input: &str) -> Result<A, DomainCreateError>
    where
        A: AllocDst<Self>,
    {
        let (root_separator_idx, suffix_separator_idx) = parse_domain(input)?;

        Ok(unsafe { Self::new_unchecked(root_separator_idx, suffix_separator_idx, input) }?)
    }

    /// Returns a string representing the domain.
    pub fn as_str(&self) -> &str {
        &self.domain
    }

    /// Returns the prefix (subdomain) of the domain.
    pub fn prefix(&self) -> Option<&str> {
        self.root_separator_idx.map(|i| &self.domain[..i])
    }

    /// Returns the root part of the domain.
    pub fn root(&self) -> &Root {
        // SAFETY: Domain and Root have the exact same fields in the same order
        // and are both repr(C), meaning that they have the same layout.
        unsafe { &*((&raw const *self) as *const Root) }
    }

    /// Returns the root part of the domain as a string.
    pub fn root_str(&self) -> &str {
        self.root().as_str()
    }

    /// Returns the suffix (TLD) of the domain.
    pub fn suffix(&self) -> &str {
        &self.domain[self.suffix_separator_idx + 1..]
    }

    /// Returns whether the domain is fully-qualified (i.e. ends in a `'`).
    pub fn is_fqdn(&self) -> bool {
        self.as_str().ends_with('.')
    }

    // Returns a Domain representing the not-fully-qualified part.
    pub fn not_fqdn(&self) -> &Self {
        if !self.is_fqdn() {
            return self;
        }

        // SAFETY: the pointer metadata for the Domain type is just the length of the
        // string in it, so creating a new pointer with the metadata of the length minus
        // one is safe. Lowering the length by one doesn't influence the stored indices.
        unsafe {
            let ptr = (&raw const *self).cast::<()>();
            // FUTURE: switch to using ptr_from_raw_parts when it has stabilised.
            &*(slice_from_raw_parts(ptr, self.len() - 1) as *const Self)
        }
    }
}

impl AsRef<str> for Domain {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Domain {
    fn borrow(&self) -> &str {
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
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Domain {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for Domain {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl FromStr for Box<Domain> {
    type Err = DomainCreateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Domain::parse(s)
    }
}

impl TryFrom<&str> for Box<Domain> {
    type Error = DomainCreateError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Domain::parse(value)
    }
}

impl<'de> Deserialize<'de> for Box<Domain> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Visitor;

        struct DomainVisitor;

        impl<'de> Visitor<'de> for DomainVisitor {
            type Value = Box<Domain>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing a domain")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Domain::parse(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(DomainVisitor)
    }
}

impl<'de> Serialize for Box<Domain> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}
