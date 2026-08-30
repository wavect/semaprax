//! Bounded offline tool inventory, not executable provenance or launch authority.
#![forbid(unsafe_code)]

use crate::{DoctorOfflineInput, DOCTOR_OFFLINE_INPUT_MAX_BYTES};

#[path = "offline_bundle/elf.rs"]
mod elf;
#[path = "offline_bundle/wire.rs"]
mod wire;

#[cfg(test)]
#[path = "offline_bundle/tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorOfflineArchitecture {
    LinuxX86_64,
    LinuxAarch64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorOfflineTool {
    Clang,
    Node,
    Rustc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorOfflineBundleError {
    Invalid,
    Limit,
    SelectorMismatch,
    ArchitectureMismatch,
    Unsupported,
    Allocation,
}

/// An immutable inventory over one retained sealed-input snapshot. Parsing
/// neither publishes these files nor grants permission to execute any of them.
pub struct DoctorOfflineBundle {
    input: DoctorOfflineInput,
    index: wire::Index,
}

impl std::fmt::Debug for DoctorOfflineBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DoctorOfflineBundle")
            .field("selector", &self.selector())
            .field("architecture", &self.architecture())
            .field("file_count", &self.index.files.len())
            .finish_non_exhaustive()
    }
}

/// A borrowed file view. Its lifetime cannot outlive the retained bundle.
#[derive(Clone, Copy, Debug)]
pub struct DoctorOfflineBundleFile<'a> {
    path: &'a str,
    bytes: &'a [u8],
    executable: bool,
}

impl<'a> DoctorOfflineBundleFile<'a> {
    pub fn path(&self) -> &'a str {
        self.path
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn is_executable(&self) -> bool {
        self.executable
    }
}

impl DoctorOfflineBundle {
    /// Bind a canonical inventory to an explicit selector and the compiled
    /// native Linux architecture. There is no ambient lookup or fallback.
    pub fn parse(
        input: DoctorOfflineInput,
        selector: &str,
    ) -> Result<Self, DoctorOfflineBundleError> {
        if !wire::valid_selector(selector) {
            return Err(DoctorOfflineBundleError::Invalid);
        }
        let architecture = current_architecture().ok_or(DoctorOfflineBundleError::Unsupported)?;
        let index = wire::parse(input.bytes(), selector, architecture)?;
        Ok(Self { input, index })
    }

    pub fn selector(&self) -> &str {
        // The private wire decoder admitted this exact ASCII range.
        std::str::from_utf8(&self.input.bytes()[self.index.selector.clone()])
            .expect("validated offline selector")
    }

    pub fn architecture(&self) -> DoctorOfflineArchitecture {
        self.index.architecture
    }

    pub fn files(&self) -> impl ExactSizeIterator<Item = DoctorOfflineBundleFile<'_>> {
        self.index.files.iter().map(|file| self.file_view(file))
    }

    pub fn tool(&self, tool: DoctorOfflineTool) -> Option<DoctorOfflineBundleFile<'_>> {
        let ordinal = match tool {
            DoctorOfflineTool::Clang => 0,
            DoctorOfflineTool::Node => 1,
            DoctorOfflineTool::Rustc => 2,
        };
        self.index.tools[ordinal].map(|index| self.file_view(&self.index.files[index]))
    }

    fn file_view<'a>(&'a self, file: &wire::FileIndex) -> DoctorOfflineBundleFile<'a> {
        DoctorOfflineBundleFile {
            path: file.path(self.input.bytes()),
            bytes: &self.input.bytes()[file.content.clone()],
            executable: file.executable,
        }
    }
}

fn current_architecture() -> Option<DoctorOfflineArchitecture> {
    if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )) {
        Some(DoctorOfflineArchitecture::LinuxX86_64)
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "aarch64",
        target_pointer_width = "64",
        target_endian = "little"
    )) {
        Some(DoctorOfflineArchitecture::LinuxAarch64)
    } else {
        None
    }
}
