use std::path::PathBuf;

pub(crate) fn parse(args: &[String]) -> Result<PathBuf, u8> {
    match args {
        [path] if !path.starts_with('-') => Ok(PathBuf::from(path)),
        [option, ..] if option.starts_with('-') => {
            eprintln!("unknown graph option `{option}`");
            Err(2)
        }
        _ => {
            eprintln!("graph requires exactly <file>");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn graph_grammar_is_closed() {
        assert_eq!(
            parse(&strings(&["source.spx"])).unwrap(),
            PathBuf::from("source.spx")
        );
        for malformed in [
            &[][..],
            &["--unknown"][..],
            &["source.spx", "extra"][..],
            &["source.spx", "--unknown"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
