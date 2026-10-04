//! Explicitly supplied tokenizers. Nothing here guesses a tokenizer from a
//! model name; a count without a supplied tokenizer is simply absent.

use super::event::{TokenCount, TokenizerId};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

pub trait Tokenizer {
    fn name(&self) -> &str;
    fn fingerprint(&self) -> &str;
    fn count(&self, text: &str) -> usize;
    /// Fallible count; `measure` only uses this.
    fn try_count(&self, text: &str) -> HarnessResult<usize> {
        Ok(self.count(text))
    }
    fn id(&self) -> TokenizerId {
        TokenizerId::Named {
            name: self.name().into(),
            fingerprint: self.fingerprint().into(),
        }
    }
}

/// Builtin `byte-v1`: UTF-8 byte length. These are bytes, not tokens, and are
/// labeled `byte_only` everywhere.
pub struct ByteTokenizer;

impl Tokenizer for ByteTokenizer {
    fn name(&self) -> &str {
        "byte-v1"
    }
    fn fingerprint(&self) -> &str {
        "utf8-bytes"
    }
    fn count(&self, text: &str) -> usize {
        text.len()
    }
    fn id(&self) -> TokenizerId {
        TokenizerId::ByteOnly
    }
}

/// Exact measurement of one envelope. The text itself is dropped here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measured {
    /// `None`: no tokenizer supplied or it failed (missing, never zero).
    pub count: Option<TokenCount>,
    /// SHA-256 of the exact UTF-8 bytes measured.
    pub digest: String,
    pub bytes: usize,
}

/// Count the final model-visible envelope text exactly as given (callers pass
/// the serialized text actually sent, escapes included).
pub fn measure(text: &str, tokenizer: Option<&dyn Tokenizer>) -> Measured {
    let count = tokenizer.and_then(|t| {
        t.try_count(text).ok().map(|n| TokenCount {
            tokenizer: t.id(),
            value: n as u64,
        })
    });
    Measured {
        count,
        digest: sha256_plain(text.as_bytes()),
        bytes: text.len(),
    }
}

fn unavailable(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPO003", msg)
}

struct Link {
    stdin: ChildStdin,
    lines: Receiver<String>,
    child: Child,
}

/// Named tokenizer served by a helper subprocess (`scripts/harness_tokenize.py`
/// or any program speaking its line protocol). No network, no telemetry.
pub struct ExternalTokenizer {
    name: String,
    fingerprint: String,
    link: Mutex<Link>,
    timeout: Duration,
}

impl ExternalTokenizer {
    /// Start `<program> <args..>` with a cleared environment plus `env`, read
    /// the handshake `{"name":..,"fingerprint":..}` and refuse (`SPX-HPO003`)
    /// when the helper reports the tokenizer unavailable.
    pub fn spawn(
        program: &Path,
        args: &[String],
        env: &BTreeMap<String, String>,
    ) -> HarnessResult<Self> {
        let mut child = Command::new(program)
            .args(args)
            .env_clear()
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| unavailable(format!("cannot start tokenizer helper: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| unavailable("helper stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| unavailable("helper stdout"))?;
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let timeout = Duration::from_secs(60);
        let mut link = Link {
            stdin,
            lines,
            child,
        };
        let hello = match link.lines.recv_timeout(timeout) {
            Ok(l) => l,
            Err(_) => {
                let _ = link.child.kill();
                return Err(unavailable("tokenizer helper gave no handshake"));
            }
        };
        let v: serde_json::Value =
            serde_json::from_str(&hello).map_err(|_| unavailable("bad helper handshake"))?;
        if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
            let _ = link.child.kill();
            return Err(unavailable(format!(
                "tokenizer unavailable: {}",
                e.chars().take(200).collect::<String>()
            )));
        }
        let get = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let (Some(name), Some(fingerprint)) = (get("name"), get("fingerprint")) else {
            let _ = link.child.kill();
            return Err(unavailable("helper handshake lacks name/fingerprint"));
        };
        Ok(Self {
            name,
            fingerprint,
            link: Mutex::new(link),
            timeout,
        })
    }
}

impl Tokenizer for ExternalTokenizer {
    fn name(&self) -> &str {
        &self.name
    }
    fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// Returns 0 on helper failure; use `try_count`/`measure` for truthful
    /// failure handling.
    fn count(&self, text: &str) -> usize {
        self.try_count(text).unwrap_or(0)
    }
    fn try_count(&self, text: &str) -> HarnessResult<usize> {
        let mut link = self
            .link
            .lock()
            .map_err(|_| unavailable("helper lock poisoned"))?;
        let req = serde_json::json!({"text": text}).to_string();
        writeln!(link.stdin, "{req}")
            .and_then(|_| link.stdin.flush())
            .map_err(|e| unavailable(format!("helper write: {e}")))?;
        let line = link
            .lines
            .recv_timeout(self.timeout)
            .map_err(|_| unavailable("helper did not answer"))?;
        let v: serde_json::Value =
            serde_json::from_str(&line).map_err(|_| unavailable("bad helper reply"))?;
        v.get("tokens")
            .and_then(|t| t.as_u64())
            .map(|n| n as usize)
            .ok_or_else(|| unavailable("helper reply lacks tokens"))
    }
}

impl Drop for ExternalTokenizer {
    fn drop(&mut self) {
        if let Ok(mut l) = self.link.lock() {
            let _ = l.child.kill();
            let _ = l.child.wait();
        }
    }
}
