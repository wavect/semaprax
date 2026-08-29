#[path = "new_project/templates.rs"]
mod templates;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;
use semaprax::project::{self, ProjectExecutionOptions};
use semaprax_native_rust_owned_data_package::{NewProjectAuthority, NewProjectAuthorityError};

use templates::TemplateFile;

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
    validate_name(name).map_err(NewProjectFailure::creation)?;
    let files = templates::render(name);
    let paths = files.iter().map(|file| file.path).collect::<Vec<_>>();
    validate_template_inventory(&paths).map_err(NewProjectFailure::creation)?;

    let file_name = destination.file_name().ok_or_else(|| {
        NewProjectFailure::creation("new project destination must name one directory")
    })?;
    let requested_parent = destination
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
    let (mut authority, staging) = create_staging_authority(&parent, file_name)?;
    hook.after_stage_created().map_err(|error| {
        NewProjectFailure::creation(format!(
            "injected failure after staged project creation: {error}"
        ))
    })?;
    require_original_parent_identity(&parent, &parent_identity)?;
    require_ambient_binding(&authority, &parent, &staging)?;
    for (index, file) in files.iter().enumerate() {
        hook.before_write(index, file.path).map_err(|error| {
            NewProjectFailure::creation(format!(
                "injected write failure before `{}`: {error}",
                file.path
            ))
        })?;
        authority
            .write(file.path, &file.bytes)
            .map_err(|error| authority_failure("write staged project", error))?;
    }
    let expected = expected_files(&files)?;
    authority
        .authenticate(expected)
        .map_err(|error| authority_failure("authenticate staged project", error))?;
    require_ambient_binding(&authority, &parent, &staging)?;

    let manifest = staging.join("semaprax.toml");
    project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let execution = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        if execution.command_succeeded() {
            Ok(())
        } else {
            Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-I001",
                "generated calculator project tests did not pass",
            )])
        }
    })
    .map_err(|diagnostics| {
        NewProjectFailure::creation(format!(
            "generated project failed authentication, check, or test: {}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        ))
    })?;

    require_ambient_binding(&authority, &parent, &staging)?;
    authority
        .publish_and_verify(expected)
        .map_err(|error| authority_failure("publish and verify new project", error))?;
    Ok(destination.to_path_buf())
}

pub(crate) fn validate_template_inventory(paths: &[&str]) -> Result<(), String> {
    let mut observed = paths.to_vec();
    observed.sort_unstable();
    let mut expected = templates::INVENTORY.to_vec();
    expected.sort_unstable();
    if observed == expected && observed.windows(2).all(|pair| pair[0] != pair[1]) {
        Ok(())
    } else {
        Err("calculator template inventory must contain exactly README.md, semaprax.toml, src/app.spx, and src/tests.spx".to_owned())
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
                if value != templates::TEMPLATE_NAME {
                    return Err(NewProjectFailure::invocation(format!(
                        "unknown new template `{value}`; expected calculator"
                    )));
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
) -> Result<(NewProjectAuthority, PathBuf), NewProjectFailure> {
    for _ in 0..MAX_STAGING_ATTEMPTS {
        let serial = STAGING_SERIAL.fetch_add(1, Ordering::Relaxed);
        let name = format!(".semaprax-new-{}-{serial}", std::process::id());
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

fn expected_files(files: &[TemplateFile]) -> Result<[(&str, &[u8]); 4], NewProjectFailure> {
    files
        .iter()
        .map(|file| (file.path, file.bytes.as_slice()))
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
    if Handle::from_path(parent)
        .ok()
        .is_some_and(|observed| observed == *expected)
    {
        Ok(())
    } else {
        Err(NewProjectFailure::creation(
            "new project parent identity changed before staged writes",
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
