#[path = "new_project/templates.rs"]
mod templates;

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;
use semaprax::project::{self, ProjectExecutionOptions};

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
    let final_path = parent.join(file_name);
    require_absent(&final_path)?;

    let staging = create_staging_directory(&parent)?;
    let staging_metadata = fs::symlink_metadata(&staging).map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot inspect new project staging directory: {error}"
        ))
    })?;
    if !is_plain_directory(&staging_metadata) {
        return Err(NewProjectFailure::creation(
            "new project staging path is not a real non-reparse directory",
        ));
    }
    let staging_identity = Handle::from_path(&staging).map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot identify new project staging directory: {error}"
        ))
    })?;
    let mut guard = StagingGuard {
        parent: parent.clone(),
        parent_identity,
        staging: staging.clone(),
        staging_identity,
        files: &files,
        armed: true,
    };

    fs::create_dir(staging.join("src")).map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot create staged project source directory: {error}"
        ))
    })?;
    for (index, file) in files.iter().enumerate() {
        hook.before_write(index, file.path).map_err(|error| {
            NewProjectFailure::creation(format!(
                "injected write failure before `{}`: {error}",
                file.path
            ))
        })?;
        write_create_new(&staging, file)?;
    }
    authenticate_complete(&staging, &guard.staging_identity, &files)?;

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

    authenticate_parent(&parent, &guard.parent_identity)?;
    authenticate_complete(&staging, &guard.staging_identity, &files)?;
    require_absent(&final_path)?;
    publish_no_replace(&staging, &final_path).map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot publish new project `{}`: {error}",
            file_name.to_string_lossy()
        ))
    })?;
    guard.armed = false;

    authenticate_parent(&parent, &guard.parent_identity)?;
    authenticate_complete(&final_path, &guard.staging_identity, &files)?;
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

fn create_staging_directory(parent: &Path) -> Result<PathBuf, NewProjectFailure> {
    for _ in 0..MAX_STAGING_ATTEMPTS {
        let serial = STAGING_SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".semaprax-new-{}-{serial}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(NewProjectFailure::creation(format!(
                    "cannot create same-parent new project staging directory: {error}"
                )))
            }
        }
    }
    Err(NewProjectFailure::creation(
        "cannot allocate a fresh same-parent new project staging directory",
    ))
}

fn write_create_new(root: &Path, file: &TemplateFile) -> Result<(), NewProjectFailure> {
    let path = root.join(file.path);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            NewProjectFailure::creation(format!(
                "cannot create staged project file `{}`: {error}",
                file.path
            ))
        })?;
    output.write_all(&file.bytes).map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot write staged project file `{}`: {error}",
            file.path
        ))
    })?;
    output.sync_all().map_err(|error| {
        NewProjectFailure::creation(format!(
            "cannot sync staged project file `{}`: {error}",
            file.path
        ))
    })
}

fn require_absent(path: &Path) -> Result<(), NewProjectFailure> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(NewProjectFailure::creation(format!(
            "new project destination already exists: {}",
            path.display()
        ))),
        Err(error) => Err(NewProjectFailure::creation(format!(
            "cannot inspect new project destination {}: {error}",
            path.display()
        ))),
    }
}

fn authenticate_parent(parent: &Path, expected: &Handle) -> Result<(), NewProjectFailure> {
    let metadata = fs::symlink_metadata(parent).map_err(|error| {
        NewProjectFailure::creation(format!("cannot recheck new project parent: {error}"))
    })?;
    if !is_plain_directory(&metadata)
        || Handle::from_path(parent)
            .ok()
            .is_none_or(|observed| observed != *expected)
    {
        Err(NewProjectFailure::creation(
            "new project parent identity changed during generation",
        ))
    } else {
        Ok(())
    }
}

fn authenticate_complete(
    root: &Path,
    expected_identity: &Handle,
    files: &[TemplateFile],
) -> Result<(), NewProjectFailure> {
    if !same_plain_directory(root, expected_identity) {
        return Err(NewProjectFailure::creation(
            "staged project directory identity changed",
        ));
    }
    if directory_names(root)? != ["README.md", "semaprax.toml", "src"]
        || directory_names(&root.join("src"))? != ["app.spx", "tests.spx"]
    {
        return Err(NewProjectFailure::creation(
            "staged project inventory changed",
        ));
    }
    for file in files {
        let path = root.join(file.path);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            NewProjectFailure::creation(format!(
                "cannot authenticate staged project file `{}`: {error}",
                file.path
            ))
        })?;
        if !is_plain_regular_file(&metadata)
            || fs::read(&path).ok().as_deref() != Some(file.bytes.as_slice())
        {
            return Err(NewProjectFailure::creation(format!(
                "staged project file `{}` changed",
                file.path
            )));
        }
    }
    Ok(())
}

fn directory_names(path: &Path) -> Result<Vec<String>, NewProjectFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        NewProjectFailure::creation(format!("cannot inspect staged project directory: {error}"))
    })?;
    if !is_plain_directory(&metadata) {
        return Err(NewProjectFailure::creation(
            "staged project contains a non-directory or reparse directory",
        ));
    }
    let mut names = fs::read_dir(path)
        .map_err(|error| {
            NewProjectFailure::creation(format!("cannot enumerate staged project: {error}"))
        })?
        .map(|entry| {
            entry
                .map_err(|error| {
                    NewProjectFailure::creation(format!(
                        "cannot inspect staged project entry: {error}"
                    ))
                })?
                .file_name()
                .into_string()
                .map_err(|_| NewProjectFailure::creation("staged project has a non-UTF-8 entry"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    Ok(names)
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

fn is_plain_regular_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
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

fn publish_no_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    ))]
    {
        use std::os::unix::ffi::OsStrExt as _;

        Ok(rustix::fs::renameat_with(
            rustix::fs::CWD,
            source.as_os_str().as_bytes(),
            rustix::fs::CWD,
            destination.as_os_str().as_bytes(),
            rustix::fs::RenameFlags::NOREPLACE,
        )?)
    }
    #[cfg(windows)]
    {
        renamore::rename_exclusive(source, destination)
    }
    #[cfg(all(
        unix,
        not(any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        ))
    ))]
    {
        let _ = (source, destination);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "atomic no-replace rename is unavailable on this Unix target",
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, destination);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "atomic no-replace rename is unavailable on this target",
        ))
    }
}

struct StagingGuard<'a> {
    parent: PathBuf,
    parent_identity: Handle,
    staging: PathBuf,
    staging_identity: Handle,
    files: &'a [TemplateFile],
    armed: bool,
}

impl Drop for StagingGuard<'_> {
    fn drop(&mut self) {
        if !self.armed
            || !same_plain_directory(&self.parent, &self.parent_identity)
            || !owned_subset(&self.staging, &self.staging_identity, self.files)
        {
            return;
        }
        for file in self.files {
            let path = self.staging.join(file.path);
            if fs::symlink_metadata(&path)
                .ok()
                .is_some_and(|metadata| is_plain_regular_file(&metadata))
                && fs::read(&path).ok().as_deref() == Some(file.bytes.as_slice())
            {
                let _ = fs::remove_file(path);
            }
        }
        let _ = fs::remove_dir(self.staging.join("src"));
        let _ = fs::remove_dir(&self.staging);
    }
}

fn owned_subset(root: &Path, identity: &Handle, files: &[TemplateFile]) -> bool {
    if !same_plain_directory(root, identity) {
        return false;
    }
    let Ok(root_entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in root_entries {
        let Ok(entry) = entry else { return false };
        let name = entry.file_name();
        if name == "src" {
            let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
                return false;
            };
            if !is_plain_directory(&metadata) {
                return false;
            }
            let Ok(source_entries) = fs::read_dir(entry.path()) else {
                return false;
            };
            for source in source_entries {
                let Ok(source) = source else { return false };
                let relative = format!("src/{}", source.file_name().to_string_lossy());
                if !matches_exact_file(&source.path(), &relative, files) {
                    return false;
                }
            }
        } else {
            let relative = name.to_string_lossy();
            if !matches_exact_file(&entry.path(), &relative, files) {
                return false;
            }
        }
    }
    true
}

fn matches_exact_file(path: &Path, relative: &str, files: &[TemplateFile]) -> bool {
    let Some(expected) = files.iter().find(|file| file.path == relative) else {
        return false;
    };
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| is_plain_regular_file(&metadata))
        && fs::read(path).ok().as_deref() == Some(expected.bytes.as_slice())
}
