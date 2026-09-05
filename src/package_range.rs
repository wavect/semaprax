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

    fn version_error(value: &str) -> Diagnostic {
        let diagnostic = parse_version(value, error).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-PR601", "{value}");
        diagnostic
    }

    fn range_error(value: &str) -> Diagnostic {
        let diagnostic = parse_range(value, error).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-PR601", "{value}");
        diagnostic
    }

    fn bounds(text: &str) -> (Version, Version) {
        match parse_range(text, error).unwrap() {
            Range::Bounded { lower, upper } => (lower, upper),
            Range::Exact(version) => panic!("{text} parsed as exact {version:?}"),
        }
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

    #[test]
    fn caret_upper_bound_advances_the_leading_nonzero_component() {
        for (text, lower, upper, last_inside, first_below) in [
            (
                "^1.2.3",
                Version(1, 2, 3),
                Version(2, 0, 0),
                Version(1, u32::MAX, u32::MAX),
                Version(1, 2, 2),
            ),
            (
                "^0.2.3",
                Version(0, 2, 3),
                Version(0, 3, 0),
                Version(0, 2, u32::MAX),
                Version(0, 2, 2),
            ),
            (
                "^0.0.3",
                Version(0, 0, 3),
                Version(0, 0, 4),
                Version(0, 0, 3),
                Version(0, 0, 2),
            ),
        ] {
            assert_eq!(bounds(text), (lower, upper), "{text}");
            let range = parse_range(text, error).unwrap();
            assert!(range.contains(lower), "{text} excludes its own lower bound");
            assert!(
                range.contains(last_inside),
                "{text} excludes {last_inside:?}"
            );
            assert!(!range.contains(upper), "{text} includes its upper bound");
            assert!(
                !range.contains(first_below),
                "{text} includes {first_below:?}"
            );
        }
    }

    #[test]
    fn tilde_upper_bound_advances_only_the_minor_component() {
        assert_eq!(bounds("~1.2.3"), (Version(1, 2, 3), Version(1, 3, 0)));
        // The same zero-major version that caret narrows to a single patch is
        // widened by tilde to the whole minor series.
        assert_eq!(bounds("~0.0.3"), (Version(0, 0, 3), Version(0, 1, 0)));
        // Tilde never touches the major component, so a maximal major is fine.
        assert_eq!(
            bounds("~4294967295.0.0"),
            (Version(u32::MAX, 0, 0), Version(u32::MAX, 1, 0))
        );
        let range = parse_range("~1.2.3", error).unwrap();
        assert!(range.contains(Version(1, 2, 3)));
        assert!(range.contains(Version(1, 2, u32::MAX)));
        assert!(!range.contains(Version(1, 3, 0)));
        assert!(!range.contains(Version(1, 2, 2)));
    }

    #[test]
    fn exact_range_admits_only_the_named_version() {
        let range = parse_range("=1.2.3", error).unwrap();
        assert_eq!(range, Range::Exact(Version(1, 2, 3)));
        assert!(range.contains(Version(1, 2, 3)));
        for outside in [
            Version(1, 2, 2),
            Version(1, 2, 4),
            Version(1, 3, 3),
            Version(0, 2, 3),
            Version(2, 2, 3),
        ] {
            assert!(!range.contains(outside), "{outside:?}");
        }
        // An exact range performs no upper-bound arithmetic, so the maximal
        // version is representable.
        assert_eq!(
            parse_range("=4294967295.4294967295.4294967295", error).unwrap(),
            Range::Exact(Version(u32::MAX, u32::MAX, u32::MAX))
        );
    }

    #[test]
    fn component_accepts_only_canonical_unsigned_decimal() {
        assert_eq!(parse_version("0.0.0", error).unwrap(), Version(0, 0, 0));
        assert_eq!(
            parse_version("10.20.30", error).unwrap(),
            Version(10, 20, 30)
        );
        assert_eq!(
            parse_version("4294967295.0.10", error).unwrap(),
            Version(u32::MAX, 0, 10)
        );
        // A bare zero is canonical; any padded form is not.
        for (text, label) in [
            ("00.0.0", "major"),
            ("0.00.0", "minor"),
            ("0.0.00", "patch"),
            ("01.2.3", "major"),
            ("1.02.3", "minor"),
            ("1.2.03", "patch"),
            ("1..3", "minor"),
            ("+1.2.3", "major"),
            ("1.-2.3", "minor"),
            ("1.2.x", "patch"),
            ("1.2. 3", "patch"),
            ("1.2.3\u{a0}", "patch"),
            // Eleven digits are refused on length before parsing.
            ("12345678901.2.3", "major"),
            ("1.12345678901.3", "minor"),
        ] {
            let diagnostic = version_error(text);
            assert!(
                diagnostic.message.contains(label),
                "{text}: {}",
                diagnostic.message
            );
            assert!(
                diagnostic.message.contains("not canonical"),
                "{text}: {}",
                diagnostic.message
            );
        }
        // Ten digits pass the length guard and are rejected by the u32 parse,
        // which is a distinct diagnostic from a malformed component.
        for (text, label) in [("4294967296.0.0", "major"), ("0.9999999999.0", "minor")] {
            let diagnostic = version_error(text);
            assert!(
                diagnostic.message.contains(label) && diagnostic.message.contains("overflows u32"),
                "{text}: {}",
                diagnostic.message
            );
        }
    }

    #[test]
    fn parse_version_requires_exactly_three_dot_separated_components() {
        assert_eq!(parse_version("1.2.3", error).unwrap(), Version(1, 2, 3));
        for (text, label) in [("1.2", "patch"), ("1", "minor")] {
            let diagnostic = version_error(text);
            assert!(
                diagnostic.message.contains(label) && diagnostic.message.contains("missing"),
                "{text}: {}",
                diagnostic.message
            );
        }
        // A trailing dot is a fourth component, not a two-component version.
        for text in ["1.2.3.4", "1.2.3."] {
            let diagnostic = version_error(text);
            assert!(
                diagnostic.message.contains("exactly three components"),
                "{text}: {}",
                diagnostic.message
            );
        }
        // A leading dot empties the major component rather than shifting the
        // remaining components left, and the empty string is a missing major.
        for text in [".1.2", ""] {
            let diagnostic = version_error(text);
            assert!(
                diagnostic.message.contains("major"),
                "{text}: {}",
                diagnostic.message
            );
        }
    }

    #[test]
    fn parse_version_enforces_the_frozen_thirty_two_byte_width() {
        let widest = "4294967295.4294967295.4294967295";
        assert_eq!(widest.len(), 32);
        assert_eq!(
            parse_version(widest, error).unwrap(),
            Version(u32::MAX, u32::MAX, u32::MAX)
        );
        let over = format!("{widest}0");
        assert_eq!(over.len(), 33);
        let diagnostic = version_error(&over);
        assert!(
            diagnostic.message.contains("byte width"),
            "{}",
            diagnostic.message
        );
        // The guard counts bytes, not characters, so a short multi-byte string
        // is refused before any component is inspected.
        let multibyte = format!("1.2.3{}", "\u{1f600}".repeat(7));
        assert_eq!(multibyte.len(), 33);
        assert!(multibyte.chars().count() < 32);
        let diagnostic = version_error(&multibyte);
        assert!(
            diagnostic.message.contains("byte width"),
            "{}",
            diagnostic.message
        );
    }

    #[test]
    fn parse_range_requires_a_supported_single_byte_operator() {
        // `split_at_checked` yields nothing for an empty string, and refuses a
        // non-boundary index, so a leading multi-byte character reads as a
        // missing operator rather than as a malformed version.
        for text in ["", "\u{1f600}1.2.3"] {
            let diagnostic = range_error(text);
            assert!(
                diagnostic.message.contains("frozen operator"),
                "{text}: {}",
                diagnostic.message
            );
        }
        for text in [">1.2.3", "<1.2.3", "*1.2.3", " 1.2.3"] {
            let diagnostic = range_error(text);
            assert!(
                diagnostic
                    .message
                    .contains("unsupported version-range operator"),
                "{text}: {}",
                diagnostic.message
            );
        }
        // The version is parsed before the operator is classified, so a bad
        // version masks an unsupported operator.
        for text in [">=1.2.3", "1.2.3"] {
            let diagnostic = range_error(text);
            assert!(
                diagnostic.message.contains("major"),
                "{text}: {}",
                diagnostic.message
            );
        }
    }

    #[test]
    fn upper_bound_arithmetic_reports_overflow_instead_of_wrapping() {
        for text in [
            "^4294967295.0.0",
            "^0.4294967295.0",
            "^0.0.4294967295",
            "~1.4294967295.0",
            "~0.4294967295.0",
        ] {
            let diagnostic = range_error(text);
            assert!(
                diagnostic.message.contains("overflows"),
                "{text}: {}",
                diagnostic.message
            );
        }
        // The guard sits exactly at u32::MAX: the neighbouring lower bound
        // still resolves to a maximal upper bound.
        assert_eq!(bounds("^4294967294.0.0").1, Version(u32::MAX, 0, 0));
        assert_eq!(bounds("^0.4294967294.0").1, Version(0, u32::MAX, 0));
        assert_eq!(bounds("^0.0.4294967294").1, Version(0, 0, u32::MAX));
        assert_eq!(bounds("~1.4294967294.0").1, Version(1, u32::MAX, 0));
    }

    #[test]
    fn version_ordering_compares_major_then_minor_then_patch() {
        assert!(Version(1, 10, 0) > Version(1, 2, 0));
        assert!(Version(2, 0, 0) > Version(1, u32::MAX, u32::MAX));
        assert!(Version(0, 1, 0) > Version(0, 0, u32::MAX));
        assert!(Version(1, 2, 3) < Version(1, 2, 4));
    }
}
