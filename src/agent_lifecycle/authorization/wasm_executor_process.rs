//! Explicit, bounded Node authority for the Core Wasm stage executor.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::process_provider::registered::{HeldProcessTool, RegisteredProcessProvider};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::process_provider::{
    ProcessFailure, ProcessProvider, ProcessRequest, ProcessTermination,
};

use crate::agent_lifecycle::stages::invariant;
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;

use super::workspace::WasmStageWorkspace;

pub(super) const MAX_NODE_STDOUT_BYTES: usize = 60 * 1024 - 32;
const MAX_NODE_STDERR_BYTES: usize = 4 * 1024;
const NODE_TIMEOUT_MS: u64 = 2_000;
const MAX_HELD_RUNTIME_BYTES: u64 = 256 * 1024 * 1024;

/// Held authority for exactly one caller-selected Node runtime. Execution
/// never searches PATH, inherits an environment, or reopens the supplied path.
#[derive(Debug)]
pub struct WasmStageHost {
    runtime: File,
    identity: String,
    digest: [u8; 32],
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl WasmStageHost {
    pub fn open(path: &Path) -> Result<Self, Diagnostic> {
        if cfg!(windows) {
            return Err(invariant("wasm_executor.host.unsupported"));
        }
        if !path.is_absolute() {
            return Err(invariant("wasm_executor.host.runtime_path"));
        }
        let canonical = path
            .canonicalize()
            .map_err(|_| invariant("wasm_executor.host.runtime_path"))?;
        let runtime = OpenOptions::new()
            .read(true)
            .open(canonical)
            .map_err(|_| invariant("wasm_executor.host.runtime_open"))?;
        let metadata = runtime
            .metadata()
            .map_err(|_| invariant("wasm_executor.host.runtime_metadata"))?;
        if !metadata.is_file() {
            return Err(invariant("wasm_executor.host.runtime_regular"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(invariant("wasm_executor.host.runtime_executable"));
            }
        }
        let digest = digest_file(&runtime)?;
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Self {
            runtime,
            identity: format!("core-wasm-node:sha256:{hex}:{}", metadata.len()),
            digest,
            len: metadata.len(),
            #[cfg(unix)]
            device: {
                use std::os::unix::fs::MetadataExt;
                metadata.dev()
            },
            #[cfg(unix)]
            inode: {
                use std::os::unix::fs::MetadataExt;
                metadata.ino()
            },
        })
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    fn recheck(&self) -> Result<(), Diagnostic> {
        let metadata = self
            .runtime
            .metadata()
            .map_err(|_| invariant("wasm_executor.host.runtime_metadata"))?;
        if !metadata.is_file() || metadata.len() != self.len {
            return Err(invariant("wasm_executor.host.runtime_drift"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != self.device || metadata.ino() != self.inode {
                return Err(invariant("wasm_executor.host.runtime_drift"));
            }
        }
        if digest_file(&self.runtime)? != self.digest {
            return Err(invariant("wasm_executor.host.runtime_drift"));
        }
        Ok(())
    }
}

fn digest_file(file: &File) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    let mut remaining = MAX_HELD_RUNTIME_BYTES;
    let mut offset = 0_u64;
    let mut bytes = [0_u8; 16 * 1024];
    loop {
        #[cfg(unix)]
        let read = {
            use std::os::unix::fs::FileExt;
            file.read_at(&mut bytes, offset)
        };
        #[cfg(not(unix))]
        let read = {
            use std::io::{Seek, SeekFrom};
            let mut clone = file
                .try_clone()
                .map_err(|_| invariant("wasm_executor.host.runtime_clone"))?;
            clone
                .seek(SeekFrom::Start(offset))
                .map_err(|_| invariant("wasm_executor.host.runtime_read"))?;
            clone.read(&mut bytes)
        };
        let read = read.map_err(|_| invariant("wasm_executor.host.runtime_read"))?;
        if read == 0 {
            break;
        }
        let read =
            u64::try_from(read).map_err(|_| invariant("wasm_executor.host.runtime_budget"))?;
        if read > remaining {
            return Err(invariant("wasm_executor.host.runtime_budget"));
        }
        remaining -= read;
        hash.update(&bytes[..read as usize]);
        offset += read;
    }
    Ok(hash.finalize().into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn accepts_observer(args: &[Vec<u8>]) -> bool {
    args.len() == 1 && args[0] == b"observe.mjs"
}

pub(super) fn run_node_process(
    host: &WasmStageHost,
    workspace: &WasmStageWorkspace,
    cancellation: Option<&AgentCancellation>,
    output_budget: usize,
) -> Result<String, Diagnostic> {
    if output_budget == 0 || output_budget > MAX_NODE_STDOUT_BYTES {
        return Err(invariant("wasm_executor.process.output_budget"));
    }
    if cancellation.is_some_and(AgentCancellation::is_cancelled) {
        return Err(invariant("wasm_executor.process.cancelled"));
    }
    host.recheck()?;
    workspace.recheck()?;
    run_held(host, workspace, cancellation, output_budget)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_held(
    host: &WasmStageHost,
    workspace: &WasmStageWorkspace,
    cancellation: Option<&AgentCancellation>,
    output_budget: usize,
) -> Result<String, Diagnostic> {
    let tool = HeldProcessTool::new(
        host.runtime
            .try_clone()
            .map_err(|_| invariant("wasm_executor.host.runtime_clone"))?,
        workspace.held_directory()?,
        b"semaprax-stage-node".to_vec(),
        Vec::new(),
        accepts_observer,
    )
    .map_err(|_| invariant("wasm_executor.process.tool"))?;
    let mut provider = RegisteredProcessProvider::new([(1, tool)])
        .map_err(|_| invariant("wasm_executor.process.tool"))?;
    let mut argv = Vec::from(1_u32.to_le_bytes());
    argv.extend_from_slice(&11_u32.to_le_bytes());
    argv.extend_from_slice(b"observe.mjs");
    let request = ProcessRequest::from_wire(
        1,
        &argv,
        argv.len(),
        &[],
        0,
        NODE_TIMEOUT_MS,
        output_budget,
        MAX_NODE_STDERR_BYTES,
    )
    .map_err(|_| invariant("wasm_executor.process.request"))?;
    let result = provider.run_cancellable(&request, cancellation);
    let settled = provider.settle();
    let output = match (result, settled) {
        (Ok(output), Ok(())) => output,
        (Err(ProcessFailure::TimedOut), _) => {
            return Err(invariant("wasm_executor.process.deadline"))
        }
        (Err(ProcessFailure::CapacityExceeded), _) => {
            return Err(invariant("wasm_executor.process.output_budget"))
        }
        (Err(ProcessFailure::Cancelled), _) => {
            return Err(invariant("wasm_executor.process.cancelled"))
        }
        _ => return Err(invariant("wasm_executor.process.run")),
    };
    if output.termination != ProcessTermination::Exited(0) {
        return Err(invariant("wasm_executor.run"));
    }
    String::from_utf8(output.stdout).map_err(|_| invariant("wasm_executor.output_utf8"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn run_held(
    _host: &WasmStageHost,
    _workspace: &WasmStageWorkspace,
    _cancellation: Option<&AgentCancellation>,
    _output_budget: usize,
) -> Result<String, Diagnostic> {
    Err(invariant("wasm_executor.host.unsupported"))
}
