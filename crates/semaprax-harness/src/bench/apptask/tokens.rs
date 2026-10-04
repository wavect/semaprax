//! Named-tokenizer counting. The tokenizer is an explicit tool (a Python with
//! `tiktoken` and a pre-populated cache); nothing is downloaded or discovered.
//! A count is the number of `o200k_base` tokens in the exact text, which is
//! NOT any provider's billing unit: provider usage is recorded separately.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

pub const TOKENIZER: &str = "tiktoken:o200k_base";

pub trait TokenCounter: Send + Sync {
    fn name(&self) -> String;
    fn count(&self, text: &str) -> Result<u64, String>;
}

const HELPER: &str = r#"
import sys, json, tiktoken
enc = tiktoken.get_encoding("o200k_base")
sys.stdout.write(json.dumps({"ready": tiktoken.__version__}) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    t = json.loads(line)["t"]
    sys.stdout.write(json.dumps({"n": len(enc.encode(t, disallowed_special=()))}) + "\n"); sys.stdout.flush()
"#;

pub struct Tiktoken {
    io: Mutex<(Child, ChildStdin, BufReader<ChildStdout>)>,
    pub version: String,
}

impl Tiktoken {
    pub fn start(python: &Path, cache_dir: &Path) -> Result<Tiktoken, String> {
        let mut child = Command::new(python)
            .args(["-c", HELPER])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("TIKTOKEN_CACHE_DIR", cache_dir)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("tokenizer helper: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let mut out = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        let mut line = String::new();
        out.read_line(&mut line).map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(line.trim()).map_err(|_| {
            "tokenizer helper did not start (tiktoken or its cache missing)".to_string()
        })?;
        let version = v["ready"].as_str().unwrap_or("?").to_string();
        Ok(Tiktoken {
            io: Mutex::new((child, stdin, out)),
            version,
        })
    }
}

impl TokenCounter for Tiktoken {
    fn name(&self) -> String {
        format!("{TOKENIZER} (tiktoken {})", self.version)
    }
    fn count(&self, text: &str) -> Result<u64, String> {
        let mut g = self.io.lock().map_err(|_| "poisoned")?;
        let (_, stdin, out) = &mut *g;
        writeln!(stdin, "{}", json!({"t": text})).map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        out.read_line(&mut line).map_err(|e| e.to_string())?;
        serde_json::from_str::<Value>(line.trim())
            .ok()
            .and_then(|v| v["n"].as_u64())
            .ok_or_else(|| "tokenizer helper closed".to_string())
    }
}

impl Drop for Tiktoken {
    fn drop(&mut self) {
        if let Ok(g) = self.io.get_mut() {
            let _ = g.0.kill();
            let _ = g.0.wait();
        }
    }
}

/// Deterministic stand-in for tests: one token per whitespace-separated word.
/// Never used for recorded results.
pub struct WordCounter;

impl TokenCounter for WordCounter {
    fn name(&self) -> String {
        "test:whitespace-words (not a recorded tokenizer)".into()
    }
    fn count(&self, text: &str) -> Result<u64, String> {
        Ok(text.split_whitespace().count() as u64)
    }
}
