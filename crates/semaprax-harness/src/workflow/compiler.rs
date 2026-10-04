//! The compiler as a service. [`CompilerService`] is what the pipeline needs;
//! [`SubprocessCompiler`] implements it by running the real `semaprax`
//! executable at an explicit path with a cleared environment. Nothing here
//! links the compiler or writes project source.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use serde_json::Value;
use std::cell::RefCell;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_OUTPUT: usize = 16 * 1024 * 1024;

fn diag(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilerDiagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<u64>,
}

impl CompilerDiagnostic {
    pub fn to_json(&self) -> Value {
        serde_json::json!({"code": self.code, "message": self.message, "path": self.path, "line": self.line})
    }
}

/// Result of `semaprax check --json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckReport {
    pub ok: bool,
    /// Compiler project revision when verified.
    pub revision: Option<String>,
    pub diagnostics: Vec<CompilerDiagnostic>,
    pub raw_digest: String,
}

/// Result of `semaprax test --json`. The verdict is the compiler's alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestReport {
    pub passed: bool,
    pub project_revision: String,
    pub outcome: String,
    /// Stable id of the function whose contract failed, when the failure is a language failure.
    pub failing_function: Option<String>,
    pub failure: Option<String>,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceChange {
    pub path: String,
    pub base_digest: String,
    pub candidate_digest: String,
    pub replacement_source: String,
}

/// Result of `semaprax project-candidate-preview` (no tests were run by it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidatePreview {
    pub base_revision: String,
    pub candidate_revision: String,
    pub source_changes: Vec<SourceChange>,
    pub requirements: Vec<String>,
    pub change_requirements: Vec<Vec<String>>,
    pub unresolved_holes: u64,
    pub tests_state: String,
    pub digest: String,
}

/// Result of `semaprax project-candidate-export`: the recovery capsule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capsule {
    pub bytes: Vec<u8>,
    pub candidate_digest: String,
    pub base_revision: String,
    pub candidate_project_revision: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublishReceipt {
    pub published_commit: String,
    pub reference: String,
    pub raw: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishError {
    /// The compiler refused before any reference update.
    Refused(CompilerDiagnostic),
    /// The reference update may have happened; never a retry signal.
    Uncertain(String),
}

/// Everything the workflow asks of the compiler.
pub trait CompilerService {
    fn version(&self) -> String;
    fn check(&self, project: &Path) -> HarnessResult<CheckReport>;
    fn test(&self, project: &Path) -> HarnessResult<TestReport>;
    /// Bounded native context text for a stable id.
    fn context(&self, project: &Path, seed: &str, max_bytes: usize) -> HarnessResult<String>;
    fn candidate_preview(&self, project: &Path, change: &[u8]) -> HarnessResult<CandidatePreview>;
    fn candidate_export(&self, project: &Path, change: &[u8]) -> HarnessResult<Capsule>;
    fn publish(
        &self,
        project: &Path,
        capsule: &Capsule,
        approved_digest: &str,
        host_policy: &Path,
    ) -> Result<PublishReceipt, PublishError>;
    /// Commands run so far (subcommand and fixed flags only).
    fn commands(&self) -> Vec<String>;
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs the real `semaprax` executable. `scratch` receives change/capsule
/// files handed to the compiler by path.
pub struct SubprocessCompiler {
    exe: PathBuf,
    scratch: PathBuf,
    timeout: Duration,
    log: RefCell<Vec<String>>,
}

impl SubprocessCompiler {
    pub fn new(exe: PathBuf, scratch: PathBuf) -> HarnessResult<Self> {
        if !exe.is_absolute() {
            return Err(diag(
                "SPX-HPD001",
                "the compiler executable must be an absolute path",
            ));
        }
        if !exe.is_file() {
            return Err(diag(
                "SPX-HPD001",
                format!("compiler {} is not a file", exe.display()),
            ));
        }
        std::fs::create_dir_all(&scratch)
            .map_err(|e| diag("SPX-HPD002", format!("scratch {}: {e}", scratch.display())))?;
        Ok(Self {
            exe,
            scratch,
            timeout: Duration::from_secs(120),
            log: RefCell::default(),
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn run(&self, label: &str, args: &[&std::ffi::OsStr]) -> HarnessResult<Run> {
        self.log.borrow_mut().push(label.to_string());
        let mut child = Command::new(&self.exe)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| diag("SPX-HPD002", format!("cannot start the compiler: {e}")))?;
        let mut out = child.stdout.take().expect("piped");
        let mut err = child.stderr.take().expect("piped");
        let t_out = std::thread::spawn(move || read_bounded(&mut out));
        let t_err = std::thread::spawn(move || read_bounded(&mut err));
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if start.elapsed() > self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(diag(
                        "SPX-HPD002",
                        format!("`semaprax {label}` exceeded {:?}", self.timeout),
                    ));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(e) => return Err(diag("SPX-HPD002", format!("waiting for the compiler: {e}"))),
            }
        };
        let stdout = t_out.join().unwrap_or_default();
        let stderr = t_err.join().unwrap_or_default();
        Ok(Run {
            code: status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }

    fn write_scratch(&self, name: &str, bytes: &[u8]) -> HarnessResult<PathBuf> {
        let path = self.scratch.join(name);
        std::fs::write(&path, bytes)
            .map_err(|e| diag("SPX-HPD002", format!("scratch {}: {e}", path.display())))?;
        Ok(path)
    }
}

fn read_bounded(r: &mut impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = r.take(MAX_OUTPUT as u64).read_to_end(&mut buf);
    buf
}

/// Parse a plain-text `error[CODE]: message at path:line:col` diagnostic.
pub fn parse_text_diagnostic(text: &str) -> CompilerDiagnostic {
    let first = text.lines().next().unwrap_or("").trim();
    let (code, rest) = match first
        .strip_prefix("error[")
        .and_then(|s| s.split_once("]: "))
    {
        Some((c, r)) => (c.to_string(), r.to_string()),
        None => ("SPX-UNKNOWN".to_string(), first.to_string()),
    };
    let (message, path, line) = match rest.rsplit_once(" at ") {
        Some((m, loc)) => {
            let mut parts = loc.split(':');
            let p = parts.next().map(str::to_string);
            let l = parts.next().and_then(|x| x.parse().ok());
            (m.to_string(), p, l)
        }
        None => (rest, None, None),
    };
    CompilerDiagnostic {
        code,
        message,
        path,
        line,
    }
}

fn json_diag(v: &Value) -> Option<CompilerDiagnostic> {
    Some(CompilerDiagnostic {
        code: v.get("code")?.as_str()?.to_string(),
        message: v
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        path: v.get("path").and_then(Value::as_str).map(str::to_string),
        line: v.pointer("/location/line").and_then(Value::as_u64),
    })
}

fn str_at(v: &Value, ptr: &str, what: &str) -> HarnessResult<String> {
    v.pointer(ptr)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| diag("SPX-HPD003", format!("compiler output lacks `{what}`")))
}

fn parse_json(text: &str, what: &str) -> HarnessResult<Value> {
    serde_json::from_str(text.trim()).map_err(|e| {
        diag(
            "SPX-HPD003",
            format!("compiler `{what}` output is not JSON: {e}"),
        )
    })
}

fn refused(run: &Run, what: &str) -> HarnessDiagnostic {
    let d = parse_text_diagnostic(if run.stderr.trim().is_empty() {
        &run.stdout
    } else {
        &run.stderr
    });
    diag(
        "SPX-HPD040",
        format!("compiler refused `{what}`: {} {}", d.code, d.message),
    )
}

impl CompilerService for SubprocessCompiler {
    fn version(&self) -> String {
        self.run("--version", &["--version".as_ref()])
            .map(|r| r.stdout.trim().to_string())
            .unwrap_or_else(|_| "unknown".into())
    }

    fn check(&self, project: &Path) -> HarnessResult<CheckReport> {
        let r = self.run(
            "check --json",
            &["check".as_ref(), project.as_os_str(), "--json".as_ref()],
        )?;
        let raw_digest = sha256_plain(r.stdout.as_bytes());
        let mut diagnostics = Vec::new();
        let mut revision = None;
        for line in r.stdout.lines().filter(|l| !l.trim().is_empty()) {
            let v = parse_json(line, "check")?;
            if v.get("status").and_then(Value::as_str) == Some("verified") {
                revision = Some(str_at(&v, "/revision", "revision")?);
            } else if let Some(d) = json_diag(&v) {
                diagnostics.push(d);
            } else {
                return Err(diag("SPX-HPD003", "unrecognised `check --json` line"));
            }
        }
        if r.code != 0 && diagnostics.is_empty() {
            diagnostics.push(parse_text_diagnostic(if r.stderr.trim().is_empty() {
                &r.stdout
            } else {
                &r.stderr
            }));
        }
        let ok = r.code == 0 && revision.is_some() && diagnostics.is_empty();
        Ok(CheckReport {
            ok,
            revision,
            diagnostics,
            raw_digest,
        })
    }

    fn test(&self, project: &Path) -> HarnessResult<TestReport> {
        let r = self.run(
            "test --json",
            &["test".as_ref(), project.as_os_str(), "--json".as_ref()],
        )?;
        let v = parse_json(&r.stdout, "test")?;
        let outcome = str_at(&v, "/outcome/kind", "outcome.kind")?;
        let passed = r.code == 0
            && outcome == "returned"
            && v.pointer("/outcome/value").and_then(Value::as_str) == Some("0");
        let failing_function = v
            .pointer("/outcome/failure/function")
            .and_then(Value::as_str)
            .map(str::to_string);
        let failure = v.pointer("/outcome/failure").map(|f| {
            let g = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            format!("{} {}: {}", g("function"), g("phase"), g("clause"))
        });
        Ok(TestReport {
            passed,
            project_revision: str_at(&v, "/project_revision", "project_revision")?,
            outcome,
            failing_function,
            failure,
            report_digest: sha256_plain(r.stdout.as_bytes()),
        })
    }

    fn context(&self, project: &Path, seed: &str, max_bytes: usize) -> HarnessResult<String> {
        let max = max_bytes.to_string();
        let r = self.run(
            "context <seed> --direction both --depth 1",
            &[
                "context".as_ref(),
                project.as_os_str(),
                seed.as_ref(),
                "--direction".as_ref(),
                "both".as_ref(),
                "--depth".as_ref(),
                "1".as_ref(),
                "--max-bytes".as_ref(),
                max.as_ref(),
            ],
        )?;
        if r.code != 0 {
            return Err(refused(&r, "context"));
        }
        parse_json(&r.stdout, "context")?;
        Ok(r.stdout.trim().to_string())
    }

    fn candidate_preview(&self, project: &Path, change: &[u8]) -> HarnessResult<CandidatePreview> {
        let file = self.write_scratch("change.json", change)?;
        let manifest = project.join(super::snapshot::MANIFEST);
        let r = self.run(
            "project-candidate-preview",
            &[
                "project-candidate-preview".as_ref(),
                manifest.as_os_str(),
                file.as_os_str(),
            ],
        )?;
        if r.code != 0 {
            return Err(refused(&r, "project-candidate-preview"));
        }
        let v = parse_json(&r.stdout, "project-candidate-preview")?;
        let strings = |p: &Value| -> Vec<String> {
            p.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut source_changes = Vec::new();
        for c in v
            .get("source_changes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            source_changes.push(SourceChange {
                path: str_at(c, "/path", "source_changes.path")?,
                base_digest: str_at(c, "/base_digest", "source_changes.base_digest")?,
                candidate_digest: str_at(
                    c,
                    "/candidate_digest",
                    "source_changes.candidate_digest",
                )?,
                replacement_source: str_at(
                    c,
                    "/replacement_source",
                    "source_changes.replacement_source",
                )?,
            });
        }
        Ok(CandidatePreview {
            base_revision: str_at(&v, "/base_revision", "base_revision")?,
            candidate_revision: str_at(&v, "/candidate_revision", "candidate_revision")?,
            source_changes,
            requirements: strings(v.get("requirements").unwrap_or(&Value::Null)),
            change_requirements: v
                .get("changes")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|c| strings(c.get("requirements").unwrap_or(&Value::Null)))
                        .collect()
                })
                .unwrap_or_default(),
            unresolved_holes: v
                .pointer("/validation/unresolved_holes")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX),
            tests_state: str_at(&v, "/validation/tests", "validation.tests")?,
            digest: sha256_plain(r.stdout.as_bytes()),
        })
    }

    fn candidate_export(&self, project: &Path, change: &[u8]) -> HarnessResult<Capsule> {
        let file = self.write_scratch("change.json", change)?;
        let manifest = project.join(super::snapshot::MANIFEST);
        let r = self.run(
            "project-candidate-export",
            &[
                "project-candidate-export".as_ref(),
                manifest.as_os_str(),
                file.as_os_str(),
            ],
        )?;
        if r.code != 0 {
            return Err(refused(&r, "project-candidate-export"));
        }
        let v = parse_json(&r.stdout, "project-candidate-export")?;
        Ok(Capsule {
            candidate_digest: str_at(&v, "/candidate_digest", "candidate_digest")?,
            base_revision: str_at(&v, "/base_revision", "base_revision")?,
            candidate_project_revision: str_at(
                &v,
                "/candidate_project_revision",
                "candidate_project_revision",
            )?,
            bytes: r.stdout.into_bytes(),
        })
    }

    fn publish(
        &self,
        project: &Path,
        capsule: &Capsule,
        approved_digest: &str,
        host_policy: &Path,
    ) -> Result<PublishReceipt, PublishError> {
        let uncertain = |m: String| PublishError::Uncertain(m);
        let file = self
            .write_scratch("capsule.json", &capsule.bytes)
            .map_err(|e| {
                PublishError::Refused(CompilerDiagnostic {
                    code: e.code.into(),
                    message: e.message,
                    path: None,
                    line: None,
                })
            })?;
        let manifest = project.join(super::snapshot::MANIFEST);
        let r = self
            .run(
                "project-candidate-git-publish",
                &[
                    "project-candidate-git-publish".as_ref(),
                    manifest.as_os_str(),
                    file.as_os_str(),
                    approved_digest.as_ref(),
                    host_policy.as_os_str(),
                ],
            )
            .map_err(|e| uncertain(format!("{}: {}", e.code, e.message)))?;
        if r.code == 0 {
            let v: Value = serde_json::from_str(r.stdout.trim())
                .map_err(|e| uncertain(format!("publication receipt is not JSON: {e}")))?;
            let get = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            return Ok(PublishReceipt {
                published_commit: get("published_commit"),
                reference: get("reference"),
                raw: v,
            });
        }
        let text = format!("{}{}", r.stderr, r.stdout);
        // SPX-G267 is the compiler's explicit "publication may have occurred".
        if r.code < 0 || text.contains("SPX-G267") || text.contains("publication may have occurred")
        {
            return Err(uncertain(text.trim().to_string()));
        }
        Err(PublishError::Refused(parse_text_diagnostic(&text)))
    }

    fn commands(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}
