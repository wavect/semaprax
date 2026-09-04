#[path = "new_project/templates.rs"]
mod templates;

#[cfg(test)]
#[path = "new_project/binding_tests.rs"]
mod binding_tests;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;
use semaprax::project::{self, ProjectScaffoldFileV1};
use semaprax_native_rust_owned_data_package::{NewProjectAuthority, NewProjectAuthorityError};

static STAGING_SERIAL: AtomicU64 = AtomicU64::new(0);
const MAX_STAGING_ATTEMPTS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
struct NewProjectOptions {
    destination: PathBuf,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewProjectFailure {
    message: String,
    exit_code: u8,
}

impl NewProjectFailure {
    fn invocation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }

    fn creation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
        }
    }

    pub(crate) const fn exit_code(&self) -> u8 {
        self.exit_code
    }
}

impl fmt::Display for NewProjectFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub(crate) trait WriteHook {
    fn after_stage_created(&self) -> Result<(), String> {
        Ok(())
    }

    fn before_write(&self, index: usize, relative_path: &str) -> Result<(), String>;

    #[cfg(test)]
    fn after_publish(&self) -> Result<(), String> {
        Ok(())
    }
}

struct NoopWriteHook;

impl WriteHook for NoopWriteHook {
    fn before_write(&self, _index: usize, _relative_path: &str) -> Result<(), String> {
        Ok(())
    }
}

pub(crate) fn run(arguments: &[String]) -> Result<PathBuf, NewProjectFailure> {
    let options = parse(arguments)?;
    create_with_hook(&options.destination, &options.name, &NoopWriteHook)
}

pub(crate) fn create_with_hook(
    destination: &Path,
    name: &str,
    hook: &dyn WriteHook,
) -> Result<PathBuf, NewProjectFailure> {
    create_with_serial(destination, name, hook, &mut || {
        STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
    })
}

fn create_with_serial(
    destination: &Path,
    name: &str,
    hook: &dyn WriteHook,
    serial: &mut dyn FnMut() -> u64,
) -> Result<PathBuf, NewProjectFailure> {
    // Capture the caller's spelling once; do not adopt a later alias target.
    let requested_destination = std::path::absolute(destination).map_err(|error| {
        NewProjectFailure::creation(format!("cannot resolve new project destination: {error}"))
    })?;
    validate_name(name).map_err(NewProjectFailure::creation)?;
    let scaffold =
        project::derive_project_scaffold_v1(name, project::PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR)
            .map_err(|diagnostics| {
                NewProjectFailure::creation(format!(
                    "generated calculator project failed exact scaffold derivation: {}",
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ))
            })?;
    let files = scaffold.files();
    let paths = files
        .iter()
        .map(ProjectScaffoldFileV1::path)
        .collect::<Vec<_>>();
    validate_template_inventory(&paths).map_err(NewProjectFailure::creation)?;
    let expected = expected_files(files)?;

    let file_name = requested_destination.file_name().ok_or_else(|| {
        NewProjectFailure::creation("new project destination must name one directory")
    })?;
    let requested_parent = requested_destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(requested_parent).map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot inspect new project parent {}: {error}",
            requested_parent.display()
        ))
    })?;
    if !is_plain_directory(&parent_metadata) {
        return Err(NewProjectFailure::creation(
            "new project parent must be a real non-reparse directory",
        ));
    }
    let parent_identity = Handle::from_path(requested_parent).map_err(|error| {
        NewProjectFailure::creation(format!("cannot identify new project parent: {error}"))
    })?;
    let parent = fs::canonicalize(requested_parent).map_err(|error| {
        NewProjectFailure::creation(format!("cannot canonicalize new project parent: {error}"))
    })?;
    if Handle::from_path(&parent)
        .ok()
        .is_none_or(|observed| observed != parent_identity)
    {
        return Err(NewProjectFailure::creation(
            "new project parent identity changed during canonicalization",
        ));
    }
    let (mut authority, staging) = create_staging_authority(&parent, file_name, serial)?;
    hook.after_stage_created().map_err(|error| {
        NewProjectFailure::creation(format!(
            "injected failure after staged project creation: {error}"
        ))
    })?;
    require_original_parent_identity(requested_parent, &parent_identity)?;
    require_ambient_binding(&authority, &parent, &staging)?;
    for (index, file) in files.iter().enumerate() {
        hook.before_write(index, file.path()).map_err(|error| {
            NewProjectFailure::creation(format!(
                "injected write failure before `{}`: {error}",
                file.path()
            ))
        })?;
        authority
            .write(file.path(), file.bytes())
            .map_err(|error| authority_failure("write staged project", error))?;
    }
    authority
        .authenticate(expected)
        .map_err(|error| authority_failure("authenticate staged project", error))?;
    require_ambient_binding(&authority, &parent, &staging)?;
    authority
        .publish_and_verify(expected)
        .map_err(|error| authority_failure("publish and verify new project", error))?;
    #[cfg(test)]
    hook.after_publish().map_err(|error| {
        NewProjectFailure::creation(format!(
            "injected failure after project publication: {error}"
        ))
    })?;
    require_original_parent_identity(requested_parent, &parent_identity)?;
    require_ambient_binding(&authority, &parent, &parent.join(file_name))?;
    Ok(destination.to_path_buf())
}

pub(crate) fn validate_template_inventory(paths: &[&str]) -> Result<(), String> {
    let mut observed = paths.to_vec();
    observed.sort_unstable();
    let mut expected = project::PROJECT_SCAFFOLD_INVENTORY.to_vec();
    expected.sort_unstable();
    if observed == expected && observed.windows(2).all(|pair| pair[0] != pair[1]) {
        Ok(())
    } else {
        Err("calculator template inventory must contain exactly README.md, AGENTS.md, semaprax.toml, src/app.spx, and src/tests.spx".to_owned())
    }
}

fn parse(arguments: &[String]) -> Result<NewProjectOptions, NewProjectFailure> {
    let mut destination = None::<PathBuf>;
    let mut explicit_name = None::<String>;
    let mut template_seen = false;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--name" if explicit_name.is_none() => {
                let value = option_value(arguments, index, "--name")?;
                explicit_name = Some(value.to_owned());
                index += 2;
            }
            "--name" => {
                return Err(NewProjectFailure::invocation(
                    "duplicate new option `--name`",
                ))
            }
            "--template" if !template_seen => {
                let value = option_value(arguments, index, "--template")?;
                if value != project::PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR {
                    // The held-parent authority publishes the calculator
                    // inventory only; the standalone compiler's `new` creates
                    // the other templates through its bounded route.
                    return Err(NewProjectFailure::invocation(
                        if project::PROJECT_SCAFFOLD_TEMPLATES.contains(&value) {
                            format!(
                                "the full toolchain's new publishes only the calculator template; \
                                 create a `{value}` project with the standalone `semaprax new`"
                            )
                        } else {
                            format!(
                                "unknown new template `{value}`; expected {}",
                                project::PROJECT_SCAFFOLD_TEMPLATES.join(" or ")
                            )
                        },
                    ));
                }
                template_seen = true;
                index += 2;
            }
            "--template" => {
                return Err(NewProjectFailure::invocation(
                    "duplicate new option `--template`",
                ))
            }
            option if option.starts_with('-') => {
                return Err(NewProjectFailure::invocation(format!(
                    "unknown new option `{option}`"
                )))
            }
            path if destination.is_none() => {
                destination = Some(PathBuf::from(path));
                index += 1;
            }
            _ => {
                return Err(NewProjectFailure::invocation(
                    "new accepts exactly one destination",
                ))
            }
        }
    }
    let destination =
        destination.ok_or_else(|| NewProjectFailure::invocation("new requires one destination"))?;
    let name = match explicit_name {
        Some(name) => name,
        None => destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                NewProjectFailure::invocation(
                    "new destination requires --name when its final component is not UTF-8",
                )
            })?
            .to_owned(),
    };
    validate_name(&name).map_err(NewProjectFailure::invocation)?;
    Ok(NewProjectOptions { destination, name })
}

fn option_value<'a>(
    arguments: &'a [String],
    index: usize,
    option: &str,
) -> Result<&'a str, NewProjectFailure> {
    arguments
        .get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .map(String::as_str)
        .ok_or_else(|| {
            NewProjectFailure::invocation(format!("new option `{option}` requires a value"))
        })
}

fn validate_name(name: &str) -> Result<(), String> {
    if (1..=64).contains(&name.len())
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Ok(())
    } else {
        Err("project name must match lowercase [a-z][a-z0-9-]* and be at most 64 bytes".to_owned())
    }
}

fn create_staging_authority(
    parent: &Path,
    output_name: &std::ffi::OsStr,
    serial: &mut dyn FnMut() -> u64,
) -> Result<(NewProjectAuthority, PathBuf), NewProjectFailure> {
    for _ in 0..MAX_STAGING_ATTEMPTS {
        let serial = serial();
        let name = format!(".semaprax-new-{}-{serial}", std::process::id());
        // The generated ASCII staging leaf must not be the final leaf. This
        // conservative comparison is not Unicode filesystem normalization.
        if name
            .as_bytes()
            .eq_ignore_ascii_case(output_name.as_encoded_bytes())
        {
            continue;
        }
        match NewProjectAuthority::create(parent, output_name, std::ffi::OsStr::new(&name)) {
            Ok(authority) => return Ok((authority, parent.join(name))),
            Err(NewProjectAuthorityError::StageExists) => continue,
            Err(error) => return Err(authority_failure("create staged project", error)),
        }
    }
    Err(NewProjectFailure::creation(
        "cannot allocate a fresh same-parent new project staging directory",
    ))
}

fn expected_files(
    files: &[ProjectScaffoldFileV1],
) -> Result<[(&str, &[u8]); project::PROJECT_SCAFFOLD_FILE_COUNT], NewProjectFailure> {
    files
        .iter()
        .map(|file| (file.path(), file.bytes()))
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| NewProjectFailure::creation("calculator template inventory is not exact"))
}

fn require_ambient_binding(
    authority: &NewProjectAuthority,
    parent: &Path,
    staging: &Path,
) -> Result<(), NewProjectFailure> {
    if authority
        .ambient_paths_still_bind(parent, staging)
        .map_err(|error| authority_failure("recheck staged project authority", error))?
    {
        Ok(())
    } else {
        Err(NewProjectFailure::creation(
            "new project parent or staging path identity changed",
        ))
    }
}

fn require_original_parent_identity(
    parent: &Path,
    expected: &Handle,
) -> Result<(), NewProjectFailure> {
    if fs::symlink_metadata(parent)
        .ok()
        .is_some_and(|metadata| is_plain_directory(&metadata))
        && Handle::from_path(parent)
            .ok()
            .is_some_and(|observed| observed == *expected)
    {
        Ok(())
    } else {
        Err(NewProjectFailure::creation(
            "new project parent identity changed",
        ))
    }
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

fn authority_failure(action: &str, error: NewProjectAuthorityError) -> NewProjectFailure {
    let reason = match error {
        NewProjectAuthorityError::Exists => "a no-clobber entry already exists",
        NewProjectAuthorityError::StageExists => "a staging entry already exists",
        NewProjectAuthorityError::Changed => "held filesystem authority changed",
        NewProjectAuthorityError::Invalid => "the requested inventory or name is invalid",
    };
    NewProjectFailure::creation(format!("cannot {action}: {reason}"))
}
