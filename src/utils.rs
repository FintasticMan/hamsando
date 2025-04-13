use addr::domain;

use crate::DomainError;

pub(crate) fn split_domain<'a>(
    name: &'a domain::Name,
) -> Result<(Option<&'a str>, &'a str), DomainError> {
    let root = name
        .root()
        .ok_or_else(|| DomainError::MissingRoot(name.to_string()))?;
    let prefix = name.prefix();

    Ok((prefix, root))
}
