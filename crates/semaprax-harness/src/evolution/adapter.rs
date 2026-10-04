//! Adapter boundary: one JSON request in, one JSON result out. The process
//! adapter runs the external implementation with a scrubbed environment inside
//! the isolated experiment workspace.

use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_OUTPUT: u64 = 1 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterError {
    /// Executable missing, backend absent, or the adapter said so (exit 69).
    Unavailable(String),
    Protocol(String),
    Timeout,
    Cancelled,
}

/// Cooperative cancellation: an in-process flag and/or a marker file.
#[derive(Clone, Debug, Default)]
pub struct Cancel {
    pub flag: Arc<AtomicBool>,
    pub file: Option<PathBuf>,
}

impl Cancel {
    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::SeqCst) || self.file.as_ref().is_some_and(|f| f.exists())
    }
}

pub trait Adapter {
    /// `workspace` is the isolated experiment directory (adapter cwd).
    fn call(
        &mut self,
        request: &Value,
        workspace: &Path,
        deadline: Instant,
        cancel: &Cancel,
    ) -> Result<Value, AdapterError>;
}

pub struct ProcessAdapter {
    pub command: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
}

impl Adapter for ProcessAdapter {
    fn call(
        &mut self,
        request: &Value,
        workspace: &Path,
        deadline: Instant,
        cancel: &Cancel,
    ) -> Result<Value, AdapterError> {
        let mut child = Command::new(&self.command[0])
            .args(&self.command[1..])
            .current_dir(workspace)
            .env_clear()
            .envs(&self.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AdapterError::Unavailable(format!("{}: {e}", self.command[0])))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(request.to_string().as_bytes());
        }
        let mut stdout = child.stdout.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = (&mut stdout).take(MAX_OUTPUT + 1).read_to_end(&mut buf);
            buf
        });
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {}
                Err(e) => return Err(AdapterError::Protocol(e.to_string())),
            }
            if cancel.is_set() || Instant::now() >= deadline {
                let cancelled = cancel.is_set();
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(if cancelled {
                    AdapterError::Cancelled
                } else {
                    AdapterError::Timeout
                });
            }
            std::thread::sleep(Duration::from_millis(15));
        };
        let out = reader.join().unwrap_or_default();
        if out.len() as u64 > MAX_OUTPUT {
            return Err(AdapterError::Protocol("output exceeds 1 MiB".into()));
        }
        let v: Value = serde_json::from_slice(&out)
            .map_err(|e| AdapterError::Protocol(format!("invalid JSON result: {e}")))?;
        if status.code() == Some(69) {
            let why = v
                .get("unavailable")
                .and_then(Value::as_str)
                .unwrap_or("adapter reported unavailable");
            return Err(AdapterError::Unavailable(why.to_string()));
        }
        if !status.success() {
            return Err(AdapterError::Protocol(format!("adapter exited {status}")));
        }
        Ok(v)
    }
}
