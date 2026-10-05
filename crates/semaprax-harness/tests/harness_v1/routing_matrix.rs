//! MR-13 matched routing evidence (fixture prefix `hp-mr13`). Everything here
//! is a contract fixture or a test double: no model, adapter process, network
//! or paid call. The out-of-tree router below is written in this test (not in
//! the harness) and adopted only through a registry entry, so a future provider
//! inherits the same lane.

use crate::support::{fixture_dir, repo_root, write};
use semaprax_harness::bench::routing_matrix::cli::gate_specs_from_json;
use semaprax_harness::bench::routing_matrix::exec::{
    load_fixture, reconcile, router_charge, AttemptResult, AttemptRun, CellExecutor, CellRun,
    FixtureExecutor,
};
use semaprax_harness::bench::routing_matrix::registry::{Registry, RouterPrice};
use semaprax_harness::bench::routing_matrix::run::{run, session_admits, Lane, MatrixRun};
use semaprax_harness::bench::routing_matrix::{Item, TaskSet};
use semaprax_harness::cli::{run as cli_run, Environment};
use semaprax_harness::contract::RequestEnvelope;
use semaprax_harness::decision::governed::{DriftMonitor, ProfileStore, SessionLock};
use semaprax_harness::decision::qualify::{evaluate_domain, DomainEvidence, DomainGateSpec};
use semaprax_harness::decision::route_v2::ExecutionDomain;
use semaprax_harness::decision::*;
use semaprax_harness::receipt::{PriceBook, Usage};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const HARDWARE: &str =
    "macOS arm64 laptop, no GPU, no paid credentials; fixture lane only (MR-13 recorded run)";
const OOT: &str = "learned:org.example/oot-router/oot-v1";

fn bench_dir() -> PathBuf {
    repo_root().join("benchmarks/harness/2026-10-05-routing-matrix")
}

fn read(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn env(cwd: &Path) -> Environment {
    Environment {
        harness_home: None,
        compiler: None,
        cwd: cwd.to_path_buf(),
        vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    }
}

fn specs(min_items: usize) -> Vec<DomainGateSpec> {
    let mut v = gate_specs_from_json(&read(&bench_dir().join("gate-spec.json"))).unwrap();
    for x in &mut v {
        x.spec.min_items = min_items;
    }
    v
}

/// The out-of-tree router: answers `model-route/v1` with the second option.
struct OotRouter {
    calls: u32,
}

impl DecisionInvoker for OotRouter {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall {
        self.calls += 1;
        let opts: Vec<String> = request.payload["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_str().unwrap().to_string())
            .collect();
        let pick = opts[1.min(opts.len() - 1)].clone();
        let scores: serde_json::Map<String, Value> = opts
            .iter()
            .map(|o| {
                (
                    o.clone(),
                    json!(if *o == pick {
                        0.8
                    } else {
                        0.2 / (opts.len() - 1) as f64
                    }),
                )
            })
            .collect();
        DecisionCall::Answered {
            result: json!({"choice": pick, "scores": scores, "abstain": false}),
            elapsed_ms: 1,
            call: None,
        }
    }
}

/// A test double that declares itself a real executor: the cheap model fails
/// three times (and rambles), the others succeed first time.
struct RealDouble {
    claim_fixture_origin_forged: bool,
}

fn attempt(input: u64, output: u64, result: AttemptResult, latency: u64) -> AttemptRun {
    AttemptRun {
        usage: Usage {
            input_total: Some(input),
            cache_read: Some(0),
            cache_write: Some(0),
            output: Some(output),
            ..Usage::default()
        },
        result,
        latency_ms: Some(latency),
        gateway_retries: 0,
    }
}

impl CellExecutor for RealDouble {
    fn class(&self) -> Origin {
        if self.claim_fixture_origin_forged {
            Origin::Fixture
        } else {
            Origin::Real
        }
    }
    fn identity(&self) -> String {
        "test-double".into()
    }
    fn execute(&mut self, _item: &Item, model: &str) -> CellRun {
        let (completed, attempts) = match model {
            "m-cheap" => (
                false,
                (0..3)
                    .map(|_| attempt(6000, 5000, AttemptResult::Failed, 100))
                    .collect(),
            ),
            _ => (true, vec![attempt(6000, 700, AttemptResult::Accepted, 200)]),
        };
        CellRun {
            origin: Origin::Real,
            completed,
            regressions: 0,
            attempts,
            retry_owner: RetryOwner::Host,
            unavailable: None,
        }
    }
}

fn item(id: &str, stratum: &str, family: &str, split: &str, domain: &str) -> Value {
    json!({"id": id, "domain": domain, "partition": "repo:example", "stratum": stratum, "split": split,
           "verifier": if domain == "development" { "dev-tests" } else { "app-typed" },
           "content_digest": format!("sha256:{id}"),
           "features": {"task_family": family, "estimated_context_tokens": 6000,
                        "requires_structured_output": false, "requires_tools": false,
                        "confidentiality": "project", "latency_class": "interactive"}})
}

/// 30 sealed localized-debug items, 4 sealed mechanical items (rules-only:
/// shadow) and 12 calibration items, all in one domain.
fn synthetic(domain: &str, mutate: impl FnOnce(&mut Vec<Value>)) -> TaskSet {
    let mut tasks = read(&bench_dir().join("tasks.json"));
    let mut items: Vec<Value> = (0..30)
        .map(|k| {
            item(
                &format!("e-{k:02}"),
                "localized_debug",
                "localized_debug",
                "eval",
                domain,
            )
        })
        .chain((0..4).map(|k| {
            item(
                &format!("m-{k:02}"),
                "mechanical",
                "mechanical",
                "eval",
                domain,
            )
        }))
        .chain((0..12).map(|k| {
            item(
                &format!("c-{k:02}"),
                "localized_debug",
                "localized_debug",
                "calibration",
                domain,
            )
        }))
        .collect();
    mutate(&mut items);
    tasks["items"] = json!(items);
    TaskSet::from_json(&tasks).unwrap()
}

fn oot_registry(dir: &Path, trained_on: &[&str]) -> Registry {
    let desc = json!({"schema": "semaprax.harness-provider.v1",
        "provider": {"id": "org.example/oot-router", "version": "0.1.0"},
        "adapter": {"runtime": "python", "entry": ["adapter.py"], "version": "0.1.0"},
        "capabilities": [{"kind": "decision.evaluate", "version": 1, "required": true, "operations": ["evaluate"]}],
        "permissions": {"read": [], "write": [], "network": [], "process": [], "secrets": []}});
    let hosted = json!({"schema": "semaprax.harness-provider.v1",
        "provider": {"id": "org.example/hosted-router", "version": "0.1.0"},
        "adapter": {"runtime": "python", "entry": ["adapter.py"], "version": "0.1.0"},
        "capabilities": [{"kind": "decision.evaluate", "version": 2, "required": true, "operations": ["evaluate"]}],
        "permissions": {"read": [], "write": [], "network": ["https://example.invalid"], "process": [],
                        "secrets": ["SEMAPRAX_HARNESS_SECRET_EXAMPLE"]}});
    let not_decision = json!({"schema": "semaprax.harness-provider.v1",
        "provider": {"id": "org.example/indexer", "version": "0.1.0"},
        "adapter": {"runtime": "python", "entry": ["adapter.py"], "version": "0.1.0"},
        "capabilities": [{"kind": "context.index", "version": 1, "required": true, "operations": ["index"]}],
        "permissions": {"read": [], "write": [], "network": [], "process": [], "secrets": []}});
    write(dir, "vendor/oot/harness-provider.json", &desc.to_string());
    write(
        dir,
        "vendor/hosted/harness-provider.json",
        &hosted.to_string(),
    );
    write(
        dir,
        "vendor/indexer/harness-provider.json",
        &not_decision.to_string(),
    );
    let both = json!(["development", "application"]);
    let mp = |id: &str, kind: &str| json!({"profile_id": id, "model": id, "checkpoint": format!("{id}-ck"), "identity_kind": kind});
    let reg = json!({"schema": "semaprax.harness-routing-registry.v1",
        "generation_profiles": [
            {"model": "m-cheap", "domains": both}, {"model": "m-mid", "domains": both},
            {"model": "m-strong", "domains": both}],
        "decision_profiles": [
            {"descriptor": "vendor/oot/harness-provider.json", "domains": both,
             "model_profile": mp("oot-v1", "local_declared"), "instance": {"instance_id": "oot"},
             "router_price": "non_billed", "trained_on": trained_on},
            {"descriptor": "vendor/hosted/harness-provider.json", "domains": both,
             "model_profile": mp("hosted-v1", "local_declared"), "instance": {"instance_id": "hosted"},
             "router_price": {"input": 1000000, "output": 1000000, "max_call_micros": 900}},
            {"descriptor": "vendor/indexer/harness-provider.json", "domains": both,
             "model_profile": mp("idx-v1", "local_declared"), "instance": {"instance_id": "idx"},
             "router_price": "non_billed"}]});
    Registry::from_json(&reg, dir).unwrap()
}

fn go_run(dir: &Path, tasks: &TaskSet, trained_on: &[&str], forged: bool) -> (Registry, MatrixRun) {
    let reg = oot_registry(dir, trained_on);
    let mut ex = RealDouble {
        claim_fixture_origin_forged: forged,
    };
    let mut router = OotRouter { calls: 0 };
    let invokers: BTreeMap<String, &mut dyn DecisionInvoker> =
        BTreeMap::from([(OOT.to_string(), &mut router as &mut dyn DecisionInvoker)]);
    let r = run(
        &reg,
        tasks,
        &specs(30),
        &env(dir),
        Lane {
            executor: &mut ex,
            invokers,
        },
    )
    .unwrap();
    (reg, r)
}

fn decision<'a>(
    r: &'a MatrixRun,
    arm: &str,
    d: ExecutionDomain,
) -> &'a semaprax_harness::bench::routing_matrix::run::ArmDecision {
    r.decisions
        .iter()
        .find(|x| x.arm == arm && x.domain == d)
        .unwrap()
}

#[test]
fn discovery_is_registry_driven_and_unavailable_cells_never_succeed() {
    let dir = fixture_dir("hp-mr13").canonicalize().unwrap();
    let tasks = synthetic("development", |_| {});
    let (reg, r) = go_run(&dir, &tasks, &[], false);
    assert_eq!(
        reg.excluded.len(),
        1,
        "a descriptor without decision.evaluate is not an arm"
    );
    let ids: Vec<&str> = r.arms.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "rules",
            "cost-aware",
            "fixed:m-cheap",
            "fixed:m-mid",
            "fixed:m-strong",
            OOT,
            "learned:org.example/hosted-router/hosted-v1"
        ]
    );
    let hosted = r.arms.iter().find(|a| a.id.contains("hosted")).unwrap();
    let why = hosted.unavailable.as_deref().unwrap();
    assert!(why.contains("SEMAPRAX_HARNESS_SECRET_EXAMPLE"), "{why}");
    let hosted_cells: Vec<_> = r.cells.iter().filter(|c| c.arm == hosted.id).collect();
    assert_eq!(hosted_cells.len(), tasks.items.len());
    assert!(hosted_cells.iter().all(|c| c.origin == Origin::Unavailable
        && !c.completed
        && c.cost_micros.is_none()
        && c.router.micros == 0));
    let d = decision(&r, &hosted.id, ExecutionDomain::Development);
    assert_eq!(d.status, "not-evaluated");
    // A missing integration is never recorded as an outcome at all.
    assert!(d.decision.is_none());
    // Matched: every arm that chose m-mid on an item shares one execution.
    let mid: Vec<_> = r
        .cells
        .iter()
        .filter(|c| c.item == "e-00" && c.model.as_deref() == Some("m-mid"))
        .collect();
    assert!(
        mid.len() >= 3,
        "fixed, learned and cost-aware all chose m-mid"
    );
    assert!(mid
        .windows(2)
        .all(|w| w[0].cost_micros == w[1].cost_micros && w[0].cold == w[1].cold));
    // TC-10 cost arm read only calibration evidence and picked the cheaper accepted strategy.
    let ca = r
        .cells
        .iter()
        .find(|c| c.arm == "cost-aware" && c.item == "e-00")
        .unwrap();
    assert_eq!(
        (ca.source.as_str(), ca.model.as_deref()),
        ("cost_aware", Some("m-mid"))
    );
}

#[test]
fn receipts_reconcile_failures_cache_retry_owner_and_router_overhead() {
    let prices =
        PriceBook::from_json(&read(&bench_dir().join("tasks.json"))["price_book"]).unwrap();
    let budget = MatchedBudget {
        max_cost_micros: 200_000,
        max_attempts: 3,
    };
    let mut run = CellRun {
        origin: Origin::Real,
        completed: true,
        regressions: 0,
        attempts: vec![
            attempt(6000, 600, AttemptResult::Failed, 100),
            attempt(6000, 600, AttemptResult::Accepted, 100),
        ],
        retry_owner: RetryOwner::Host,
        unavailable: None,
    };
    let r = reconcile(&run, "m-cheap", &prices, &budget);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!((r.attempts, r.failed_attempts), (2, 1));
    assert_eq!(
        r.cost_micros,
        Some(2 * (900 + 360)),
        "the failed generation is charged"
    );
    // Cache: a read above the input total does not reconcile; a valid read is `warm`.
    run.attempts[1].usage.cache_read = Some(7000);
    assert!(reconcile(&run, "m-cheap", &prices, &budget).errors[0].contains("cache read"));
    run.attempts[1].usage.cache_read = Some(4000);
    assert_eq!(
        reconcile(&run, "m-cheap", &prices, &budget).cache,
        ["cold", "warm"]
    );
    // Retry owner: gateway-owned transport retries retried again by the host count twice.
    run.attempts[0].result = AttemptResult::TransportError;
    run.retry_owner = RetryOwner::Gateway;
    assert!(reconcile(&run, "m-cheap", &prices, &budget).errors[0].contains("double count"));
    run.retry_owner = RetryOwner::Host;
    run.attempts[0].gateway_retries = 2;
    assert!(reconcile(&run, "m-cheap", &prices, &budget).errors[0].contains("host owns retries"));
    // Unknown price: never a zero-cost cell.
    run.attempts[0].gateway_retries = 0;
    assert_eq!(
        reconcile(&run, "m-unpriced", &prices, &budget).cost_micros,
        None
    );
    // Router overhead (MR-03 CallMetadata): provider-reported usage of one call
    // settles exactly; unknown usage keeps the reservation even when the
    // adapter claims `billing: local`.
    let priced = RouterPrice::Priced {
        input: 1_000_000,
        output: 2_000_000,
        max_call_micros: 900,
    };
    let call = |basis: UsageBasis, billing: Billing| CallMetadata {
        adapter: "org.example/hosted-router@0.1.0".into(),
        requested_model: Some("hosted-v1".into()),
        answering_model: Some("hosted-v1".into()),
        checkpoint: None,
        identity_kind: IdentityKind::LocalDeclared,
        rendered_digest: format!("sha256:{}", "a".repeat(64)),
        wire_bytes: 100,
        usage: semaprax_harness::decision::Usage {
            input_tokens: Some(120),
            output_tokens: Some(5),
            basis,
        },
        billing,
    };
    let exact = router_charge(
        &priced,
        1,
        Some(&call(UsageBasis::ProviderReported, Billing::Api)),
    );
    assert_eq!((exact.micros, exact.basis), (130, "provider_reported"));
    let claimed = router_charge(&priced, 1, Some(&call(UsageBasis::Unknown, Billing::Local)));
    assert_eq!((claimed.micros, claimed.basis), (900, "reserved_uncertain"));
    assert_eq!(router_charge(&priced, 2, None).micros, 1800);
    assert_eq!(
        router_charge(&RouterPrice::NonBilled, 1, None).basis,
        "non_billed"
    );
    assert_eq!(router_charge(&priced, 0, None).micros, 0);
}

#[test]
fn a_qualifying_profile_activates_only_under_its_exact_key_and_rolls_back() {
    let dir = fixture_dir("hp-mr13").canonicalize().unwrap();
    let tasks = synthetic("development", |_| {});
    let (_, r) = go_run(&dir, &tasks, &[], false);
    let d = decision(&r, OOT, ExecutionDomain::Development);
    assert_eq!(d.status, "go", "{:?}", d.reasons);
    let key = d.key.clone().unwrap();
    let gate = d.gate.clone().unwrap();
    assert!(gate_attests_key(&gate, &key));
    let store = r.stores.get(&ExecutionDomain::Development);
    assert!(session_admits(store, &key));
    // Any change of model/checkpoint, renderer or candidate revision, or domain is another key.
    let mut other = key.clone();
    other.weights_digest = "oot-v1-ck2".into();
    assert!(!session_admits(store, &other));
    let renderer = EvidenceKey {
        distribution: "x".into(),
        ..key.clone()
    };
    assert!(!session_admits(store, &renderer));
    assert!(!r.stores.contains_key(&ExecutionDomain::Application));
    // Shadow strata never route and never enter the gate record.
    let rec = r.registry.get(&key).unwrap();
    assert_eq!(rec.eval_items.len(), 30);
    assert!(r
        .cells
        .iter()
        .filter(|c| c.arm == OOT && c.item.starts_with("m-"))
        .all(|c| c.shadow));
    assert!(!rec.eval_items.iter().any(|i| i.starts_with("m-")));
    // Reports keep option calibration apart from downstream success.
    let cal = &r.calibration[0];
    assert!(cal["raw_option_calibration"]["bins"].is_array());
    assert!(cal["downstream_success"]["bins"].is_array());
    assert_eq!(cal["threshold"], json!(0.8));
    assert!(cal["note"].as_str().unwrap().contains("not a probability"));
    // Drift: new sessions return to the previous qualified profile; a running
    // session's lock is an owned copy and does not change.
    let mut store = ProfileStore::default();
    let first = SessionLock {
        key_digest: key.digest(),
        record_digest: rec.digest(),
    };
    store.install(first.clone());
    store.install(SessionLock {
        key_digest: other.digest(),
        record_digest: "sha256:new".into(),
    });
    let running = store.lock_session().unwrap();
    let mut drift = DriftMonitor::new(1.0, 0.1, 5);
    for _ in 0..5 {
        drift.record(false);
    }
    assert!(drift.enforce(&mut store));
    assert_eq!(store.lock_session(), Some(first));
    assert_eq!(running.key_digest, other.digest());
    assert!(session_admits(Some(&store), &key) && !session_admits(Some(&store), &other));
}

#[test]
fn leakage_wrong_identity_and_forged_origin_prevent_qualification() {
    let dir = fixture_dir("hp-mr13").canonicalize().unwrap();
    let tasks = synthetic("development", |_| {});
    let reasons = |r: &MatrixRun| {
        decision(r, OOT, ExecutionDomain::Development)
            .reasons
            .join("; ")
    };
    let (_, r) = go_run(&dir, &tasks, &["e-03"], false);
    assert!(
        reasons(&r).contains("trained or calibrated on"),
        "{}",
        reasons(&r)
    );
    let dup = synthetic("development", |items| {
        items[5]["content_digest"] = items[40]["content_digest"].clone();
    });
    let (_, r) = go_run(&dir, &dup, &[], false);
    assert!(
        reasons(&r).contains("trained or calibrated on"),
        "content leakage: {}",
        reasons(&r)
    );
    let (_, r) = go_run(&dir, &tasks, &[], true);
    assert_eq!(
        decision(&r, OOT, ExecutionDomain::Development).status,
        "no-go"
    );
    assert!(
        reasons(&r).contains("forged fixture origin"),
        "{}",
        reasons(&r)
    );
    assert!(r.cells.iter().all(|c| c.origin != Origin::Real));
    // Wrong identity: the record is evaluated against a live key at another
    // renderer revision, and an outcome labelled by a router is not ground truth.
    let (_, r) = go_run(&dir, &tasks, &[], false);
    let d = decision(&r, OOT, ExecutionDomain::Development);
    let key = d.key.clone().unwrap();
    let rec = r.registry.get(&key).unwrap().clone();
    let spec = specs(30)
        .into_iter()
        .find(|x| x.domain == ExecutionDomain::Development)
        .unwrap();
    let mut ev = DomainEvidence {
        domain: ExecutionDomain::Development,
        record: rec,
        forged_origin: 0,
        unreconciled: 0,
        shadow_only: false,
    };
    assert!(evaluate_domain(&spec, &ev, &key).go);
    let base = EvidenceKey::live_versioned(
        &ProviderProfile {
            provider_id: key.provider_id.clone(),
            ..Default::default()
        },
        &key.catalog_digest,
        1,
    );
    let moved = base.bound(
        ExecutionDomain::Development,
        &tasks.candidate_revision,
        "semaprax.route-render.v3",
    );
    let no = evaluate_domain(&spec, &ev, &moved);
    assert!(no
        .reasons
        .iter()
        .any(|x| x.contains("differs from the live profile")));
    ev.record.outcomes[0].verified_by = "router:org.example/oot-router".into();
    assert!(evaluate_domain(&spec, &ev, &key)
        .reasons
        .iter()
        .any(|x| x.contains("independent development verifier")));
}

#[test]
fn development_and_application_evidence_never_cross_qualify() {
    let dir = fixture_dir("hp-mr13").canonicalize().unwrap();
    let dev = synthetic("development", |_| {});
    let (_, r) = go_run(&dir, &dev, &[], false);
    let key = decision(&r, OOT, ExecutionDomain::Development)
        .key
        .clone()
        .unwrap();
    let ev = DomainEvidence {
        domain: ExecutionDomain::Development,
        record: r.registry.get(&key).unwrap().clone(),
        forged_origin: 0,
        unreconciled: 0,
        shadow_only: false,
    };
    let app_spec = specs(30)
        .into_iter()
        .find(|x| x.domain == ExecutionDomain::Application)
        .unwrap();
    let app_key = EvidenceKey {
        distribution: key.distribution.clone(),
        ..key.clone()
    };
    let no = evaluate_domain(&app_spec, &ev, &app_key);
    assert!(!no.go);
    assert!(
        no.reasons[0].contains("never cross-qualify"),
        "{:?}",
        no.reasons
    );
    // The domain is bound into the key itself.
    let unbound = EvidenceKey::live_versioned(&ProviderProfile::default(), "c", 1);
    assert_ne!(
        unbound
            .bound(ExecutionDomain::Development, "r", "v")
            .digest(),
        unbound
            .bound(ExecutionDomain::Application, "r", "v")
            .digest()
    );
    // Application items with development verifiers do not qualify either.
    let app = synthetic("application", |items| {
        for i in items.iter_mut() {
            i["verifier"] = json!("dev-tests");
        }
    });
    let (_, r) = go_run(&dir, &app, &[], false);
    let d = decision(&r, OOT, ExecutionDomain::Application);
    assert_eq!(d.status, "no-go");
    assert!(d
        .reasons
        .iter()
        .any(|x| x.contains("independent application verifier")));
    assert!(r.stores.is_empty());
}

#[test]
fn weakened_gate_and_unbacked_real_runs_are_refused() {
    let dir = fixture_dir("hp-mr13").canonicalize().unwrap();
    let tasks = synthetic("development", |_| {});
    let reg = oot_registry(&dir, &[]);
    let mut fx = FixtureExecutor::from_json(
        &json!({"schema": "semaprax.harness-routing-fixture-outcomes.v1", "rows": []}),
    )
    .unwrap();
    let e = run(
        &reg,
        &tasks,
        &specs(10),
        &env(&dir),
        Lane {
            executor: &mut fx,
            invokers: BTreeMap::new(),
        },
    )
    .err()
    .unwrap();
    assert!(
        e.message.contains("weaker than the documented floor"),
        "{}",
        e.message
    );
    let b = bench_dir();
    let base = |extra: &[&str]| {
        let mut a = s(&["bench", "routing-matrix", "--registry"]);
        a.push(b.join("registry.json").display().to_string());
        a.push("--tasks".into());
        a.push(b.join("tasks.json").display().to_string());
        a.push("--gate-spec".into());
        a.push(b.join("gate-spec.json").display().to_string());
        a.push("--repo".into());
        a.push(repo_root().display().to_string());
        a.extend(s(extra));
        a
    };
    let out = dir.join("real");
    let o = cli_run(
        &base(&["--real", "--out", out.to_str().unwrap()]),
        &env(&dir),
    );
    assert_ne!(o.code, 0, "{}", o.stdout);
    assert!(
        o.stderr.contains("real executor unavailable") || o.stderr.contains("generation profiles"),
        "{}",
        o.stderr
    );
    assert!(!out.exists(), "a refused real run writes nothing");
    let o = cli_run(&base(&["--plan"]), &env(&dir));
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.starts_with("cost ceiling: "), "{}", o.stdout);
}

/// The recorded artifact is exactly what the fixture lane reproduces. Set
/// `SEMAPRAX_MR13_RECORD=1` to rewrite it after an intended change.
#[test]
fn recorded_routing_matrix_reproduces_and_is_an_honest_no_go() {
    let dir = fixture_dir("hp-mr13").canonicalize().unwrap();
    let b = bench_dir();
    let out = dir.join("out");
    let mut args = s(&["bench", "routing-matrix"]);
    for (k, f) in [
        ("--registry", "registry.json"),
        ("--tasks", "tasks.json"),
        ("--gate-spec", "gate-spec.json"),
        ("--fixture-outcomes", "fixture-outcomes.json"),
    ] {
        args.push(k.into());
        args.push(b.join(f).display().to_string());
    }
    args.extend(s(&[
        "--repo",
        repo_root().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--hardware",
        HARDWARE,
        "--pin",
        "worktree_base=db753cd01",
    ]));
    let o = cli_run(&args, &env(&dir));
    assert_eq!(o.code, 0, "{}", o.stderr);
    for f in ["run-manifest.json", "gate-decision.json"] {
        let got = std::fs::read_to_string(out.join(f)).unwrap();
        if std::env::var("SEMAPRAX_MR13_RECORD").as_deref() == Ok("1") {
            std::fs::write(b.join(f), &got).unwrap();
        }
        assert_eq!(
            got,
            std::fs::read_to_string(b.join(f)).unwrap_or_default(),
            "{f} drifted"
        );
    }
    let m = read(&out.join("run-manifest.json"));
    assert_ne!(m["decision"], "go");
    assert_eq!(m["mode"], "fixture");
    assert_eq!(m["billable_usage"]["real"]["executions"], 0);
    for d in ["development", "application"] {
        assert_eq!(m["active"][d]["mode"], "rules");
    }
    let cells = m["cells"].as_array().unwrap();
    assert!(cells.iter().all(|c| c["origin"] != "real"));
    for arm in m["arms"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["kind"] == "learned")
    {
        assert_eq!(arm["status"], "unavailable", "{arm}");
        assert!(cells
            .iter()
            .filter(|c| c["arm"] == arm["id"])
            .all(|c| c["origin"] == "unavailable"
                && c["completed"] == false
                && c["cost_micros"].is_null()));
    }
    let fx = load_fixture(&b.join("fixture-outcomes.json")).unwrap();
    assert_eq!(fx.class(), Origin::Fixture);
}
