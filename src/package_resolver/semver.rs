use std::cmp::Ordering;

use super::wire;
use crate::diagnostic::Diagnostic;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Version(pub(super) u32, pub(super) u32, pub(super) u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Range {
    Exact(Version),
    Bounded { lower: Version, upper: Version },
}

impl Range {
    pub(super) fn contains(self, version: Version) -> bool {
        match self {
            Self::Exact(expected) => version == expected,
            Self::Bounded { lower, upper } => version >= lower && version < upper,
        }
    }
}

pub(super) fn parse_version(value: &str) -> Result<Version, Diagnostic> {
    const MAX_VERSION_BYTES: usize = 10 + 1 + 10 + 1 + 10;
    if value.len() > MAX_VERSION_BYTES {
        return Err(wire::input_error(
            "version exceeds the frozen three-u32 byte width",
        ));
    }
    let mut parts = value.split('.');
    let major = component(parts.next(), "major")?;
    let minor = component(parts.next(), "minor")?;
    let patch = component(parts.next(), "patch")?;
    if parts.next().is_some() {
        return Err(wire::input_error(
            "version must contain exactly three components",
        ));
    }
    Ok(Version(major, minor, patch))
}

pub(super) fn parse_range(value: &str) -> Result<Range, Diagnostic> {
    let operator = value
        .get(..1)
        .ok_or_else(|| wire::input_error("range must begin with one frozen operator"))?;
    let version = value
        .get(1..)
        .ok_or_else(|| wire::input_error("range must contain a version"))?;
    let lower = parse_version(version)?;
    match operator {
        "=" => Ok(Range::Exact(lower)),
        "~" => Ok(Range::Bounded {
            lower,
            upper: Version(
                lower.0,
                lower
                    .1
                    .checked_add(1)
                    .ok_or_else(|| wire::input_error("tilde upper bound overflows"))?,
                0,
            ),
        }),
        "^" => {
            let upper = match lower {
                Version(major @ 1..=u32::MAX, _, _) => Version(
                    major
                        .checked_add(1)
                        .ok_or_else(|| wire::input_error("caret upper bound overflows"))?,
                    0,
                    0,
                ),
                Version(0, minor @ 1..=u32::MAX, _) => Version(
                    0,
                    minor
                        .checked_add(1)
                        .ok_or_else(|| wire::input_error("caret upper bound overflows"))?,
                    0,
                ),
                Version(0, 0, patch) => Version(
                    0,
                    0,
                    patch
                        .checked_add(1)
                        .ok_or_else(|| wire::input_error("caret upper bound overflows"))?,
                ),
            };
            Ok(Range::Bounded { lower, upper })
        }
        _ => Err(wire::input_error("unsupported version-range operator")),
    }
}

fn component(value: Option<&str>, label: &str) -> Result<u32, Diagnostic> {
    let value = value.ok_or_else(|| wire::input_error(format!("missing {label} component")))?;
    if value.len() > 10 {
        return Err(wire::input_error(format!(
            "{label} component exceeds the u32 decimal width"
        )));
    }
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(wire::input_error(format!(
            "{label} component is not canonical unsigned decimal"
        )));
    }
    value
        .parse()
        .map_err(|_| wire::input_error(format!("{label} component overflows u32")))
}

pub(super) fn compare_coordinates(
    left_package: &str,
    left_version: Version,
    right_package: &str,
    right_version: Version,
) -> Ordering {
    left_package
        .cmp(right_package)
        .then_with(|| left_version.cmp(&right_version))
}
