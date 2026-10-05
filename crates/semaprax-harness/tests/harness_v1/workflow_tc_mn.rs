//! MN-01 (output-cap retry recovery before dispatch), MN-04 (session limits
//! before an output-cap retry) and MN-07 (strict task controls). Reuses the
//! counted capture adapter of `workflow_tc.rs`; the restart tests keep one
//! `RunConfig` (same project, task identity and cache) across both runs.

use super::*;
use std::sync::atomic::Ordering;

/// Latest journal state of every step of the run's lineage.
fn states(cfg: &RunConfig, lineage: &str) -> std::collections::BTreeMap<String, String> {
    let j =
        std::fs::read_to_string(cfg.cache_dir.join(format!("{lineage}.journal.jsonl"))).unwrap();
    j.lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .map(|v| {
            (
                v["step"].as_str().unwrap().to_string(),
                v["state"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn lcap_artifact(cfg: &RunConfig, lineage: &str) -> std::path::PathBuf {
    cfg.cache_dir
        .join(format!("{lineage}.generate-lcap.proposal.json"))
}

fn retry_cfg(e: &Env) -> RunConfig {
    let mut cfg = config(e, task(Some(100), Destination::Local), None);
    cfg.budget.generation = GenerationPolicy {
        length_retry_cap: Some(400),
        ..Default::default()
    };
    cfg
}

/// First run: a terminal length-limited reply, then a completed larger-cap
/// proposal. Publication stays disabled so recovery reaches acquisition.
fn completed_retry(e: &Env, fake: &Fake) -> (RunConfig, String) {
    let cfg = retry_cfg(e);
    let a = Adapter::new(vec![cut_reply(100), plain_reply()]);
    let (r, _) = drive(&cfg, fake, &a);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(a.calls.get(), 2);
    let s = states(&cfg, &r.lineage);
    assert_eq!(s["generate"], "refused");
    assert_eq!(s["generate-lcap"], "done");
    (cfg, r.lineage.clone())
}

fn ledger_kinds(r: &Report, kind: &str) -> usize {
    r.context["task_ledger"]["entries"]
        .as_array()
        .map_or(0, |a| a.iter().filter(|x| x["kind"] == kind).count())
}

#[test]
fn mn01_a_completed_larger_cap_retry_is_reused_after_restart_with_zero_calls() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let (cfg, lineage) = completed_retry(&e, &fake);
    // Restart on the same lineage: any call would succeed at the original cap.
    let again = Adapter::new(vec![plain_reply()]);
    let (r, _) = drive(&cfg, &fake, &again);
    assert_eq!(r.lineage, lineage);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(again.calls.get(), 0, "no new router or model call");
    let acq = &r.context["proposal_acquisition"];
    assert_eq!(
        (acq["source"].as_str(), acq["model_calls"].as_u64()),
        (Some("journal"), Some(0))
    );
    assert_eq!(acq["step"], "generate-lcap");
    // No new inference reservation; the historical spend stays visible.
    assert_eq!(ledger_kinds(&r, "generation"), 0, "{}", r.context);
    assert!(!acq["historical_incurred"].is_null(), "{acq}");
    assert_eq!(states(&cfg, &lineage)["generate-lcap.local-reuse"], "done");
}

#[test]
fn mn01_an_unresolved_retry_blocks_original_and_larger_cap_dispatch_after_restart() {
    let fake = Fake::new(CHANGED);
    // Uncertain larger-cap attempt.
    let e = setup(FIXED);
    let cfg = retry_cfg(&e);
    let lost: AdapterFn = Box::new(|_| (ResultStatus::Failed, json!({})));
    let a = Adapter::new(vec![cut_reply(100), lost]);
    let (r, _) = drive(&cfg, &fake, &a);
    assert_eq!((codes(&r), a.calls.get()), (vec!["SPX-HPD072"], 2));
    let s = states(&cfg, &r.lineage);
    assert_eq!(
        (s["generate"].as_str(), s["generate-lcap"].as_str()),
        ("refused", "uncertain")
    );
    // Restart: an adapter that would succeed immediately must not be called.
    let again = Adapter::new(vec![plain_reply()]);
    let (r2, _) = drive(&cfg, &fake, &again);
    assert_eq!((codes(&r2), again.calls.get()), (vec!["SPX-HPD072"], 0));
    assert!(
        r2.refusals[0].message.contains("generate-lcap"),
        "{}",
        r2.refusals[0].message
    );
    assert_eq!(ledger_kinds(&r2, "generation"), 0);
    // A begun larger-cap attempt without a terminal record blocks the same way.
    let e = setup(FIXED);
    let (cfg, lineage) = completed_retry(&e, &fake);
    let path = cfg.cache_dir.join(format!("{lineage}.journal.jsonl"));
    let n = std::fs::read_to_string(&path).unwrap().lines().count();
    let mut j = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    use std::io::Write as _;
    writeln!(
        j,
        "{}",
        json!({"detail": {}, "seq": n + 1, "state": "begin", "step": "generate-lcap"})
    )
    .unwrap();
    drop(j);
    let again = Adapter::new(vec![plain_reply()]);
    let (r3, _) = drive(&cfg, &fake, &again);
    assert_eq!((codes(&r3), again.calls.get()), (vec!["SPX-HPD072"], 0));
}

#[test]
fn mn01_unresolved_retry_in_session_stops_before_any_further_dispatch() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut t = task(Some(100), Destination::Local);
    t.session = Some(SessionBounds {
        max_attempts: 3,
        ..Default::default()
    });
    let mut cfg = config(&e, t, None);
    cfg.budget.generation = GenerationPolicy {
        length_retry_cap: Some(400),
        ..Default::default()
    };
    let lost: AdapterFn = Box::new(|_| (ResultStatus::Failed, json!({})));
    let a = Adapter::new(vec![cut_reply(100), lost, plain_reply()]);
    let (r, _) = drive(&cfg, &fake, &a);
    assert_eq!((codes(&r), a.calls.get()), (vec!["SPX-HPD072"], 2));
    let again = Adapter::new(vec![plain_reply()]);
    let (r2, _) = drive(&cfg, &fake, &again);
    assert_eq!((codes(&r2), again.calls.get()), (vec!["SPX-HPD072"], 0));
}

#[test]
fn mn01_missing_tampered_or_oversized_retry_artifacts_fail_closed_without_a_paid_replacement() {
    let fake = Fake::new(CHANGED);
    let tamper: [(&str, fn(&std::path::Path)); 3] = [
        ("missing", |p| std::fs::remove_file(p).unwrap()),
        ("digest mismatch", |p| {
            std::fs::write(p, body(json!({"claims": {"tampered": true}}))).unwrap()
        }),
        ("size bound", |p| {
            let mut b = body(json!({}));
            b.extend(vec![b' '; 1024 * 1024 + 1]);
            std::fs::write(p, b).unwrap()
        }),
    ];
    for (why, f) in tamper {
        let e = setup(FIXED);
        let (cfg, lineage) = completed_retry(&e, &fake);
        f(&lcap_artifact(&cfg, &lineage));
        // A truncated original reply would reach the larger-cap cache read.
        let again = Adapter::new(vec![cut_reply(100), plain_reply()]);
        let (r, _) = drive(&cfg, &fake, &again);
        assert_eq!(
            (codes(&r), again.calls.get()),
            (vec!["SPX-HPD072"], 0),
            "{why}: {:?}",
            r.refusals
        );
        assert!(
            r.refusals[0].message.contains(why),
            "{}",
            r.refusals[0].message
        );
        assert_ne!(r.status, "candidate-ready");
        assert!(
            r.candidate.is_null(),
            "{why}: unchecked bytes reached preview"
        );
    }
}

// ---- MN-04 -------------------------------------------------------------

fn session_cfg(e: &Env, bounds: SessionBounds) -> RunConfig {
    let mut t = task(Some(100), Destination::Local);
    t.session = Some(bounds);
    let mut cfg = config(e, t, None);
    cfg.budget.generation = GenerationPolicy {
        length_retry_cap: Some(400),
        ..Default::default()
    };
    cfg
}

fn refused_body() -> AdapterFn {
    ok_reply(
        json!({"schema": "semaprax.harness-proposal.v1",
               "intent": {"kind": "replace_function_body", "target": "t.f", "fake_refuse": "SPX-G225 bad"}})
        .to_string()
        .into_bytes(),
        json!({"protocol": "responses", "finish_reason": "completed",
               "usage": {"input_tokens": 10, "output_tokens": 3}}),
    )
}

#[test]
fn mn04_max_attempts_one_permits_exactly_one_model_attempt_including_the_retry() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cfg = session_cfg(
        &e,
        SessionBounds {
            max_attempts: 1,
            ..Default::default()
        },
    );
    let a = Adapter::new(vec![cut_reply(100), plain_reply()]);
    let (r, _) = drive(&cfg, &fake, &a);
    assert_eq!(
        a.calls.get(),
        1,
        "the retry is a model attempt: {:?}",
        r.refusals
    );
    assert_eq!(codes(&r), ["SPX-HPD111"]);
    assert!(
        r.refusals[0].message.contains("max_attempts 1"),
        "{}",
        r.refusals[0].message
    );
    let s = &r.session;
    assert_eq!(
        (
            &s["proposal_turns"],
            &s["generation_attempts"],
            &s["router_calls"]
        ),
        (&json!(1), &json!(1), &json!(0)),
        "{s}"
    );
    assert_eq!(ledger_kinds(&r, "generation"), 1);
}

#[test]
fn mn04_a_permitted_retry_counts_once_against_the_session_allowance() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cfg = session_cfg(
        &e,
        SessionBounds {
            max_attempts: 2,
            ..Default::default()
        },
    );
    // Turn one: truncated, then a larger-cap proposal the compiler refuses.
    let a = Adapter::new(vec![cut_reply(100), refused_body(), plain_reply()]);
    let (r, _) = drive(&cfg, &fake, &a);
    assert_eq!(a.calls.get(), 2, "{:?}", r.refusals);
    assert_eq!(codes(&r), ["SPX-HPD111"]);
    let s = &r.session;
    assert_eq!(
        (
            &s["proposal_turns"],
            &s["output_cap_retries"],
            &s["generation_attempts"],
            &s["router_calls"]
        ),
        (&json!(1), &json!(1), &json!(2), &json!(0)),
        "{s}"
    );
    // Reconciled with the ledger and the receipts.
    assert_eq!(ledger_kinds(&r, "generation"), 2);
    assert_eq!(r.context["usage_receipts"]["attempts"], 2);
}

#[test]
fn mn04_cancellation_between_terminal_attempts_prevents_routing_and_generation() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut cfg = session_cfg(
        &e,
        SessionBounds {
            max_attempts: 3,
            ..Default::default()
        },
    );
    let flag: CancelFlag = Default::default();
    cfg.cancel = Some(flag.clone());
    let cut = cut_reply(100);
    let first: AdapterFn = Box::new(move |req| {
        flag.store(true, Ordering::SeqCst);
        cut(req)
    });
    let a = Adapter::new(vec![first, plain_reply()]);
    let (r, _) = drive(&cfg, &fake, &a);
    assert_eq!((codes(&r), a.calls.get()), (vec!["SPX-HPD113"], 1));
    assert_eq!(ledger_kinds(&r, "generation"), 1, "no retry reservation");
    assert_eq!(ledger_kinds(&r, "router"), 0);
}

#[test]
fn mn04_an_expired_session_does_not_start_a_retry() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cfg = session_cfg(
        &e,
        SessionBounds {
            max_attempts: 3,
            max_elapsed_ms: 1000,
            ..Default::default()
        },
    );
    let cut = cut_reply(100);
    let slow: AdapterFn = Box::new(move |req| {
        std::thread::sleep(std::time::Duration::from_millis(1100));
        cut(req)
    });
    let a = Adapter::new(vec![slow, plain_reply()]);
    let (r, _) = drive(&cfg, &fake, &a);
    assert_eq!((codes(&r), a.calls.get()), (vec!["SPX-HPD111"], 1));
    assert!(
        r.refusals[0].message.contains("max_elapsed_ms 1000"),
        "{}",
        r.refusals[0].message
    );
    assert_eq!(ledger_kinds(&r, "generation"), 1);
}

// ---- MN-07 -------------------------------------------------------------

fn v2(extra: Value) -> Vec<u8> {
    let mut v = json!({"schema": "semaprax.harness-task.v2", "goal": "g"});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v.to_string().into_bytes()
}

#[test]
fn mn07_every_present_non_string_mode_is_rejected_and_names_the_member() {
    for bad in [
        json!(true),
        json!(false),
        json!(null),
        json!(1),
        json!([]),
        json!({}),
    ] {
        let e = Task::parse(&v2(json!({"mode": bad}))).unwrap_err();
        assert_eq!(e.code, "SPX-HPD081", "{bad}");
        assert!(e.message.contains("`mode`"), "{bad}: {}", e.message);
    }
    // Omission keeps the legacy default; every documented string keeps its mode.
    assert_eq!(Task::parse(&v2(json!({}))).unwrap().mode, TaskMode::Repair);
    for (s, m) in [
        ("repair", TaskMode::Repair),
        ("change", TaskMode::Change),
        ("plan", TaskMode::Plan),
        ("inspect", TaskMode::Plan),
    ] {
        assert_eq!(Task::parse(&v2(json!({"mode": s}))).unwrap().mode, m, "{s}");
    }
}

#[test]
fn mn07_u32_session_limits_reject_out_of_range_values_without_wraparound() {
    let max = u64::from(u32::MAX);
    for k in [
        "max_attempts",
        "max_candidates",
        "max_tool_calls",
        "max_steps",
    ] {
        for n in [0, 1, 7, max] {
            let t = Task::parse(&v2(json!({"session": {k: n}}))).unwrap();
            let got = t.session.as_ref().unwrap().to_json()[k].as_u64();
            assert_eq!(got, Some(n), "{k}={n}");
            assert_eq!(
                t.digest(),
                Task::parse(&v2(json!({"session": {k: n}})))
                    .unwrap()
                    .digest()
            );
        }
        for n in [max + 1, max + 2] {
            let e = Task::parse(&v2(json!({"session": {k: n}}))).unwrap_err();
            assert_eq!(e.code, "SPX-HPD081", "{k}={n}");
            assert!(
                e.message.contains(&format!("`session.{k}`")),
                "{}",
                e.message
            );
        }
        for bad in [json!(-1), json!(1.5), json!("3"), json!(null)] {
            let e = Task::parse(&v2(json!({"session": {k: bad}}))).unwrap_err();
            assert!(
                e.message.contains(&format!("`session.{k}`")),
                "{}",
                e.message
            );
        }
    }
    // u64-backed limits keep their declared width.
    let big = max + 2;
    let t = Task::parse(&v2(
        json!({"session": {"max_elapsed_ms": big, "max_tokens": big}}),
    ))
    .unwrap();
    let s = t.session.unwrap();
    assert_eq!((s.max_elapsed_ms, s.max_tokens), (big, Some(big)));
}

#[test]
fn mn07_invalid_controls_cause_no_workflow_or_provider_work_and_no_durable_attempt() {
    for (bad, member) in [
        (json!({"mode": true}), "`mode`"),
        (
            json!({"mode": "change", "session": {"max_attempts": u64::from(u32::MAX) + 1}}),
            "`session.max_attempts`",
        ),
    ] {
        let e = setup(FIXED);
        let fake = Fake::new(CHANGED);
        let task_file = e.root.join("host/task.json");
        std::fs::create_dir_all(task_file.parent().unwrap()).unwrap();
        std::fs::write(&task_file, v2(bad.clone())).unwrap();
        let o = RunOptions {
            task: Some(task_file),
            proposal: None,
            apply_policy: None,
            python: None,
            node: None,
            compiler: None,
            observations: None,
            tokenizer_python: None,
            tokenizer_script: None,
            tokenizer_cache: None,
            tokenizers: vec![],
            cancel: None,
            cancel_file: None,
            frozen: true,
            offline: true,
            updates_fixture: None,
            updates_gh: None,
            updates_now: None,
            disable: true,
            json: true,
        };
        let env = semaprax_harness::cli::Environment {
            harness_home: None,
            compiler: None,
            cwd: e.root.clone(),
            vars: Default::default(),
        };
        let snap = Snapshot::capture(&e.project).unwrap();
        let err = run_with(&o, &env, snap, e.cache.clone(), &fake)
            .err()
            .expect("invalid task controls are refused");
        assert_eq!(err.code, "SPX-HPD081", "{bad}");
        assert!(err.message.contains(member), "{bad}: {}", err.message);
        assert!(
            fake.log.borrow().is_empty(),
            "compiler work: {:?}",
            fake.log.borrow()
        );
        assert!(
            !e.cache.exists() || std::fs::read_dir(&e.cache).unwrap().next().is_none(),
            "no journal or cache entry is created"
        );
    }
}
