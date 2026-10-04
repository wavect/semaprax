//! Adapter SDK conformance and hostility suites (HP-16).
//!
//! The runner launches the adapter through the real host with a throwaway
//! harness home (descriptor adopted and trusted via the profile verbs, no
//! global state), runs one suite per active capability plus the common
//! hostility suites, and emits `semaprax.harness-conformance-report.v1`.
//! A report is evidence, never a support decision. Diagnostics `SPX-HPP001..`:
//! 001 usage, 002 target, 003 rig setup.

pub mod cli;
pub mod command;
pub mod common;
pub mod context;
pub mod decision;
pub mod model;
pub mod report;
pub mod skill;
pub mod suite;

pub use cli::cli_conformance;
pub use report::{Case, Report, Suite, Verdict, REPORT_SCHEMA, SUPPORT_DECISION};
pub use suite::Target;

use crate::cli::Environment;
use crate::contract::{negotiate, CapabilityKind, HostSupport};
use crate::diag::HarnessResult;
use crate::profile::resolve::current_platform;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SUITES: [&str; 6] = ["context", "command", "decision", "skill", "common", "all"];

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub descriptor: PathBuf,
    pub suites: Vec<String>,
    pub upstream: Option<PathBuf>,
    pub runtime: Option<PathBuf>,
    /// Python used for the hostile fixture when the target is not python.
    pub hostile_runtime: Option<PathBuf>,
    pub hostile_dir: Option<PathBuf>,
    pub forward_env: BTreeMap<String, String>,
    pub restricted: bool,
}

fn default_hostile_dir() -> Option<PathBuf> {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/semaprax-harness-adapters/examples/hostile-python");
    d.join("harness-provider.json").is_file().then_some(d)
}

pub fn run(opts: &Options, env: &Environment) -> HarnessResult<Report> {
    let tmp = env
        .vars
        .get("TMPDIR")
        .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    let t = Target::new(
        &opts.descriptor,
        opts.runtime.clone(),
        opts.upstream.clone(),
        opts.forward_env.clone(),
        opts.restricted,
        tmp.clone(),
    )?;
    let d = &t.descriptor;
    let neg = negotiate(d, &HostSupport::first_wave())?;
    let active: Vec<CapabilityKind> = neg.active.iter().map(|a| a.kind).collect();
    let wanted =
        |s: &str| opts.suites.is_empty() || opts.suites.iter().any(|x| x == s || x == "all");
    let mut suites = Vec::new();
    let mut kind_suite = |name: &str, kind: CapabilityKind, run: &dyn Fn(&Target) -> Suite| {
        if opts.suites.iter().any(|x| x == name) || (wanted(name) && active.contains(&kind)) {
            suites.push(if active.contains(&kind) {
                run(&t)
            } else {
                Suite::new(
                    kind.as_str(),
                    "adapter",
                    vec![Case::unverified(
                        "capability-active",
                        format!(
                            "{} is not an active negotiated capability of this descriptor",
                            kind.as_str()
                        ),
                    )],
                )
            });
        }
    };
    kind_suite("context", CapabilityKind::ContextRepository, &context::run);
    kind_suite("command", CapabilityKind::CommandView, &command::run);
    kind_suite("decision", CapabilityKind::DecisionEvaluate, &decision::run);
    kind_suite("skill", CapabilityKind::SkillCatalog, &skill::run);
    if active.contains(&CapabilityKind::ModelGenerate) && wanted("all") {
        suites.push(model::run());
    }
    if wanted("common") {
        suites.push(common::run_adapter(&t, &active));
        let hostile = opts
            .hostile_dir
            .clone()
            .or_else(default_hostile_dir)
            .and_then(|dir| {
                let py = if d.runtime == crate::contract::Runtime::Python {
                    opts.runtime.clone()
                } else {
                    opts.hostile_runtime.clone()
                }?;
                Target::new(
                    &dir.join("harness-provider.json"),
                    Some(py),
                    None,
                    BTreeMap::new(),
                    false,
                    tmp.clone(),
                )
                .ok()
            });
        suites.push(common::run_hostility(hostile.as_ref()));
    }
    let probe = suites.iter().flat_map(|s| &s.cases).find_map(|c| {
        c.evidence["isolation_observed"]
            .as_str()
            .map(str::to_string)
    });
    let ops: BTreeMap<&str, &Vec<String>> = d
        .capabilities
        .iter()
        .filter_map(|c| c.kind.map(|k| (k.as_str(), &c.operations)))
        .collect();
    let subject: Value = json!({
        "provider_id": d.provider_id,
        "provider_version": d.provider_version,
        "adapter_version": d.adapter_version,
        "descriptor_digest": d.digest(),
        "upstream": d.upstream.as_ref().map(|u| json!({"name": u.name, "declared_versions": u.versions})),
        "license": d.support.license,
        "os": current_platform(),
        "declared_platforms": d.platforms,
        "runtime": {"kind": d.runtime.as_str(), "executable": opts.runtime.as_ref().map(|p| p.to_string_lossy().into_owned())},
        "operations": ops,
        "isolation": {"declared": d.support.isolation, "requested": if opts.restricted { "restricted" } else { "none" },
                      "observed": probe.unwrap_or_else(|| "not-recorded".into())},
        "declared_tested": d.support.tested.iter().map(|r| json!({"upstream": r.upstream, "os": r.os, "result": r.result})).collect::<Vec<_>>(),
    });
    let inactive = neg.inactive.iter().map(|i| json!({"kind": i.kind_name, "version": i.version, "reason": i.reason, "suite": "none"})).collect();
    Ok(Report {
        subject,
        inactive,
        suites,
    })
}
