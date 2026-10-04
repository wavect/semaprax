//! `setup` discovery: locate and identify tools only in user-approved
//! directories or explicit absolute paths, never in the project, never via
//! PATH. The only process started is a descriptor's `identity_probe` (or
//! `--version` for an interpreter), through the bounded `adopt` probe.

use super::adopt::run_probe;
use super::installations::file_digest;
use crate::contract::{Descriptor, Runtime};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Names setup can look for, and the executable names that satisfy each.
pub const TOOLS: &[(&str, &[&str])] = &[
    ("rtk", &["rtk"]),
    ("graft", &["graft"]),
    ("graphify", &["graphify"]),
    ("node", &["node"]),
    ("python", &["python3", "python"]),
];

pub fn is_tool(name: &str) -> bool {
    TOOLS.iter().any(|(n, _)| *n == name)
}

/// A usable provider: upstream (and interpreter) chosen and identified.
#[derive(Clone, Debug)]
pub struct Ready {
    pub upstream: PathBuf,
    pub upstream_digest: String,
    pub version: String,
    pub runtime: Option<PathBuf>,
}

/// Outcome of evaluating one shipped provider.
#[derive(Clone, Debug)]
pub enum Evaluation {
    Ready(Ready),
    /// Why it cannot be used, with an actionable hint folded in.
    Unavailable(String),
}

pub struct Finder {
    pub project: PathBuf,
    pub path_dirs: Vec<PathBuf>,
    pub explicit: BTreeMap<String, PathBuf>,
    pub scratch: PathBuf,
    /// Notes about refused or skipped locations, in discovery order.
    pub notes: Vec<String>,
}

fn executable(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

impl Finder {
    fn inside_project(&self, p: &Path) -> bool {
        p.starts_with(&self.project)
    }

    /// Candidate executables for `name`, in approved order, de-duplicated by
    /// real path. Project-local ones are refused and noted.
    pub fn candidates(&mut self, name: &str) -> Vec<PathBuf> {
        let names: &[&str] = TOOLS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[], |(_, e)| *e);
        let mut raw: Vec<PathBuf> = Vec::new();
        if let Some(p) = self.explicit.get(name) {
            raw.push(p.clone());
        } else {
            for d in &self.path_dirs {
                for n in names {
                    raw.push(d.join(n));
                }
            }
        }
        let mut out: Vec<PathBuf> = Vec::new();
        for p in raw {
            let Ok(real) = p.canonicalize() else {
                if self.explicit.contains_key(name) {
                    self.notes
                        .push(format!("--tool {name}={} does not exist", p.display()));
                }
                continue;
            };
            if !executable(&real) {
                self.notes.push(format!(
                    "{} is not an executable file; skipped",
                    real.display()
                ));
                continue;
            }
            if self.inside_project(&real) {
                self.notes.push(format!(
                    "{name} candidate {} is inside the project; a repository-supplied executable is never auto-trusted (adopt it by hand with --allow-project-local if you mean it)",
                    real.display()
                ));
                continue;
            }
            if !out.contains(&real) {
                out.push(real);
            }
        }
        out
    }

    fn probe(&self, exe: &Path, argv: &[String], runtime: Option<&Path>) -> Result<String, String> {
        let dirs: Vec<PathBuf> = runtime
            .and_then(Path::parent)
            .map(|d| vec![d.to_path_buf()])
            .unwrap_or_default();
        let r = run_probe(&self.scratch, exe, argv, &dirs);
        let _ = std::fs::remove_dir(self.scratch.join("tmp"));
        r
    }

    /// First interpreter candidate for `name` (`node` / `python`) that reports a version.
    pub fn interpreter(&mut self, name: &str) -> Result<(PathBuf, String), String> {
        let cands = self.candidates(name);
        if cands.is_empty() {
            return Err(format!(
                "no {name} found in the approved locations (--path-dirs / --tool {name}=<abs path>)"
            ));
        }
        let mut why = Vec::new();
        for c in cands {
            match self.probe(&c, &["--version".to_string()], None) {
                Ok(v) => return Ok((c, v)),
                Err(e) => why.push(format!("{}: {e}", c.display())),
            }
        }
        Err(format!("no usable {name}: {}", why.join("; ")))
    }

    /// Evaluate a shipped provider: platform, upstream identity, interpreter.
    pub fn evaluate(
        &mut self,
        short: &str,
        d: &Descriptor,
        node: &Result<(PathBuf, String), String>,
        python: &Result<(PathBuf, String), String>,
    ) -> Evaluation {
        let platform = super::resolve::current_platform();
        if !d.platforms.contains(&platform) {
            return Evaluation::Unavailable(format!(
                "the shipped descriptor lists {} and not this platform ({platform}); it is untested here, so it is not chosen",
                d.platforms.join(", ")
            ));
        }
        let runtime = match d.runtime {
            Runtime::Node => match node {
                Ok((p, _)) => Some(p.clone()),
                Err(e) => return Evaluation::Unavailable(format!("needs node: {e}")),
            },
            Runtime::Python => match python {
                Ok((p, _)) => Some(p.clone()),
                Err(e) => return Evaluation::Unavailable(format!("needs python: {e}")),
            },
            _ => None,
        };
        let Some(up) = &d.upstream else {
            return Evaluation::Unavailable("descriptor declares no upstream".into());
        };
        let cands = self.candidates(short);
        if cands.is_empty() {
            return Evaluation::Unavailable(format!(
                "`{}` was not found in the approved locations; install {} ({}) from {} and pass --path-dirs or --tool {}=<abs path>, or use a pinned managed install via `semaprax harness updates` when available",
                up.name, up.package, up.versions.join(" or "), up.repository, short
            ));
        }
        let mut why = Vec::new();
        for c in cands {
            match self.probe(&c, &up.identity_probe, runtime.as_deref()) {
                Ok(v) if up.versions.contains(&v) => {
                    let Ok(digest) = file_digest(&c) else {
                        why.push(format!("{} cannot be hashed", c.display()));
                        continue;
                    };
                    return Evaluation::Ready(Ready {
                        upstream: c,
                        upstream_digest: digest,
                        version: v,
                        runtime,
                    });
                }
                Ok(v) => why.push(format!(
                    "{} is {} {v}, installed but untested (supported: {}); not chosen and no other version is substituted",
                    c.display(), up.name, up.versions.join(", ")
                )),
                Err(e) => why.push(format!("{}: {e}; unidentified", c.display())),
            }
        }
        Evaluation::Unavailable(why.join("; "))
    }
}
