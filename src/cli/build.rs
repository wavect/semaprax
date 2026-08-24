use std::path::PathBuf;

use super::project::{is_project_manifest, DEFAULT_MANIFEST};

pub(crate) struct BuildOptions {
    pub(crate) input: BuildInput,
    pub(crate) output: Option<PathBuf>,
    pub(crate) target: String,
    pub(crate) function: Option<String>,
    pub(crate) exports: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BuildInput {
    Source(PathBuf),
    Project(PathBuf),
}

pub(crate) fn parse(args: &[String]) -> Result<BuildOptions, u8> {
    let mut input = None::<PathBuf>;
    let mut output = None::<PathBuf>;
    let mut manifest_path = None::<PathBuf>;
    let mut target = None::<String>;
    let mut function = None::<String>;
    let mut exports = Vec::<String>::new();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if let Some(value) = argument.strip_prefix("--export=") {
            if value.is_empty() {
                eprintln!("build option `--export` requires a value");
                return Err(2);
            }
            exports.push(value.to_owned());
            index += 1;
            continue;
        }
        if matches!(
            argument.as_str(),
            "--target" | "--function" | "--export" | "--manifest-path" | "-o" | "--output"
        ) {
            let value = args
                .get(index + 1)
                .filter(|value| {
                    argument == "--export"
                        && !matches!(
                            value.as_str(),
                            "--target"
                                | "--function"
                                | "--export"
                                | "--manifest-path"
                                | "-o"
                                | "--output"
                        )
                        || !value.starts_with('-')
                })
                .ok_or_else(|| {
                    eprintln!("build option `{argument}` requires a value");
                    2
                })?;
            match argument.as_str() {
                "--target" if target.is_none() => target = Some(value.clone()),
                "--function" if function.is_none() => function = Some(value.clone()),
                "--export" => exports.push(value.clone()),
                "--manifest-path" if manifest_path.is_none() => {
                    manifest_path = Some(PathBuf::from(value));
                }
                "-o" | "--output" if output.is_none() => output = Some(PathBuf::from(value)),
                _ => {
                    eprintln!("build option `{argument}` may not be repeated");
                    return Err(2);
                }
            }
            index += 2;
            continue;
        }
        if argument.starts_with('-') {
            eprintln!("unknown build option `{argument}`");
            return Err(2);
        }
        if input.replace(PathBuf::from(argument)).is_some() {
            eprintln!("build requires exactly one input file");
            return Err(2);
        }
        index += 1;
    }
    if input.is_some() && manifest_path.is_some() {
        eprintln!("build cannot combine an input file with --manifest-path");
        return Err(2);
    }
    let input = match (input, manifest_path) {
        (Some(path), None) if is_project_manifest(&path) => BuildInput::Project(path),
        (Some(path), None) => BuildInput::Source(path),
        (None, Some(path)) => BuildInput::Project(path),
        (None, None) => BuildInput::Project(PathBuf::from(DEFAULT_MANIFEST)),
        (Some(_), Some(_)) => unreachable!("ambiguity rejected above"),
    };
    let target = target.unwrap_or_else(|| {
        if matches!(&input, BuildInput::Project(_)) {
            "web".to_owned()
        } else {
            "native".to_owned()
        }
    });
    if !matches!(
        target.as_str(),
        "native" | "native-callable" | "web" | "wasm" | "npm"
    ) {
        eprintln!(
            "unsupported target `{target}`; available: native, native-callable, web, wasm, npm"
        );
        return Err(2);
    }
    if target == "native-callable" {
        if function.is_none() {
            eprintln!("native-callable target requires --function <stable-id>");
            return Err(2);
        }
    } else if function.is_some() {
        eprintln!("--function is only valid with --target native-callable");
        return Err(2);
    }
    if !exports.is_empty() && !matches!(target.as_str(), "web" | "wasm") {
        eprintln!("--export is only valid with --target web or wasm");
        return Err(2);
    }
    if matches!(&input, BuildInput::Source(_)) && target == "npm" {
        eprintln!("npm is only valid with an authenticated Project v2 manifest");
        return Err(2);
    }
    if matches!(&input, BuildInput::Project(_)) {
        if !matches!(target.as_str(), "web" | "wasm" | "native" | "npm") {
            eprintln!(
                "Project v1 publishes only explicit web and native targets; native-callable publication remains held"
            );
            return Err(2);
        }
        if function.is_some() || !exports.is_empty() {
            eprintln!(
                "Project v1 takes its entry and web exports only from the authenticated manifest"
            );
            return Err(2);
        }
    }
    let output = match &input {
        BuildInput::Source(path) => Some(output.unwrap_or_else(|| path.with_extension("out"))),
        BuildInput::Project(_) => output,
    };
    Ok(BuildOptions {
        input,
        output,
        target,
        function,
        exports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn repeated_scalar_exports_preserve_caller_order() {
        let options = parse(&strings(&[
            "calculator.spx",
            "--target",
            "web",
            "--export",
            "calculator.subtract",
            "--export",
            "calculator.add",
            "-o",
            "site",
        ]))
        .unwrap();
        assert_eq!(
            options.input,
            BuildInput::Source(PathBuf::from("calculator.spx"))
        );
        assert_eq!(options.output, Some(PathBuf::from("site")));
        assert_eq!(
            options.exports,
            strings(&["calculator.subtract", "calculator.add"])
        );

        let hyphenated = parse(&strings(&[
            "calculator.spx",
            "--target",
            "web",
            "--export",
            "-x",
            "--export=--target",
        ]))
        .unwrap();
        assert_eq!(hyphenated.exports, strings(&["-x", "--target"]));
    }

    #[test]
    fn rejects_unknown_repeated_and_cross_target_flags() {
        assert!(parse(&strings(&["app.spx", "--unknown", "x"])).is_err());
        assert!(parse(&strings(&[
            "app.spx", "--target", "web", "--target", "wasm",
        ]))
        .is_err());
        assert!(parse(&strings(&[
            "app.spx", "--target", "native", "--export", "app.main",
        ]))
        .is_err());
    }

    #[test]
    fn project_selectors_do_not_confuse_legacy_sources() {
        let implicit = parse(&[]).unwrap();
        assert_eq!(
            implicit.input,
            BuildInput::Project(PathBuf::from(DEFAULT_MANIFEST))
        );
        assert_eq!(implicit.target, "web");
        assert_eq!(implicit.output, None);

        let explicit = parse(&strings(&[
            "--manifest-path",
            "fixtures/semaprax.toml",
            "--target",
            "web",
            "-o",
            "site",
        ]))
        .unwrap();
        assert_eq!(
            explicit.input,
            BuildInput::Project(PathBuf::from("fixtures/semaprax.toml"))
        );
        assert_eq!(explicit.output, Some(PathBuf::from("site")));
        assert!(parse(&strings(&["app.spx", "--manifest-path", DEFAULT_MANIFEST,])).is_err());
        assert!(parse(&strings(&[
            DEFAULT_MANIFEST,
            "--target",
            "web",
            "--export",
            "app.main",
        ]))
        .is_err());

        let npm = parse(&strings(&[
            "--manifest-path",
            "fixtures/semaprax.toml",
            "--target",
            "npm",
            "-o",
            "package",
        ]))
        .unwrap();
        assert_eq!(npm.target, "npm");
        assert_eq!(npm.output, Some(PathBuf::from("package")));
        assert!(matches!(npm.input, BuildInput::Project(_)));
        assert!(parse(&strings(&["app.spx", "--target", "npm"])).is_err());
        assert!(parse(&strings(&[
            "--manifest-path",
            DEFAULT_MANIFEST,
            "--target",
            "npm",
            "--export",
            "app.main",
        ]))
        .is_err());
    }
}
