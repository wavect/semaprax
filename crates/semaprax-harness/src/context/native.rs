//! The compiler as a service. Native `.spx` facts always come from the real
//! `semaprax context` engine; the broker embeds its output verbatim and never
//! re-derives, reorders or reinterprets it.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Smallest `--max-bytes` the compiler accepts.
pub const COMPILER_MIN_BYTES: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeQuery {
    pub depth: u32,
    pub max_bytes: usize,
    pub filters: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeFacts {
    /// Compiler stable id of the root declaration.
    pub root: String,
    /// Exact standalone stdout, without the final line break.
    pub raw: String,
    pub truncated: bool,
}

pub trait NativeContextSource {
    /// Identity of the compiler answering (part of every cache key's world).
    fn identity(&self) -> String;
    fn facts(&self, project: &Path, target: &str, q: &NativeQuery) -> HarnessResult<NativeFacts>;
    /// True iff the compiler resolves `stable_id` to itself in `project`.
    fn confirm_identity(&self, project: &Path, stable_id: &str) -> bool {
        let q = NativeQuery {
            depth: 0,
            max_bytes: COMPILER_MIN_BYTES,
            filters: vec![],
        };
        self.facts(project, stable_id, &q)
            .map(|f| f.root == stable_id)
            .unwrap_or(false)
    }
}

pub fn parse_facts(stdout: &str) -> HarnessResult<NativeFacts> {
    let raw = stdout.strip_suffix('\n').unwrap_or(stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        HarnessDiagnostic::new(
            "SPX-HPE020",
            format!("compiler context output is not JSON: {e}"),
        )
    })?;
    // File mode names `root`; project mode names `target[0]`.
    let root = v["root"]
        .as_str()
        .or_else(|| v["target"][0].as_str())
        .ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPE020",
                "compiler context output names no root declaration",
            )
        })?
        .to_string();
    let truncated = v["truncation"]["truncated"].as_bool().unwrap_or(false);
    Ok(NativeFacts {
        root,
        raw,
        truncated,
    })
}

/// Runs the compiler at an explicit absolute path with a cleared environment.
pub struct SubprocessNative {
    pub compiler: PathBuf,
    pub timeout: Duration,
}

impl SubprocessNative {
    pub fn new(compiler: PathBuf) -> Self {
        Self {
            compiler,
            timeout: Duration::from_secs(60),
        }
    }
}

impl NativeContextSource for SubprocessNative {
    fn identity(&self) -> String {
        let out = Command::new(&self.compiler)
            .arg("--version")
            .env_clear()
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(_) => "unknown".into(),
        }
    }

    fn facts(&self, project: &Path, target: &str, q: &NativeQuery) -> HarnessResult<NativeFacts> {
        if !self.compiler.is_absolute() {
            return Err(HarnessDiagnostic::new(
                "SPX-HPE021",
                "compiler path must be absolute",
            ));
        }
        let mut cmd = Command::new(&self.compiler);
        cmd.arg("context").arg(project).arg(target);
        cmd.arg("--depth").arg(q.depth.to_string());
        cmd.arg("--max-bytes").arg(q.max_bytes.to_string());
        // Filters exist only for single-file inputs; project mode refuses them.
        if !q.filters.is_empty() && !project.is_dir() {
            cmd.arg("--filters").arg(q.filters.join(","));
        }
        cmd.env_clear()
            .env("PATH", "/usr/bin:/bin")
            .current_dir(if project.is_dir() {
                project
            } else {
                project.parent().unwrap_or(project)
            });
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| {
            HarnessDiagnostic::new(
                "SPX-HPE021",
                format!("cannot run compiler {}: {e}", self.compiler.display()),
            )
        })?;
        let mut out = child.stdout.take().expect("piped");
        let mut errp = child.stderr.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let ereader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = errp.by_ref().take(8192).read_to_string(&mut s);
            s
        });
        let until = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(5)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(HarnessDiagnostic::new(
                        "SPX-HPE021",
                        "compiler context timed out",
                    ));
                }
            }
        };
        let stdout = reader.join().unwrap_or_default();
        let stderr = ereader.join().unwrap_or_default();
        if !status.success() {
            return Err(HarnessDiagnostic::new(
                "SPX-HPE022",
                format!(
                    "compiler refused context for `{target}`: {}",
                    stderr.lines().next().unwrap_or("").trim()
                ),
            ));
        }
        parse_facts(&stdout)
    }
}
