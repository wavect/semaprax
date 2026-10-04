//! Explicit adoption of an existing installation, and the restricted upstream
//! identity probe. Adoption downloads nothing, installs nothing and edits no
//! tool's configuration; the only process it may start is the descriptor's own
//! `identity_probe` against an executable the user named by absolute path.

use super::builtin;
use super::installations::{entry_path, file_digest, Installation, LocalState, UpstreamRecord};
use crate::cli::Environment;
use crate::contract::descriptor::parse_version;
use crate::contract::{Descriptor, Runtime};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_OUTPUT_CAP: usize = 64 * 1024;
static PROBE_SERIAL: AtomicUsize = AtomicUsize::new(0);

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug)]
pub struct AdoptOptions {
    pub upstream: Option<PathBuf>,
    pub project: PathBuf,
    /// Permit an upstream inside the project root (never automatic).
    pub allow_project_local: bool,
}

#[derive(Clone, Debug)]
pub struct AdoptReport {
    pub installation: Installation,
    pub notes: Vec<String>,
}

/// First dotted-numeric token of probe output (`graft 0.18.0`, `v1.2.3`).
pub fn extract_version(output: &str) -> Option<String> {
    output
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')' | ':' | '"'))
        .map(|t| t.strip_prefix('v').unwrap_or(t))
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()) && parse_version(t).is_some())
        .map(str::to_string)
}

fn read_capped<R: Read + Send + 'static>(mut r: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = r.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let room = PROBE_OUTPUT_CAP.saturating_sub(buf.len());
            buf.extend_from_slice(&chunk[..n.min(room)]);
        }
        buf
    })
}

/// Run `exe <argv>` with a scrubbed environment, a private temp working
/// directory, a 5 s deadline and a 64 KiB output cap. `Err` explains why no
/// version could be read.
pub fn run_probe(
    home: &Path,
    exe: &Path,
    argv: &[String],
    runtime_dirs: &[PathBuf],
) -> Result<String, String> {
    let dir = home.join("tmp").join(format!(
        "probe-{}-{}",
        std::process::id(),
        PROBE_SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create probe directory: {e}"))?;
    let result = (|| {
        let mut child = Command::new(exe)
            .args(argv)
            .env_clear()
            // Interpreter-shebang upstreams (`#!/usr/bin/env node`) need their
            // runtime: only the directories of the explicitly named runtimes.
            .env("PATH", {
                let mut p: Vec<String> = runtime_dirs
                    .iter()
                    .map(|d| d.display().to_string())
                    .collect();
                p.extend(["/usr/bin".into(), "/bin".into()]);
                p.join(":")
            })
            .env("HOME", &dir)
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start the identity probe: {e}"))?;
        let out = read_capped(child.stdout.take().expect("piped"));
        let err = read_capped(child.stderr.take().expect("piped"));
        let deadline = Instant::now() + PROBE_TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("identity probe exceeded 5s and was killed".to_string());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => return Err(format!("identity probe failed: {e}")),
            }
        };
        let text = |h: std::thread::JoinHandle<Vec<u8>>| {
            String::from_utf8_lossy(&h.join().unwrap_or_default()).into_owned()
        };
        let (stdout, stderr) = (text(out), text(err));
        if !status.success() {
            return Err(format!("identity probe exited with {status}"));
        }
        extract_version(&stdout)
            .or_else(|| extract_version(&stderr))
            .ok_or_else(|| "identity probe printed no version".to_string())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

pub fn adopt(
    env: &Environment,
    descriptor_path: &Path,
    opts: &AdoptOptions,
) -> HarnessResult<AdoptReport> {
    let mut state = LocalState::load(env)?;
    let home = state.home()?.to_path_buf();
    let path = descriptor_path.canonicalize().map_err(|e| {
        bad(
            "SPX-HPB021",
            format!("cannot read descriptor {}: {e}", descriptor_path.display()),
        )
    })?;
    let bytes = std::fs::read(&path)
        .map_err(|e| bad("SPX-HPB021", format!("cannot read descriptor: {e}")))?;
    let d = Descriptor::parse(&bytes)?;
    if d.runtime == Runtime::Builtin || builtin::is_builtin(&d.provider_id) {
        return Err(bad(
            "SPX-HPB021",
            format!(
                "`{}` is a builtin identity; builtin providers are compiled in and are not adopted",
                d.provider_id
            ),
        ));
    }
    if d.entry
        .first()
        .is_some_and(|e| e.starts_with('/') || e.split('/').any(|s| s == ".."))
    {
        return Err(bad(
            "SPX-HPB021",
            "an adapter `entry` must be relative to the descriptor directory",
        ));
    }
    let entry = entry_path(&path, &d).expect("non-builtin has an entry");
    let entry_digest = file_digest(&entry).map_err(|e| {
        bad(
            "SPX-HPB021",
            format!("adapter entry {} unreadable: {e}", entry.display()),
        )
    })?;

    // Bind the whole adapter closure (entry, helper modules, descriptor), not
    // just the entry file: helper changes must change the identity.
    let closure = super::installations::closure_label_of(&path, &d)
        .map_err(|e| bad("SPX-HPB021", e.message))?;
    let mut notes = vec![format!("adapter identity artifact-v2 recorded ({closure}); the entry file alone was {entry_digest}")];
    let mut upstream = None;
    match (&d.upstream, &opts.upstream) {
        (None, Some(_)) => {
            return Err(bad(
                "SPX-HPB021",
                "the descriptor declares no upstream; --upstream does not apply",
            ))
        }
        (Some(up), None) => {
            if let Some(old) = state
                .installations
                .get(&d.provider_id)
                .and_then(|i| i.upstream.clone())
            {
                notes.push("kept the previously adopted upstream record (re-checked by digest on every resolution)".to_string());
                upstream = Some(old);
            } else {
                notes.push(format!(
            "upstream `{}` was not given. Nothing was installed or downloaded. Install {} ({}) yourself from {}, then run `semaprax harness adopt {} --upstream <absolute path to the executable>`.",
            up.name, up.package, up.versions.join(" or "), up.repository, path.display()
                ));
            }
        }
        (Some(up), Some(exe)) => {
            if !exe.is_absolute() {
                return Err(bad(
                    "SPX-HPB021",
                    "--upstream must be an absolute path; PATH is never searched",
                ));
            }
            let exe = exe.canonicalize().map_err(|e| {
                bad(
                    "SPX-HPB021",
                    format!("upstream {} unreadable: {e}", exe.display()),
                )
            })?;
            if !exe.is_file() {
                return Err(bad(
                    "SPX-HPB021",
                    format!("upstream {} is not a file", exe.display()),
                ));
            }
            let root = opts
                .project
                .canonicalize()
                .unwrap_or_else(|_| opts.project.clone());
            if exe.starts_with(&root) && !opts.allow_project_local {
                return Err(bad(
                    "SPX-HPB024",
                    format!("upstream {} is inside the project; a workspace-supplied executable is refused unless you pass --allow-project-local", exe.display()),
                ));
            }
            let digest = file_digest(&exe)
                .map_err(|e| bad("SPX-HPB021", format!("cannot hash upstream: {e}")))?;
            let (version, compatible) = if up.identity_probe.is_empty() {
                notes.push("the descriptor declares no identity probe; the upstream is recorded as unidentified".into());
                (None, false)
            } else {
                let runtime_dirs: Vec<PathBuf> = ["HARNESS_NODE", "HARNESS_PYTHON"]
                    .iter()
                    .filter_map(|k| env.vars.get(*k))
                    .filter_map(|v| Path::new(v).parent().map(Path::to_path_buf))
                    .filter(|d| d.is_absolute())
                    .collect();
                match run_probe(&home, &exe, &up.identity_probe, &runtime_dirs) {
                    Ok(v) => {
                        let ok = up.versions.contains(&v);
                        if !ok {
                            notes.push(format!("detected `{}` {v}, which is not supported (supported: {}); recorded as detected-but-incompatible", up.name, up.versions.join(", ")));
                        }
                        (Some(v), ok)
                    }
                    Err(why) => {
                        notes.push(format!("{why}; recorded as unidentified and incompatible"));
                        (None, false)
                    }
                }
            };
            upstream = Some(UpstreamRecord {
                path: exe,
                digest,
                version,
                compatible,
            });
        }
        (None, None) => {}
    }

    let installation = Installation {
        provider_id: d.provider_id.clone(),
        descriptor_path: path,
        descriptor_digest: d.digest().to_string(),
        entry_digest: Some(closure),
        upstream,
        runtime: state
            .installations
            .get(&d.provider_id)
            .and_then(|i| i.runtime.clone()),
    };
    state
        .installations
        .insert(d.provider_id.clone(), installation.clone());
    state.save_installations()?;
    Ok(AdoptReport {
        installation,
        notes,
    })
}

fn outside_project(p: &Path, project: &Path, what: &str) -> HarnessResult<PathBuf> {
    if !p.is_absolute() {
        return Err(bad(
            "SPX-HPB021",
            format!("{what} must be an absolute path; PATH is never searched"),
        ));
    }
    let c = p.canonicalize().map_err(|e| {
        bad(
            "SPX-HPB021",
            format!("{what} {} unreadable: {e}", p.display()),
        )
    })?;
    let root = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    if c.starts_with(&root) {
        return Err(bad(
            "SPX-HPB024",
            format!("{what} {} is inside the project; machine-local choices are never taken from the workspace", c.display()),
        ));
    }
    Ok(c)
}

/// Record the explicit runtime executable (`node`/`python`) for an adopted
/// provider. Machine-local; the project can never name it.
pub fn set_runtime(
    env: &Environment,
    provider_id: &str,
    runtime: &Path,
    project: &Path,
) -> HarnessResult<PathBuf> {
    let mut state = LocalState::load(env)?;
    let exe = outside_project(runtime, project, "--runtime")?;
    if !exe.is_file() {
        return Err(bad(
            "SPX-HPB021",
            format!("--runtime {} is not a file", exe.display()),
        ));
    }
    let inst = state.installations.get_mut(provider_id).ok_or_else(|| {
        bad(
            "SPX-HPB023",
            format!("provider `{provider_id}` is not adopted"),
        )
    })?;
    inst.runtime = Some(exe.clone());
    state.save_installations()?;
    Ok(exe)
}

/// Approve a machine-local skill root. Never read from the project.
pub fn add_skill_root(
    env: &Environment,
    dir: &Path,
    origin: Option<&str>,
    project: &Path,
) -> HarnessResult<super::installations::SkillRootRecord> {
    let mut state = LocalState::load(env)?;
    let path = outside_project(dir, project, "--skills")?;
    if !path.is_dir() {
        return Err(bad(
            "SPX-HPB021",
            format!("--skills {} is not a directory", path.display()),
        ));
    }
    let rec = super::installations::SkillRootRecord {
        origin: origin
            .map(String::from)
            .unwrap_or_else(|| format!("local:{}", path.display())),
        path,
    };
    state.skill_roots.retain(|r| r.path != rec.path);
    state.skill_roots.push(rec.clone());
    state.skill_roots.sort_by(|a, b| a.path.cmp(&b.path));
    state.save_installations()?;
    Ok(rec)
}
