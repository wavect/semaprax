//! Standalone creation of a built-in project template.
//!
//! The published compiler has no private host, so it cannot use the
//! held-parent staged publication behind the full toolchain's `new`. This is
//! the bounded route it uses instead: derive the exact scaffold bytes, refuse
//! anything but a fresh destination under a real parent directory, write every
//! file with create-new semantics, read the files back, and authenticate the
//! result as a project before reporting success. Grammar, template, file bytes,
//! and the success line match the full toolchain. A failure after the
//! destination exists leaves whatever was written in place and reports it; the
//! caller decides what to do with the residue. There is no staging directory,
//! atomic rename, or identity re-verification against concurrent parent
//! substitution. [Standalone project creation v1](../../docs/NEW-PROJECT-STANDALONE-V1.md)
//! owns the contract.

use std::fmt;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use super::MANIFEST_FILE;
use super::{derive_project_scaffold_v1_with_layout, with_authenticated_project, ScaffoldLayout};

/// Why standalone project creation stopped. Every variant maps to exit status
/// one at the CLI; invocation errors are rejected before this is reached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProjectError {
    message: String,
}

impl CreateProjectError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CreateProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CreateProjectError {}

/// Create and verify the built-in project `template` at `destination`.
///
/// `template` is one of `PROJECT_SCAFFOLD_TEMPLATES`; an unknown template is
/// reported before the filesystem is touched. Returns the destination as the
/// caller spelled it.
pub fn create_project(
    destination: &Path,
    name: &str,
    template: &str,
) -> Result<PathBuf, CreateProjectError> {
    let scaffold = derive_project_scaffold_v1_with_layout(name, template, ScaffoldLayout::Tables)
        .map_err(|diagnostics| {
        CreateProjectError::new(format!(
            "cannot derive the {template} template: {}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        ))
    })?;
    let requested_destination = std::path::absolute(destination).map_err(|error| {
        CreateProjectError::new(format!("cannot resolve new project destination: {error}"))
    })?;
    let file_name = requested_destination
        .file_name()
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            CreateProjectError::new("new project destination must name one directory")
        })?;
    let requested_parent = requested_destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(requested_parent).map_err(|error| {
        CreateProjectError::new(format!(
            "cannot inspect new project parent {}: {error}",
            requested_parent.display()
        ))
    })?;
    if !parent_metadata.is_dir() {
        return Err(CreateProjectError::new(
            "new project parent must be a real directory",
        ));
    }
    let absolute = fs::canonicalize(requested_parent)
        .map_err(|error| {
            CreateProjectError::new(format!(
                "cannot canonicalize new project parent {}: {error}",
                requested_parent.display()
            ))
        })?
        .join(file_name);
    match fs::symlink_metadata(&absolute) {
        Ok(_) => {
            return Err(CreateProjectError::new(format!(
                "cannot create project {}: an entry already exists",
                destination.display()
            )))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(CreateProjectError::new(format!(
                "cannot inspect new project destination {}: {error}",
                destination.display()
            )))
        }
    }
    fs::create_dir(&absolute).map_err(|error| {
        if error.kind() == ErrorKind::AlreadyExists {
            CreateProjectError::new(format!(
                "cannot create project {}: an entry already exists",
                destination.display()
            ))
        } else {
            CreateProjectError::new(format!(
                "cannot create project directory {}: {error}",
                destination.display()
            ))
        }
    })?;
    let mut created_directories = std::collections::BTreeSet::new();
    for file in scaffold.files() {
        let path = absolute.join(file.path());
        let directory = path
            .parent()
            .expect("scaffold paths are relative file paths");
        if directory != absolute && created_directories.insert(directory.to_path_buf()) {
            fs::create_dir(directory).map_err(|error| {
                CreateProjectError::new(format!(
                    "cannot create project directory {}: {error}",
                    directory.display()
                ))
            })?;
        }
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                CreateProjectError::new(format!(
                    "cannot create project file {}: {error}",
                    file.path()
                ))
            })?;
        handle.write_all(file.bytes()).map_err(|error| {
            CreateProjectError::new(format!(
                "cannot write project file {}: {error}",
                file.path()
            ))
        })?;
    }
    for file in scaffold.files() {
        let observed = fs::read(absolute.join(file.path())).map_err(|error| {
            CreateProjectError::new(format!(
                "cannot read back project file {}: {error}",
                file.path()
            ))
        })?;
        if observed != file.bytes() {
            return Err(CreateProjectError::new(format!(
                "project file {} differs from the template after writing",
                file.path()
            )));
        }
    }
    with_authenticated_project(&absolute.join(MANIFEST_FILE), |snapshot| snapshot.check())
        .map_err(|diagnostics| {
            CreateProjectError::new(format!(
                "created project failed verification: {}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        })?;
    Ok(destination.to_path_buf())
}
