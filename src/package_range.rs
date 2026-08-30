use crate::diagnostic::Diagnostic;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Version(pub(crate) u32, pub(crate) u32, pub(crate) u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Range {
    Exact(Version),
    Bounded { lower: Version, upper: Version },
}

impl Range {
    pub(crate) fn contains(self, version: Version) -> bool {
        match self {
            Self::Exact(v) => v == version,
            Self::Bounded { lower, upper } => version >= lower && version < upper,
        }
    }
}

pub(crate) fn parse_version(
    value: &str,
    error: fn(String) -> Diagnostic,
) -> Result<Version, Diagnostic> {
    if value.len() > 32 {
        return Err(error(
            "version exceeds the frozen three-u32 byte width".into(),
        ));
    }
    let mut parts = value.split('.');
    let result = Version(
        component(parts.next(), "major", error)?,
        component(parts.next(), "minor", error)?,
        component(parts.next(), "patch", error)?,
    );
    if parts.next().is_some() {
        return Err(error(
            "version must contain exactly three components".into(),
        ));
    }
    Ok(result)
}

pub(crate) fn parse_range(
    value: &str,
    error: fn(String) -> Diagnostic,
) -> Result<Range, Diagnostic> {
    let (operator, version) = value
        .split_at_checked(1)
        .ok_or_else(|| error("range must begin with one frozen operator".into()))?;
    let lower = parse_version(version, error)?;
    let upper = match operator {
        "=" => return Ok(Range::Exact(lower)),
        "~" => Version(
            lower.0,
            lower
                .1
                .checked_add(1)
                .ok_or_else(|| error("tilde upper bound overflows".into()))?,
            0,
        ),
        "^" if lower.0 > 0 => Version(
            lower
                .0
                .checked_add(1)
                .ok_or_else(|| error("caret upper bound overflows".into()))?,
            0,
            0,
        ),
        "^" if lower.1 > 0 => Version(
            0,
            lower
                .1
                .checked_add(1)
                .ok_or_else(|| error("caret upper bound overflows".into()))?,
            0,
        ),
        "^" => Version(
            0,
            0,
            lower
                .2
                .checked_add(1)
                .ok_or_else(|| error("caret upper bound overflows".into()))?,
        ),
        _ => return Err(error("unsupported version-range operator".into())),
    };
    Ok(Range::Bounded { lower, upper })
}

fn component(
    value: Option<&str>,
    label: &str,
    error: fn(String) -> Diagnostic,
) -> Result<u32, Diagnostic> {
    let value = value.ok_or_else(|| error(format!("missing {label} component")))?;
    if value.is_empty()
        || value.len() > 10
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(error(format!(
            "{label} component is not canonical unsigned decimal"
        )));
    }
    value
        .parse()
        .map_err(|_| error(format!("{label} component overflows u32")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(message: String) -> Diagnostic {
        Diagnostic::io("SPX-PR601", message)
    }

    #[test]
    fn exact_tilde_and_caret_boundaries_match_the_frozen_v1_language() {
        for (text, last, outside) in [
            ("=1.2.3", Version(1, 2, 3), Version(1, 2, 4)),
            ("~1.2.3", Version(1, 2, u32::MAX), Version(1, 3, 0)),
            ("^1.2.3", Version(1, u32::MAX, u32::MAX), Version(2, 0, 0)),
            ("^0.2.3", Version(0, 2, u32::MAX), Version(0, 3, 0)),
            ("^0.0.3", Version(0, 0, 3), Version(0, 0, 4)),
        ] {
            let range = parse_range(text, error).unwrap();
            assert!(range.contains(last));
            assert!(!range.contains(outside));
        }
        for text in [
            "",
            "1.2.3",
            "=1.2",
            "=01.2.3",
            "=+1.2.3",
            "=1.2.3-beta",
            "^4294967295.0.0",
            "~0.4294967295.0",
            "^0.0.4294967295",
            "=1.2.3 || =2.0.0",
            "😀1.2.3",
        ] {
            assert!(parse_range(text, error).is_err(), "{text}");
        }
        assert!(parse_range("=4294967295.4294967295.4294967295", error).is_ok());
        assert!(parse_version("1.10.0", error).unwrap() > parse_version("1.2.0", error).unwrap());
    }
}
