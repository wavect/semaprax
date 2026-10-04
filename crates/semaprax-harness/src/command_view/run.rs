//! One `exec`: authorize, resolve, execute exactly once, retain, then build the
//! authoritative result and a separate model view.

use super::executor::{self, ExecSpec};
use super::guard::{self, decode, raw_text, redact};
use super::intent::{check_syntax, resolve_executable};
use super::lineage;
use super::measure::{self, delivered_text, Measurement, ViewTokenizer};
use super::policy::{exclusion, looks_binary, looks_like_digests, looks_like_json, Policy};
use super::result::{argv_digest, CommandResult, Envelope, ModelView, StreamRecord};
use super::retention::{handle_for, Retention, StreamName};
use super::view::{lineage_env, PlanRoute, Provider, ViewOptions};
use super::wrapper;
use crate::cli::Environment;
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::CancelToken;
use crate::json::{canonical, sha256_plain};
use crate::observe::event::{Availability, Observation, Role, Stage, TokenCount};
use crate::observe::sink::Observer;
use crate::profile::{resolve_project, HarnessConfig, Mode};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

pub const HOST_PROVIDER: &str = "semaprax.host/command-view";

#[derive(Clone, Debug, Default)]
pub struct ExecOptions {
    pub raw: bool,
    pub json: bool,
    pub external_owner: Option<String>,
    pub extra_env: BTreeMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub cancel: CancelToken,
    /// Named tokenizer for delivered-view measurement and the growth guard.
    pub tokenizer: Option<ViewTokenizer>,
}

pub struct ExecReport {
    pub envelope: Envelope,
    /// Exactly what is emitted to the model (human text or canonical JSON).
    pub display: String,
}

fn refuse(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn recovery_hint(project: &Path, handle: &str) -> String {
    format!(
        "semaprax-harness recover {} {handle} --offset 0 --limit 65536",
        project.display()
    )
}

pub fn execute(
    env: &Environment,
    project: &Path,
    argv: &[String],
    opts: &ExecOptions,
    observer: Option<&mut Observer>,
) -> HarnessResult<ExecReport> {
    let started = Instant::now();
    let policy = Policy::load(env)?;
    let project = project
        .canonicalize()
        .map_err(|e| refuse("SPX-HPH010", format!("project path: {e}")))?;
    let project_id = sha256_plain(project.to_string_lossy().as_bytes());
    check_syntax(argv)?;
    let cwd = env
        .cwd
        .canonicalize()
        .ok()
        .filter(|c| c.starts_with(&project))
        .unwrap_or_else(|| project.clone());
    let exe = resolve_executable(&argv[0], &cwd, env.vars.get("PATH").map(String::as_str))?;
    let a_digest = argv_digest(argv);
    let own = lineage::negotiate(
        argv,
        &env.vars,
        opts.external_owner.as_deref(),
        &policy.known_wrappers,
        &format!("exec:{}", &a_digest[7..19]),
    );
    let mut notes: Vec<String> = Vec::new();
    let mut transform = own.host_owns;
    if !own.host_owns {
        notes.push(own.reason.clone());
    }
    if let Some(r) = exclusion(argv, opts.raw) {
        transform = false;
        notes.push(format!("not transformed: {r}"));
    }
    let config = HarnessConfig::load(&project)?;
    let cap = config.capability(CapabilityKind::CommandView);
    if cap.mode == Mode::Disabled {
        transform = false;
    }
    if !cap.scope.is_empty() {
        let rel = cwd
            .strip_prefix(&project)
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .into_owned();
        if !cap
            .scope
            .iter()
            .any(|s| rel == *s || rel.starts_with(&format!("{s}/")))
        {
            transform = false;
            notes.push("not transformed: outside the capability scope".into());
        }
    }

    // Provider resolution happens before launch; a required provider that is
    // missing refuses here, while the command has not run.
    let mut provider: Option<Provider> = None;
    let mut provider_failed = false;
    if transform {
        let res = resolve_project(env, &project)?;
        match Provider::open(env, &project, &project_id, &res, &policy) {
            Ok(p) => provider = p,
            Err(e) if cap.mode == Mode::Required => {
                return Err(refuse(
                    "SPX-HPI001",
                    format!("required command.view provider is unusable: {}", e.message),
                ))
            }
            Err(e) => {
                provider_failed = true;
                notes.push(format!(
                    "provider unavailable, raw view used: {}",
                    e.message
                ));
            }
        }
    }

    let mut effective: Option<Vec<String>> = None;
    let mut run_exe = exe.clone();
    let mut plan_bypass = false;
    if let Some(p) = provider.as_ref() {
        let cwd_rel = match cwd.strip_prefix(&project) {
            Ok(r) if !r.as_os_str().is_empty() => r.to_string_lossy().into_owned(),
            _ => ".".to_string(),
        };
        let mut wrapped = None;
        if p.declares("plan") {
            match p.plan(
                argv,
                &cwd_rel,
                &own.chain,
                policy.allow_wrapper,
                policy.min_bytes,
            ) {
                Ok(PlanRoute::PostExecution) => {}
                Ok(PlanRoute::Bypass(why)) => {
                    plan_bypass = true;
                    notes.push(format!("provider plan: bypass ({why})"));
                }
                // Only a route that declares complete raw recovery and needs no
                // extra environment may wrap; otherwise run once, unwrapped, and view.
                Ok(PlanRoute::Wrapped(plan)) => {
                    if policy.allow_wrapper && plan.raw_recovery_declared && !plan.needs_env {
                        wrapped = Some(plan);
                    } else {
                        notes.push(
                            "provider plan: wrapped route lacks complete raw recovery here; running unwrapped"
                                .into(),
                        );
                    }
                }
                Err(m) => notes.push(format!("provider plan unavailable, running unwrapped: {m}")),
            }
        } else if policy.allow_wrapper && p.declares("wrap") {
            match p.wrap(argv, &own.chain) {
                Ok(plan) => wrapped = Some(plan),
                Err(m) => notes.push(format!("wrapper plan unavailable, running unwrapped: {m}")),
            }
        }
        if let Some(plan) = wrapped {
            let (eff, e) = wrapper::authorize(&plan, argv, &exe, &cwd, &project)?;
            if eff != argv {
                effective = Some(eff);
                run_exe = e;
            }
        }
    }
    let run_argv: &[String] = effective.as_deref().unwrap_or(argv);

    let retention = match (&policy.retention, &env.harness_home) {
        (Some(rp), Some(home)) => Some(Retention::open(home, &project_id, rp)?),
        _ => None,
    };

    let mut cenv: BTreeMap<String, String> = BTreeMap::new();
    for k in &policy.env_grant {
        if let Some(v) = env.vars.get(k) {
            cenv.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in &opts.extra_env {
        if k.is_empty()
            || k.contains(['=', '\0'])
            || k.starts_with("SEMAPRAX_HARNESS_")
            || v.contains('\0')
        {
            return Err(refuse(
                "SPX-HPH010",
                format!("environment name `{k}` is reserved or malformed"),
            ));
        }
        cenv.insert(k.clone(), v.clone());
    }
    let (lk, lv) = lineage_env(&own.chain);
    cenv.insert(lk, lv);
    // The host, never the caller, advances the nesting depth for its children.
    let depth = env
        .vars
        .get(crate::bridge::negotiate::DEPTH_VAR)
        .and_then(|d| d.parse::<u64>().ok())
        .unwrap_or(0);
    cenv.insert(
        crate::bridge::negotiate::DEPTH_VAR.to_string(),
        (depth + 1).to_string(),
    );

    let spill = retention.as_ref().map(Retention::spill);
    let cap_run = executor::run(&ExecSpec {
        argv: run_argv,
        executable: &run_exe,
        cwd: &cwd,
        env: &cenv,
        timeout: Duration::from_millis(opts.timeout_ms.unwrap_or(policy.timeout_ms)),
        mem_cap: policy.mem_cap,
        spill: spill.as_ref().map(|s| executor::Spill {
            stdout: s.stdout.clone(),
            stderr: s.stderr.clone(),
            max_stream: s.max_stream,
        }),
        cancel: &opts.cancel,
    });
    let cap_run = match cap_run {
        Ok(c) => c,
        Err(e) => {
            if let (Some(r), Some(s)) = (&retention, &spill) {
                r.discard(s);
            }
            return Err(refuse("SPX-HPH020", format!("command did not start: {e}")));
        }
    };
    // From here the command has run: nothing below may execute it again.
    let (so, se) = (&cap_run.stdout, &cap_run.stderr);
    let status = cap_run.termination.label();
    let mut handle = None;
    if let (Some(r), Some(s)) = (&retention, &spill) {
        let h = handle_for(&a_digest, &so.digest, &se.digest, &status);
        let meta = json!({"argv_digest": a_digest, "status": status,
            "stdout": {"bytes": so.total, "digest": so.digest, "stored": so.stored},
            "stderr": {"bytes": se.total, "digest": se.digest, "stored": se.stored}});
        match r.commit(s, &h, &meta) {
            Ok(true) => handle = Some(h),
            Ok(false) => notes.push("output exceeds the retention size bound; not retained".into()),
            Err(e) => {
                r.discard(s);
                notes.push(format!("retention failed: {}", e.message));
            }
        }
    }
    let rec = |s: &executor::Stream| StreamRecord {
        bytes: s.total,
        digest: s.digest.clone(),
        retained_complete: handle.is_some() && s.fully_stored(),
    };
    let (ro, re) = (rec(so), rec(se));
    let result = CommandResult {
        argv: argv.to_vec(),
        argv_digest: a_digest.clone(),
        effective_argv: effective.clone(),
        executable: run_exe.to_string_lossy().into_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
        env_grant: cenv
            .keys()
            .filter(|k| *k != lineage::LINEAGE_VAR && *k != crate::bridge::negotiate::DEPTH_VAR)
            .cloned()
            .collect(),
        termination: cap_run.termination.clone(),
        stdout: ro.clone(),
        stderr: re.clone(),
        recovery_handle: handle.clone(),
        executions: 1,
        lineage: own.chain.clone(),
    };

    let display_cap = config.budget.command_view_max_bytes as usize;
    let total = so.total + se.total;
    let in_mem = so.in_memory() && se.in_memory();
    let retained = ro.retained_complete && re.retained_complete;
    let (out_t, out_bad) = decode(&so.head);
    let (err_t, err_bad) = decode(&se.head);
    let raw_all = raw_text(&out_t, &err_t);

    // Decide who produces the text.
    let mut use_provider = provider.as_ref().is_some_and(|p| p.declares("view"))
        && effective.is_none()
        && !plan_bypass;
    let skip = |why: &str, flag: &mut bool, notes: &mut Vec<String>| {
        if *flag {
            notes.push(format!("provider not consulted: {why}"));
        }
        *flag = false;
    };
    if effective.is_some() {
        notes.push(
            "pre-execution wrapper ran; raw recovery is provider-held, view is not proven complete"
                .into(),
        );
    }
    if total < policy.min_bytes {
        skip("small output", &mut use_provider, &mut notes);
    }
    if in_mem && looks_like_json(&so.head) {
        skip("machine-readable stdout", &mut use_provider, &mut notes);
    }
    if looks_binary(&so.head) || looks_binary(&se.head) {
        skip("binary output", &mut use_provider, &mut notes);
    }
    if se.total == 0 && looks_like_digests(&so.head) {
        skip("hash output", &mut use_provider, &mut notes);
    }
    let tk = opts.tokenizer.as_ref();
    let (raw_display, raw_cut) = guard::bound(&raw_all, display_cap);
    let raw_delivered = delivered_text(&raw_display, raw_cut.then_some(()).and(handle.as_deref()));
    let raw_tokens = measure::count(tk, &raw_delivered);
    let mut below_tokens = false;
    if let (Some(min), Some(t)) = (policy.min_tokens, raw_tokens) {
        if t < min {
            below_tokens = use_provider;
            skip(
                "below the measured token threshold",
                &mut use_provider,
                &mut notes,
            );
        }
    }
    let (mut grew, mut rejected_tokens, mut overhead_ms) = (false, None, 0u64);
    if !cap_run.termination.certain() {
        skip("status is uncertain", &mut use_provider, &mut notes);
    }
    if !retained || !in_mem {
        skip(
            "raw output is not fully retained in memory and on disk",
            &mut use_provider,
            &mut notes,
        );
    }
    if let Some(p) = &provider {
        if total as usize > p.input_limit() {
            skip(
                "output exceeds the provider frame",
                &mut use_provider,
                &mut notes,
            );
        }
    }

    let mut view: Option<ModelView> = None;
    if use_provider {
        let p = provider.as_ref().expect("checked");
        let red_o = redact(&out_t, &policy.redact);
        let red_e = redact(&err_t, &policy.redact);
        let opts = ViewOptions {
            min_bytes: policy.min_bytes,
            max_bytes: display_cap as u64,
            recovery_handle: handle.clone(),
        };
        let t0 = Instant::now();
        let pres = p.view(argv, &red_o, &red_e, &opts, &own.chain);
        overhead_ms = t0.elapsed().as_millis() as u64;
        match pres {
            Ok(pv) => {
                let (mut text, mut incomplete) = guard::bound(&pv.text, display_cap);
                let reference = raw_text(&red_o, &red_e);
                let missing = guard::missing_critical(&reference, &text);
                if !missing.is_empty() {
                    guard::append_critical(&mut text, &missing);
                    incomplete = true;
                    notes.push(format!(
                        "provider view dropped {} critical line(s); view marked incomplete",
                        missing.len()
                    ));
                }
                // The provider only saw lossily decoded text; undecodable bytes are omissions it cannot know about.
                let bad = out_bad + err_bad;
                let pv = super::view::ProviderView {
                    lossless: pv.lossless && bad == 0,
                    omissions: pv.omissions + bad,
                    text: pv.text,
                };
                let lossy = !pv.lossless || pv.omissions > 0 || incomplete;
                let cand = delivered_text(&text, if lossy { handle.as_deref() } else { None });
                let cand_tokens = measure::count(tk, &cand);
                // The view must be strictly smaller where it is measured; unavailable
                // token counts fall back to bytes (growth only, never a savings claim).
                grew = match (cand_tokens, raw_tokens) {
                    (Some(c), Some(r)) => c >= r,
                    _ => cand.len() >= raw_delivered.len(),
                };
                rejected_tokens = cand_tokens;
                if grew {
                    notes.push(
                        "provider view did not reduce the delivered size: raw view used".into(),
                    );
                } else {
                    view = Some(ModelView {
                        text,
                        lossless: pv.lossless && !incomplete,
                        omissions: pv.omissions,
                        provenance: p.provider_id.clone(),
                        recovery_handle: if lossy { handle.clone() } else { None },
                        incomplete,
                        route: "provider".into(),
                        notes: vec![],
                        measurement: None,
                    });
                }
            }
            Err(m) => {
                provider_failed = true;
                notes.push(format!(
                    "provider failed, raw view used (command not re-run): {m}"
                ));
            }
        }
    }
    let mut view = view.unwrap_or_else(|| {
        let (mut text, cut) = guard::bound(&raw_all, display_cap);
        let mut incomplete = cut || !in_mem;
        let mut crit: Vec<String> = Vec::new();
        for (st, s, t) in [
            (StreamName::Stdout, so, &out_t),
            (StreamName::Stderr, se, &err_t),
        ] {
            if !s.in_memory() {
                if let (Some(r), Some(h)) = (&retention, &handle) {
                    crit.extend(r.critical_lines(h, st, 20));
                    continue;
                }
            }
            crit.extend(
                t.lines()
                    .filter(|l| guard::is_critical(l))
                    .map(String::from),
            );
        }
        crit.retain(|l| !text.contains(l.trim()));
        if cut || !in_mem {
            guard::append_critical(&mut text, &crit);
        }
        if !cap_run.termination.certain() {
            incomplete = true;
        }
        let kept = text.lines().count() as u64;
        let omissions = out_bad
            + err_bad
            + if cut {
                (raw_all.lines().count() as u64).saturating_sub(kept)
            } else {
                0
            };
        ModelView {
            lossless: !incomplete && omissions == 0,
            omissions,
            provenance: HOST_PROVIDER.into(),
            recovery_handle: if cut || !in_mem || omissions > 0 {
                handle.clone()
            } else {
                None
            },
            incomplete,
            route: if effective.is_some() {
                "wrapper".into()
            } else {
                "raw".into()
            },
            text,
            notes: vec![],
            measurement: None,
        }
    });
    if effective.is_some() {
        view.incomplete = true;
        view.lossless = false;
    }
    if !cap_run.termination.certain() {
        view.incomplete = true;
        notes.push("status is uncertain: the command was killed before it finished".into());
    }
    if view.incomplete && view.recovery_handle.is_none() {
        view.recovery_handle = handle.clone();
    }
    if let Some(h) = &view.recovery_handle {
        notes.push(format!(
            "raw output recoverable: {}",
            recovery_hint(&project, h)
        ));
    } else if view.incomplete {
        notes.push("raw output is not retained; enable retention to make it recoverable".into());
    }
    view.notes = notes;
    let delivered = delivered_text(&view.text, view.recovery_handle.as_deref());
    let dtokens = measure::count(tk, &delivered);
    // The host's own raw view is the baseline; when it is what was delivered,
    // nothing was saved (its critical-line guard text is part of both sides).
    let (raw_delivered, raw_tokens) = if view.route == "provider" {
        (raw_delivered, raw_tokens)
    } else {
        (delivered.clone(), dtokens)
    };
    let named = raw_tokens.is_some() && dtokens.is_some();
    view.measurement = Some(Measurement {
        decision: if view.route == "provider" {
            "provider-smaller"
        } else if grew {
            "provider-grew-raw-used"
        } else if below_tokens {
            "below-token-threshold-raw-used"
        } else if provider_failed {
            "provider-failed-raw-used"
        } else if provider.is_some() {
            "provider-not-consulted"
        } else {
            "raw-by-policy"
        },
        basis: if named { "tokens" } else { "bytes-only" },
        tokenizer: tk
            .filter(|_| named)
            .map(|t| (t.0.name().to_string(), t.0.fingerprint().to_string())),
        raw_bytes: raw_delivered.len() as u64,
        delivered_bytes: delivered.len() as u64,
        raw_tokens: if named { raw_tokens } else { None },
        delivered_tokens: if named { dtokens } else { None },
        rejected_view_tokens: rejected_tokens,
        overhead_ms,
    });

    let envelope = Envelope { result, view };
    let display = if opts.json {
        format!("{}\n", canonical(&envelope.to_json()))
    } else {
        envelope.render()
    };
    if let Some(obs) = observer {
        let id = format!("inv-cv-{}", &a_digest[7..19]);
        let mut o = Observation::new(
            &envelope.view.provenance,
            "command.view",
            Stage::CommandView,
            Role::Transform,
            &id,
        );
        o.payload_id = Some(id.clone());
        o.availability = if provider_failed {
            Availability::Fallback
        } else {
            Availability::Available
        };
        o.latency_ms = started.elapsed().as_millis() as u64;
        o.before = Some(TokenCount::bytes(total));
        o.after = Some(TokenCount::bytes(display.len() as u64));
        if let Some(m) = &envelope.view.measurement {
            if let (Some((n, f)), Some(r), Some(d)) =
                (&m.tokenizer, m.raw_tokens, m.delivered_tokens)
            {
                o.before = Some(TokenCount::named(n, f, r));
                o.after = Some(TokenCount::named(n, f, d));
            }
        }
        o.model_visible = true;
        o.before_digest = Some(sha256_plain(
            format!("{}\0{}", so.digest, se.digest).as_bytes(),
        ));
        o.after_digest = Some(sha256_plain(display.as_bytes()));
        obs.record(o);
    }
    Ok(ExecReport { envelope, display })
}
