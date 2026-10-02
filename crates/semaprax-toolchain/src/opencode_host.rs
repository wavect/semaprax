//! Explicit physical OpenCode host adapter for the local #112 provider slice.
//!
//! This module is the only OpenCode process boundary. It returns raw response
//! bytes to the existing `ModelHandler` seam; the compiler-owned
//! `SourceInteractionProposalDecoder` remains the only proposal admission
//! decoder. `--dir` and the deny-all OpenCode policy constrain OpenCode's tool
//! permissions. They are not claimed to provide operating-system isolation.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

use semaprax::digest_hex::LowerHex;
use semaprax::live_invocation::{
    ModelFailure, ModelHandler, ModelInvocationOutcome, ModelInvocationRequest,
    ModelInvokeCapability,
};
use sha2::{Digest, Sha256};

/// The single explicitly configured free profile. There is no fallback model.
pub const OPENCODE_MODEL: &str = "opencode/muse-spark-1.3-contributor-free";
const OPENCODE_AGENT: &str = "semaprax-live";
const MAX_PROMPT_BYTES: usize = 65_536;
const MAX_EVENTS_BYTES: usize = 1_048_576;
const MAX_EXPORT_BYTES: usize = 1_048_576;
const MAX_GRAMMAR_BYTES: usize = 65_536;
/// The private host admits one complete frozen executable snapshot up to 160 MiB.
/// This is deliberately independent from the much smaller provider wire limits.
const MAX_EXECUTABLE_BINDING_BYTES: u64 = 160 * 1024 * 1024;
/// Staged-executable reauthentication compares the frozen bytes incrementally.
/// It must not allocate another executable-sized buffer while dispatching.
const EXECUTABLE_REAUTH_BUFFER_BYTES: usize = 64 * 1024;
const STAGED_EXECUTABLE: &str = ".semaprax-opencode-executable";

/// Host-owned process settings. Constructing this value is distinct from
/// granting the per-call `ModelInvokeCapability`; both are required to invoke.
/// A host-held cancellation hook for the production runner.
#[derive(Clone, Debug, Default)]
pub struct OpenCodeCancellation(Arc<AtomicBool>);

impl OpenCodeCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Compiler-derived response guidance, supplied alongside the same compiled
/// schema that the live driver later gives to `SourceInteractionProposalDecoder`.
#[derive(Clone, Debug)]
pub struct OpenCodeGrammar {
    digest: String,
    canonical_schema: String,
    provider_schema: String,
}

impl OpenCodeGrammar {
    pub fn from_compiled(
        schema: &semaprax::agent_interaction_schema::CompiledInteractionSchema,
    ) -> Result<Self, String> {
        let canonical_schema = schema.schema().canonical_json().to_owned();
        let provider_schema = schema.provider_json_schema();
        if canonical_schema.len().saturating_add(provider_schema.len()) > MAX_GRAMMAR_BYTES {
            return Err("OpenCode grammar guidance exceeds its host byte budget".into());
        }
        Ok(Self {
            digest: schema.schema().digest().to_owned(),
            canonical_schema,
            provider_schema,
        })
    }
}

#[derive(Clone, Debug)]
pub struct OpenCodeHostConfig {
    executable: PathBuf,
    executable_bytes: Arc<[u8]>,
    executable_permissions: std::fs::Permissions,
    sandbox: PathBuf,
    executable_binding: String,
    deadline: Duration,
    cancellation: OpenCodeCancellation,
    grammar: OpenCodeGrammar,
}

fn is_absolute_like(path: &std::path::Path) -> bool {
    path.is_absolute() || path.to_string_lossy().starts_with('/')
}

impl OpenCodeHostConfig {
    /// Accepts only an absolute executable and an existing, non-symlink
    /// workspace containing no foreign state. Exact interrupted host-owned
    /// state is authenticated and removed before admission.
    pub fn new(
        executable: PathBuf,
        sandbox: PathBuf,
        deadline: Duration,
        grammar: OpenCodeGrammar,
    ) -> Result<Self, String> {
        if !is_absolute_like(&executable) || !is_absolute_like(&sandbox) || deadline.is_zero() {
            return Err("OpenCode host requires absolute paths and a positive deadline".into());
        }
        let executable = executable
            .canonicalize()
            .map_err(|_| "OpenCode executable must be a readable regular file".to_owned())?;
        let (executable_binding, executable_bytes, executable_permissions) =
            executable_snapshot(&executable)
                .ok_or_else(|| "OpenCode executable must be a readable regular file".to_owned())?;
        if sandbox
            .symlink_metadata()
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("OpenCode host sandbox must not be a symlink".into());
        }
        let sandbox = sandbox.canonicalize().map_err(|error| error.to_string())?;
        if !sandbox.is_dir() {
            return Err("OpenCode host sandbox must be an existing empty directory".into());
        }
        #[cfg(unix)]
        {
            let metadata = sandbox
                .metadata()
                .map_err(|_| "OpenCode host sandbox metadata is unavailable".to_owned())?;
            if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0
            {
                return Err(
                    "OpenCode host sandbox must be owned and not writable by other users".into(),
                );
            }
        }
        let policy = policy_document();
        if !cleanup_interrupted_staged_executable(&sandbox, &executable_bytes) {
            return Err("OpenCode host could not authenticate interrupted executable state".into());
        }
        if scratch_inventory_is_session(&sandbox, policy.as_bytes())
            && !cleanup_owned_scratch(&sandbox, policy.as_bytes())
        {
            return Err("OpenCode host could not clear its interrupted session state".into());
        }
        if !scratch_inventory_is_admitted(&sandbox, policy.as_bytes()) {
            return Err(
                "OpenCode host sandbox must be empty or contain only its exact policy".into(),
            );
        }
        Ok(Self {
            executable,
            executable_bytes: executable_bytes.into(),
            executable_permissions,
            sandbox,
            executable_binding,
            deadline,
            cancellation: OpenCodeCancellation::new(),
            grammar,
        })
    }

    /// Lets the owning deployment cancel an in-flight direct child.
    #[must_use]
    pub fn cancellation(&self) -> OpenCodeCancellation {
        self.cancellation.clone()
    }
}

/// Closed runner failures, deliberately without provider stderr or secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCodeRunnerFailure {
    Refused,
    Timeout,
    Capacity,
    Provider,
    Malformed,
    Cancelled,
    ProviderStatus(provider_error::OpenCodeProviderFailure),
}

/// Injectable process seam. Production binds `ProcessOpenCodeRunner`; tests
/// bind a local executable or deterministic fixture without a network call.
pub trait OpenCodeRunner {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure>;
    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        session: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure>;
    /// Abandon host-owned run state when receipt admission stops before export.
    fn abandon(&mut self, _config: &OpenCodeHostConfig) -> Result<(), OpenCodeRunnerFailure> {
        Ok(())
    }
    /// Only an observed caller cancellation may map an in-flight failure to
    /// `Cancelled`; transport uncertainty is otherwise `ProviderError`.
    fn cancelled(&self, config: &OpenCodeHostConfig) -> bool {
        config.cancellation.is_cancelled()
    }
}

/// The actual bounded subprocess runner. It uses no shell, inherited stdin,
/// prompt-supplied path, model fallback, or source-derived credential.
pub struct ProcessOpenCodeRunner;

#[cfg(unix)]
struct StagedExecutable {
    path: PathBuf,
    file: std::fs::File,
}

#[cfg(unix)]
impl StagedExecutable {
    fn create(config: &OpenCodeHostConfig) -> Result<Self, OpenCodeRunnerFailure> {
        let path = config.sandbox.join(STAGED_EXECUTABLE);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&path)
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        let mut staged = Self { path, file };
        staged
            .file
            .write_all(&config.executable_bytes)
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        std::fs::set_permissions(&staged.path, config.executable_permissions.clone())
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        if !held_file_matches(&mut staged.file, &config.executable_bytes) {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        // Linux rejects exec while this inode has a writable descriptor
        // (ETXTBSY). Open and authenticate a read-only pin before releasing
        // the writer; retain the same inode and exact bytes through dispatch.
        let read_only = std::fs::File::from(
            rustix::fs::open(
                &staged.path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| OpenCodeRunnerFailure::Refused)?,
        );
        let before = staged
            .file
            .metadata()
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        let after = read_only
            .metadata()
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        staged.file = read_only;
        if !staged.authenticate(&config.executable_bytes) {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        Ok(staged)
    }

    fn authenticate(&mut self, expected: &[u8]) -> bool {
        let Ok(path) = self.path.symlink_metadata() else {
            return false;
        };
        let Ok(held) = self.file.metadata() else {
            return false;
        };
        path.file_type().is_file()
            && path.dev() == held.dev()
            && path.ino() == held.ino()
            && held_file_matches(&mut self.file, expected)
    }
}

#[cfg(unix)]
impl Drop for StagedExecutable {
    fn drop(&mut self) {
        let Ok(path) = self.path.symlink_metadata() else {
            return;
        };
        let Ok(held) = self.file.metadata() else {
            return;
        };
        if path.file_type().is_file() && path.dev() == held.dev() && path.ino() == held.ino() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl ProcessOpenCodeRunner {
    #[cfg(unix)]
    fn kill_group(child: &mut std::process::Child) {
        if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
    }

    #[cfg(unix)]
    fn terminate(child: &mut std::process::Child) {
        Self::kill_group(child);
        let _ = child.kill();
        let _ = child.wait();
    }

    fn capture(
        config: &OpenCodeHostConfig,
        args: &[String],
        limit: usize,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        if executable_binding(&config.executable).as_deref()
            != Some(config.executable_binding.as_str())
        {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        // This v1 runner has a bounded, same-thread nonblocking pipe loop only
        // on Unix. Refuse before spawn elsewhere rather than leave a blocking
        // `ChildStdout::read` path that could outlive its deadline.
        #[cfg(not(unix))]
        {
            let _ = (config, args, limit);
            return Err(OpenCodeRunnerFailure::Refused);
        }
        #[cfg(unix)]
        {
            let deadline = Instant::now()
                .checked_add(config.deadline)
                .ok_or(OpenCodeRunnerFailure::Refused)?;
            let mut staged_executable = StagedExecutable::create(config)?;
            if !staged_executable.authenticate(&config.executable_bytes) {
                return Err(OpenCodeRunnerFailure::Refused);
            }
            let mut command = Command::new(&staged_executable.path);
            environment::configure_command(&mut command, &config.sandbox)
                .map_err(|_| OpenCodeRunnerFailure::Refused)?;
            command
                .args(args)
                .current_dir(&config.sandbox)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            command.process_group(0);
            let mut child = command
                .spawn()
                .map_err(|_| OpenCodeRunnerFailure::Refused)?;
            let Some(stdout) = child.stdout.take() else {
                Self::terminate(&mut child);
                return Err(OpenCodeRunnerFailure::Provider);
            };
            let flags = match rustix::fs::fcntl_getfl(&stdout) {
                Ok(flags) => flags,
                Err(_) => {
                    Self::terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Provider);
                }
            };
            if rustix::fs::fcntl_setfl(&stdout, flags | rustix::fs::OFlags::NONBLOCK).is_err() {
                Self::terminate(&mut child);
                return Err(OpenCodeRunnerFailure::Provider);
            }
            let mut output = Vec::new();
            let mut eof = false;
            let mut status = None;
            let mut chunk = [0u8; 8192];
            loop {
                if config.cancellation.is_cancelled() {
                    Self::terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Cancelled);
                }
                if Instant::now() >= deadline {
                    Self::terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Timeout);
                }
                loop {
                    match rustix::io::read(&stdout, &mut chunk[..]) {
                        Ok(0) => {
                            eof = true;
                            break;
                        }
                        Ok(count) if output.len().saturating_add(count) <= limit => {
                            output.extend_from_slice(&chunk[..count])
                        }
                        Ok(_) => {
                            Self::terminate(&mut child);
                            return Err(OpenCodeRunnerFailure::Malformed);
                        }
                        Err(rustix::io::Errno::AGAIN) => break,
                        Err(_) => {
                            Self::terminate(&mut child);
                            return Err(OpenCodeRunnerFailure::Provider);
                        }
                    }
                }
                if status.is_none() {
                    match child.try_wait() {
                        Ok(Some(exit)) => {
                            status = Some(exit);
                            // The leader may have exited while a descendant still
                            // owns stdout. Group kill forces the pipe to EOF.
                            Self::kill_group(&mut child);
                        }
                        Ok(None) => {}
                        Err(_) => {
                            Self::terminate(&mut child);
                            return Err(OpenCodeRunnerFailure::Provider);
                        }
                    }
                }
                if let Some(exit) = status {
                    if eof {
                        return if exit.success() && !output.is_empty() {
                            Ok(output)
                        } else {
                            Err(provider_error::classify_provider_failure(&output)
                                .map(OpenCodeRunnerFailure::ProviderStatus)
                                .unwrap_or(OpenCodeRunnerFailure::Provider))
                        };
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

impl OpenCodeRunner for ProcessOpenCodeRunner {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let policy = policy_document();
        let policy_path = config.sandbox.join("opencode.json");
        if !scratch_inventory_is_admitted(&config.sandbox, policy.as_bytes()) {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        match std::fs::read(&policy_path) {
            Ok(existing) if existing == policy.as_bytes() => {}
            Ok(_) => return Err(OpenCodeRunnerFailure::Refused),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::write(&policy_path, &policy)
                    .map_err(|_| OpenCodeRunnerFailure::Refused)?;
            }
            Err(_) => return Err(OpenCodeRunnerFailure::Refused),
        }
        let args = vec![
            "run".into(),
            "--pure".into(),
            "--agent".into(),
            OPENCODE_AGENT.into(),
            "--model".into(),
            OPENCODE_MODEL.into(),
            "--format".into(),
            "json".into(),
            "--dir".into(),
            config.sandbox.display().to_string(),
            prompt.into(),
        ];
        let result = Self::capture(config, &args, MAX_EVENTS_BYTES);
        if result.is_ok() && scratch_inventory_is_session(&config.sandbox, policy.as_bytes()) {
            return result;
        }
        if !cleanup_owned_scratch(&config.sandbox, policy.as_bytes()) {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        result.and(Err(OpenCodeRunnerFailure::Refused))
    }

    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        session: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let policy = policy_document();
        if !scratch_inventory_is_session(&config.sandbox, policy.as_bytes()) {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        let result = Self::capture(
            config,
            &["export".into(), session.into(), "--pure".into()],
            MAX_EXPORT_BYTES,
        );
        if !cleanup_owned_scratch(&config.sandbox, policy.as_bytes()) {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        result
    }

    fn abandon(&mut self, config: &OpenCodeHostConfig) -> Result<(), OpenCodeRunnerFailure> {
        cleanup_owned_scratch(&config.sandbox, policy_document().as_bytes())
            .then_some(())
            .ok_or(OpenCodeRunnerFailure::Refused)
    }
}

fn policy_document() -> String {
    serde_json::json!({"$schema":"https://opencode.ai/config.json", "snapshot":false, "agent": {OPENCODE_AGENT: {
        "permission":{"*":"deny"}, "steps":1,
        "prompt":"You return canonical structured responses. All schema and context are supplied in the user message. Never inspect files or call tools. Do not narrate plans or explain your work. Return only the requested JSON document, without markdown or extra text. After the final closing brace, press Enter exactly once: the final byte must be a literal newline (U+000A). Do not output a backslash followed by n, and do not omit the newline."
    }}}).to_string()
}

fn cleanup_policy(path: &std::path::Path, expected: &[u8]) -> bool {
    let Ok(before) = path.symlink_metadata() else {
        return true;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    if !before.file_type().is_file() || bytes != expected {
        return false;
    }
    let Ok(after) = path.symlink_metadata() else {
        return false;
    };
    #[cfg(unix)]
    let same_inode = before.dev() == after.dev() && before.ino() == after.ino();
    #[cfg(not(unix))]
    let same_inode = before.len() == after.len();
    if same_inode {
        return std::fs::remove_file(path).is_ok();
    }
    false
}

fn scratch_inventory_is_empty(sandbox: &std::path::Path) -> bool {
    sandbox
        .read_dir()
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

fn scratch_inventory_is_admitted(sandbox: &std::path::Path, policy: &[u8]) -> bool {
    let Ok(mut entries) = sandbox.read_dir() else {
        return false;
    };
    let Some(entry) = entries.next() else {
        return true;
    };
    let Ok(entry) = entry else {
        return false;
    };
    if entries.next().is_some() || entry.file_name() != "opencode.json" {
        return false;
    }
    let path = entry.path();
    path.symlink_metadata()
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
        && std::fs::read(path)
            .map(|bytes| bytes == policy)
            .unwrap_or(false)
}

fn scratch_inventory_is_session(sandbox: &std::path::Path, policy: &[u8]) -> bool {
    let Ok(entries) = sandbox.read_dir() else {
        return false;
    };
    let mut saw_policy = false;
    let mut saw_private = false;
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        if entry.file_name() == "opencode.json" && !saw_policy {
            let path = entry.path();
            saw_policy = path
                .symlink_metadata()
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false)
                && std::fs::read(path)
                    .map(|bytes| bytes == policy)
                    .unwrap_or(false);
        } else if entry.file_name() == environment::PRIVATE && !saw_private {
            saw_private = entry
                .path()
                .symlink_metadata()
                .map(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
                .unwrap_or(false);
        } else {
            return false;
        }
    }
    saw_policy && saw_private
}

fn cleanup_owned_scratch(sandbox: &std::path::Path, policy: &[u8]) -> bool {
    let policy_ok = cleanup_policy(&sandbox.join("opencode.json"), policy);
    let private = sandbox.join(environment::PRIVATE);
    let private_ok = match private.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            std::fs::remove_dir_all(private).is_ok()
        }
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    };
    policy_ok && private_ok && scratch_inventory_is_empty(sandbox)
}

fn cleanup_interrupted_staged_executable(sandbox: &std::path::Path, expected: &[u8]) -> bool {
    let path = sandbox.join(STAGED_EXECUTABLE);
    let mut file = match std::fs::OpenOptions::new().read(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
    };
    let Ok(after) = path.symlink_metadata() else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if after.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    if !after.file_type().is_file()
        || !held_path_is_same_file(&file, &path)
        || !held_file_matches(&mut file, expected)
    {
        return false;
    }
    std::fs::remove_file(path).is_ok()
}

fn held_path_is_same_file(file: &std::fs::File, path: &std::path::Path) -> bool {
    let Ok(clone) = file.try_clone() else {
        return false;
    };
    let Ok(held) = same_file::Handle::from_file(clone) else {
        return false;
    };
    let Ok(rebound) = same_file::Handle::from_path(path) else {
        return false;
    };
    held == rebound
}

fn held_file_matches(file: &mut std::fs::File, expected: &[u8]) -> bool {
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != expected.len() as u64 {
        return false;
    }
    if file.seek(SeekFrom::Start(0)).is_err() {
        return false;
    }
    let read = frozen_bytes_match(file, expected);
    let rewound = file.seek(SeekFrom::Start(0)).is_ok();
    read && rewound
}

fn frozen_bytes_match(reader: &mut impl Read, expected: &[u8]) -> bool {
    let mut actual = [0_u8; EXECUTABLE_REAUTH_BUFFER_BYTES];
    let read = expected
        .chunks(EXECUTABLE_REAUTH_BUFFER_BYTES)
        .all(|expected| {
            reader.read_exact(&mut actual[..expected.len()]).is_ok()
                && actual[..expected.len()] == expected[..]
        });
    let mut trailing = [0_u8; 1];
    read && matches!(reader.read(&mut trailing), Ok(0))
}

fn executable_snapshot(path: &std::path::Path) -> Option<(String, Vec<u8>, std::fs::Permissions)> {
    if !path.symlink_metadata().ok()?.file_type().is_file() {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_EXECUTABLE_BINDING_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_EXECUTABLE_BINDING_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_EXECUTABLE_BINDING_BYTES {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.opencode-executable-binding.v1\0");
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(&bytes);
    #[cfg(unix)]
    {
        hasher.update(metadata.dev().to_be_bytes());
        hasher.update(metadata.ino().to_be_bytes());
        hasher.update(metadata.mode().to_be_bytes());
        hasher.update(metadata.len().to_be_bytes());
        hasher.update(metadata.mtime().to_be_bytes());
        hasher.update(metadata.mtime_nsec().to_be_bytes());
    }
    Some((
        format!("sha256:{:x}", LowerHex(hasher.finalize())),
        bytes,
        metadata.permissions(),
    ))
}

pub(crate) fn executable_binding(path: &std::path::Path) -> Option<String> {
    executable_snapshot(path).map(|(binding, _, _)| binding)
}

/// Self-reported host receipt. Usage is optional and carries no proof of cost
/// or provider authorization; it is never copied into the language journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCodeReceipt {
    pub session_id: String,
    pub message_id: String,
    pub model: &'static str,
    pub usage_total: Option<u64>,
    pub usage: Option<OpenCodeUsage>,
    pub reported_cost: Option<serde_json::Number>,
}

/// Provider-reported counters, preserved individually without estimating billing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCodeUsage {
    pub total: Option<u64>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

/// An explicit handler which has both configured host settings and a runner.
pub struct OpenCodeModelHandler<R> {
    config: OpenCodeHostConfig,
    runner: R,
    pub last_receipt: Option<OpenCodeReceipt>,
    pub last_provider_failure: Option<provider_error::OpenCodeProviderFailure>,
}

impl<R> OpenCodeModelHandler<R> {
    pub fn new(config: OpenCodeHostConfig, runner: R) -> Self {
        Self {
            config,
            runner,
            last_receipt: None,
            last_provider_failure: None,
        }
    }
}

fn wire_prompt(
    request: &ModelInvocationRequest,
    grammar: &OpenCodeGrammar,
) -> Result<String, ModelFailure> {
    if request.proposal_grammar_digest != grammar.digest {
        return Err(ModelFailure::Refused);
    }
    let bytes = request
        .task
        .len()
        .saturating_add(request.observation.len())
        .saturating_mul(2)
        .saturating_add(request.deployment_binding.len())
        .saturating_add(grammar.digest.len())
        .saturating_add(grammar.canonical_schema.len())
        .saturating_add(grammar.provider_schema.len());
    if bytes > MAX_PROMPT_BYTES {
        return Err(ModelFailure::Refused);
    }
    let prompt = format!(
        "SEMAPRAX live proposal v1\ntask={}\nobservation={}\ngrammar_digest={}\ndeployment={}\nturn={}\ncanonical_interaction_schema={}\nprovider_value_json_schema={}\nReturn one canonical semaprax.agent-interaction-value.v1 document bound to grammar_digest.\n",
        hex(&request.task), hex(&request.observation), grammar.digest, request.deployment_binding,
        request.turn, grammar.canonical_schema, grammar.provider_schema,
    );
    (prompt.len() <= MAX_PROMPT_BYTES)
        .then_some(prompt)
        .ok_or(ModelFailure::Refused)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub mod accounting;
mod environment;
pub mod provider_error;
mod receipt;
pub mod repair_adapter;
pub mod source;
use receipt::{event_text, validate_export};

fn failure(
    error: OpenCodeRunnerFailure,
    attempted_bytes: usize,
    cancelled: bool,
) -> ModelInvocationOutcome {
    let failure = if cancelled {
        ModelFailure::Cancelled
    } else {
        match error {
            OpenCodeRunnerFailure::Refused => ModelFailure::Refused,
            OpenCodeRunnerFailure::Timeout => ModelFailure::Timeout,
            OpenCodeRunnerFailure::Capacity => ModelFailure::CapacityExceeded,
            OpenCodeRunnerFailure::Provider => ModelFailure::ProviderError,
            OpenCodeRunnerFailure::Malformed => ModelFailure::MalformedResponse,
            OpenCodeRunnerFailure::Cancelled => ModelFailure::Cancelled,
            OpenCodeRunnerFailure::ProviderStatus(status) => match status {
                provider_error::OpenCodeProviderFailure::Refused
                | provider_error::OpenCodeProviderFailure::Authentication => ModelFailure::Refused,
                provider_error::OpenCodeProviderFailure::Incomplete => {
                    ModelFailure::MalformedResponse
                }
                _ => ModelFailure::ProviderError,
            },
        }
    };
    ModelInvocationOutcome::Failed {
        failure,
        attempted_bytes,
    }
}

impl<R: OpenCodeRunner> ModelHandler for OpenCodeModelHandler<R> {
    fn invoke(
        &mut self,
        _capability: &ModelInvokeCapability,
        request: &ModelInvocationRequest,
    ) -> ModelInvocationOutcome {
        self.last_receipt = None;
        self.last_provider_failure = None;
        let prompt = match wire_prompt(request, &self.config.grammar) {
            Ok(prompt) => prompt,
            Err(failure) => {
                return ModelInvocationOutcome::Failed {
                    failure,
                    attempted_bytes: 0,
                }
            }
        };
        self.invoke_prompt(&prompt, request.max_response_bytes)
    }
}

impl<R: OpenCodeRunner> OpenCodeModelHandler<R> {
    pub(super) fn invoke_prompt(
        &mut self,
        prompt: &str,
        max_response_bytes: usize,
    ) -> ModelInvocationOutcome {
        self.last_receipt = None;
        self.last_provider_failure = None;
        if prompt.len() > MAX_PROMPT_BYTES || max_response_bytes == 0 {
            return ModelInvocationOutcome::Failed {
                failure: ModelFailure::Refused,
                attempted_bytes: 0,
            };
        }
        if self.runner.cancelled(&self.config) {
            return ModelInvocationOutcome::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            };
        }
        let events = match self.runner.run(&self.config, prompt) {
            Ok(events) => events,
            Err(error) => {
                if let OpenCodeRunnerFailure::ProviderStatus(status) = error {
                    self.last_provider_failure = Some(status);
                }
                return failure(error, 0, self.runner.cancelled(&self.config));
            }
        };
        let attempted_bytes = events.len().min(max_response_bytes);
        if let Some(status) = provider_error::classify_provider_failure(&events) {
            self.last_provider_failure = Some(status);
            if self.runner.abandon(&self.config).is_err() {
                return failure(OpenCodeRunnerFailure::Refused, attempted_bytes, false);
            }
            return failure(
                OpenCodeRunnerFailure::ProviderStatus(status),
                attempted_bytes,
                self.runner.cancelled(&self.config),
            );
        }

        let (session, message, answer) = match event_text(&events) {
            Ok(event) => event,
            Err(error) => {
                if self.runner.abandon(&self.config).is_err() {
                    return failure(OpenCodeRunnerFailure::Refused, attempted_bytes, false);
                }
                return ModelInvocationOutcome::Failed {
                    failure: error,
                    attempted_bytes,
                };
            }
        };
        let export = match self.runner.export(&self.config, &session) {
            Ok(export) => export,
            Err(error) => {
                return failure(error, attempted_bytes, self.runner.cancelled(&self.config))
            }
        };
        match validate_export(&export, &events, &session, &message, prompt, &answer) {
            Ok(receipt) if answer.len() <= max_response_bytes => {
                self.last_receipt = Some(receipt);
                ModelInvocationOutcome::Settled(answer.into_bytes())
            }
            Ok(_) => ModelInvocationOutcome::Failed {
                failure: ModelFailure::MalformedResponse,
                attempted_bytes,
            },
            Err(error) => ModelInvocationOutcome::Failed {
                failure: error,
                attempted_bytes,
            },
        }
    }
}

/// A small inert executable identity for injected-runner tests on every host.
/// No test dispatches this file; the production config still snapshots and
/// authenticates its exact regular-file bytes.
#[cfg(test)]
fn fixture_executable() -> PathBuf {
    static EXECUTABLE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXECUTABLE
        .get_or_init(|| {
            let path = std::env::temp_dir().join(format!(
                "semaprax-opencode-fixture-executable-{}",
                std::process::id()
            ));
            std::fs::write(&path, b"fixture executable identity").unwrap();
            path
        })
        .clone()
}

#[cfg(test)]
mod bounds_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod source_tests;
