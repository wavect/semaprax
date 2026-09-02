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
    pub(crate) profile: Option<String>,
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

pub(crate) fn bind_rust_output_parent(path: &Path) -> Result<PathBuf, Diagnostic> {
    #[cfg(windows)]
    {
        let name = path.file_name().ok_or_else(|| {
            parent_error("Project v8 Rust output must name one package directory")
        })?;
        let parent = path.parent().ok_or_else(|| {
            parent_error("Project v8 Rust output must have an explicit parent directory")
        })?;
        let canonical_parent = parent.canonicalize().map_err(|error| {
            parent_error(format!(
                "cannot bind Project v8 Rust output parent {}: {error}",
                parent.display()
            ))
        })?;
        Ok(canonical_parent.join(name))
    }
    #[cfg(not(windows))]
    {
        Ok(path.to_path_buf())
    }
}

pub(crate) fn parse(args: &[String]) -> Result<BuildOptions, u8> {
    let mut input = None::<PathBuf>;
    let mut output = None::<PathBuf>;
    let mut manifest_path = None::<PathBuf>;
    let mut target = None::<String>;
    let mut function = None::<String>;
    let mut exports = Vec::<String>::new();
    let mut profile = None::<String>;
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
            "--target"
                | "--function"
                | "--export"
                | "--profile"
                | "--manifest-path"
                | "-o"
                | "--output"
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
                                | "--profile"
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
                "--profile" if profile.is_none() => profile = Some(value.clone()),
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
    if let Some(profile) = &profile {
        if profile != "internal-strings-v1"
            || !matches!(&input, BuildInput::Source(_))
            || !matches!(target.as_str(), "web" | "wasm")
            || !(1..=32).contains(&exports.len())
        {
            eprintln!("--profile internal-strings-v1 requires a source file, --target web or wasm, and 1..=32 --export selections");
            return Err(2);
        }
    }
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
        profile,
    })
}

#[cfg(test)]
#[path = "build/tests.rs"]
mod tests;
