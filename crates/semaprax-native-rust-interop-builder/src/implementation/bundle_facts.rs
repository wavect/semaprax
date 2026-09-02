//! Pending phase-B facts and the exact path construction the bundle
//! facts are built from.

use super::*;

pub(super) struct PendingBundleFacts {
    output_directory: PathBuf,
    object_path: PathBuf,
    descriptor_path: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: String,
    descriptor_digest: String,
}

impl PendingBundleFacts {
    pub(super) fn new(
        output: &Path,
        object_name: &'static str,
        descriptor_digest: &str,
    ) -> Result<Self, Diagnostic> {
        use std::path::Component;

        if !canonical_sha256_text(descriptor_digest) {
            return Err(b108());
        }

        let parent = output.parent().ok_or_else(platform_publication_error)?;
        let output_name = output.file_name().ok_or_else(platform_publication_error)?;
        let mut components = Path::new(output_name).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || output.strip_prefix(parent).ok() != Some(Path::new(output_name))
        {
            return Err(platform_publication_error());
        }

        let output_bytes = output.as_os_str().len();
        let child_capacity = |name: &str| exact_child_path_capacity(output, name.len());
        let object_capacity = child_capacity(object_name)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let descriptor_capacity = child_capacity("descriptor.json")
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let manifest_capacity = child_capacity("semaprax.native-rust-interop.json")
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let retained = output_bytes
            .checked_add(object_capacity)
            .and_then(|bytes| bytes.checked_add(descriptor_capacity))
            .and_then(|bytes| bytes.checked_add(manifest_capacity))
            .and_then(|bytes| bytes.checked_add(SHA256_TEXT_BYTES))
            .and_then(|bytes| bytes.checked_add(descriptor_digest.len()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let authority = reserve_temporary_exact(retained)?;

        let output_directory = exact_path_copy(output, output_bytes)?;
        let object_path = exact_child_path(output, object_name, object_capacity)?;
        let descriptor_path = exact_child_path(output, "descriptor.json", descriptor_capacity)?;
        let manifest_path = exact_child_path(
            output,
            "semaprax.native-rust-interop.json",
            manifest_capacity,
        )?;
        let manifest_digest = String::with_capacity(SHA256_TEXT_BYTES);
        let descriptor_digest = descriptor_digest.to_owned();
        if descriptor_digest.capacity() != SHA256_TEXT_BYTES {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        if manifest_digest.capacity() != SHA256_TEXT_BYTES {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        authority.retain(retained)?;
        Ok(Self {
            output_directory,
            object_path,
            descriptor_path,
            manifest_path,
            manifest_digest,
            descriptor_digest,
        })
    }

    pub(super) fn bind_manifest_digest(
        &mut self,
        manifest: &[u8],
        project: bool,
    ) -> Result<(), PhaseBLocalError> {
        if !self.manifest_digest.is_empty() || self.manifest_digest.capacity() != SHA256_TEXT_BYTES
        {
            return Err(PhaseBLocalError::Replay);
        }
        self.manifest_digest.push_str("sha256:");
        let mut hasher = Sha256::new();
        hasher.update(if project {
            PROJECT_BUNDLE_DIGEST_DOMAIN
        } else {
            BUNDLE_DIGEST_DOMAIN
        });
        hasher.update(manifest);
        let digest = hasher.finalize();
        for byte in digest {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            self.manifest_digest
                .push(char::from(HEX[usize::from(byte >> 4)]));
            self.manifest_digest
                .push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        if self.manifest_digest.len() != SHA256_TEXT_BYTES
            || self.manifest_digest.capacity() != SHA256_TEXT_BYTES
        {
            return Err(PhaseBLocalError::Replay);
        }
        Ok(())
    }

    pub(super) fn finish(self) -> NativeRustInteropBundleFacts {
        NativeRustInteropBundleFacts {
            output_directory: self.output_directory,
            object_path: self.object_path,
            descriptor_path: self.descriptor_path,
            manifest_path: self.manifest_path,
            manifest_digest: self.manifest_digest,
            descriptor_digest: self.descriptor_digest,
        }
    }
}

pub(super) fn exact_path_copy(path: &Path, capacity: usize) -> Result<PathBuf, Diagnostic> {
    if path.as_os_str().len() != capacity {
        return Err(platform_publication_error());
    }
    let mut output = OsString::with_capacity(capacity);
    output.push(path.as_os_str());
    let output = PathBuf::from(output);
    if output != path || output.capacity() != capacity {
        return Err(platform_publication_error());
    }
    Ok(output)
}

pub(super) fn exact_child_path_capacity(parent: &Path, child_bytes: usize) -> Option<usize> {
    parent
        .as_os_str()
        .len()
        .checked_add(usize::from(path_needs_separator(parent)))?
        .checked_add(child_bytes)
}

fn path_needs_separator(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};

        let mut components = path.components();
        if matches!(
            components.next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        ) && components.next().is_none()
        {
            return false;
        }
    }
    let Some(last) = path.as_os_str().as_encoded_bytes().last().copied() else {
        return false;
    };
    last != b'/' && (!cfg!(windows) || last != b'\\')
}

pub(super) fn fill_exact_child_path(output: &mut PathBuf, parent: &Path, name: &OsStr) -> bool {
    if exact_child_path_capacity(parent, name.len())
        .is_none_or(|required| required > output.capacity())
    {
        return false;
    }
    let mut storage = std::mem::take(output).into_os_string();
    storage.clear();
    storage.push(parent.as_os_str());
    if path_needs_separator(parent) {
        storage.push(std::path::MAIN_SEPARATOR_STR);
    }
    storage.push(name);
    *output = PathBuf::from(storage);
    true
}

pub(super) fn exact_child_path_matches(output: &Path, parent: &Path, name: &OsStr) -> bool {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(value)) if value == name)
        || components.next().is_some()
    {
        return false;
    }
    let separator = usize::from(path_needs_separator(parent));
    let output = output.as_os_str().as_encoded_bytes();
    let parent = parent.as_os_str().as_encoded_bytes();
    let name = name.as_encoded_bytes();
    output.len() == parent.len() + separator + name.len()
        && output.starts_with(parent)
        && (separator == 0 || output.get(parent.len()) == Some(&(std::path::MAIN_SEPARATOR as u8)))
        && &output[parent.len() + separator..] == name
}

pub(super) fn exact_child_path(
    parent: &Path,
    name: &str,
    capacity: usize,
) -> Result<PathBuf, Diagnostic> {
    if exact_child_path_capacity(parent, name.len()) != Some(capacity) {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let mut storage = OsString::with_capacity(capacity);
    storage.push(parent.as_os_str());
    if path_needs_separator(parent) {
        storage.push(std::path::MAIN_SEPARATOR_STR);
    }
    storage.push(name);
    let output = PathBuf::from(storage);
    if output.capacity() != capacity || !exact_child_path_matches(&output, parent, OsStr::new(name))
    {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    Ok(output)
}

impl NativeRustInteropBundleFacts {
    pub(crate) fn output_directory(&self) -> &Path {
        &self.output_directory
    }
    pub(crate) fn object_path(&self) -> &Path {
        &self.object_path
    }
    pub(crate) fn descriptor_path(&self) -> &Path {
        &self.descriptor_path
    }
    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }
    pub(crate) fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }
    pub(crate) fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
}
