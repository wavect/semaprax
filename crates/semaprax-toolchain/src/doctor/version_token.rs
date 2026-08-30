//! Complete tool-version token admission, not a tool authenticity claim.
//!
//! Three decimal u64 components are mandatory (no signs or leading zeroes).
//! Optional SemVer-style `-prerelease` and `+build` dot-separated ASCII
//! alphanumeric/hyphen identifiers are admitted. Numeric prerelease identifiers
//! cannot have leading zeroes; build identifiers may. Threshold comparison uses
//! the numeric tuple only, preserving the existing acceptance of tool channels.

pub(super) fn parse(token: &str) -> Option<(u64, u64, u64)> {
    let (without_build, build) = token
        .split_once('+')
        .map_or((token, None), |(core, suffix)| (core, Some(suffix)));
    if build.is_some_and(|suffix| !identifiers(suffix, false)) {
        return None;
    }
    let (core, prerelease) = without_build
        .split_once('-')
        .map_or((without_build, None), |(core, suffix)| (core, Some(suffix)));
    if prerelease.is_some_and(|suffix| !identifiers(suffix, true)) {
        return None;
    }
    let mut components = core.split('.');
    let value = (
        number(components.next()?)?,
        number(components.next()?)?,
        number(components.next()?)?,
    );
    components.next().is_none().then_some(value)
}

fn number(value: &str) -> Option<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

fn identifiers(value: &str, prerelease: bool) -> bool {
    value.split('.').all(|identifier| {
        !identifier.is_empty()
            && identifier
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !(prerelease
                && identifier.len() > 1
                && identifier.starts_with('0')
                && identifier.bytes().all(|byte| byte.is_ascii_digit()))
    })
}
