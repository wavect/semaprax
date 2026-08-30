//! Explicit-input, read-only semantic image commands.
//!
//! Output is returned only after the Project authority's final source recheck.
//! An image is a disposable derived input, never source or commit authority.

use std::io::Read as _;
use std::path::Path;

use semaprax::diagnostic::{quote_json, Diagnostic};
use semaprax::project::{self, ProjectSemanticImage, MAX_SEMANTIC_IMAGE_BYTES};

pub(crate) fn derive(manifest: &Path) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(manifest, |snapshot| {
        let image =
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())?;
        Ok(image.to_json().to_owned())
    })
}

pub(crate) fn verify(manifest: &Path, path: &Path) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(manifest, |snapshot| {
        let bytes = read_image(path).map_err(|error| vec![error])?;
        let image = ProjectSemanticImage::replay(
            snapshot.retain_revision(),
            snapshot.project_revision(),
            &bytes,
        )?;
        Ok(format!(
            "{{\"schema\":\"semaprax.semantic-workspace-image-receipt.v1\",\"project_revision\":{},\"image_digest\":{},\"verified\":true,\"source_authority\":false}}\n",
            quote_json(snapshot.project_revision()),
            quote_json(image.image_digest()),
        ))
    })
}

pub(crate) fn symbol(manifest: &Path, stable_id: &str) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(manifest, |snapshot| {
        let image =
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())?;
        let mut output = image.symbol(image.image_digest(), stable_id)?;
        output.push('\n');
        Ok(output)
    })
}

fn read_image(path: &Path) -> Result<Vec<u8>, Diagnostic> {
    // Nonblocking, no-follow leaf opens avoid following a substituted symlink
    // or waiting on a FIFO before the held file's type can be checked.
    let file = open_image(path)?;
    let metadata = file
        .metadata()
        .map_err(|_| input_error("cannot inspect semantic image input"))?;
    if !metadata.is_file() {
        return Err(input_error("semantic image input must be a regular file"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(input_error(
                "semantic image input must not be a reparse point",
            ));
        }
    }
    if metadata.len() > MAX_SEMANTIC_IMAGE_BYTES as u64 {
        return Err(capacity_error());
    }
    let mut bytes = Vec::new();
    file.take(MAX_SEMANTIC_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| input_error("cannot read semantic image input"))?;
    if bytes.len() > MAX_SEMANTIC_IMAGE_BYTES {
        return Err(capacity_error());
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_image(path: &Path) -> Result<std::fs::File, Diagnostic> {
    use rustix::fs::{open, Mode, OFlags};
    open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(|_| input_error("cannot open semantic image input without following links"))
}

#[cfg(windows)]
fn open_image(path: &Path) -> Result<std::fs::File, Diagnostic> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| input_error("cannot open semantic image input reparse-safely"))
}

#[cfg(not(any(unix, windows)))]
fn open_image(_path: &Path) -> Result<std::fs::File, Diagnostic> {
    Err(input_error(
        "semantic image input is unsupported on this host",
    ))
}

fn input_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-G219", message)
}

fn capacity_error() -> Diagnostic {
    Diagnostic::io("SPX-G220", "semantic image input exceeds the byte limit")
}
