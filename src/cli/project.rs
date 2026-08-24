use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_MANIFEST: &str = "semaprax.toml";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CheckOptions {
    pub(crate) manifest_path: PathBuf,
    pub(crate) json: bool,
}

pub(crate) fn parse_check_options(args: &[String]) -> Result<Option<CheckOptions>, u8> {
    let mut positional = None::<PathBuf>;
    let mut manifest = None::<PathBuf>;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" if !json => {
                json = true;
                index += 1;
            }
            "--json" => {
                eprintln!("check option `--json` may not be repeated");
                return Err(2);
            }
            "--manifest-path" if manifest.is_none() => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| {
                        eprintln!("check option `--manifest-path` requires a value");
                        2
                    })?;
                manifest = Some(PathBuf::from(value));
                index += 2;
            }
            "--manifest-path" => {
                eprintln!("check option `--manifest-path` may not be repeated");
                return Err(2);
            }
            option if option.starts_with('-') => {
                eprintln!("unknown check option `{option}`");
                return Err(2);
            }
            path if positional.is_none() => {
                positional = Some(PathBuf::from(path));
                index += 1;
            }
            _ => {
                eprintln!("check accepts at most one input selector");
                return Err(2);
            }
        }
    }
    if positional.is_some() && manifest.is_some() {
        eprintln!("check cannot combine an input file with --manifest-path");
        return Err(2);
    }
    match (positional, manifest) {
        (None, None) => Ok(Some(CheckOptions {
            manifest_path: PathBuf::from(DEFAULT_MANIFEST),
            json,
        })),
        (None, Some(path)) => Ok(Some(CheckOptions {
            manifest_path: path,
            json,
        })),
        (Some(path), None) if is_project_manifest(&path) => Ok(Some(CheckOptions {
            manifest_path: path,
            json,
        })),
        (Some(_), None) => Ok(None),
        (Some(_), Some(_)) => unreachable!("ambiguity rejected above"),
    }
}

pub(crate) fn is_project_manifest(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(DEFAULT_MANIFEST)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn project_check_selectors_preserve_legacy_source_detection() {
        assert_eq!(
            parse_check_options(&[]).unwrap(),
            Some(CheckOptions {
                manifest_path: PathBuf::from(DEFAULT_MANIFEST),
                json: false,
            })
        );
        assert_eq!(
            parse_check_options(&strings(&[
                "--manifest-path",
                "fixtures/semaprax.toml",
                "--json",
            ]))
            .unwrap(),
            Some(CheckOptions {
                manifest_path: PathBuf::from("fixtures/semaprax.toml"),
                json: true,
            })
        );
        assert_eq!(
            parse_check_options(&strings(&[
                "--json",
                "--manifest-path",
                "fixtures/semaprax.toml",
            ]))
            .unwrap(),
            Some(CheckOptions {
                manifest_path: PathBuf::from("fixtures/semaprax.toml"),
                json: true,
            })
        );
        assert_eq!(
            parse_check_options(&strings(&["--json"])).unwrap(),
            Some(CheckOptions {
                manifest_path: PathBuf::from(DEFAULT_MANIFEST),
                json: true,
            })
        );
        assert_eq!(
            parse_check_options(&strings(&["legacy.spx", "--json"])).unwrap(),
            None
        );
        assert!(parse_check_options(&strings(&[
            "legacy.spx",
            "--manifest-path",
            DEFAULT_MANIFEST,
        ]))
        .is_err());
        assert!(parse_check_options(&strings(&["--json", "--json"])).is_err());
        assert!(parse_check_options(&strings(&["legacy.spx", "--unknown"])).is_err());
    }
}
