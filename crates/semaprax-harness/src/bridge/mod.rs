//! External-host bridge and single-owner negotiation (HP-14).
//! Specification: `docs/HARNESS-BRIDGE-V1.md`. Diagnostics `SPX-HPN`: 001
//! handshake/protocol, 002 recursion, 003 publication refused, 004 method/order,
//! 005 params, 006 hook input, 007 usage/host/unsupported version, 008 delegated
//! verb failed, 009 competing rewriter, 010 log write, 011 setup refused, 012 too
//! many in flight, 013 duplicate id, 014 no provider for invoke, 015 step not
//! replayed (HN-18).

pub mod claude;
pub mod frame;
pub mod hostskills;
pub mod inflight;
pub mod mcp;
pub mod negotiate;
pub(crate) mod outbox;
pub mod rpc;
pub mod setup;
pub mod shell;
pub mod skills_bridge;

use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use crate::json::canonical;
use std::io::{Read, Write};
use std::path::PathBuf;

fn usage(m: &str) -> Outcome {
    Outcome::usage(format!("bridge: {m}\nbridge <project> --stdio | --mcp | --setup claude-code [--write] | --host claude-code [--hook pre-tool-use | --print-config] [--settings-file F]... [--log F] [--harness-bin P] [--session ID] [--host-skills-dir D] [--harness-home D]"))
}

/// Environment override for the stdio output-stall allowance (milliseconds).
const OUTPUT_STALL_VAR: &str = "SEMAPRAX_BRIDGE_OUTPUT_STALL_MS";

fn output_stall(env: &Environment) -> std::time::Duration {
    // `Environment::from_process` forwards only a fixed allowlist, so the
    // process environment is read here directly for this one tuning knob.
    env.vars
        .get(OUTPUT_STALL_VAR)
        .cloned()
        .or_else(|| std::env::var(OUTPUT_STALL_VAR).ok())
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|ms| (1..=600_000).contains(ms))
        .map_or(rpc::OUTPUT_STALL, std::time::Duration::from_millis)
}

/// Response writer for the stdio server. On Unix this is a duplicate of fd 1
/// rather than the process-global `Stdout`: a writer thread left blocked on a
/// stalled consumer would otherwise keep the global stdout lock and wedge the
/// CLI's own final `print!` after the session ended.
fn stdout_writer() -> Box<dyn std::io::Write + Send> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        if let Ok(fd) = std::io::stdout().as_fd().try_clone_to_owned() {
            return Box::new(std::fs::File::from(fd));
        }
    }
    Box::new(std::io::stdout())
}

pub fn cli_bridge(args: &[String], env: &Environment) -> Outcome {
    let mut pos = Vec::new();
    let (mut stdio, mut print, mut host, mut hook) = (false, false, None::<String>, None::<String>);
    let (mut mcp, mut write, mut setup) = (false, false, None::<String>);
    let (mut session, mut skills_dir) = (None::<String>, None::<PathBuf>);
    let mut harness_home = None::<PathBuf>;
    let (mut settings_files, mut log, mut bin) =
        (Vec::<PathBuf>::new(), None::<PathBuf>, None::<PathBuf>);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned();
        match a.as_str() {
            "--stdio" => stdio = true,
            "--print-config" => print = true,
            "--mcp" => mcp = true,
            "--write" => write = true,
            "--setup" => setup = val(),
            "--session" => session = val(),
            "--harness-home" => harness_home = val().map(|v| env.cwd.join(v)),
            "--host-skills-dir" => skills_dir = val().map(|v| env.cwd.join(v)),
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
    if write && setup.is_none() {
        return usage("--write belongs to --setup");
    }
    if let Some(h) = setup.as_deref() {
        if h != "claude-code" {
            return Outcome::refused(&HarnessDiagnostic::new(
                "SPX-HPN007",
                format!(
                    "no setup for host `{h}`; supported: `claude-code` (MCP stdio, pinned {})",
                    claude::PINNED_VERSION
                ),
            ));
        }
        let o = setup::SetupOptions {
            project: project.canonicalize().unwrap_or(project),
            harness_bin: bin
                .or_else(|| std::env::current_exe().ok())
                .unwrap_or_default(),
            session,
            write,
            log,
            harness_home,
        };
        return match setup::claude_code(&o) {
            Ok(v) => Outcome::ok(format!("{}\n", canonical(&v))),
            Err(d) => Outcome::refused(&d),
        };
    }
    if mcp {
        let stdin = std::io::stdin();
        let opts = mcp::McpOptions {
            session,
            host_skills_dir: skills_dir,
            log,
        };
        return match mcp::serve(stdin.lock(), std::io::stdout(), env, &project, opts) {
            Ok(()) => Outcome::ok(""),
            Err(e) => Outcome::refused(&HarnessDiagnostic::new("SPX-HPN007", format!("mcp: {e}"))),
        };
    }
    if stdio {
        let stdin = std::io::stdin();
        let server = rpc::Server::new(env, &project)
            .with_session(session)
            .with_host_skills_dir(skills_dir)
            .with_log(log)
            .with_output_stall(output_stall(env));
        return match rpc::serve_detached(stdin.lock(), stdout_writer(), server) {
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
