use std::fs;
use std::path::{Component, Path, PathBuf};

use same_file::Handle;
use semaprax::diagnostic::Diagnostic;

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

pub(crate) trait ProjectBuildParentHook {
    fn before_create(&self, grandparent: &Path, parent: &Path) -> Result<(), String>;
}

struct NoopProjectBuildParentHook;

impl ProjectBuildParentHook for NoopProjectBuildParentHook {
    fn before_create(&self, _grandparent: &Path, _parent: &Path) -> Result<(), String> {
        Ok(())
    }
}

pub(crate) struct ProjectOutputParent {
    created: Option<CreatedProjectOutputParent>,
    retain: bool,
}

struct CreatedProjectOutputParent {
    grandparent: PathBuf,
    grandparent_identity: Handle,
    parent: PathBuf,
    parent_identity: Handle,
}

impl ProjectOutputParent {
    pub(crate) fn prepare(output: &Path) -> Result<Self, Diagnostic> {
        Self::prepare_with_hook(output, &NoopProjectBuildParentHook)
    }

    pub(crate) fn prepare_with_hook(
        output: &Path,
        hook: &dyn ProjectBuildParentHook,
    ) -> Result<Self, Diagnostic> {
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        match fs::symlink_metadata(parent) {
            Ok(metadata) => {
                if !is_plain_directory(&metadata) {
                    return Err(parent_error(
                        "explicit Project output parent must be a real non-reparse directory",
                    ));
                }
                return Ok(Self {
                    created: None,
                    retain: true,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(parent_error(format!(
                    "cannot inspect explicit Project output parent {}: {error}",
                    parent.display()
                )))
            }
        }

        let grandparent = parent
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let metadata = fs::symlink_metadata(grandparent).map_err(|error| {
            parent_error(format!(
                "explicit Project output may create only one missing parent; cannot inspect grandparent {}: {error}",
                grandparent.display()
            ))
        })?;
        if !is_plain_directory(&metadata) {
            return Err(parent_error(
                "explicit Project output grandparent must be a real non-reparse directory",
            ));
        }
        let grandparent_identity = Handle::from_path(grandparent).map_err(|error| {
            parent_error(format!(
                "cannot identify explicit Project output grandparent: {error}"
            ))
        })?;
        hook.before_create(grandparent, parent).map_err(|error| {
            parent_error(format!(
                "explicit Project output parent creation was interrupted before effects: {error}"
            ))
        })?;
        authenticate_directory(
            grandparent,
            &grandparent_identity,
            "explicit Project output grandparent changed before parent creation",
        )?;

        fs::create_dir(parent).map_err(|error| {
            parent_error(format!(
                "cannot create the single missing explicit Project output parent {}: {error}",
                parent.display()
            ))
        })?;
        let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
            parent_error(format!(
                "cannot inspect created explicit Project output parent: {error}"
            ))
        })?;
        if !is_plain_directory(&parent_metadata) {
            return Err(parent_error(
                "created explicit Project output parent is not a real non-reparse directory",
            ));
        }
        let parent_identity = Handle::from_path(parent).map_err(|error| {
            parent_error(format!(
                "cannot identify created explicit Project output parent: {error}"
            ))
        })?;
        let lease = Self {
            created: Some(CreatedProjectOutputParent {
                grandparent: grandparent.to_path_buf(),
                grandparent_identity,
                parent: parent.to_path_buf(),
                parent_identity,
            }),
            retain: false,
        };
        let created = lease.created.as_ref().expect("created parent lease");
        authenticate_directory(
            &created.grandparent,
            &created.grandparent_identity,
            "explicit Project output grandparent changed after parent creation",
        )?;
        authenticate_directory(
            &created.parent,
            &created.parent_identity,
            "created explicit Project output parent identity changed",
        )?;
        Ok(lease)
    }

    pub(crate) fn retain(&mut self) -> Result<(), Diagnostic> {
        if let Some(created) = &self.created {
            authenticate_directory(
                &created.grandparent,
                &created.grandparent_identity,
                "explicit Project output grandparent changed after child publication",
            )?;
            authenticate_directory(
                &created.parent,
                &created.parent_identity,
                "created explicit Project output parent changed after child publication",
            )?;
        }
        self.retain = true;
        Ok(())
    }
}

impl Drop for ProjectOutputParent {
    fn drop(&mut self) {
        let Some(created) = &self.created else {
            return;
        };
        if self.retain
            || !same_plain_directory(&created.grandparent, &created.grandparent_identity)
            || !same_plain_directory(&created.parent, &created.parent_identity)
            || !directory_is_empty(&created.parent)
            || !same_plain_directory(&created.grandparent, &created.grandparent_identity)
            || !same_plain_directory(&created.parent, &created.parent_identity)
        {
            return;
        }
        let _ = fs::remove_dir(&created.parent);
    }
}

fn directory_is_empty(path: &Path) -> bool {
    fs::read_dir(path)
        .ok()
        .is_some_and(|mut entries| entries.next().is_none())
}

fn authenticate_directory(path: &Path, expected: &Handle, message: &str) -> Result<(), Diagnostic> {
    if same_plain_directory(path, expected) {
        Ok(())
    } else {
        Err(parent_error(message))
    }
}

fn same_plain_directory(path: &Path, expected: &Handle) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| is_plain_directory(&metadata))
        && Handle::from_path(path)
            .ok()
            .is_some_and(|identity| identity == *expected)
}

fn is_plain_directory(metadata: &fs::Metadata) -> bool {
    metadata.is_dir() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn parent_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-I301", message)
}

pub(crate) fn absolute_rust_output(path: &Path) -> Result<PathBuf, Diagnostic> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                parent_error(format!(
                    "cannot resolve Project v8 Rust output {}: {error}",
                    path.display()
                ))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(parent_error(
                    "Project v8 Rust output may not contain parent traversal",
                ))
            }
        }
    }
    if normalized.file_name().is_none() {
        return Err(parent_error(
            "Project v8 Rust output must name one package directory",
        ));
    }
    Ok(normalized)
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
    let project_rust = matches!(&input, BuildInput::Project(_)) && target == "rust";
    if !project_rust
        && !matches!(
            target.as_str(),
            "native" | "native-callable" | "web" | "wasm" | "npm"
        )
    {
        if matches!(&input, BuildInput::Project(_)) {
            eprintln!("unsupported target `{target}`; available: native, web, wasm, npm, rust");
        } else {
            eprintln!(
                "unsupported target `{target}`; available: native, native-callable, web, wasm, npm"
            );
        }
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
        if !matches!(target.as_str(), "web" | "wasm" | "native" | "npm" | "rust") {
            eprintln!(
                "Project manifests publish only explicit web, native, npm, and Project-v8 rust targets; native-callable publication remains held"
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

        let rust = parse(&strings(&[
            "--manifest-path",
            "fixtures/semaprax.toml",
            "--target",
            "rust",
            "-o",
            "sdk",
        ]))
        .unwrap();
        assert_eq!(rust.target, "rust");
        assert_eq!(rust.output, Some(PathBuf::from("sdk")));
        assert!(matches!(rust.input, BuildInput::Project(_)));
        assert!(parse(&strings(&["app.spx", "--target", "rust"])).is_err());
        assert!(parse(&strings(&[
            "--manifest-path",
            DEFAULT_MANIFEST,
            "--target",
            "rust",
            "--export",
            "app.main",
        ]))
        .is_err());
        let normalized = absolute_rust_output(Path::new("dist/rust")).unwrap();
        assert!(normalized.is_absolute());
        assert!(normalized.ends_with(Path::new("dist/rust")));
        assert!(absolute_rust_output(Path::new("dist/../rust")).is_err());
    }
}
