use std::path::PathBuf;

use super::project::{is_project_manifest, resolve_positional, DEFAULT_MANIFEST};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ExecutionInput {
    Source(PathBuf),
    Project(PathBuf),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ExecutionOptions {
    pub(crate) input: ExecutionInput,
    pub(crate) json: bool,
    pub(crate) max_steps: Option<usize>,
    pub(crate) max_bytes: Option<usize>,
    pub(crate) native: bool,
}

pub(crate) fn parse_run(args: &[String]) -> Result<ExecutionOptions, u8> {
    parse(args, "run", true)
}

pub(crate) fn parse_test(args: &[String]) -> Result<ExecutionOptions, u8> {
    parse(args, "test", false)
}

fn parse(args: &[String], command: &str, allow_source: bool) -> Result<ExecutionOptions, u8> {
    let mut positional = None::<PathBuf>;
    let mut manifest = None::<PathBuf>;
    let mut json = false;
    let mut max_steps = None;
    let mut max_bytes = None;
    let mut native = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "--json" if !json => {
                json = true;
                index += 1;
            }
            "--json" => {
                eprintln!("{command} option `--json` may not be repeated");
                return Err(2);
            }
            "--manifest-path" if manifest.is_none() => {
                manifest = Some(PathBuf::from(option_value(args, index, command, argument)?));
                index += 2;
            }
            "--manifest-path" => {
                eprintln!("{command} option `--manifest-path` may not be repeated");
                return Err(2);
            }
            "--max-steps" if max_steps.is_none() => {
                max_steps = Some(positive_number(
                    command,
                    argument,
                    option_value(args, index, command, argument)?,
                )?);
                index += 2;
            }
            "--max-steps" => {
                eprintln!("{command} option `--max-steps` may not be repeated");
                return Err(2);
            }
            "--max-bytes" if max_bytes.is_none() => {
                max_bytes = Some(positive_number(
                    command,
                    argument,
                    option_value(args, index, command, argument)?,
                )?);
                index += 2;
            }
            "--max-bytes" => {
                eprintln!("{command} option `--max-bytes` may not be repeated");
                return Err(2);
            }
            "--native" if allow_source && !native => {
                native = true;
                index += 1;
            }
            "--native" if allow_source => {
                eprintln!("{command} option `--native` may not be repeated");
                return Err(2);
            }
            option if option.starts_with('-') => {
                eprintln!("unknown {command} option `{option}`");
                return Err(2);
            }
            path if positional.is_none() => {
                positional = Some(PathBuf::from(path));
                index += 1;
            }
            _ => {
                eprintln!("{command} accepts at most one input selector");
                return Err(2);
            }
        }
    }
    if positional.is_some() && manifest.is_some() {
        eprintln!("{command} cannot combine an input file with --manifest-path");
        return Err(2);
    }
    let input = match (positional, manifest) {
        (None, None) => ExecutionInput::Project(PathBuf::from(DEFAULT_MANIFEST)),
        (None, Some(path)) => ExecutionInput::Project(path),
        (Some(path), None) => match resolve_positional(path) {
            path if is_project_manifest(&path) => ExecutionInput::Project(path),
            path if allow_source => ExecutionInput::Source(path),
            _ => {
                eprintln!("{command} requires a Project v1 semaprax.toml manifest");
                return Err(2);
            }
        },
        (Some(_), Some(_)) => unreachable!("ambiguity rejected above"),
    };
    if native && !matches!(input, ExecutionInput::Source(_)) {
        eprintln!("run option `--native` requires a single .spx source file");
        return Err(2);
    }
    if native && (json || max_steps.is_some() || max_bytes.is_some()) {
        eprintln!(
            "native single-file run cannot combine `--native` with interpreter output or capacity options"
        );
        return Err(2);
    }
    Ok(ExecutionOptions {
        input,
        json,
        max_steps,
        max_bytes,
        native,
    })
}

fn option_value<'a>(
    args: &'a [String],
    index: usize,
    command: &str,
    option: &str,
) -> Result<&'a str, u8> {
    args.get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .map(String::as_str)
        .ok_or_else(|| {
            eprintln!("{command} option `{option}` requires a value");
            2
        })
}

fn positive_number(command: &str, option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || value == "0"
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        eprintln!("{command} option `{option}` requires a canonical positive integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("{command} option `{option}` requires a canonical positive integer");
        2
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn run_preserves_legacy_source_and_selects_projects_explicitly() {
        assert_eq!(
            parse_run(&strings(&["legacy.spx"])).unwrap(),
            ExecutionOptions {
                input: ExecutionInput::Source(PathBuf::from("legacy.spx")),
                json: false,
                max_steps: None,
                max_bytes: None,
                native: false,
            }
        );
        assert_eq!(
            parse_run(&[]).unwrap(),
            ExecutionOptions {
                input: ExecutionInput::Project(PathBuf::from(DEFAULT_MANIFEST)),
                json: false,
                max_steps: None,
                max_bytes: None,
                native: false,
            }
        );
        assert_eq!(
            parse_run(&strings(&[
                "--manifest-path",
                "fixtures/semaprax.toml",
                "--json",
                "--max-steps",
                "4096",
                "--max-bytes",
                "65536",
            ]))
            .unwrap(),
            ExecutionOptions {
                input: ExecutionInput::Project(PathBuf::from("fixtures/semaprax.toml")),
                json: true,
                max_steps: Some(4096),
                max_bytes: Some(65536),
                native: false,
            }
        );
        assert_eq!(
            parse_run(&strings(&[
                "legacy.spx",
                "--json",
                "--max-steps",
                "4096",
                "--max-bytes",
                "65536",
            ]))
            .unwrap(),
            ExecutionOptions {
                input: ExecutionInput::Source(PathBuf::from("legacy.spx")),
                json: true,
                max_steps: Some(4096),
                max_bytes: Some(65536),
                native: false,
            }
        );
        assert!(parse_run(&strings(&["legacy.spx", "--native", "--json"])).is_err());
    }

    #[test]
    fn test_accepts_only_default_or_explicit_project_manifests() {
        assert_eq!(
            parse_test(&strings(&["fixtures/semaprax.toml", "--max-steps", "1",])).unwrap(),
            ExecutionOptions {
                input: ExecutionInput::Project(PathBuf::from("fixtures/semaprax.toml")),
                json: false,
                max_steps: Some(1),
                native: false,
                max_bytes: None,
            }
        );
        assert!(parse_test(&strings(&["legacy.spx"])).is_err());
        let directory = std::env::temp_dir().join(format!(
            "semaprax-test-directory-operand-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let operand = directory.to_string_lossy().into_owned();
        let expected = ExecutionInput::Project(directory.join(DEFAULT_MANIFEST));
        assert_eq!(
            parse_test(std::slice::from_ref(&operand)).unwrap().input,
            expected
        );
        assert_eq!(parse_run(&[operand]).unwrap().input, expected);
        std::fs::remove_dir(&directory).unwrap();
        assert!(parse_test(&strings(&["--max-bytes", "0"])).is_err());
        assert!(parse_test(&strings(&["--max-steps", "01"])).is_err());
        assert!(parse_test(&strings(&[
            DEFAULT_MANIFEST,
            "--manifest-path",
            "fixtures/semaprax.toml",
        ]))
        .is_err());
    }
}
