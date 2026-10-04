//! `semaprax harness <verb>` dispatcher. Each verb is owned by one module;
//! this table only routes, it holds no verb logic.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Explicit process environment handed to every verb. Verbs read no ambient
/// environment except through this value.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// `$SEMAPRAX_HARNESS_HOME` or `$HOME/.config/semaprax/harness`.
    pub harness_home: Option<PathBuf>,
    /// Compiler executable used as a service (`--compiler`, `$SEMAPRAX_COMPILER`,
    /// or the full toolchain's own executable).
    pub compiler: Option<PathBuf>,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Selected variables the host may forward deliberately (never wholesale).
    pub vars: BTreeMap<String, String>,
}

impl Environment {
    pub fn from_process() -> Self {
        let home = std::env::var_os("SEMAPRAX_HARNESS_HOME").map(PathBuf::from).or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/semaprax/harness"))
        });
        let compiler = std::env::var_os("SEMAPRAX_COMPILER").map(PathBuf::from);
        let mut vars = BTreeMap::new();
        for key in ["HOME", "PATH", "TMPDIR"] {
            if let Ok(v) = std::env::var(key) {
                vars.insert(key.to_string(), v);
            }
        }
        Self { harness_home: home, compiler, cwd: std::env::current_dir().unwrap_or_default(), vars }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self { code: 0, stdout: stdout.into(), stderr: String::new() }
    }
    pub fn refused(diagnostic: &crate::diag::HarnessDiagnostic) -> Self {
        Self { code: 1, stdout: String::new(), stderr: format!("{diagnostic}\n") }
    }
    pub fn usage(message: impl Into<String>) -> Self {
        Self { code: 2, stdout: String::new(), stderr: format!("{}\n{USAGE}", message.into()) }
    }
}

pub const USAGE: &str = "usage: semaprax harness <verb> [args]\n\
verbs: status explain resolve adopt trust revoke inspect run context exec recover decide endpoints skills bridge report conformance bench\n";

pub type Verb = fn(&[String], &Environment) -> Outcome;

/// Closed verb table: verb name -> owning module entry point.
pub const VERBS: &[(&str, Verb)] = &[
    ("status", crate::profile::cli_status),
    ("explain", crate::profile::cli_explain),
    ("resolve", crate::profile::cli_resolve),
    ("adopt", crate::profile::cli_adopt),
    ("trust", crate::profile::cli_trust),
    ("revoke", crate::profile::cli_revoke),
    ("inspect", crate::profile::cli_inspect),
    ("run", crate::workflow::cli_run),
    ("context", crate::context::cli_context),
    ("exec", crate::command_view::cli_exec),
    ("recover", crate::command_view::cli_recover),
    ("decide", crate::decision::cli_decide),
    ("endpoints", crate::endpoint::cli_endpoints),
    ("skills", crate::skills::cli_skills),
    ("bridge", crate::bridge::cli_bridge),
    ("report", crate::observe::cli_report),
    ("conformance", crate::conformance::cli_conformance),
    ("bench", crate::bench::cli_bench),
];

pub fn run(args: &[String], env: &Environment) -> Outcome {
    let Some((verb, rest)) = args.split_first() else {
        return Outcome::usage("missing verb");
    };
    if verb == "--help" || verb == "-h" || verb == "help" {
        return Outcome::ok(USAGE);
    }
    match VERBS.iter().find(|(name, _)| name == verb) {
        Some((_, entry)) => entry(rest, env),
        None => Outcome::usage(format!("unknown harness verb `{verb}`")),
    }
}

/// Placeholder used by a verb whose owning work item has not landed yet.
pub fn unimplemented_verb(verb: &str) -> Outcome {
    Outcome { code: 2, stdout: String::new(), stderr: format!("harness verb `{verb}` is not implemented yet\n") }
}
