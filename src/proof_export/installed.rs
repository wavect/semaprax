//! Explicit installed-tool execution using the existing held process provider.
//! This trusted-local profile does not promise memory or OS confinement.
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::process_provider::{ProcessInvocationBudget, ProcessRequest, ProcessTermination};
use std::{cell::RefCell, path::Path};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::process_provider::registered::{HeldProcessTool, RegisteredProcessProvider};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostProfile {
    /// Explicit permission to run this installed tool with the host user's
    /// filesystem/network rights. Wall time, I/O and owned group settlement
    /// are enforced; memory and escaped descendants are not confined.
    TrustedLocal,
    /// Refused before executable acquisition: this adapter cannot enforce it.
    Confined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    Lean,
    Z3,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub version_timeout_ms: u64,
    pub proof_timeout_ms: u64,
    pub stream_max: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            version_timeout_ms: 2_000,
            proof_timeout_ms: 10_000,
            stream_max: 32_752,
        }
    }
}

pub struct InstalledProofTool {
    kind: ToolKind,
    expected_version: String,
    limits: Limits,
    cancellation: AgentCancellation,
    budget: RefCell<ProcessInvocationBudget>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    provider: RefCell<RegisteredProcessProvider>,
}

fn refused(reason: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-LW140",
        format!("installed proof tool refused: {reason}"),
    )
}

impl InstalledProofTool {
    pub fn kind(&self) -> ToolKind {
        self.kind
    }
    pub fn expected_version(&self) -> &str {
        &self.expected_version
    }
    pub fn proof_timeout_ms(&self) -> u64 {
        self.limits.proof_timeout_ms
    }
    /// Acquisition is itself explicit execution authorization for the selected
    /// installed binary. No PATH lookup, inherited environment or installation.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        executable: &Path,
        cwd: &Path,
        kind: ToolKind,
        expected_version: &str,
        profile: HostProfile,
        limits: Limits,
        cancellation: AgentCancellation,
    ) -> Result<Self, Diagnostic> {
        if profile != HostProfile::TrustedLocal {
            return Err(refused("requested confinement is unavailable"));
        }
        if !executable.is_absolute()
            || !cwd.is_absolute()
            || expected_version.is_empty()
            || expected_version.len() > 256
            || expected_version.contains(['\n', '\r', '\0'])
            || !(1..=30_000).contains(&limits.version_timeout_ms)
            || !(1..=30_000).contains(&limits.proof_timeout_ms)
            || !(1..=32_752).contains(&limits.stream_max)
        {
            return Err(refused("invalid path, version pin or process limits"));
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let executable = executable
                .canonicalize()
                .map_err(|_| refused("executable unavailable"))?;
            let binary =
                std::fs::File::open(&executable).map_err(|_| refused("executable unavailable"))?;
            let directory =
                std::fs::File::open(cwd).map_err(|_| refused("working directory unavailable"))?;
            let arguments: fn(&[Vec<u8>]) -> bool = match kind {
                ToolKind::Lean => {
                    |args| args == [b"--version".to_vec()] || args == [b"--stdin".to_vec()]
                }
                ToolKind::Z3 => |args| {
                    args == [b"--version".to_vec()] || args == [b"-in".to_vec(), b"-smt2".to_vec()]
                },
            };
            let argv0 = match kind {
                ToolKind::Lean => b"lean".to_vec(),
                ToolKind::Z3 => b"z3".to_vec(),
            };
            let tool = HeldProcessTool::new(binary, directory, argv0, vec![], arguments)
                .map_err(|_| refused("held tool registration failed"))?;
            #[cfg(target_os = "macos")]
            let tool = tool
                .with_invocation_path(Some(&executable))
                .map_err(|_| refused("held invocation path unavailable"))?;
            Ok(Self {
                kind,
                expected_version: expected_version.to_owned(),
                limits,
                cancellation,
                budget: RefCell::new(ProcessInvocationBudget::new()),
                provider: RefCell::new(
                    RegisteredProcessProvider::new([(1, tool)])
                        .map_err(|_| refused("held registry unavailable"))?,
                ),
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (kind, cancellation);
            Err(refused("host process settlement unavailable"))
        }
    }

    pub fn version(&self) -> Result<String, Diagnostic> {
        let output = self.run(&[b"--version"], b"", self.limits.version_timeout_ms)?;
        let version = output.trim();
        if version != self.expected_version {
            return Err(refused("exact toolchain version pin differs"));
        }
        Ok(version.to_owned())
    }

    fn run(&self, args: &[&[u8]], input: &[u8], timeout: u64) -> Result<String, Diagnostic> {
        if self.cancellation.is_cancelled() {
            return Err(refused("cancelled"));
        }
        let mut argv = (args.len() as u32).to_le_bytes().to_vec();
        for arg in args {
            argv.extend_from_slice(&(arg.len() as u32).to_le_bytes());
            argv.extend_from_slice(arg);
        }
        let request = ProcessRequest::from_wire(
            1,
            &argv,
            argv.len(),
            input,
            input.len(),
            timeout,
            self.limits.stream_max,
            self.limits.stream_max,
        )
        .map_err(|_| refused("input/output request exceeds process bounds"))?;
        self.budget
            .borrow_mut()
            .reserve(&request)
            .map_err(|_| refused("invocation process budget exhausted"))?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let output = self
                .provider
                .borrow_mut()
                .run_cancellable(&request, Some(&self.cancellation))
                .map_err(|error| refused(&format!("bounded process failed: {error:?}")))?;
            if output.termination != ProcessTermination::Exited(0) {
                return Err(refused("tool exited unsuccessfully; no proof accepted"));
            }
            let mut bytes = output.stdout;
            bytes.extend_from_slice(&output.stderr);
            String::from_utf8(bytes).map_err(|_| refused("tool output is not UTF-8"))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        Err(refused("host process settlement unavailable"))
    }

    /// This exact check is intentionally stricter than the older solver's
    /// first-token classifier. Any surplus/partial output is a non-result.
    pub fn confirm_smt(&self, script: &str) -> Result<(), Diagnostic> {
        if self.kind != ToolKind::Z3 {
            return Err(refused("Z3 capability required"));
        }
        self.version()?;
        let output = self.run(
            &[b"-in", b"-smt2"],
            script.as_bytes(),
            self.limits.proof_timeout_ms,
        )?;
        if output.trim() != "unsat" {
            return Err(refused(
                "solver returned false, unknown, malformed or partial evidence",
            ));
        }
        Ok(())
    }
}

impl super::LeanKernel for InstalledProofTool {
    fn check(&self, lean_source: &str) -> Result<super::KernelRun, Diagnostic> {
        if self.kind != ToolKind::Lean {
            return Err(refused("Lean capability required"));
        }
        // The semantic profile's pin is separate from the exact installed
        // binary version line selected by the host.
        let version = self.version()?;
        let semantic_version = super::PINNED_TOOLCHAIN.rsplit(':').next().unwrap_or("");
        if !version.starts_with(&format!(
            "Lean (version {},",
            semantic_version.trim_start_matches('v')
        )) {
            return Err(refused(
                "Lean semantic profile requires its pinned toolchain",
            ));
        }
        let output = self.run(
            &[b"--stdin"],
            lean_source.as_bytes(),
            self.limits.proof_timeout_ms,
        )?;
        Ok(super::KernelRun {
            toolchain: super::PINNED_TOOLCHAIN.into(),
            output,
        })
    }
}

impl crate::assurance_manifest::proof_certificate::ExternalKernelCapability for InstalledProofTool {
    fn confirm(&self, script: &str) -> Result<(), Diagnostic> {
        self.confirm_smt(script)
    }
}
