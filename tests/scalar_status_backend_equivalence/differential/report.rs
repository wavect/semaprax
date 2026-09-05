//! The discrepancy report.
//!
//! A differential failure is only useful if it can be handed to someone else.
//! This renders everything needed to reproduce one without the machine that
//! found it: the seed, the exact source, the compiler commit, every toolchain
//! identity that took part, the exact commands each lane ran, the expected and
//! observed outcomes per case, and the minimized module.

use std::fmt::Write as _;
use std::process::Command;

use super::grammar::Module;
use super::observe::{Comparison, Finding, LaneReport, LaneStatus};

/// Identities of everything that took part, resolved once per run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Toolchains {
    pub(crate) compiler_commit: String,
    pub(crate) compiler_version: String,
    pub(crate) rustc: String,
    pub(crate) clang: Option<String>,
    pub(crate) node: Option<String>,
}

impl Toolchains {
    pub(crate) fn resolve() -> Self {
        Self {
            compiler_commit: git_head(),
            compiler_version: env!("CARGO_PKG_VERSION").to_owned(),
            rustc: first_line("rustc", &["--version"])
                .unwrap_or_else(|| "rustc identity unavailable".to_owned()),
            clang: first_line("clang", &["--version"]),
            node: first_line("node", &["--version"]),
        }
    }
}

fn git_head() -> String {
    // The harness reads its own checkout's revision so a report can be replayed
    // against the exact compiler that produced it. Generated modules never run
    // a process; this is the harness identifying itself.
    match Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        _ => "compiler commit unavailable".to_owned(),
    }
}

fn first_line(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    )
}

/// Everything one differential run observed, rendered on demand.
pub(crate) struct RunReport<'a> {
    pub(crate) seed: u64,
    pub(crate) module: &'a Module,
    pub(crate) minimized: Option<&'a Module>,
    pub(crate) minimization_steps: usize,
    pub(crate) toolchains: &'a Toolchains,
    pub(crate) reference: &'a LaneReport,
    pub(crate) lanes: &'a [LaneReport],
    pub(crate) frontend: &'a [Finding],
    pub(crate) comparison: &'a Comparison,
}

impl RunReport<'_> {
    pub(crate) fn render(&self) -> String {
        let mut text = String::new();
        let _ = writeln!(text, "SEMAPRAX differential discrepancy");
        let _ = writeln!(text, "seed: {:#018x}", self.seed);
        let _ = writeln!(text, "compiler commit: {}", self.toolchains.compiler_commit);
        let _ = writeln!(
            text,
            "compiler version: {}",
            self.toolchains.compiler_version
        );
        let _ = writeln!(text, "rustc: {}", self.toolchains.rustc);
        let _ = writeln!(
            text,
            "clang: {}",
            self.toolchains
                .clang
                .as_deref()
                .unwrap_or("absent (native lanes unavailable)")
        );
        let _ = writeln!(
            text,
            "node: {}",
            self.toolchains
                .node
                .as_deref()
                .unwrap_or("absent (Core-Wasm lane unavailable)")
        );

        let _ = writeln!(text, "\nlanes compared: {}", render_lanes(self.comparison));
        if self.comparison.unavailable.is_empty() {
            let _ = writeln!(text, "lanes unavailable: none");
        } else {
            let _ = writeln!(text, "lanes unavailable (NOT counted as parity):");
            for (lane, reason) in &self.comparison.unavailable {
                let _ = writeln!(text, "  {}: {reason}", lane.label());
            }
        }

        let _ = writeln!(text, "\ncommands:");
        for lane in std::iter::once(self.reference).chain(self.lanes.iter()) {
            for command in &lane.commands {
                let _ = writeln!(text, "  [{}] {command}", lane.lane.label());
            }
            if let LaneStatus::Unavailable { reason } = &lane.status {
                let _ = writeln!(text, "  [{}] not run: {reason}", lane.lane.label());
            }
        }

        if !self.frontend.is_empty() {
            let _ = writeln!(text, "\nfrontend findings:");
            for finding in self.frontend {
                let _ = writeln!(
                    text,
                    "  {}: expected {} / observed {}",
                    finding.class, finding.expected, finding.observed
                );
            }
        }

        if !self.comparison.findings.is_empty() {
            let _ = writeln!(text, "\nbackend findings:");
            for finding in &self.comparison.findings {
                let _ = writeln!(
                    text,
                    "  {} [{}] case {}: expected {} / observed {}",
                    finding.class,
                    finding.lane.map(|lane| lane.label()).unwrap_or("frontend"),
                    finding.case.as_deref().unwrap_or("(module)"),
                    finding.expected,
                    finding.observed
                );
            }
        }

        let _ = writeln!(text, "\ngenerated source:\n{}", self.module.render());
        match self.minimized {
            Some(minimized) => {
                let _ = writeln!(
                    text,
                    "minimized source after {} shrink steps:\n{}",
                    self.minimization_steps,
                    minimized.render()
                );
            }
            None => {
                let _ = writeln!(text, "minimized source: not attempted");
            }
        }
        text
    }
}

fn render_lanes(comparison: &Comparison) -> String {
    if comparison.compared.is_empty() {
        return "none".to_owned();
    }
    comparison
        .compared
        .iter()
        .map(|lane| lane.label())
        .collect::<Vec<_>>()
        .join(", ")
}
