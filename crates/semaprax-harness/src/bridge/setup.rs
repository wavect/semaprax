//! Reviewed per-project setup for hosts that attach through MCP. Prints a plan;
//! `--write` merges exactly one server entry into `<project>/.mcp.json`. Nothing
//! global or user-level is read or written, unrelated keys and servers are kept,
//! and no skill text is copied into the project (the entry points at the bridge,
//! which serves the central catalog).

use super::claude::launcher;
use super::mcp::SERVER_NAME;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "semaprax.harness-bridge-setup.v1";
const FILE: &str = ".mcp.json";
const READ_LIMIT: u64 = 1 << 20;

pub struct SetupOptions {
    pub project: PathBuf,
    pub harness_bin: PathBuf,
    pub session: Option<String>,
    pub write: bool,
    /// Delivery-observation log appended by the server (`--log`).
    pub log: Option<PathBuf>,
    /// Harness home for this server (`env.SEMAPRAX_HARNESS_HOME`); default is the user's own.
    pub harness_home: Option<PathBuf>,
}

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPN011", msg)
}

/// The `mcpServers` entry Claude Code starts (stdio transport).
pub fn server_entry(o: &SetupOptions) -> Value {
    let mut parts = launcher(&o.harness_bin);
    let command = parts.remove(0);
    parts.extend([
        "bridge".into(),
        o.project.to_string_lossy().into_owned(),
        "--mcp".into(),
        "--host-skills-dir".into(),
        o.project
            .join(".claude/skills")
            .to_string_lossy()
            .into_owned(),
    ]);
    if let Some(s) = &o.session {
        parts.extend(["--session".into(), s.clone()]);
    }
    if let Some(l) = &o.log {
        parts.extend(["--log".into(), l.to_string_lossy().into_owned()]);
    }
    let mut entry = json!({"type": "stdio", "command": command, "args": parts});
    if let Some(h) = &o.harness_home {
        entry["env"] = json!({"SEMAPRAX_HARNESS_HOME": h.to_string_lossy()});
    }
    entry
}

fn read_existing(path: &Path) -> HarnessResult<Option<Map<String, Value>>> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(bad(format!("{}: {e}", path.display()))),
    };
    if !meta.is_file() || meta.len() > READ_LIMIT {
        return Err(bad(format!(
            "{} is a symlink, not a file, or over 1 MiB; not touched",
            path.display()
        )));
    }
    let text =
        std::fs::read_to_string(path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => Ok(Some(m)),
        _ => Err(bad(format!(
            "{} is not a JSON object; fix or remove it, nothing was changed",
            path.display()
        ))),
    }
}

pub fn claude_code(o: &SetupOptions) -> HarnessResult<Value> {
    if !o.project.is_dir() {
        return Err(bad(format!(
            "project `{}` is not a directory",
            o.project.display()
        )));
    }
    if let Some(s) = &o.session {
        crate::skills::modes::check_ident("session", s).map_err(|e| bad(e.message))?;
    }
    let path = o.project.join(FILE);
    let entry = server_entry(o);
    let mut doc = read_existing(&path)?.unwrap_or_default();
    let existed = path.exists();
    let servers = match doc.entry("mcpServers").or_insert_with(|| json!({})) {
        Value::Object(m) => m,
        _ => {
            return Err(bad(format!(
                "{FILE}: `mcpServers` is not an object; nothing was changed"
            )))
        }
    };
    let previous = servers.get(SERVER_NAME).cloned();
    let action = match &previous {
        Some(p) if *p == entry => "noop",
        Some(_) => "update",
        None if existed => "add",
        None => "create",
    };
    servers.insert(SERVER_NAME.into(), entry.clone());
    let mut written = false;
    if o.write && action != "noop" {
        let text = format!(
            "{}\n",
            serde_json::to_string_pretty(&Value::Object(doc)).map_err(|e| bad(e.to_string()))?
        );
        let tmp = o.project.join(".mcp.json.semaprax-tmp");
        std::fs::write(&tmp, text)
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| bad(format!("write {}: {e}", path.display())))?;
        written = true;
    }
    let tools: Vec<String> = ["list", "load", "use", "off", "status", "resource"]
        .iter()
        .map(|t| format!("mcp__{SERVER_NAME}__skills_{t}"))
        .collect();
    Ok(json!({
        "schema": SCHEMA,
        "host": "claude-code",
        "mode": if o.write { "write" } else { "plan" },
        "file": FILE,
        "action": action,
        "written": written,
        "previous": previous,
        "entry": entry,
        "suggested_permissions": {"permissions": {"allow": tools}},
        "notes": [
            "plan only unless --write; only <project>/.mcp.json is touched and only the `semaprax-skills` entry changes",
            "no ~/.claude or user-level file is read or written; unrelated servers and keys are preserved",
            "no skill text is copied into the project: the server serves the central catalog (`semaprax-harness skills`)",
            "an existing project .claude/skills copy of an official skill is detected read-only and then not injected twice",
            "Claude Code asks you to approve project .mcp.json servers interactively; permission rules above are a suggestion, not written",
            "model choice stays with Claude Code (not-delegated)"
        ],
    }))
}
