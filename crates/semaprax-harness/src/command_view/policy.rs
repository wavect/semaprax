//! Machine-local command-view policy (`$SEMAPRAX_HARNESS_HOME/command-view.json`)
//! and the exclusion rules that keep authoritative envelopes out of lossy
//! transforms. Nothing is enabled unless the file says so.

use super::intent::basename;
use crate::cli::Environment;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const POLICY_FILE: &str = "command-view.json";

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPH030", msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub ttl_secs: u64,
    /// Total bytes retained for the project; oldest entries are pruned first.
    pub max_bytes: u64,
    /// Largest single stream kept; beyond it the stream is marked truncated.
    pub max_stream_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Outputs smaller than this bypass transformation.
    pub min_bytes: u64,
    /// With a named tokenizer, raw output below this many tokens bypasses the provider.
    pub min_tokens: Option<u64>,
    pub timeout_ms: u64,
    /// Per-stream bytes kept in memory.
    pub mem_cap: usize,
    pub provider_timeout_ms: u64,
    pub retention: Option<RetentionPolicy>,
    /// Case-insensitive substrings; matching lines never reach a provider.
    pub redact: Vec<String>,
    /// Executable names treated as output-rewriting wrappers (no transform).
    pub known_wrappers: Vec<String>,
    /// Absolute interpreter paths for adapter runtimes (`python`, `node`).
    pub runtimes: BTreeMap<String, PathBuf>,
    /// Environment names forwarded from the host environment to the command.
    pub env_grant: Vec<String>,
    /// Accept a provider's pre-execution wrapper plan (still validated).
    pub allow_wrapper: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            min_bytes: 1024,
            min_tokens: None,
            timeout_ms: 120_000,
            mem_cap: 1 << 20,
            provider_timeout_ms: 15_000,
            retention: None,
            redact: Vec::new(),
            known_wrappers: Vec::new(),
            runtimes: BTreeMap::new(),
            env_grant: ["PATH", "HOME", "TMPDIR", "LANG", "LC_ALL"]
                .map(String::from)
                .to_vec(),
            allow_wrapper: false,
        }
    }
}

fn uint(m: &serde_json::Map<String, Value>, key: &str, max: u64) -> HarnessResult<Option<u64>> {
    match m.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_u64()
            .filter(|n| *n <= max)
            .map(Some)
            .ok_or_else(|| bad(format!("`{key}` must be an integer in 0..={max}"))),
    }
}

fn strings(m: &serde_json::Map<String, Value>, key: &str) -> HarnessResult<Option<Vec<String>>> {
    match m.get(key) {
        None => Ok(None),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| {
                x.as_str()
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .ok_or_else(|| bad(format!("`{key}` must hold non-empty strings")))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(bad(format!("`{key}` must be an array of strings"))),
    }
}

fn closed<'a>(
    v: &'a Value,
    what: &str,
    keys: &[&str],
) -> HarnessResult<&'a serde_json::Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| bad(format!("{what} must be an object")))?;
    if let Some(k) = m.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(bad(format!("{what} has unknown member `{k}`")));
    }
    Ok(m)
}

impl Policy {
    pub fn parse(bytes: &[u8]) -> HarnessResult<Policy> {
        let v: Value = serde_json::from_slice(bytes)
            .map_err(|e| bad(format!("policy is not valid JSON: {e}")))?;
        let m = closed(
            &v,
            "policy",
            &[
                "schema",
                "min_bytes",
                "min_tokens",
                "timeout_ms",
                "mem_cap_bytes",
                "provider_timeout_ms",
                "retention",
                "redact_lines_containing",
                "known_wrappers",
                "runtimes",
                "env_grant",
                "allow_wrapper",
            ],
        )?;
        if m.get("schema").and_then(Value::as_str)
            != Some("semaprax.harness-command-view-policy.v1")
        {
            return Err(bad(
                "policy schema must be `semaprax.harness-command-view-policy.v1`",
            ));
        }
        let mut p = Policy::default();
        if let Some(n) = uint(m, "min_bytes", 1 << 30)? {
            p.min_bytes = n;
        }
        p.min_tokens = uint(m, "min_tokens", 1 << 30)?;
        if let Some(n) = uint(m, "timeout_ms", 600_000)?.filter(|n| *n > 0) {
            p.timeout_ms = n;
        }
        if let Some(n) = uint(m, "mem_cap_bytes", 4 << 20)? {
            p.mem_cap = n as usize;
        }
        if let Some(n) = uint(m, "provider_timeout_ms", 600_000)?.filter(|n| *n > 0) {
            p.provider_timeout_ms = n;
        }
        if let Some(r) = m.get("retention") {
            let r = closed(
                r,
                "retention",
                &["enabled", "ttl_secs", "max_bytes", "max_stream_bytes"],
            )?;
            if r.get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| bad("retention.enabled must be a boolean"))?
            {
                p.retention = Some(RetentionPolicy {
                    ttl_secs: uint(r, "ttl_secs", 31_536_000)?
                        .filter(|n| *n > 0)
                        .ok_or_else(|| bad("retention.ttl_secs is required"))?,
                    max_bytes: uint(r, "max_bytes", 1 << 40)?
                        .filter(|n| *n > 0)
                        .ok_or_else(|| bad("retention.max_bytes is required"))?,
                    max_stream_bytes: uint(r, "max_stream_bytes", 1 << 36)?.unwrap_or(64 << 20),
                });
            }
        }
        if let Some(x) = strings(m, "redact_lines_containing")? {
            p.redact = x.into_iter().map(|s| s.to_lowercase()).collect();
        }
        if let Some(x) = strings(m, "known_wrappers")? {
            p.known_wrappers = x;
        }
        if let Some(x) = strings(m, "env_grant")? {
            if x.iter()
                .any(|k| k.starts_with("SEMAPRAX_HARNESS_") || k.contains('='))
            {
                return Err(bad("env_grant names must be plain and not reserved"));
            }
            p.env_grant = x;
        }
        if let Some(rt) = m.get("runtimes") {
            for (k, v) in closed(rt, "runtimes", &["python", "node"])? {
                let path = PathBuf::from(
                    v.as_str()
                        .ok_or_else(|| bad("runtime paths must be strings"))?,
                );
                if !path.is_absolute() {
                    return Err(bad(format!("runtime `{k}` must be an absolute path")));
                }
                p.runtimes.insert(k.clone(), path);
            }
        }
        if let Some(b) = m.get("allow_wrapper") {
            p.allow_wrapper = b
                .as_bool()
                .ok_or_else(|| bad("allow_wrapper must be a boolean"))?;
        }
        Ok(p)
    }

    /// Absent file means defaults (retention off, no wrapper, no redaction).
    pub fn load(env: &Environment) -> HarnessResult<Policy> {
        let Some(home) = &env.harness_home else {
            return Ok(Policy::default());
        };
        match std::fs::read(home.join(POLICY_FILE)) {
            Ok(b) => Policy::parse(&b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Policy::default()),
            Err(e) => Err(bad(format!("cannot read {POLICY_FILE}: {e}"))),
        }
    }
}

const SIGNERS: &[&str] = &[
    "gpg",
    "gpg2",
    "ssh-keygen",
    "openssl",
    "cosign",
    "minisign",
    "signify",
    "codesign",
];
/// Executables whose output is a digest the next step must match byte for byte.
const HASHERS: &[&str] = &[
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "shasum",
    "md5",
    "md5sum",
    "b3sum",
    "cksum",
];
/// Executables that emit already-compressed or archive data.
const COMPRESSORS: &[&str] = &[
    "gzip", "gunzip", "zcat", "bzip2", "bunzip2", "xz", "unxz", "zstd", "unzstd", "tar", "zip",
    "unzip", "base64",
];
/// Commands that need a terminal or answer prompts.
const INTERACTIVE: &[&str] = &[
    "vim", "vi", "nvim", "nano", "emacs", "less", "more", "top", "htop", "man", "ssh", "sftp",
    "telnet", "passwd", "sudo", "su",
];
/// Executables whose purpose is to write source (the edit itself, not its report).
const EDITORS: &[&str] = &["patch", "ed", "ex"];
/// Flags that make output something another tool must parse exactly.
const MACHINE_FLAGS: &[&str] = &[
    "--json",
    "--format",
    "--message-format",
    "--porcelain",
    "--output",
    "--junit",
    "--report",
    "--sarif",
    "--null",
    "--raw-output",
    "-z",
    "-0",
    "--patch-bytes",
    "--emit",
];

/// Why a command's output must not be lossy-compressed, when it must not.
pub fn exclusion(argv: &[String], raw_flag: bool) -> Option<&'static str> {
    if raw_flag {
        return Some("raw-requested");
    }
    let name = basename(&argv[0]);
    if name.starts_with("semaprax") {
        return Some("authoritative-envelope");
    }
    if SIGNERS.contains(&name) {
        return Some("signature-tool");
    }
    if HASHERS.contains(&name) {
        return Some("hash-output");
    }
    if COMPRESSORS.contains(&name) {
        return Some("already-compressed-output");
    }
    if INTERACTIVE.contains(&name) {
        return Some("interactive-command");
    }
    if EDITORS.contains(&name) {
        return Some("source-edit-output");
    }
    if name == "git" {
        let sub = argv[1..]
            .iter()
            .find(|a| !a.starts_with('-'))
            .map(String::as_str);
        match sub {
            Some(
                "apply" | "am" | "format-patch" | "hash-object" | "rev-parse" | "rev-list"
                | "cat-file",
            ) => return Some("git-machine-output"),
            _ => {}
        }
        if matches!(
            sub,
            Some("add" | "rebase" | "checkout" | "reset" | "stash" | "commit" | "clean")
        ) && argv[1..]
            .iter()
            .any(|a| matches!(a.as_str(), "-i" | "-p" | "--interactive" | "--patch"))
        {
            return Some("interactive-command");
        }
    }
    let machine = argv[1..].iter().any(|a| {
        let flag = a.split('=').next().unwrap_or(a);
        MACHINE_FLAGS.contains(&flag)
    });
    if machine {
        return Some("machine-output-flag");
    }
    None
}

/// Binary output: a NUL byte or a large share of undecodable bytes.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
        || (!bytes.is_empty()
            && bytes
                .utf8_chunks()
                .map(|c| c.invalid().len())
                .sum::<usize>()
                * 20
                > bytes.len())
}

/// Output made only of hex digests (hashes the next step must match exactly).
pub fn looks_like_digests(stdout: &[u8]) -> bool {
    let t = String::from_utf8_lossy(stdout);
    let mut n = 0;
    for l in t.lines().filter(|l| !l.trim().is_empty()) {
        let tok = l.split_whitespace().next().unwrap_or("");
        let tok = tok.strip_prefix("sha256:").unwrap_or(tok);
        if !matches!(tok.len(), 32 | 40 | 64 | 128) || !tok.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        n += 1;
    }
    n > 0
}

/// Output that is itself a JSON document is a machine envelope.
pub fn looks_like_json(stdout: &[u8]) -> bool {
    let t = String::from_utf8_lossy(stdout);
    let t = t.trim();
    matches!(t.as_bytes().first(), Some(b'{' | b'[')) && serde_json::from_str::<Value>(t).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusions() {
        let a = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            exclusion(&a(&["/x/semaprax", "check"]), false),
            Some("authoritative-envelope")
        );
        assert_eq!(
            exclusion(&a(&["cargo", "test", "--message-format=json"]), false),
            Some("machine-output-flag")
        );
        assert_eq!(exclusion(&a(&["ls"]), true), Some("raw-requested"));
        assert_eq!(exclusion(&a(&["ls", "-l"]), false), None);
        for (argv, why) in [
            (&["sha256sum", "f"][..], "hash-output"),
            (&["gzip", "-c", "f"], "already-compressed-output"),
            (&["vim", "f"], "interactive-command"),
            (&["git", "rebase", "-i", "HEAD~2"], "interactive-command"),
            (&["git", "add", "-p"], "interactive-command"),
            (&["patch", "-p1"], "source-edit-output"),
            (&["git", "apply", "x.diff"], "git-machine-output"),
            (&["git", "rev-parse", "HEAD"], "git-machine-output"),
        ] {
            assert_eq!(exclusion(&a(argv), false), Some(why), "{argv:?}");
        }
        assert_eq!(exclusion(&a(&["rg", "-i", "x"]), false), None);
        assert!(looks_binary(b"ab\0cd"));
        assert!(!looks_binary("héllo wörld".as_bytes()));
        assert!(looks_like_digests(
            b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  f\n"
        ));
        assert!(!looks_like_digests(b"abc123 not a digest\n"));
        assert!(looks_like_json(b" {\"a\":1}\n"));
        assert!(!looks_like_json(b"{not json"));
    }

    #[test]
    fn policy_is_closed() {
        assert!(Policy::parse(
            br#"{"schema":"semaprax.harness-command-view-policy.v1","bogus":1}"#
        )
        .is_err());
        let p = Policy::parse(br#"{"schema":"semaprax.harness-command-view-policy.v1","retention":{"enabled":true,"ttl_secs":5,"max_bytes":100}}"#).unwrap();
        assert!(p.retention.is_some());
    }
}
