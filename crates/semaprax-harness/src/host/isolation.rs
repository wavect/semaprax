//! OS-enforced restriction. A plain subprocess is never labelled sandboxed:
//! `Restricted` is honoured only through `sandbox-exec` (macOS) or `bwrap`
//! (Linux, `/usr/bin/bwrap`); where neither exists the request is refused with
//! `SPX-HPC003`, never silently downgraded.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkPolicy {
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IsolationRequest {
    /// Trusted native/subprocess execution: no OS enforcement.
    None,
    /// File reads/writes only below the listed roots (plus the host-supplied
    /// runtime and cache roots) and no network.
    Restricted {
        allow_read: Vec<PathBuf>,
        allow_write: Vec<PathBuf>,
        network: NetworkPolicy,
    },
}

/// How an adapter actually ran. `Subprocess` is never "sandboxed".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolationMode {
    Subprocess,
    OsEnforced { mechanism: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Os {
    Macos,
    Linux,
    Other,
}

/// Which enforcement tools exist. Only fixed absolute paths are probed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsolationBackend {
    os: Os,
    sandbox_exec: Option<PathBuf>,
    bwrap: Option<PathBuf>,
}

/// Command prefix that launches the adapter under enforcement.
pub(crate) struct Wrapper {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

const SYSTEM_READ: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/System",
    "/Library",
    "/private/etc",
    "/private/var/db",
    "/dev",
    "/etc",
    "/var/db",
    "/Applications/Xcode.app",
    "/opt/homebrew",
];

impl IsolationBackend {
    pub fn detect() -> Self {
        let exists = |p: &str| Some(PathBuf::from(p)).filter(|p| p.is_file());
        Self {
            os: if cfg!(target_os = "macos") {
                Os::Macos
            } else if cfg!(target_os = "linux") {
                Os::Linux
            } else {
                Os::Other
            },
            sandbox_exec: if cfg!(target_os = "macos") {
                exists("/usr/bin/sandbox-exec")
            } else {
                None
            },
            bwrap: if cfg!(target_os = "linux") {
                exists("/usr/bin/bwrap")
            } else {
                None
            },
        }
    }

    /// A backend that can enforce nothing (tests the refusal path).
    pub fn unavailable() -> Self {
        Self {
            os: Os::Other,
            sandbox_exec: None,
            bwrap: None,
        }
    }

    /// Mechanism this host would use for `Restricted`, if any.
    pub fn mechanism(&self) -> Option<&'static str> {
        match (self.os, &self.sandbox_exec, &self.bwrap) {
            (Os::Macos, Some(_), _) => Some("sandbox-exec"),
            (Os::Linux, _, Some(_)) => Some("bwrap"),
            _ => None,
        }
    }

    /// Refuse what cannot be enforced; otherwise the launch prefix (if any).
    /// `read`/`write`/`exec` are the host-computed roots added to the request.
    pub(crate) fn plan(
        &self,
        request: &IsolationRequest,
        read: &[PathBuf],
        write: &[PathBuf],
        exec: &[PathBuf],
    ) -> HarnessResult<(IsolationMode, Option<Wrapper>)> {
        let IsolationRequest::Restricted {
            allow_read,
            allow_write,
            network: NetworkPolicy::Deny,
        } = request
        else {
            return Ok((IsolationMode::Subprocess, None));
        };
        let mut r = canon_all(allow_read.iter().chain(read))?;
        let w = canon_all(allow_write.iter().chain(write))?;
        let x = canon_all(exec.iter())?;
        r.extend(w.iter().cloned());
        let Some(mechanism) = self.mechanism() else {
            return Err(HarnessDiagnostic::new(
                "SPX-HPC003",
                "restricted isolation is not enforceable on this host (no sandbox-exec or bwrap); refusing instead of downgrading",
            ));
        };
        let wrapper = if mechanism == "sandbox-exec" {
            Wrapper {
                program: self.sandbox_exec.clone().expect("checked"),
                args: vec!["-p".into(), macos_profile(&r, &w, &x)?.into()],
            }
        } else {
            Wrapper {
                program: self.bwrap.clone().expect("checked"),
                args: bwrap_args(&r, &w),
            }
        };
        Ok((IsolationMode::OsEnforced { mechanism }, Some(wrapper)))
    }
}

fn canon_all<'a>(paths: impl Iterator<Item = &'a PathBuf>) -> HarnessResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        let c = p.canonicalize().map_err(|e| {
            HarnessDiagnostic::new(
                "SPX-HPC003",
                format!("isolation root {} is not usable: {e}", p.display()),
            )
        })?;
        if !out.contains(&c) {
            out.push(c);
        }
    }
    Ok(out)
}

fn quote(p: &Path) -> HarnessResult<String> {
    let s = p
        .to_str()
        .filter(|s| !s.chars().any(char::is_control))
        .ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPC003",
                format!("isolation root {} is not representable", p.display()),
            )
        })?;
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Deny-by-default SBPL: no network, reads only below system and allowed
/// roots (metadata everywhere so path resolution works), writes only below
/// write roots, exec only from system and exec roots.
pub(crate) fn macos_profile(
    read: &[PathBuf],
    write: &[PathBuf],
    exec: &[PathBuf],
) -> HarnessResult<String> {
    let mut p = String::from(
        "(version 1)\n(deny default)\n(deny network*)\n(allow process-fork)\n(allow sysctl-read)\n\
         (allow signal (target self))\n(allow file-read-metadata)\n(allow file-read* (literal \"/\"))\n\
         (allow file-write* (literal \"/dev/null\") (literal \"/dev/dtracehelper\"))\n",
    );
    p.push_str("(allow file-read*");
    for s in SYSTEM_READ {
        p.push_str(&format!(" (subpath \"{s}\")"));
    }
    for r in read {
        p.push_str(&format!(" (subpath {})", quote(r)?));
    }
    p.push_str(")\n(allow process-exec");
    for s in [
        "/usr",
        "/bin",
        "/sbin",
        "/System",
        "/Library",
        "/Applications/Xcode.app",
        "/opt/homebrew",
    ] {
        p.push_str(&format!(" (subpath \"{s}\")"));
    }
    for x in exec {
        p.push_str(&format!(" (subpath {})", quote(x)?));
    }
    p.push_str(")\n");
    if !write.is_empty() {
        p.push_str("(allow file-write*");
        for w in write {
            p.push_str(&format!(" (subpath {})", quote(w)?));
        }
        p.push_str(")\n");
    }
    Ok(p)
}

/// Not executed on this development host (see docs/HARNESS-HOST-V1.md).
pub(crate) fn bwrap_args(read: &[PathBuf], write: &[PathBuf]) -> Vec<OsString> {
    let mut a: Vec<OsString> = [
        "--die-with-parent",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-pid",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
    ]
    .map(OsString::from)
    .into();
    for sys in ["/usr", "/bin", "/lib", "/lib64", "/etc/ld.so.cache"] {
        a.extend(["--ro-bind-try".into(), sys.into(), sys.into()]);
    }
    for r in read.iter().filter(|r| !write.contains(r)) {
        a.extend(["--ro-bind".into(), r.into(), r.into()]);
    }
    for w in write {
        a.extend(["--bind".into(), w.into(), w.into()]);
    }
    a.push("--".into());
    a
}

impl IsolationBackend {
    #[cfg(test)]
    pub(crate) fn for_tests(os_macos: bool, tool: Option<&str>) -> Self {
        let t = tool.map(PathBuf::from);
        if os_macos {
            Self {
                os: Os::Macos,
                sandbox_exec: t,
                bwrap: None,
            }
        } else {
            Self {
                os: Os::Linux,
                sandbox_exec: None,
                bwrap: t,
            }
        }
    }
}
