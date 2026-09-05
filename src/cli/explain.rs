//! `semaprax explain`: installed, compiler-version-matched diagnostics.

use semaprax::diagnostic::Diagnostic;
use semaprax::installed_diagnostics::explain_installed_diagnostic;

pub(crate) struct ExplainOptions {
    code: String,
    json: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<ExplainOptions, u8> {
    let (code, json) = match args {
        [code] => (code, false),
        [code, option] if option == "--json" => (code, true),
        [_, option] if option.starts_with('-') => {
            eprintln!("unknown explain option `{option}`");
            return Err(2);
        }
        _ => return usage(),
    };
    if code.is_empty() || code.starts_with('-') {
        return usage();
    }
    Ok(ExplainOptions {
        code: code.to_owned(),
        json,
    })
}

pub(crate) fn run(options: ExplainOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let explanation =
        explain_installed_diagnostic(&options.code).map_err(|errors| report(&errors))?;
    if options.json {
        print!("{}", explanation.to_json());
    } else {
        print!("{}", explanation.to_text());
    }
    Ok(())
}

fn usage<T>() -> Result<T, u8> {
    eprintln!("explain requires exactly <SPX-CODE> [--json]");
    Err(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn grammar_is_closed() {
        let text = parse(&strings(&["SPX-T001"])).unwrap();
        assert_eq!(text.code, "SPX-T001");
        assert!(!text.json);
        let json = parse(&strings(&["SPX-G531", "--json"])).unwrap();
        assert_eq!(json.code, "SPX-G531");
        assert!(json.json);

        for malformed in [
            &[][..],
            &[""][..],
            &["--json"][..],
            &["SPX-T001", "--unknown"][..],
            &["SPX-T001", "--json", "extra"][..],
            &["SPX-T001", "SPX-T002"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
