#![allow(dead_code)]
use std::{fmt,fs,path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};
static STAGING_SERIAL: AtomicU64 = AtomicU64::new(0);
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

mod application {
//! Explicit full-host fresh installation, reusing the scaffold publisher.
use super::*;
use semaprax_native_rust_interop_platform as platform;

#[path = "/Volumes/Semaprax/worktrees/ci-canonical-54e/crates/semaprax-toolchain/src/new_project/application/held_input_path.rs"]
mod held_input_path;

fn failure(message: impl Into<String>) -> NewProjectFailure {
    NewProjectFailure::creation(message)
}


fn read_bound(path: &Path, maximum: usize) -> Result<Vec<u8>, NewProjectFailure> {
    read_bound_with_hook(path, maximum, &|_| Ok(()))
}

fn selected_input_path(path: &Path) -> Result<PathBuf, NewProjectFailure> {
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(failure("starter input parent traversal is not admitted"));
    }
    std::path::absolute(path)
        .map_err(|error| failure(format!("cannot resolve starter input: {error}")))
}

fn read_bound_with_hook(
    path: &Path,
    maximum: usize,
    before_open: &dyn Fn(&Path) -> Result<(), NewProjectFailure>,
) -> Result<Vec<u8>, NewProjectFailure> {
    let parent_path = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| failure("starter input must name one regular file"))?;
    let held_parent_path = held_input_path::parent_spelling(parent_path)?;
    let parent = platform::hold_directory(&held_parent_path)
        .map_err(|_| failure("cannot hold starter input parent"))?;
    // This early inspection is a convenience refusal only. The actual file
    // classification and no-follow/nonblocking open belong to the held owner.
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| failure(format!("cannot inspect starter input: {e}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(failure("starter input must be a regular non-symlink file"));
    }
    before_open(path)?;
    let file = platform::hold_regular_file_bounded(&parent, name, maximum)
        .map_err(|_| failure("starter input is not a bounded held regular non-symlink file"))?;
    let bytes = platform::read_exact(&file, maximum)
        .map_err(|_| failure("starter input changed while reading"))?;
    platform::recheck_regular_file_named_bounded(&parent, name, &file, maximum)
        .map_err(|_| failure("starter input binding changed while reading"))?;
    if !platform::same_directory_path(&parent, parent_path)
        .map_err(|_| failure("starter input parent changed"))?
    {
        return Err(failure("starter input parent binding changed"));
    }
    Ok(bytes)
}


fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "spx-application-publish-{}-{}",
            std::process::id(),
            STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }
#[test]
fn starter_input_held_regular_reader_preserves_exact_bytes_and_bounds() {
        let parent = root();
        let input = parent.join("plan.json");
        fs::write(&input, b"stable").unwrap();
        assert_eq!(read_bound(&input, 6).unwrap(), b"stable");
        assert!(read_bound(&input, 5).is_err());
        assert_eq!(fs::read(&input).unwrap(), b"stable");
        fs::remove_dir_all(parent).unwrap();
    }
#[test]
fn starter_reader_refuses_parent_rebinding_after_the_held_open() {
        let parent = root();
        let held = parent.with_extension("held");
        let input = parent.join("plan.json");
        fs::write(&input, b"stable").unwrap();
        let result = read_bound_with_hook(&input, 6, &|_| {
            fs::rename(&parent, &held).unwrap();
            fs::create_dir(&parent).unwrap();
            fs::write(parent.join("plan.json"), b"forged").unwrap();
            Ok(())
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "starter input parent binding changed"
        );
        assert_eq!(fs::read(held.join("plan.json")).unwrap(), b"stable");
        assert_eq!(fs::read(&input).unwrap(), b"forged");
        fs::remove_dir_all(held).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }
}
