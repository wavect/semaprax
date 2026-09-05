//! `semaprax package report|lock|resolve ...`: the 1.0 namespace over the
//! offline package routes. Each subcommand is exactly its long-form command
//! with the same operands, so the receipts, diagnostics, and statuses are the
//! owning route's own.

const USAGE: &str =
    "package accepts exactly `report|lock|resolve <arguments>`; see `semaprax help package`";

/// The long-form command a `package` subcommand stands for.
pub(crate) const SUBCOMMANDS: &[(&str, &str)] = &[
    ("report", "package-report"),
    ("lock", "package-lock"),
    ("resolve", "package-resolve"),
];

/// Rewrite `package <sub> <rest...>` as `<long-form> <rest...>`.
pub(crate) fn long_form(args: &[String]) -> Result<Vec<String>, u8> {
    let Some((subcommand, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return Err(2);
    };
    let Some((_, long)) = SUBCOMMANDS
        .iter()
        .find(|(short, _)| *short == subcommand.as_str())
    else {
        eprintln!("unknown package subcommand `{subcommand}`; {USAGE}");
        return Err(2);
    };
    let mut rewritten = Vec::with_capacity(rest.len() + 1);
    rewritten.push((*long).to_owned());
    rewritten.extend(rest.iter().cloned());
    Ok(rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn package_namespace_rewrites_to_the_long_forms() {
        assert_eq!(
            long_form(&strings(&["report", "m.spx", "--max-bytes", "9"])).unwrap(),
            strings(&["package-report", "m.spx", "--max-bytes", "9"])
        );
        assert_eq!(
            long_form(&strings(&["lock"])).unwrap(),
            strings(&["package-lock"])
        );
        assert_eq!(
            long_form(&strings(&["resolve", "a.json"])).unwrap(),
            strings(&["package-resolve", "a.json"])
        );
        for malformed in [
            &[][..],
            &["build"][..],
            &["--report"][..],
            &["package-report"][..],
        ] {
            assert!(long_form(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
