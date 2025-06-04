use super::*;

#[test]
fn root_parsing() {
    // Valid cases
    assert_eq!(
        Root::parse::<Box<_>>("example.com").unwrap().as_str(),
        "example.com"
    );
    assert_eq!(
        Root::parse::<Box<_>>("example.co.uk").unwrap().as_str(),
        "example.co.uk"
    );
    assert_eq!(
        Root::parse::<Box<_>>("example.org").unwrap().as_str(),
        "example.org"
    );
    assert_eq!(
        Root::parse::<Box<_>>("example.com.").unwrap().as_str(),
        "example.com."
    ); // trailing dot
    assert_eq!(
        Root::parse::<Box<_>>("example.com.")
            .unwrap()
            .not_fqdn()
            .as_str(),
        "example.com"
    ); // remove trailing dot

    // Invalid cases
    assert_eq!(
        Root::parse::<Box<_>>("www.example.com"),
        Err(DomainCreateError::Parse(DomainParseError::HasPrefix {
            domain: "www.example.com".to_string(),
            prefix: "www".to_string(),
        }))
    );
    assert_eq!(
        Root::parse::<Box<_>>(".example.com"),
        Err(DomainCreateError::Parse(DomainParseError::EmptyLabel {
            domain: ".example.com".to_string()
        }))
    );
    assert_eq!(
        Root::parse::<Box<_>>("com"),
        Err(DomainCreateError::Parse(DomainParseError::MissingRoot {
            domain: "com".to_string()
        }))
    );
    assert_eq!(
        Root::parse::<Box<_>>(""),
        Err(DomainCreateError::Parse(DomainParseError::Empty))
    );

    // Test for overly long domain
    let too_long_domain = "a.".repeat(254) + ".com";
    assert_eq!(
        Root::parse::<Box<_>>(&too_long_domain),
        Err(DomainCreateError::Parse(DomainParseError::TooLong {
            domain: too_long_domain.to_string()
        }))
    );

    // Test for overly long label
    let too_long_label = "a".repeat(64) + ".com";
    assert_eq!(
        Root::parse::<Box<_>>(&too_long_label),
        Err(DomainCreateError::Parse(DomainParseError::TooLongLabel {
            domain: too_long_label.to_string(),
            label: "a".repeat(64)
        }))
    );
}

#[test]
fn root_methods() {
    let root = Root::parse::<Box<_>>("example.com").unwrap();
    assert_eq!(root.as_str(), "example.com");
    assert_eq!(root.suffix(), "com");

    let root_with_complex_suffix = Root::parse::<Box<_>>("example.co.uk").unwrap();
    assert_eq!(root_with_complex_suffix.as_str(), "example.co.uk");
    assert_eq!(root_with_complex_suffix.suffix(), "co.uk");
}

#[test]
fn domain_parsing() {
    // Valid cases
    let domain = Domain::parse::<Box<_>>("example.com").unwrap();
    assert_eq!(domain.as_str(), "example.com");
    assert_eq!(domain.prefix(), None);
    assert_eq!(domain.root().as_str(), "example.com");
    assert_eq!(domain.suffix(), "com");

    let domain_with_prefix = Domain::parse::<Box<_>>("www.example.com").unwrap();
    assert_eq!(domain_with_prefix.as_str(), "www.example.com");
    assert_eq!(domain_with_prefix.prefix(), Some("www"));
    assert_eq!(domain_with_prefix.root().as_str(), "example.com");
    assert_eq!(domain_with_prefix.suffix(), "com");

    let domain_with_complex_suffix = Domain::parse::<Box<_>>("blog.example.co.uk").unwrap();
    assert_eq!(domain_with_complex_suffix.as_str(), "blog.example.co.uk");
    assert_eq!(domain_with_complex_suffix.prefix(), Some("blog"));
    assert_eq!(domain_with_complex_suffix.root().as_str(), "example.co.uk");
    assert_eq!(domain_with_complex_suffix.suffix(), "co.uk");

    // Multiple level prefixes
    let multi_prefix = Domain::parse::<Box<_>>("dev.api.example.org").unwrap();
    assert_eq!(multi_prefix.as_str(), "dev.api.example.org");
    assert_eq!(multi_prefix.prefix(), Some("dev.api"));
    assert_eq!(multi_prefix.root().as_str(), "example.org");
    assert_eq!(multi_prefix.suffix(), "org");

    // Invalid cases
    assert_eq!(
        Domain::parse::<Box<_>>(""),
        Err(DomainCreateError::Parse(DomainParseError::Empty))
    );

    // Test for unknown suffix
    let invalid_domain = "example.invalid";
    assert_eq!(
        Domain::parse::<Box<_>>(invalid_domain),
        Err(DomainCreateError::Parse(DomainParseError::UnknownSuffix {
            domain: invalid_domain.to_string(),
            suffix: "invalid".to_string()
        }))
    );

    // Test for trailing dot
    let domain_with_trailing_dot = Domain::parse::<Box<_>>("example.com.").unwrap();
    assert_eq!(domain_with_trailing_dot.as_str(), "example.com.");
}

#[test]
fn error_handling() {
    // Test empty domains
    assert_eq!(
        Domain::parse::<Box<_>>(""),
        Err(DomainCreateError::Parse(DomainParseError::Empty))
    );

    // Test too long domains
    let too_long = "a".repeat(254) + ".com";
    assert_eq!(
        Domain::parse::<Box<_>>(&too_long),
        Err(DomainCreateError::Parse(DomainParseError::TooLong {
            domain: too_long.to_string()
        }))
    );

    // Test label errors
    let domain_with_empty_label = "example..com";
    assert_eq!(
        Domain::parse::<Box<_>>(domain_with_empty_label),
        Err(DomainCreateError::Parse(DomainParseError::EmptyLabel {
            domain: domain_with_empty_label.to_string()
        }))
    );

    let too_long_label = "a".repeat(64) + ".com";
    assert_eq!(
        Domain::parse::<Box<_>>(&too_long_label),
        Err(DomainCreateError::Parse(DomainParseError::TooLongLabel {
            domain: too_long_label.to_string(),
            label: "a".repeat(64)
        }))
    );

    // Test invalid suffix
    // It doesn't seem to be possible to hit the missing suffix edge case due to the
    // way psl is implemented, so there's no test for that.
    let domain_with_invalid_suffix = "example.notarealsuffix";
    assert_eq!(
        Domain::parse::<Box<_>>(domain_with_invalid_suffix),
        Err(DomainCreateError::Parse(DomainParseError::UnknownSuffix {
            domain: domain_with_invalid_suffix.to_string(),
            suffix: "notarealsuffix".to_string()
        }))
    );

    // Test TLD with no root
    let tld_only = "com";
    assert_eq!(
        Domain::parse::<Box<_>>(tld_only),
        Err(DomainCreateError::Parse(DomainParseError::MissingRoot {
            domain: tld_only.to_string()
        }))
    );
}
