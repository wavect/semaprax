use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;

pub(crate) fn project_output(path: &Path) -> Result<Option<String>, Vec<Diagnostic>> {
    if !super::project::is_project_manifest(path) {
        return Ok(None);
    }
    semaprax::project::with_authenticated_project(path, |snapshot| {
        Ok(Some(snapshot.semantic_graph().to_owned()))
    })
}

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
