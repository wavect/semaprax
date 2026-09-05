pub(super) fn valid_package_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (byte == b'-' && index != 0)
        })
        && value.as_bytes()[0].is_ascii_lowercase()
}

pub(super) fn valid_sha256_fact(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub(super) fn valid_package_semver(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 || !value.is_ascii() {
        return false;
    }
    let mut build = value.split('+');
    let Some(core_pre) = build.next() else {
        return false;
    };
    if build
        .next()
        .is_some_and(|part| !valid_semver_ids(part, false))
        || build.next().is_some()
    {
        return false;
    }
    let (core, pre) = core_pre
        .split_once('-')
        .map_or((core_pre, None), |(a, b)| (a, Some(b)));
    if pre.is_some_and(|part| !valid_semver_ids(part, true)) {
        return false;
    }
    let mut core = core.split('.');
    let parts = [core.next(), core.next(), core.next()];
    core.next().is_none()
        && parts
            .into_iter()
            .all(|part| part.is_some_and(valid_semver_number))
}

fn valid_semver_ids(value: &str, numeric_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !(numeric_zero
                    && part.bytes().all(|byte| byte.is_ascii_digit())
                    && part.len() > 1
                    && part.starts_with('0'))
        })
}

fn valid_semver_number(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}
