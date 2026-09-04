use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_MANIFEST: &str = "semaprax.toml";

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CheckInput {
    Source(PathBuf),
    Project(PathBuf),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CheckOptions {
    pub(crate) input: CheckInput,
    pub(crate) json: bool,
}

pub(crate) fn parse_check_options(args: &[String]) -> Result<CheckOptions, u8> {
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
    let input = match (positional, manifest) {
        (None, None) => CheckInput::Project(PathBuf::from(DEFAULT_MANIFEST)),
        (None, Some(path)) => CheckInput::Project(path),
        (Some(path), None) => match resolve_positional(path) {
            path if is_project_manifest(&path) => CheckInput::Project(path),
            path => CheckInput::Source(path),
        },
        (Some(_), Some(_)) => unreachable!("ambiguity rejected above"),
    };
    Ok(CheckOptions { input, json })
}

pub(crate) fn is_project_manifest(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(DEFAULT_MANIFEST)
}

/// A positional operand that names an existing directory selects the
/// `semaprax.toml` inside it, so `semaprax check my-project` means the same as
/// `semaprax check my-project/semaprax.toml`. Only the positional operand is
/// resolved; `--manifest-path` stays exact, and a missing manifest surfaces as
/// the ordinary `SPX-J102` manifest diagnostic rather than an unreadable
/// directory.
pub(crate) fn resolve_positional(path: PathBuf) -> PathBuf {
    if !path.is_dir() {
        return path;
    }
    // `.` components are inert, and the manifest authenticator rejects them,
    // so `semaprax check .` selects plain `semaprax.toml`.
    let mut manifest: PathBuf = path
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect();
    manifest.push(DEFAULT_MANIFEST);
    manifest
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
            CheckOptions {
                input: CheckInput::Project(PathBuf::from(DEFAULT_MANIFEST)),
                json: false,
            }
        );
        assert_eq!(
            parse_check_options(&strings(&[
                "--manifest-path",
                "fixtures/semaprax.toml",
                "--json",
            ]))
            .unwrap(),
            CheckOptions {
                input: CheckInput::Project(PathBuf::from("fixtures/semaprax.toml")),
                json: true,
            }
        );
        assert_eq!(
            parse_check_options(&strings(&[
                "--json",
                "--manifest-path",
                "fixtures/semaprax.toml",
            ]))
            .unwrap(),
            CheckOptions {
                input: CheckInput::Project(PathBuf::from("fixtures/semaprax.toml")),
                json: true,
            }
        );
        assert_eq!(
            parse_check_options(&strings(&["--json"])).unwrap(),
            CheckOptions {
                input: CheckInput::Project(PathBuf::from(DEFAULT_MANIFEST)),
                json: true,
            }
        );
        assert_eq!(
            parse_check_options(&strings(&["legacy.spx", "--json"])).unwrap(),
            CheckOptions {
                input: CheckInput::Source(PathBuf::from("legacy.spx")),
                json: true,
            }
        );
        assert_eq!(
            parse_check_options(&strings(&["--json", "legacy.spx"])).unwrap(),
            parse_check_options(&strings(&["legacy.spx", "--json"])).unwrap()
        );
        assert_eq!(
            parse_check_options(&strings(&["legacy.spx"])).unwrap(),
            CheckOptions {
                input: CheckInput::Source(PathBuf::from("legacy.spx")),
                json: false,
            }
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

    #[test]
    fn directory_operand_selects_the_manifest_inside_it() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-check-directory-operand-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let expected = directory.join(DEFAULT_MANIFEST);
        assert_eq!(
            parse_check_options(&[directory.to_string_lossy().into_owned()]).unwrap(),
            CheckOptions {
                input: CheckInput::Project(expected.clone()),
                json: false,
            }
        );
        // A missing path is still a source operand; nothing is probed further.
        let missing = directory.join("missing.spx");
        assert_eq!(
            parse_check_options(&[missing.to_string_lossy().into_owned()])
                .unwrap()
                .input,
            CheckInput::Source(missing)
        );
        assert_eq!(resolve_positional(expected.clone()), expected);
        assert_eq!(
            resolve_positional(PathBuf::from(".")),
            PathBuf::from(DEFAULT_MANIFEST)
        );
        let dotted = Path::new(".").join(&directory);
        assert_eq!(resolve_positional(dotted), expected);
        std::fs::remove_dir(&directory).unwrap();
    }
}
