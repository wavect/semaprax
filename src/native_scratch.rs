//! Private source shared by codegen and the CLI; no public scratch authority.
//! Identity checks are observations in a trusted, quiescent namespace, not
//! protection against concurrent same-principal substitution. Drop is inert.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use same_file::Handle;

const MAX_ATTEMPTS: usize = 64;
static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(super) struct Scratch {
    parent: PathBuf,
    parent_identity: Handle,
    directory: PathBuf,
    directory_identity: Handle,
    file: PathBuf,
    file_identity: Option<Handle>,
    sealed: bool,
}

impl Scratch {
    pub(super) fn create(leaf: &str, contents: Option<&[u8]>) -> io::Result<Self> {
        Self::create_in(&std::env::temp_dir(), leaf, contents, || {
            std::format!(
                ".semaprax-native-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            )
        })
    }

    fn create_in(
        parent: &Path,
        leaf: &str,
        contents: Option<&[u8]>,
        mut candidate: impl FnMut() -> String,
    ) -> io::Result<Self> {
        if !one_component(Path::new(leaf)) {
            return Err(changed("scratch leaf must be one normal path component"));
        }
        // Resolve legitimate platform temp aliases once, before creating anything.
        let parent = fs::canonicalize(parent)?;
        plain(&parent, true)?;
        let parent_identity = Handle::from_path(&parent)?;
        for _ in 0..MAX_ATTEMPTS {
            let name = candidate();
            if !one_component(Path::new(&name)) {
                return Err(changed("scratch directory name is invalid"));
            }
            bind(&parent, &parent_identity, true)?;
            let directory = parent.join(name);
            #[cfg(unix)]
            let builder = {
                use std::os::unix::fs::DirBuilderExt as _;
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700);
                builder
            };
            #[cfg(not(unix))]
            let builder = fs::DirBuilder::new();
            match builder.create(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
            plain(&directory, true)?;
            let directory_identity = Handle::from_path(&directory)?;
            let file = directory.join(leaf);
            let mut scratch = Self {
                parent,
                parent_identity,
                directory,
                directory_identity,
                file,
                file_identity: None,
                sealed: false,
            };
            scratch.bind_directory()?;
            scratch.inventory(false)?;
            if let Some(contents) = contents {
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(&scratch.file)?;
                // Retain identity from the actual create-new file, not a reopen.
                scratch.file_identity = Some(Handle::from_file(file)?);
                scratch
                    .file_identity
                    .as_mut()
                    .expect("created file identity")
                    .as_file_mut()
                    .write_all(contents)?;
            }
            return Ok(scratch);
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no fresh native scratch directory within the attempt bound",
        ))
    }

    pub(super) fn path(&self) -> &Path {
        &self.file
    }

    /// For an absent output, call only after the trusted compiler succeeded.
    /// No executable handle is held while the linker creates its output.
    pub(super) fn seal(&mut self) -> io::Result<()> {
        self.bind_directory()?;
        self.inventory(true)?;
        plain(&self.file, false)?;
        if self.file_identity.is_none() {
            self.file_identity = Some(Handle::from_file(File::open(&self.file)?)?);
        }
        self.verify_file()?;
        self.sealed = true;
        Ok(())
    }

    /// Call only after successful process completion. On any uncertainty or
    /// failure, simply drop this object to retain the entire scratch directory.
    pub(super) fn cleanup(mut self) -> io::Result<()> {
        if !self.sealed {
            return Err(changed("unsealed native scratch cannot be removed"));
        }
        self.bind_directory()?;
        self.inventory(true)?;
        self.verify_file()?;
        self.bind_directory()?;
        fs::remove_file(&self.file)?;
        // Windows delete-pending files must close before the directory is empty.
        drop(self.file_identity.take());
        self.bind_directory()?;
        self.inventory(false)?;
        fs::remove_dir(&self.directory)
    }

    fn bind_directory(&self) -> io::Result<()> {
        bind(&self.parent, &self.parent_identity, true)?;
        bind(&self.directory, &self.directory_identity, true)
    }

    fn verify_file(&self) -> io::Result<()> {
        let identity = self
            .file_identity
            .as_ref()
            .ok_or_else(|| changed("native scratch has no retained file identity"))?;
        bind(&self.file, identity, false)?;
        single_link(identity.as_file())
    }

    fn inventory(&self, has_file: bool) -> io::Result<()> {
        let mut entries = fs::read_dir(&self.directory)?;
        if has_file {
            let entry = entries
                .next()
                .transpose()?
                .ok_or_else(|| changed("native scratch file is missing"))?;
            if entry.file_name() != self.file.file_name().expect("fixed normal leaf") {
                return Err(changed("native scratch contains a foreign entry"));
            }
        }
        if entries.next().transpose()?.is_some() {
            return Err(changed("native scratch inventory changed"));
        }
        Ok(())
    }
}

fn one_component(path: &Path) -> bool {
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(value)), None) => value == path.as_os_str(),
        _ => false,
    }
}

fn plain(path: &Path, directory: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(changed(
            "native scratch path is not a plain expected object",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(changed("native scratch path is a reparse point"));
        }
    }
    Ok(())
}

fn bind(path: &Path, expected: &Handle, directory: bool) -> io::Result<()> {
    plain(path, directory)?;
    if Handle::from_path(path)? != *expected {
        return Err(changed("native scratch path identity changed"));
    }
    Ok(())
}

#[cfg(any(unix, windows))]
fn single_link(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    let links = {
        use std::os::unix::fs::MetadataExt as _;
        file.metadata()?.nlink()
    };
    #[cfg(windows)]
    let links = winapi_util::file::information(file)?.number_of_links();
    if links != 1 {
        return Err(changed("native scratch file must have exactly one link"));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn single_link(_file: &File) -> io::Result<()> {
    Err(changed("native scratch link identity is unsupported"))
}

fn changed(message: &str) -> io::Error {
    io::Error::other(message)
}

#[cfg(test)]
#[path = "native_scratch/tests.rs"]
mod tests;
