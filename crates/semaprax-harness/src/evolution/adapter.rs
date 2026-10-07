//! Adapter boundary: one JSON request in, one JSON result out. The process
//! adapter runs the external implementation with a scrubbed environment inside
//! the isolated experiment workspace.

use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
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
        if cancel.is_set() {
            return Err(AdapterError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(AdapterError::Timeout);
        }
        if self.command.is_empty() {
            return Err(AdapterError::Unavailable("empty adapter command".into()));
        }
        // Own process group so a kill also reaches the backend CLI children
        // (a paid model call must not outlive a cancelled or timed-out run).
        // stderr is kept in the isolated workspace for diagnosis, never forwarded.
        let log = std::fs::create_dir_all(workspace.join("evidence"))
            .and_then(|_| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(workspace.join("evidence/adapter-stderr.log"))
            })
            .map(Stdio::from)
            .unwrap_or_else(|_| Stdio::null());
        let mut child = Command::new(&self.command[0])
            .args(&self.command[1..])
            .current_dir(workspace)
            .env_clear()
            .envs(&self.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log)
            .process_group(0)
            .spawn()
            .map_err(|e| AdapterError::Unavailable(format!("{}: {e}", self.command[0])))?;
        let mut stdin = child.stdin.take().expect("piped");
        let mut stdout = child.stdout.take().expect("piped");
        let bytes = request.to_string().into_bytes();
        let (write_tx, write_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = stdin.write_all(&bytes);
            drop(stdin);
            let _ = write_tx.send(result);
        });
        let (read_tx, read_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let result = (&mut stdout).take(MAX_OUTPUT + 1).read_to_end(&mut buf);
            drop(stdout);
            let _ = read_tx.send(result.map(|_| buf));
        });
        // Never join an I/O worker: inherited pipe descriptors can outlive the
        // direct child. Supervise both pipes and the whole group to the deadline.
        let mut status = None;
        let mut written = false;
        let mut output = None;
        let terminal = loop {
            if cancel.is_set() {
                break Err(AdapterError::Cancelled);
            }
            if Instant::now() >= deadline {
                break Err(AdapterError::Timeout);
            }
            if !written {
                match write_rx.try_recv() {
                    Ok(Ok(())) => written = true,
                    Ok(Err(e)) => {
                        break Err(AdapterError::Protocol(format!("request write failed: {e}")))
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break Err(AdapterError::Protocol("request writer stopped".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
            }
            if output.is_none() {
                match read_rx.try_recv() {
                    Ok(Ok(buf)) if buf.len() as u64 > MAX_OUTPUT => {
                        break Err(AdapterError::Protocol("output exceeds 1 MiB".into()))
                    }
                    Ok(Ok(buf)) => output = Some(buf),
                    Ok(Err(e)) => {
                        break Err(AdapterError::Protocol(format!("output read failed: {e}")))
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break Err(AdapterError::Protocol("output reader stopped".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
            }
            if status.is_none() {
                match child.try_wait() {
                    Ok(s) => status = s,
                    Err(e) => break Err(AdapterError::Protocol(e.to_string())),
                }
            }
            if written && output.is_some() && status.is_some() {
                if cancel.is_set() {
                    break Err(AdapterError::Cancelled);
                }
                if Instant::now() >= deadline {
                    break Err(AdapterError::Timeout);
                }
                break Ok(());
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        // Even a successful adapter must not leave backend children alive.
        kill_group(&child);
        let _ = child.kill();
        let _ = child.wait();
        terminal?;
        let status = status.expect("completed child");
        let out = output.expect("completed output");
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
            let why = v.get("error").and_then(Value::as_str).unwrap_or("");
            let why: String = why.chars().take(240).collect();
            return Err(AdapterError::Protocol(format!(
                "adapter exited {status}: {why}"
            )));
        }
        Ok(v)
    }
}

/// SIGKILL the adapter's whole process group (it leads its own group).
fn kill_group(child: &std::process::Child) {
    if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}
