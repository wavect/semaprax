//! Explicit installed-tool execution using the existing held process provider.
//! This trusted-local profile does not promise memory or OS confinement.
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::process_provider::{ProcessInvocationBudget, ProcessRequest, ProcessTermination};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use sha2::{Digest as _, Sha256};
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

/// Diagnostic result of one bounded installed query. A SAT model is untrusted
/// until the caller independently replays it against checked source semantics.
pub enum SmtDiagnosticResult {
    Unsat,
    Sat(crate::assurance_manifest::smt_discharge::Model),
    Unknown,
    TimedOut,
}

enum RunFailure {
    TimedOut,
    Diagnostic(Diagnostic),
}

impl RunFailure {
    fn diagnostic(self) -> Diagnostic {
        match self {
            Self::TimedOut => refused("bounded process failed: TimedOut"),
            Self::Diagnostic(error) => error,
        }
    }
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
    modular_scalar: bool,
    expected_version: String,
    executable_digest: String,
    limits: Limits,
    cancellation: AgentCancellation,
    budget: RefCell<ProcessInvocationBudget>,
    reserved_solver_queries: RefCell<usize>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    provider: RefCell<RegisteredProcessProvider>,
}

/// Monotone host reservations for one held installed proof tool. These are
/// attempted process calls, not a bill, proof, or claim of completed execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstalledProofWork {
    pub reserved_process_invocations: usize,
    pub reserved_solver_queries: usize,
    pub reserved_io_bytes: usize,
}

impl InstalledProofWork {
    pub fn since(self, earlier: Self) -> Self {
        Self {
            reserved_process_invocations: self
                .reserved_process_invocations
                .checked_sub(earlier.reserved_process_invocations)
                .expect("held process reservations are monotone"),
            reserved_solver_queries: self
                .reserved_solver_queries
                .checked_sub(earlier.reserved_solver_queries)
                .expect("held solver reservations are monotone"),
            reserved_io_bytes: self
                .reserved_io_bytes
                .checked_sub(earlier.reserved_io_bytes)
                .expect("held process byte reservations are monotone"),
        }
    }
}

fn refused(reason: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-LW140",
        format!("installed proof tool refused: {reason}"),
    )
}

impl InstalledProofTool {
    /// Host-selected larger finite ledger for the checked modular scalar
    /// profile. Ordinary installed proof tools retain their original budget.
    #[allow(clippy::too_many_arguments)]
    pub fn open_modular_scalar(
        executable: &Path,
        cwd: &Path,
        expected_version: &str,
        profile: HostProfile,
        limits: Limits,
        cancellation: AgentCancellation,
    ) -> Result<Self, Diagnostic> {
        let mut tool = Self::open(
            executable,
            cwd,
            ToolKind::Z3,
            expected_version,
            profile,
            limits,
            cancellation,
        )?;
        tool.budget = RefCell::new(ProcessInvocationBudget::modular_scalar());
        tool.modular_scalar = true;
        Ok(tool)
    }

    pub fn is_modular_scalar(&self) -> bool {
        self.modular_scalar
    }

    pub fn kind(&self) -> ToolKind {
        self.kind
    }
    pub fn expected_version(&self) -> &str {
        &self.expected_version
    }
    pub fn proof_timeout_ms(&self) -> u64 {
        self.limits.proof_timeout_ms
    }
    pub fn work_snapshot(&self) -> InstalledProofWork {
        let budget = self.budget.borrow();
        InstalledProofWork {
            reserved_process_invocations: budget.runs(),
            reserved_solver_queries: *self.reserved_solver_queries.borrow(),
            reserved_io_bytes: budget.total_bytes(),
        }
    }
    /// Complete pinned process options included in logical proof-task keys.
    pub(crate) fn proof_cache_active(&self) -> bool {
        !self.cancellation.is_cancelled()
    }
    pub(crate) fn proof_cache_options(&self) -> (u64, u64, usize) {
        (
            self.limits.version_timeout_ms,
            self.limits.proof_timeout_ms,
            self.limits.stream_max,
        )
    }
    pub(crate) fn proof_cache_executable_digest(&self) -> &str {
        &self.executable_digest
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
            let mut binary =
                std::fs::File::open(&executable).map_err(|_| refused("executable unavailable"))?;
            let executable_digest = {
                use std::io::{Read, Seek as _};
                let mut hash = Sha256::new();
                let mut buffer = [0u8; 64 * 1024];
                loop {
                    let size = binary
                        .read(&mut buffer)
                        .map_err(|_| refused("executable digest read failed"))?;
                    if size == 0 {
                        break;
                    }
                    hash.update(&buffer[..size]);
                }
                binary
                    .rewind()
                    .map_err(|_| refused("executable digest rewind failed"))?;
                format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
            };
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
                modular_scalar: false,
                expected_version: expected_version.to_owned(),
                executable_digest,
                limits,
                cancellation,
                budget: RefCell::new(ProcessInvocationBudget::new()),
                reserved_solver_queries: RefCell::new(0),
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
        self.version_observed().map_err(RunFailure::diagnostic)
    }

    fn version_observed(&self) -> Result<String, RunFailure> {
        let output = self.run_observed(&[b"--version"], b"", self.limits.version_timeout_ms)?;
        let version = output.trim();
        if version != self.expected_version {
            return Err(RunFailure::Diagnostic(refused(
                "exact toolchain version pin differs",
            )));
        }
        Ok(version.to_owned())
    }

    fn run(&self, args: &[&[u8]], input: &[u8], timeout: u64) -> Result<String, Diagnostic> {
        self.run_observed(args, input, timeout)
            .map_err(RunFailure::diagnostic)
    }

    fn run_observed(
        &self,
        args: &[&[u8]],
        input: &[u8],
        timeout: u64,
    ) -> Result<String, RunFailure> {
        if self.cancellation.is_cancelled() {
            return Err(RunFailure::Diagnostic(refused("cancelled")));
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
        .map_err(|_| {
            RunFailure::Diagnostic(refused("input/output request exceeds process bounds"))
        })?;
        self.budget
            .borrow_mut()
            .reserve(&request)
            .map_err(|_| RunFailure::Diagnostic(refused("invocation process budget exhausted")))?;
        if (matches!(self.kind, ToolKind::Z3) && args == [b"-in".as_slice(), b"-smt2".as_slice()])
            || (matches!(self.kind, ToolKind::Lean) && args == [b"--stdin".as_slice()])
        {
            *self.reserved_solver_queries.borrow_mut() += 1;
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let output = self
                .provider
                .borrow_mut()
                .run_cancellable(&request, Some(&self.cancellation))
                .map_err(|error| match error {
                    crate::process_provider::ProcessFailure::TimedOut => RunFailure::TimedOut,
                    other => RunFailure::Diagnostic(refused(&format!(
                        "bounded process failed: {other:?}"
                    ))),
                })?;
            if output.termination != ProcessTermination::Exited(0) {
                return Err(RunFailure::Diagnostic(refused(
                    "tool exited unsuccessfully; no proof accepted",
                )));
            }
            let mut bytes = output.stdout;
            bytes.extend_from_slice(&output.stderr);
            String::from_utf8(bytes)
                .map_err(|_| RunFailure::Diagnostic(refused("tool output is not UTF-8")))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        Err(RunFailure::Diagnostic(refused(
            "host process settlement unavailable",
        )))
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

    /// Inspect a failed proof goal through the same held process capability.
    /// This never creates a proof token; callers must replay a returned model.
    pub fn smt_diagnostic_query(&self, script: &str) -> Result<SmtDiagnosticResult, Diagnostic> {
        if self.kind != ToolKind::Z3 {
            return Err(refused("Z3 capability required"));
        }
        match self.version_observed() {
            Ok(_) => {}
            Err(RunFailure::TimedOut) => return Ok(SmtDiagnosticResult::TimedOut),
            Err(RunFailure::Diagnostic(error)) => return Err(error),
        }
        let output = match self.run_observed(
            &[b"-in", b"-smt2"],
            script.as_bytes(),
            self.limits.proof_timeout_ms,
        ) {
            Ok(output) => output,
            Err(RunFailure::TimedOut) => return Ok(SmtDiagnosticResult::TimedOut),
            Err(RunFailure::Diagnostic(error)) => return Err(error),
        };
        let (status, body) = output
            .split_once('\n')
            .map(|(status, body)| (status.trim(), body.trim()))
            .unwrap_or((output.trim(), ""));
        match status {
            "unsat" if body.is_empty() => Ok(SmtDiagnosticResult::Unsat),
            "unknown" if body.is_empty() => Ok(SmtDiagnosticResult::Unknown),
            "sat" if !body.is_empty() => {
                let model = crate::assurance_manifest::smt_discharge::parse_model(body)
                    .map_err(|_| refused("solver returned an unsupported diagnostic model"))?;
                Ok(SmtDiagnosticResult::Sat(model))
            }
            _ => Err(refused(
                "solver diagnostic result is malformed or incomplete",
            )),
        }
    }

    /// Obtain a concrete SAT model through the same held, bounded Z3
    /// capability. Callers must replay the model against the source before
    /// treating it as a satisfiable precondition domain.
    pub fn smt_domain_model(
        &self,
        script: &str,
    ) -> Result<crate::assurance_manifest::smt_discharge::Model, Diagnostic> {
        if self.kind != ToolKind::Z3 {
            return Err(refused("Z3 capability required"));
        }
        self.version()?;
        let output = self.run(
            &[b"-in", b"-smt2"],
            script.as_bytes(),
            self.limits.proof_timeout_ms,
        )?;
        let (status, body) = output
            .split_once('\n')
            .ok_or_else(|| refused("domain query did not return a complete SAT model"))?;
        if status.trim() != "sat" {
            return Err(refused("domain is contradictory, unknown or unavailable"));
        }
        crate::assurance_manifest::smt_discharge::parse_model(body.trim())
            .map_err(|_| refused("domain query returned an unsupported model"))
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
