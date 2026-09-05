//! Create-new reservation for one Project native executable.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use same_file::Handle;

use crate::diagnostic::Diagnostic;

pub(super) struct NativeOutput {
    path: PathBuf,
    identity: Option<Handle>,
    file: Option<File>,
    retained: bool,
}

impl NativeOutput {
    pub(super) fn prepare(path: &Path) -> Result<Self, Diagnostic> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| {
                let code = if error.kind() == io::ErrorKind::AlreadyExists {
                    "SPX-I307"
                } else {
                    "SPX-I301"
                };
                Diagnostic::io(
                    code,
                    if error.kind() == io::ErrorKind::AlreadyExists {
                        format!(
                            "cannot reserve fresh project native destination {}: destination already exists",
                            path.display()
                        )
                    } else {
                        format!(
                            "cannot reserve fresh project native destination {}: {error}",
                            path.display()
                        )
                    },
                )
            })?;
        let identity = Handle::from_file(file).map_err(|error| {
            Diagnostic::io(
                "SPX-I301",
                format!("cannot identify reserved project native destination: {error}"),
            )
        })?;
        let file = identity.as_file().try_clone().map_err(|error| {
            Diagnostic::io(
                "SPX-I301",
                format!("cannot retain project native destination handle: {error}"),
            )
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            identity: Some(identity),
            file: Some(file),
            retained: false,
        })
    }

    pub(super) fn file(&mut self) -> &mut File {
        self.file.as_mut().expect("reserved native output file")
    }

    pub(super) fn retain(&mut self) -> Result<(), Diagnostic> {
        if Handle::from_path(&self.path).ok().as_ref() != self.identity.as_ref() {
            return Err(Diagnostic::io(
                "SPX-I301",
                "project native destination changed during publication",
            ));
        }
        self.retained = true;
        Ok(())
    }
}

impl Drop for NativeOutput {
    fn drop(&mut self) {
        if self.retained || Handle::from_path(&self.path).ok().as_ref() != self.identity.as_ref() {
            return;
        }
        drop(self.file.take());
        drop(self.identity.take());
        let _ = std::fs::remove_file(&self.path);
    }
}
