//! Common suites. `common` checks the adapter under test (identity, binding,
//! declared limits, authority, cancellation); `common.hostility` drives the
//! hostile fixture and asserts the HOST refused each misbehaviour.

use super::report::{check, fail, fail_with, Case, Fail, Suite};
use super::suite::{cancellation, completed, describe, minimal, Rig, Setup, Target};
use crate::contract::{negotiate, CapabilityKind, HostSupport};
use crate::decision::*;
use crate::host::{
    AdapterState, CancelToken, HostBudget, InvocationClass, IsolationRequest, Outcome,
};
use crate::profile::installations::LocalState;
use crate::profile::resolve::current_platform;
use crate::profile::trust::{grant_for, requested_as_granted, TrustRecord};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const ORDER: [CapabilityKind; 4] = [
    CapabilityKind::ContextRepository,
    CapabilityKind::CommandView,
    CapabilityKind::DecisionEvaluate,
    CapabilityKind::SkillCatalog,
];

fn first_active(t: &Target, active: &[CapabilityKind]) -> Option<(CapabilityKind, String, Value)> {
    ORDER
        .iter()
        .filter(|k| active.contains(k))
        .find_map(|k| minimal(t, *k).map(|(o, p)| (*k, o, p)))
}

pub fn run_adapter(t: &Target, active: &[CapabilityKind]) -> Suite {
    let d = &t.descriptor;
    let mut cases = vec![
        check("platform-declared", || {
            let p = current_platform();
            if d.platforms.contains(&p) {
                Ok(json!({"platform": p}))
            } else {
                Err(fail_with(
                    "this OS/arch is not listed by the descriptor; results do not apply to it",
                    json!({"platform": p, "declared": d.platforms}),
                ))
            }
        }),
        check("negotiation-visible", || {
            let n = negotiate(d, &HostSupport::first_wave()).map_err(|e| fail(e.to_string()))?;
            Ok(json!({
                "active": n.active.iter().map(|a| json!({"kind": a.kind.as_str(), "version": a.version, "operations": a.operations})).collect::<Vec<_>>(),
                "inactive": n.inactive.iter().map(|i| json!({"kind": i.kind_name, "version": i.version, "reason": i.reason})).collect::<Vec<_>>(),
            }))
        }),
    ];
    let Some((kind, op, payload)) = first_active(t, active) else {
        cases.push(Case::unverified(
            "adapter-behaviour",
            "no active capability with a minimal request",
        ));
        return Suite::new("common", "adapter", cases);
    };
    let open = |setup: Setup| Rig::open_with(t, setup);
    cases.push(match open(Setup::default()) {
        Err(e) => Case::failed("identity-and-binding", fail(e.to_string())),
        Ok(mut rig) => check("identity-and-binding", || {
            let r = completed(rig.run(kind, &op, payload.clone()))?;
            if r.provenance.provider_id != d.provider_id {
                return Err(fail_with("result provenance names a different provider (spoofed identity)", json!({"claimed": r.provenance.provider_id, "declared": d.provider_id})));
            }
            Ok(json!({"provider_id": r.provenance.provider_id, "adapter_version": r.provenance.adapter_version, "state": rig.handle.state().name()}))
        }),
    });
    cases.push(match open(Setup::default()) {
        Err(e) => Case::failed("revision-rebinding", fail(e.to_string())),
        Ok(mut rig) => check("revision-rebinding", || {
            completed(rig.run(kind, &op, payload.clone()))?;
            rig.bump();
            completed(rig.run(kind, &op, payload.clone()))?;
            Ok(json!({"revisions": 2, "note": "a result bound to a different revision is quarantined (SPX-HPA032)"}))
        }),
    });
    cases.push(frame_limit(t, kind));
    cases.push(match open(Setup::default()) {
        Err(e) => Case::failed("no-ambient-authority", fail(e.to_string())),
        Ok(rig) => check("no-ambient-authority", || {
            let g = rig.grant.permissions();
            if !(g.network.is_empty() && g.process.is_empty() && g.secrets.is_empty()) {
                return Err(fail("the conformance grant carries network, process or secret authority"));
            }
            Ok(json!({"requested": d.permissions.network.len() + d.permissions.process.len() + d.permissions.secrets.len(),
                      "granted_read": g.read, "granted_write": g.write,
                      "isolation_requested": rig.requested_isolation, "isolation_observed": rig.isolation_name(),
                      "isolation_declared": d.support.isolation}))
        }),
    });
    cases.push(check("endpoint-escalation-refused", || {
        endpoint_escalation(t)
    }));
    cases.push(match open(Setup::default()) {
        Err(e) => Case::failed("cancellation-cooperative", fail(e.to_string())),
        Ok(mut rig) => check("cancellation-cooperative", || {
            cancellation(&mut rig, kind, &op, payload.clone())
        }),
    });
    cases.push(check("recursive-invocation-refused", || recursion(t)));
    Suite::new("common", "adapter", cases)
}

/// The descriptor's declared `max_frame_bytes` is enforced by the host.
fn frame_limit(t: &Target, kind: CapabilityKind) -> Case {
    let name = "declared-frame-limit-enforced";
    let (op, payload, files): (&str, Value, Vec<(String, String)>) = match kind {
        CapabilityKind::ContextRepository => {
            let body: String = (0..80)
                .map(|i| format!("fn f{i}() {{ conformance_inflate(); }}\n"))
                .collect();
            let q = if t.ops(kind).iter().any(|o| o == "search") {
                ("search", json!({"query": "conformance_inflate"}))
            } else {
                ("references", json!({"symbol": "conformance_inflate"}))
            };
            (q.0, q.1, vec![("src/big.rs".into(), body)])
        }
        CapabilityKind::CommandView => {
            let out: String = (0..400)
                .map(|i| format!("unique output line {i}\n"))
                .collect();
            (
                "view",
                json!({"form": "post-execution", "argv": ["x"], "stdout": out, "stderr": ""}),
                vec![],
            )
        }
        CapabilityKind::DecisionEvaluate => {
            let opts: Vec<String> = (0..300).map(|i| format!("opt-{i:04}")).collect();
            (
                "evaluate",
                json!({"task": "model-route/v1", "features": {"complexity": 0.5}, "options": opts}),
                vec![],
            )
        }
        _ => {
            return Case::unverified(
                name,
                "no way to make this capability answer above the limit",
            )
        }
    };
    let setup = Setup {
        files: files
            .into_iter()
            .map(|(p, c)| (p, c.into_bytes()))
            .collect(),
        edit: Some(Box::new(|v| {
            v["resources"]["max_frame_bytes"] = json!(2048)
        })),
        ..Default::default()
    };
    let mut rig = match Rig::open_with(t, setup) {
        Ok(r) => r,
        Err(e) => return Case::failed(name, fail(e.to_string())),
    };
    match rig.run(kind, op, payload) {
        Outcome::Quarantined(d) if d.code == "SPX-HPC012" => {
            Case::pass(name, json!({"limit": 2048, "code": d.code}))
        }
        Outcome::Completed(_) => Case::unverified(
            name,
            "the response fit within 2048 bytes; the cap was not exercised",
        ),
        o => Case::failed(
            name,
            fail_with(
                "unexpected host outcome for an oversized frame",
                describe(&o),
            ),
        ),
    }
}

/// A descriptor that requests network without a grant never gets one.
fn endpoint_escalation(t: &Target) -> Result<Value, Fail> {
    let id = &t.descriptor.provider_id;
    let mut v = t.descriptor.to_json();
    v["permissions"]["network"] = json!(["example.invalid:443"]);
    let wider = crate::contract::Descriptor::parse(v.to_string().as_bytes())
        .map_err(|e| fail(e.to_string()))?;
    let cur = crate::profile::installations::CurrentDigests {
        descriptor_digest: wider.digest().to_string(),
        entry_digest: Some("sha256:e".into()),
        upstream_digest: None,
        requires_upstream: false,
        requested: wider.permissions.clone(),
    };
    let untrusted = grant_for(&LocalState::default(), id, &cur)
        .err()
        .map(|e| e.code);
    let mut st = LocalState::default();
    let mut base = cur.clone();
    base.requested = t.descriptor.permissions.clone();
    st.trust.insert(
        id.clone(),
        TrustRecord {
            descriptor_digest: cur.descriptor_digest.clone(),
            entry_digest: cur.entry_digest.clone(),
            upstream_digest: None,
            granted: requested_as_granted(&base.requested),
        },
    );
    let widened = grant_for(&st, id, &cur).err().map(|e| e.code);
    match (untrusted, widened) {
        (Some("SPX-HPB030"), Some("SPX-HPB032")) => {
            Ok(json!({"untrusted": "SPX-HPB030", "widened": "SPX-HPB032"}))
        }
        other => Err(fail_with(
            "a network request was granted without approval",
            json!({"got": format!("{other:?}")}),
        )),
    }
}

/// The router's recursion guard blocks an adapter whose lineage contains it,
/// before any adapter call is made.
fn recursion(t: &Target) -> Result<Value, Fail> {
    struct Never(u32);
    impl DecisionInvoker for Never {
        fn evaluate(&mut self, _: &crate::contract::RequestEnvelope) -> DecisionCall {
            self.0 += 1;
            DecisionCall::Unavailable
        }
    }
    let plan = |id: &str, dest, cost, rank| ModelPlan {
        id: id.into(),
        destination: dest,
        structured_output: true,
        tools: true,
        max_context: 100_000,
        est_cost_micros: cost,
        est_latency_ms: 500,
        strength_rank: rank,
    };
    let features = TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 1000,
        requires_structured_output: false,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let catalog = vec![
        plan("cheap-local", Destination::Local, 10, 1),
        plan("mid-local", Destination::Local, 50, 4),
    ];
    let budget = Budget {
        max_cost_micros: 10_000,
        max_latency_ms: 5_000,
        max_router_calls: 2,
    };
    let inputs = RouteInputs {
        request: RouteRequest::new(features, catalog, budget).map_err(|e| fail(e.to_string()))?,
        policy: RoutePolicy::default(),
    };
    let id = t.descriptor.provider_id.clone();
    let ctx = RouteContext {
        project: crate::contract::ProjectBinding {
            id: "p".into(),
            worktree: "w".into(),
            revision: "r".into(),
        },
        lock_digest: "lock".into(),
        invocation_id: "inv-1".into(),
        lineage_id: "lin-1".into(),
        router_lineage: vec![id.clone()],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    let mut inv = Never(0);
    let mut p = ConfiguredProvider {
        profile: ProviderProfile {
            provider_id: id.clone(),
            model_id: "m".into(),
            checkpoint: "c".into(),
            min_confidence: None,
            max_context_tokens: None,
            supported_families: None,
        },
        invoker: &mut inv,
        mode: ProviderMode::Explicit,
        gate: EnablementGate::not_evaluated("model-route/v1", &id),
    };
    let d = decide(&inputs, &ctx, Some(&mut p), &|| inputs.clone(), None)
        .map_err(|e| fail(e.to_string()))?;
    drop(p);
    if d.source == DecisionSource::Fallback(FallbackReason::RecursionBlocked) && inv.0 == 0 {
        Ok(json!({"source": "fallback:recursion-blocked", "adapter_calls": 0}))
    } else {
        Err(fail_with(
            "a provider already on the lineage was consulted",
            json!({"calls": inv.0}),
        ))
    }
}

// ---- host hostility (fixture-driven) -------------------------------------

type Expect = fn(&Outcome, &Rig) -> Result<Value, Fail>;

fn quarantined(o: &Outcome, rig: &Rig) -> Result<Value, Fail> {
    match o {
        Outcome::Quarantined(d)
            if matches!(rig.handle.state(), AdapterState::Quarantined(_))
                && rig.handle.pid().is_none() =>
        {
            Ok(json!({"code": d.code}))
        }
        other => Err(fail_with(
            "host did not quarantine the adapter",
            describe(other),
        )),
    }
}

fn refused(code: &'static str) -> impl Fn(&Outcome) -> Result<Value, Fail> {
    move |o| match o {
        Outcome::Refused(d) if d.code == code => Ok(json!({"code": d.code})),
        other => Err(fail_with(
            format!("host did not refuse with {code}"),
            describe(other),
        )),
    }
}

fn hostile_case(
    h: &Target,
    name: &str,
    mode: &str,
    kind: CapabilityKind,
    op: &str,
    payload: Value,
    expect: Expect,
) -> Case {
    let mut rig = match Rig::open_with(h, Setup::default().env("HOSTILE_MODE", mode)) {
        Ok(r) => r,
        Err(e) => return Case::failed(name, fail(e.to_string())),
    };
    let started = Instant::now();
    let o = rig.run(kind, op, payload);
    check(name, || {
        let mut ev = expect(&o, &rig)?;
        ev["mode"] = json!(mode);
        if started.elapsed() > Duration::from_secs(15) {
            return Err(fail("host took longer than 15s to settle"));
        }
        Ok(ev)
    })
}

pub fn run_hostility(h: Option<&Target>) -> Suite {
    let name = "common.hostility";
    let Some(h) = h else {
        return Suite::new(name, "host", vec![Case::unverified(
            "hostile-fixture",
            "hostile-python fixture or a python runtime (`--hostile-runtime`) is not available; host refusals not exercised",
        )]);
    };
    let ctx = (
        CapabilityKind::ContextRepository,
        "search",
        json!({"query": "x"}),
    );
    let dec = (
        CapabilityKind::DecisionEvaluate,
        "evaluate",
        json!({"task": "model-route/v1", "features": {}, "options": ["a", "b"]}),
    );
    let mut cases = Vec::new();
    for (case, mode) in [
        ("spoofed-invocation-id", "spoof_invocation"),
        ("spoofed-project-id", "spoof_project"),
        ("stale-revision-refused", "fake_revision"),
        ("protocol-version-mismatch", "wrong_protocol"),
        ("malformed-frame", "malformed_frame"),
        ("oversized-frame", "oversized_frame"),
        ("response-flood", "flood"),
        ("unsolicited-host-request", "unsolicited_request"),
        ("sampling-request", "sampling_request"),
    ] {
        cases.push(hostile_case(
            h,
            case,
            mode,
            dec.0,
            dec.1,
            dec.2.clone(),
            quarantined,
        ));
    }
    for (case, mode) in [
        ("path-escape", "path_escape"),
        ("absolute-path", "absolute_path"),
    ] {
        cases.push(hostile_case(
            h,
            case,
            mode,
            ctx.0,
            ctx.1,
            ctx.2.clone(),
            |o, _| refused("SPX-HPA041")(o),
        ));
    }
    cases.push(hostile_case(
        h,
        "forbidden-choice",
        "forbidden_model",
        dec.0,
        dec.1,
        dec.2.clone(),
        |o, _| refused("SPX-HPA043")(o),
    ));
    cases.push(hostile_case(
        h,
        "crash-is-contained",
        "crash_on_invoke",
        dec.0,
        dec.1,
        dec.2.clone(),
        |o, _| match o {
            Outcome::Unavailable {
                fallback_allowed: true,
                ..
            } => Ok(describe(o)),
            other => Err(fail_with(
                "crash not reported as retryable unavailability",
                describe(other),
            )),
        },
    ));
    cases.push(hostile_case(
        h,
        "handshake-timeout",
        "hang_on_initialize",
        dec.0,
        dec.1,
        dec.2.clone(),
        |o, _| match o {
            Outcome::Unavailable { .. } => Ok(describe(o)),
            other => Err(fail_with("hung handshake not bounded", describe(other))),
        },
    ));
    cases.push(hostile_case(
        h,
        "stderr-flood-bounded",
        "stderr_flood",
        dec.0,
        dec.1,
        dec.2.clone(),
        |_, rig| {
            let (tail, dropped) = rig.handle.stderr_tail();
            if tail.len() > 64 * 1024 {
                return Err(fail("stderr tail exceeds the ring bound"));
            }
            Ok(json!({"tail_bytes_le": 65536, "dropped_some": dropped > 0}))
        },
    ));
    cases.push(job_budget(h, &dec));
    cases.push(cancel_group_kill(h, &dec));
    cases.push(secrets_unrestricted(h, &ctx));
    cases.push(secrets_restricted(h, &ctx));
    Suite::new(name, "host", cases)
}

fn job_budget(h: &Target, dec: &(CapabilityKind, &str, Value)) -> Case {
    let name = "budget-abuse-job-cap";
    let setup = Setup {
        cfg: Some(Box::new(|c| {
            c.budget = HostBudget {
                max_jobs: 1,
                ..HostBudget::default()
            }
        })),
        ..Default::default()
    };
    let mut rig = match Rig::open_with(h, setup) {
        Ok(r) => r,
        Err(e) => return Case::failed(name, fail(e.to_string())),
    };
    let _ = rig.run(dec.0, dec.1, dec.2.clone());
    let second = rig.run(dec.0, dec.1, dec.2.clone());
    check(name, || refused("SPX-HPC022")(&second))
}

fn pid_alive(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

fn cancel_group_kill(h: &Target, dec: &(CapabilityKind, &str, Value)) -> Case {
    let name = "cancellation-group-kill-with-grandchild";
    let pidfile = h
        .tmp
        .canonicalize()
        .unwrap_or_else(|_| h.tmp.clone())
        .join(format!("hp-conformance-pids-{}", std::process::id()));
    let _ = std::fs::remove_file(&pidfile);
    let setup = Setup::default()
        .env("HOSTILE_MODE", "ignore_cancel")
        .env("HOSTILE_PIDFILE", &pidfile.to_string_lossy());
    let mut rig = match Rig::open_with(h, setup) {
        Ok(r) => r,
        Err(e) => return Case::failed(name, fail(e.to_string())),
    };
    let req = rig.request(dec.0, dec.1, dec.2.clone());
    let token = CancelToken::new();
    let t2 = token.clone();
    let pf = pidfile.clone();
    let waiter = std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(10);
        while !pf.exists() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(50));
        t2.cancel();
    });
    let out = rig.handle.invoke(&req, InvocationClass::SafeRead, &token);
    let _ = waiter.join();
    let case = check(name, || {
        if out != Outcome::Cancelled {
            return Err(fail_with(
                "cancel of a non-cooperative adapter did not end Cancelled",
                describe(&out),
            ));
        }
        let pids: Vec<i32> = std::fs::read_to_string(&pidfile)
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if pids.len() != 2 {
            return Err(fail("hostile adapter never recorded its pids"));
        }
        let end = Instant::now() + Duration::from_secs(5);
        while pids.iter().any(|p| pid_alive(*p)) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        if pids.iter().any(|p| pid_alive(*p)) {
            return Err(fail("adapter or grandchild survived group kill"));
        }
        Ok(json!({"adapter_and_grandchild_reaped": true, "state": rig.handle.state().name()}))
    });
    let _ = std::fs::remove_file(&pidfile);
    case
}

fn secret_setup(h: &Target, restricted: bool) -> Result<(Rig, String), Case> {
    let name = if restricted {
        "secrets-restricted-isolation"
    } else {
        "secrets-unrestricted-recorded-honestly"
    };
    let secret_dir = h
        .tmp
        .canonicalize()
        .unwrap_or_else(|_| h.tmp.clone())
        .join(format!(
            "hp-conformance-secret-{}-{}",
            std::process::id(),
            restricted
        ));
    let _ = std::fs::create_dir_all(&secret_dir);
    let secret = secret_dir.join("secret.txt");
    let _ = std::fs::write(&secret, "TOPSECRET-conformance-planted");
    let mut s = Setup::default()
        .env("HOSTILE_MODE", "secret_probe")
        .env("SECRET_PATH", &secret.to_string_lossy());
    if restricted {
        // allow_read is filled with the project path by the rig's own default
        s.isolation = None;
    } else {
        s.isolation = Some(IsolationRequest::None);
    }
    let mut h2 = h.clone();
    h2.restricted = restricted;
    Rig::open_with(&h2, s)
        .map(|r| (r, secret_dir.to_string_lossy().into_owned()))
        .map_err(|e| Case::failed(name, fail(e.to_string())))
}

fn secrets_unrestricted(h: &Target, ctx: &(CapabilityKind, &str, Value)) -> Case {
    let name = "secrets-unrestricted-recorded-honestly";
    let (mut rig, dir) = match secret_setup(h, false) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let o = rig.run(ctx.0, ctx.1, ctx.2.clone());
    let _ = std::fs::remove_dir_all(dir);
    check(name, || {
        let mode = rig.isolation_name();
        if mode != "subprocess" {
            return Err(fail_with(
                "an unrestricted launch was labelled as isolated",
                json!({"mode": mode}),
            ));
        }
        completed(o)?;
        Ok(
            json!({"isolation": mode, "note": "no OS enforcement: a subprocess can read what the user can read"}),
        )
    })
}

fn secrets_restricted(h: &Target, ctx: &(CapabilityKind, &str, Value)) -> Case {
    let name = "secrets-restricted-isolation";
    let backend = crate::host::IsolationBackend::detect();
    if backend.mechanism().is_none() {
        return Case::unverified(
            name,
            "no sandbox-exec or bwrap here; restriction is not enforceable and was not claimed",
        );
    }
    let (mut rig, dir) = match secret_setup(h, true) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let o = rig.run(ctx.0, ctx.1, ctx.2.clone());
    let _ = std::fs::remove_dir_all(dir);
    check(name, || {
        let r = completed(o)?;
        let text = r
            .payload
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default();
        if text.contains("TOPSECRET") {
            return Err(fail("restricted adapter read the planted secret"));
        }
        Ok(json!({"isolation": rig.isolation_name(), "secret_readable": false}))
    })
}
