use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FmtOptions {
    pub(crate) path: PathBuf,
    pub(crate) check: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<FmtOptions, u8> {
    let (path, check) = match args {
        [path] if !path.starts_with('-') => (path, false),
        [path, option] if !path.starts_with('-') && option == "--check" => (path, true),
        [option, ..] if option.starts_with('-') => {
            eprintln!("unknown fmt option `{option}`");
            return Err(2);
        }
        _ => {
            eprintln!("fmt requires exactly <file> [--check]");
            return Err(2);
        }
    };
    Ok(FmtOptions {
        path: PathBuf::from(path),
        check,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn formatter_grammar_is_closed() {
        assert_eq!(
            parse(&strings(&["source.spx"])).unwrap(),
            FmtOptions {
                path: PathBuf::from("source.spx"),
                check: false,
            }
        );
        assert_eq!(
            parse(&strings(&["source.spx", "--check"])).unwrap(),
            FmtOptions {
                path: PathBuf::from("source.spx"),
                check: true,
            }
        );
        for malformed in [
            &[][..],
            &["--check"][..],
            &["--unknown"][..],
            &["source.spx", "extra"][..],
            &["source.spx", "--unknown"][..],
            &["source.spx", "--check", "--check"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
