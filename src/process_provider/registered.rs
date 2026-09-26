//! Registered, handle-only process tools for supported Unix hosts.
//!
//! Registration receives already-open directory and executable handles. It
//! never resolves a path, consults `PATH`, inherits a host environment, or
//! discovers a current directory.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::File;
#[cfg(target_os = "macos")]
use std::fs::Metadata;

use super::{ProcessFailure, ProcessOutput, ProcessProvider, ProcessRequest};
use crate::agent_runtime::AgentCancellation;

pub const MAX_ENVIRONMENT_ENTRIES: usize = 256;
pub const MAX_ENVIRONMENT_BYTES: usize = 65_536;

/// One registered executable and its fixed host-side launch policy.
pub struct HeldProcessTool {
    pub(super) executable: File,
    pub(super) cwd: File,
    pub(super) argv0: CString,
    /// Canonical `name=value` C strings, byte-sorted by their name prefixes.
    pub(super) environment: Vec<CString>,
    #[cfg(target_os = "macos")]
    pub(super) executable_metadata: Metadata,
    /// Canonical invocation path recorded when the executable was held, used
    /// for the macOS spawn instead of resolving the descriptor. Required when
    /// the executable may be multiply linked: a descriptor-derived path is an
    /// arbitrary link, and a multicall binary dispatches on invocation path.
    #[cfg(target_os = "macos")]
    pub(super) invocation_path: Option<CString>,
    accepts: fn(&[Vec<u8>]) -> bool,
}

impl HeldProcessTool {
    /// Register only already-open resources and a fixed argument policy.
    pub fn new(
        executable: File,
        cwd: File,
        argv0: Vec<u8>,
        environment: Vec<(Vec<u8>, Vec<u8>)>,
        accepts: fn(&[Vec<u8>]) -> bool,
    ) -> Result<Self, ProcessFailure> {
        let executable_metadata = executable
            .metadata()
            .map_err(|_| ProcessFailure::IoFailure)?;
        if !executable_metadata.is_file() {
            return Err(ProcessFailure::InvalidInput);
        }
        if !cwd
            .metadata()
            .map_err(|_| ProcessFailure::IoFailure)?
            .is_dir()
        {
            return Err(ProcessFailure::InvalidInput);
        }
        if argv0.is_empty() || argv0.contains(&0) {
            return Err(ProcessFailure::InvalidInput);
        }
        let argv0 = CString::new(argv0).map_err(|_| ProcessFailure::InvalidInput)?;
        let environment = canonical_environment(environment)?;
        Ok(Self {
            executable,
            cwd,
            argv0,
            environment,
            #[cfg(target_os = "macos")]
            executable_metadata,
            #[cfg(target_os = "macos")]
            invocation_path: None,
            accepts,
        })
    }

    /// Records the canonical path used to establish the held executable, to be
    /// used as the macOS spawn invocation path. This does not weaken the
    /// held-descriptor authority: macOS attestation still verifies the
    /// suspended child's mapped executable vnode against the held file before
    /// it runs, so a path swapped after opening fails closed instead of
    /// running. `None` keeps the previous descriptor-derived path, which is
    /// only deterministic for single-link executables.
    #[cfg(target_os = "macos")]
    pub fn with_invocation_path(
        mut self,
        path: Option<&std::path::Path>,
    ) -> Result<Self, ProcessFailure> {
        let Some(path) = path else {
            return Ok(self);
        };
        let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
        self.invocation_path = Some(CString::new(bytes).map_err(|_| ProcessFailure::InvalidInput)?);
        Ok(self)
    }

    fn accepts(&self, arguments: &[Vec<u8>]) -> bool {
        (self.accepts)(arguments)
    }
}

fn canonical_environment(
    mut entries: Vec<(Vec<u8>, Vec<u8>)>,
) -> Result<Vec<CString>, ProcessFailure> {
    if entries.len() > MAX_ENVIRONMENT_ENTRIES {
        return Err(ProcessFailure::CapacityExceeded);
    }
    let mut total = 0usize;
    for (name, value) in &entries {
        if name.is_empty() || name.contains(&0) || name.contains(&b'=') || value.contains(&0) {
            return Err(ProcessFailure::InvalidInput);
        }
        total = total
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_add(value.len()))
            .ok_or(ProcessFailure::CapacityExceeded)?;
        if total > MAX_ENVIRONMENT_BYTES {
            return Err(ProcessFailure::CapacityExceeded);
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(ProcessFailure::InvalidInput);
    }
    entries
        .into_iter()
        .map(|(name, value)| {
            let mut entry = Vec::with_capacity(name.len() + 1 + value.len());
            entry.extend_from_slice(&name);
            entry.push(b'=');
            entry.extend_from_slice(&value);
            CString::new(entry).map_err(|_| ProcessFailure::InvalidInput)
        })
        .collect()
}

/// Explicit map of caller-selected numeric tool identities to held tools.
pub struct RegisteredProcessProvider {
    tools: BTreeMap<u64, HeldProcessTool>,
}

impl RegisteredProcessProvider {
    pub fn new(
        tools: impl IntoIterator<Item = (u64, HeldProcessTool)>,
    ) -> Result<Self, ProcessFailure> {
        let mut registered = BTreeMap::new();
        for (id, tool) in tools {
            if registered.insert(id, tool).is_some() {
                return Err(ProcessFailure::InvalidInput);
            }
        }
        Ok(Self { tools: registered })
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Runs a held tool while observing an explicitly supplied monotonic
    /// cancellation handle. The platform owns kill-and-settle before it
    /// reports cancellation, so callers never receive while a child group is
    /// still live.
    pub fn run_cancellable(
        &mut self,
        request: &ProcessRequest,
        cancellation: Option<&AgentCancellation>,
    ) -> Result<ProcessOutput, ProcessFailure> {
        let tool = self
            .tools
            .get(&request.tool())
            .ok_or(ProcessFailure::AuthorityDenied)?;
        if !tool.accepts(request.arguments()) {
            return Err(ProcessFailure::AuthorityDenied);
        }
        platform::run(tool, request, cancellation)
    }
}

impl ProcessProvider for RegisteredProcessProvider {
    fn run(&mut self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessFailure> {
        let tool = self
            .tools
            .get(&request.tool())
            .ok_or(ProcessFailure::AuthorityDenied)?;
        if !tool.accepts(request.arguments()) {
            return Err(ProcessFailure::AuthorityDenied);
        }
        platform::run(tool, request, None)
    }

    fn settle(&mut self) -> Result<(), ProcessFailure> {
        platform::settle()
    }
}

mod platform;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir() -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "semaprax-process-provider-{}-{stamp}",
            std::process::id()
        ))
    }

    fn held(environment: Vec<(Vec<u8>, Vec<u8>)>) -> HeldProcessTool {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let executable_path = root.join("tool");
        fs::write(&executable_path, b"fixture").unwrap();
        let executable = OpenOptions::new().read(true).open(executable_path).unwrap();
        let cwd = OpenOptions::new().read(true).open(&root).unwrap();
        let tool =
            HeldProcessTool::new(executable, cwd, b"tool".to_vec(), environment, |_| true).unwrap();
        fs::remove_dir_all(root).unwrap();
        tool
    }

    #[test]
    fn held_tool_canonicalizes_environment_and_rejects_invalid_names() {
        let tool = held(vec![
            (b"Z".to_vec(), b"last".to_vec()),
            (b"A".to_vec(), Vec::new()),
        ]);
        assert_eq!(tool.environment[0].as_bytes(), b"A=");
        assert_eq!(tool.environment[1].as_bytes(), b"Z=last");
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let executable_path = root.join("tool");
        fs::write(&executable_path, b"fixture").unwrap();
        let executable = OpenOptions::new()
            .read(true)
            .open(&executable_path)
            .unwrap();
        let cwd = OpenOptions::new().read(true).open(&root).unwrap();
        assert!(matches!(
            HeldProcessTool::new(
                executable,
                cwd,
                b"tool".to_vec(),
                vec![(b"A=B".to_vec(), vec![])],
                |_| true
            ),
            Err(ProcessFailure::InvalidInput)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registry_rejects_duplicate_tool_id() {
        let first = held(Vec::new());
        let second = held(Vec::new());
        assert_eq!(
            RegisteredProcessProvider::new([(3, first), (3, second)]).err(),
            Some(ProcessFailure::InvalidInput)
        );
    }

    #[test]
    fn environment_bounds_and_duplicate_names_fail_closed() {
        assert_eq!(
            canonical_environment(vec![(b"A".to_vec(), vec![]), (b"A".to_vec(), vec![])]),
            Err(ProcessFailure::InvalidInput)
        );
        let entries = (0..=MAX_ENVIRONMENT_ENTRIES)
            .map(|index| (format!("K{index}").into_bytes(), Vec::new()))
            .collect();
        assert_eq!(
            canonical_environment(entries),
            Err(ProcessFailure::CapacityExceeded)
        );
    }
}

#[cfg(test)]
mod physical_tests;
