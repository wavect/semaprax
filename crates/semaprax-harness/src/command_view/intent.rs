//! Argv authorization: free-form shell, shell syntax, remote execution and
//! unresolvable executables are refused before any launch. Used for both the
//! original intent and a provider's effective wrapped argv.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn refuse(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "csh", "tcsh", "fish", "ash", "eval",
];
/// Programs whose purpose is to reach another host.
const REMOTE: &[&str] = &[
    "ssh", "scp", "sftp", "rsync", "nc", "ncat", "telnet", "curl", "wget",
];
/// Launchers that run the rest of argv as a command.
const LAUNCHERS: &[&str] = &[
    "env", "nice", "nohup", "time", "command", "sudo", "exec", "xargs",
];

pub fn basename(arg0: &str) -> &str {
    arg0.rsplit('/').next().unwrap_or(arg0)
}

fn operator_like(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '<' | '>' | '&' | '|' | ';'))
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '<' | '>' | '&' | '|' | ';' | '-'))
}

/// Refuse shell syntax carried as argv: `HPH011` free-form shell, `HPH012`
/// pipelines, redirections, `;`, `&&`, backticks, `$(`; `HPH015` remote execution.
pub fn check_syntax(argv: &[String]) -> HarnessResult<()> {
    let Some(first) = argv.first() else {
        return Err(refuse("SPX-HPH010", "argv must not be empty"));
    };
    if first.is_empty() || argv.iter().any(|a| a.contains('\0')) {
        return Err(refuse(
            "SPX-HPH010",
            "argv elements must be non-empty program text without NUL",
        ));
    }
    if first.chars().any(char::is_whitespace) {
        return Err(refuse(
            "SPX-HPH011",
            "argv[0] is a command string; pass a program and separate arguments",
        ));
    }
    let mut i = 0;
    while i < argv.len() && LAUNCHERS.contains(&basename(&argv[i])) {
        i += 1;
        while i < argv.len() && (argv[i].starts_with('-') || argv[i].contains('=')) {
            i += 1;
        }
    }
    if let Some(cmd) = argv.get(i) {
        let name = basename(cmd);
        if SHELLS.contains(&name) {
            let c = argv[i + 1..].iter().any(|a| {
                a == "-c" || (a.starts_with('-') && !a.starts_with("--") && a.contains('c'))
            });
            if c || name == "eval" {
                return Err(refuse(
                    "SPX-HPH011",
                    format!("free-form shell `{name} -c` is not admitted"),
                ));
            }
        }
        if REMOTE.contains(&name) {
            return Err(refuse(
                "SPX-HPH015",
                format!("remote execution through `{name}` is not admitted"),
            ));
        }
    }
    for a in argv {
        if operator_like(a) || ["&&", "||", "`", "$("].iter().any(|t| a.contains(t)) {
            return Err(refuse(
                "SPX-HPH012",
                format!("argv element `{a}` is shell syntax; pipelines, redirections and command substitution are not supported"),
            ));
        }
    }
    Ok(())
}

fn is_executable(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Resolve `argv[0]` to one absolute executable. A bare name is looked up only
/// in the explicit `path_var` the caller supplies; nothing is read ambiently.
pub fn resolve_executable(
    arg0: &str,
    cwd: &Path,
    path_var: Option<&str>,
) -> HarnessResult<PathBuf> {
    let found = if arg0.contains('/') {
        let p = Path::new(arg0);
        Some(if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        })
    } else {
        let paths = path_var.ok_or_else(|| {
            refuse(
                "SPX-HPH013",
                format!("`{arg0}` is a bare name and no PATH was supplied"),
            )
        })?;
        paths
            .split(':')
            .filter(|d| Path::new(d).is_absolute())
            .map(|d| Path::new(d).join(arg0))
            .find(|c| is_executable(c))
    };
    let p = found
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| is_executable(p))
        .ok_or_else(|| {
            refuse(
                "SPX-HPH013",
                format!("executable `{arg0}` was not found or is not executable"),
            )
        })?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn shell_syntax_is_refused() {
        for bad in [
            v(&["sh", "-c", "echo hi"]),
            v(&["env", "FOO=1", "bash", "-lc", "x"]),
            v(&["ls", "|", "wc"]),
            v(&["ls", ">", "f"]),
            v(&["ls", "2>&1"]),
            v(&["echo", "a;"]).into_iter().chain(v(&[";"])).collect(),
            v(&["echo", "$(id)"]),
            v(&["echo", "`id`"]),
            v(&["true", "&&", "false"]),
            v(&["ssh", "host"]),
            v(&["echo hi"]),
        ] {
            assert!(check_syntax(&bad).is_err(), "{bad:?}");
        }
        assert!(check_syntax(&v(&["git", "log", "-n", "3"])).is_ok());
        assert!(check_syntax(&v(&["sh", "script.sh"])).is_ok());
    }
}
