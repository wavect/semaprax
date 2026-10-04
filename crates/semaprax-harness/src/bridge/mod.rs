//! External-host bridge and single-owner negotiation (HP-14).
//! Specification: `docs/HARNESS-BRIDGE-V1.md`. Diagnostics `SPX-HPN`: 001
//! handshake/protocol, 002 recursion, 003 publication refused, 004 method/order,
//! 005 params, 006 hook input, 007 usage/host, 008 delegated verb failed, 009
//! competing rewriter, 010 log write.

pub mod claude;
pub mod negotiate;
pub mod rpc;
pub mod shell;

use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use crate::json::canonical;
use std::io::{Read, Write};
use std::path::PathBuf;

fn usage(m: &str) -> Outcome {
    Outcome::usage(format!("bridge: {m}\nbridge <project> --stdio | --host claude-code [--hook pre-tool-use | --print-config] [--settings-file F]... [--log F] [--harness-bin P]"))
}

pub fn cli_bridge(args: &[String], env: &Environment) -> Outcome {
    let mut pos = Vec::new();
    let (mut stdio, mut print, mut host, mut hook) = (false, false, None::<String>, None::<String>);
    let (mut settings_files, mut log, mut bin) =
        (Vec::<PathBuf>::new(), None::<PathBuf>, None::<PathBuf>);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned();
        match a.as_str() {
            "--stdio" => stdio = true,
            "--print-config" => print = true,
            "--host" => host = val(),
            "--hook" => hook = val(),
            "--settings-file" => match val() {
                Some(v) => settings_files.push(env.cwd.join(v)),
                None => return usage("--settings-file needs a value"),
            },
            "--log" => log = val().map(|v| env.cwd.join(v)),
            "--harness-bin" => bin = val().map(|v| env.cwd.join(v)),
            f if f.starts_with("--") => return usage(&format!("unknown option `{f}`")),
            _ => pos.push(a.clone()),
        }
    }
    let [project] = pos.as_slice() else {
        return usage("expected exactly one <project>");
    };
    let project = env.cwd.join(project);
    if stdio {
        let stdin = std::io::stdin();
        return match rpc::serve(stdin.lock(), std::io::stdout(), env, &project) {
            Ok(()) => Outcome::ok(""),
            Err(e) => {
                Outcome::refused(&HarnessDiagnostic::new("SPX-HPN007", format!("stdio: {e}")))
            }
        };
    }
    match host.as_deref() {
        Some("claude-code") => {}
        Some(other) => {
            return Outcome::refused(&HarnessDiagnostic::new("SPX-HPN007", format!("no adapter for host `{other}`; supported: `claude-code` (pinned {}) and the generic `--stdio` bridge", claude::PINNED_VERSION)))
        }
        None => return usage("choose --stdio or --host <name>"),
    }
    // Settings text is read-only input the caller chose to disclose.
    let mut texts = Vec::new();
    for f in &settings_files {
        if let Ok(t) = std::fs::read_to_string(f) {
            texts.push(t);
        }
    }
    let harness_bin = bin
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_default();
    let opts = claude::HookOptions {
        project: project.canonicalize().unwrap_or(project),
        harness_bin,
        settings: texts,
    };
    if print {
        return match claude::print_config(&opts, &settings_files) {
            Ok(v) => Outcome {
                code: 0,
                stdout: format!("{}\n", canonical(&v)),
                stderr: "review before saving as <project>/.claude/settings.json; semaprax never writes agent configuration\n".into(),
            },
            Err(d) => Outcome::refused(&d),
        };
    }
    match hook.as_deref() {
        None => Outcome::ok(format!("{}\n", canonical(&claude::host_profile()))),
        Some("pre-tool-use") => {
            let mut input = String::new();
            if let Err(e) = std::io::stdin().take(1 << 20).read_to_string(&mut input) {
                return Outcome::refused(&HarnessDiagnostic::new(
                    "SPX-HPN006",
                    format!("stdin: {e}"),
                ));
            }
            match claude::pre_tool_use(&input, &opts, env) {
                Ok(r) => {
                    if let Some(path) = &log {
                        let line = format!("{}\n", canonical(&r.record));
                        let w = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .and_then(|mut f| f.write_all(line.as_bytes()));
                        if let Err(e) = w {
                            return Outcome::refused(&HarnessDiagnostic::new(
                                "SPX-HPN010",
                                format!("log: {e}"),
                            ));
                        }
                    }
                    Outcome::ok(
                        r.stdout
                            .map(|v| format!("{}\n", canonical(&v)))
                            .unwrap_or_default(),
                    )
                }
                Err(d) => Outcome::refused(&d),
            }
        }
        Some(other) => usage(&format!("unknown hook `{other}`")),
    }
}
