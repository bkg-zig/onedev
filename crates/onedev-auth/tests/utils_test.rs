use onedev_auth::utils::{constant_time_eq, format_principal, parse_principal, PrincipalType};

#[test]
fn principal_roundtrip() {
    assert_eq!(
        parse_principal(&format_principal(PrincipalType::User, 42)),
        Some((PrincipalType::User, 42))
    );
    assert_eq!(
        parse_principal(&format_principal(PrincipalType::AccessToken, 7)),
        Some((PrincipalType::AccessToken, 7))
    );
}

#[test]
fn parse_principal_rejects_unknown_prefix_and_bad_id() {
    assert_eq!(parse_principal("x:1"), None);
    assert_eq!(parse_principal("u:abc"), None);
    assert_eq!(parse_principal(""), None);
}

#[test]
fn constant_time_eq_compares_content_and_length() {
    assert!(constant_time_eq("secret", "secret"));
    assert!(!constant_time_eq("secret", "secreu"));
    assert!(!constant_time_eq("secret", "secre"));
}
