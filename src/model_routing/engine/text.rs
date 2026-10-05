//! Bounded-text predicates shared by every consumer: digest syntax and the
//! conservative secret heuristic applied to identifiers and excerpts.

/// `sha256:` + 64 lowercase hex.
pub fn is_digest(s: &str) -> bool {
    s.strip_prefix("sha256:")
        .is_some_and(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// Conservative secret heuristics: well-known token prefixes, credentials in a
/// URL, `key=value` credential assignments and long opaque tokens.
pub fn looks_like_secret(s: &str) -> bool {
    const PREFIXES: [&str; 11] = [
        "sk-",
        "ghp_",
        "gho_",
        "ghs_",
        "github_pat_",
        "xox",
        "AKIA",
        "AIza",
        "-----BEGIN",
        "Bearer ",
        "eyJ",
    ];
    if PREFIXES.iter().any(|p| s.starts_with(p)) {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    if ["api_key=", "apikey=", "token=", "password=", "secret="]
        .iter()
        .any(|p| lower.contains(p))
    {
        return true;
    }
    if let Some((_, rest)) = s.split_once("://") {
        let authority = rest.split('/').next().unwrap_or("");
        if authority.contains('@') && authority.split('@').next().is_some_and(|u| u.contains(':')) {
            return true;
        }
    }
    s.len() >= 32
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'=' | b'_' | b'-'))
        && s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes().any(|b| b.is_ascii_alphabetic())
}
