//! Claude Code adapter over its documented PreToolUse hook API.
//!
//! Pinned: Claude Code 2.1.289. Source: <https://code.claude.com/docs/en/hooks>
//! (PreToolUse stdin carries `hook_event_name`, `tool_name`, `tool_input`,
//! `cwd`; stdout `hookSpecificOutput.updatedInput` replaces the tool input) and
//! `claude --help` (`--settings`, `--setting-sources`, `--allowedTools`).
//! Nothing here writes any configuration; `print_config` only prints.

use super::negotiate::DEPTH_VAR;
use super::shell::{detect_rtk_hook, quote, tokenize};
use crate::cli::Environment;
use crate::command_view::intent::{check_syntax, resolve_executable};
use crate::command_view::policy::exclusion;
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{parse_strict, JsonLimits};
use crate::profile::{HarnessConfig, Mode};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const PINNED_VERSION: &str = "2.1.289";
pub const HOOKS_DOC: &str = "https://code.claude.com/docs/en/hooks";

pub struct HookOptions {
    pub project: PathBuf,
    pub harness_bin: PathBuf,
    /// Text of settings files the caller chose to disclose (read-only).
    pub settings: Vec<String>,
}

#[derive(Debug)]
pub struct HookResult {
    /// Hook stdout, or `None` for "no decision" (pass through).
    pub stdout: Option<Value>,
    /// One audit record (the CLI appends it to `--log`).
    pub record: Value,
}

/// Argv prefix that runs the harness host: the standalone binary directly, or
/// `semaprax harness` when the full toolchain executable is given.
pub fn launcher(bin: &Path) -> Vec<String> {
    let mut v = vec![bin.to_string_lossy().into_owned()];
    if bin.file_name().and_then(|n| n.to_str()) != Some("semaprax-harness") {
        v.push("harness".into());
    }
    v
}

pub fn rtk_present(settings: &[String]) -> bool {
    settings.iter().any(|s| detect_rtk_hook(s))
}

fn pass(reason: impl Into<String>, tool: &str, original: Option<&str>) -> HookResult {
    HookResult {
        stdout: None,
        record: json!({"event": "PreToolUse", "host": "claude-code", "tool": tool, "decision": "pass-through", "reason": reason.into(), "original": original}),
    }
}

/// Decide one PreToolUse event. A rewrite happens only for an admitted plain
/// command with no other owner; everything else passes through untouched.
pub fn pre_tool_use(
    input: &str,
    opts: &HookOptions,
    env: &Environment,
) -> HarnessResult<HookResult> {
    let bad = |m: String| HarnessDiagnostic::new("SPX-HPN006", m);
    let v = parse_strict(input.as_bytes(), &JsonLimits::frame(1 << 20))
        .map_err(|e| bad(format!("hook input: {}", e.message)))?;
    if v.get("hook_event_name").and_then(Value::as_str) != Some("PreToolUse") {
        return Err(bad("hook input is not a PreToolUse event".into()));
    }
    let tool = v
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("missing tool_name".into()))?;
    if tool != "Bash" {
        return Ok(pass("not a Bash tool call", tool, None));
    }
    let input_obj = v
        .get("tool_input")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("missing tool_input".into()))?;
    let cmd = input_obj
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("missing tool_input.command".into()))?;
    let p = |r: &str| Ok(pass(r, tool, Some(cmd)));
    if env.vars.get(DEPTH_VAR).is_some_and(|d| d != "0")
        || env
            .vars
            .contains_key(crate::command_view::lineage::LINEAGE_VAR)
    {
        return p("recursion guard: invoked from within a Semaprax-owned command");
    }
    if rtk_present(&opts.settings) {
        return p("an RTK hook is already configured in the disclosed settings; Semaprax does not wrap again");
    }
    match HarnessConfig::load(&opts.project) {
        Ok(c) if c.capability(CapabilityKind::CommandView).mode == Mode::Disabled => {
            return p("command.view is disabled by the project profile")
        }
        Err(e) => return p(&format!("profile unreadable: {}", e.message)),
        Ok(_) => {}
    }
    let argv = match tokenize(cmd) {
        Ok(a) => a,
        Err(r) => return p(&format!("not an admitted plain command: {r}")),
    };
    if let Err(e) = check_syntax(&argv) {
        return p(&format!("not admitted by command_view: {}", e.message));
    }
    let cwd = v
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| env.cwd.clone());
    if resolve_executable(&argv[0], &cwd, env.vars.get("PATH").map(String::as_str)).is_err() {
        return p("executable is not resolvable (shell builtin or missing)");
    }
    if let Some(r) = exclusion(&argv, false) {
        return p(&format!("excluded from transformation: {r}"));
    }
    let mut parts = launcher(&opts.harness_bin);
    parts.extend([
        "exec".into(),
        opts.project.to_string_lossy().into_owned(),
        "--".into(),
    ]);
    parts.extend(argv);
    let rewritten = parts.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
    let mut updated = input_obj.clone();
    updated.insert("command".into(), json!(rewritten));
    Ok(HookResult {
        stdout: Some(json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": updated,
            "additionalContext": "Command routed once through semaprax-harness exec (authoritative result plus compact view)."
        }})),
        record: json!({"event": "PreToolUse", "host": "claude-code", "tool": tool, "decision": "rewritten", "original": cmd, "rewritten": rewritten}),
    })
}

/// Reviewed project-local settings snippet. Refused when an RTK hook is
/// already disclosed, so two rewriters never compete.
pub fn print_config(opts: &HookOptions, settings_files: &[PathBuf]) -> HarnessResult<Value> {
    if rtk_present(&opts.settings) {
        return Err(HarnessDiagnostic::new(
            "SPX-HPN009",
            "an RTK hook is already configured; not emitting a second command rewriter (RTK owns command_wrapper)",
        ));
    }
    let mut cmd = launcher(&opts.harness_bin);
    cmd.extend([
        "bridge".into(),
        opts.project.to_string_lossy().into_owned(),
        "--host".into(),
        "claude-code".into(),
        "--hook".into(),
        "pre-tool-use".into(),
    ]);
    for f in settings_files {
        cmd.extend(["--settings-file".into(), f.to_string_lossy().into_owned()]);
    }
    let command = cmd.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ");
    Ok(
        json!({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": command}]}]}}),
    )
}

/// Honest description of what this host integration can and cannot do.
pub fn host_profile() -> Value {
    json!({
        "schema": "semaprax.harness-bridge-host.v1",
        "host": "claude-code",
        "pinned_version": PINNED_VERSION,
        "api": "PreToolUse hook (hookSpecificOutput.updatedInput)",
        "documentation": HOOKS_DOC,
        "capabilities": {
            "command_wrapper": {"owner": "semaprax", "reason": "Bash calls are rewritten to `semaprax-harness exec` unless an RTK hook is configured (then external-host)"},
            "model_routing": {"owner": "external-host", "reason": "host-controlled: Claude Code does not delegate model choice; no Jev/Laya routing is applied or advertised"},
            "publication": {"owner": "external-host", "reason": "publication stays with the host-authorized compiler route"},
            "semantic_query": {"owner": "external-host", "reason": "no hook delegates queries; use `bridge --stdio` or the CLI explicitly"},
            "tool_result_observation": {"owner": "external-host", "reason": "no PostToolUse adapter is shipped"},
            "cancellation": {"owner": "external-host", "reason": "the host cancels its own tool calls"}
        },
        "observed_scope": {
            "observed": "semaprax-routed-bash-calls-only",
            "not_observed": ["other host tools", "conversation history", "model choice"],
            "whole_session_savings_claimed": false
        },
        "configuration": "printed by `--print-config`; never written by Semaprax"
    })
}
