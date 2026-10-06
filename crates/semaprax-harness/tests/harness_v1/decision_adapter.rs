//! MR-15: an out-of-tree, non-SystemOne decision adapter (a keyword router
//! written to a temporary directory by this test) adopted through the normal
//! descriptor → adopt → trust → resolve → negotiate path, with no bundled-asset
//! registration and no vendor switch. Two model profiles of the same adapter
//! select different models; a scoreless profile never gets scores. Fixture
//! prefix `hp-mr15`.

use crate::decision_v2::{catalog, ctx};
use crate::support::{fixture_dir, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::decision::*;
use semaprax_harness::profile::resolve::current_platform;
use semaprax_harness::workflow::decision_open::open_decision;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ID: &str = "org.example/keyword-router";

/// The whole adapter: picks the candidate whose label holds the profile's
/// keyword. No SystemOne codec, no network, standard library only.
const ADAPTER: &str = r#"#!/usr/bin/env python3
import json, os, sys
PROTOCOL = "semaprax.harness-rpc.v1"
PROFILE = json.loads(os.environ.get("SEMAPRAX_HARNESS_CFG_MODEL_PROFILE") or os.environ.get("SEMAPRAX_HARNESS_MODEL_PROFILE") or "{}")
KEYWORD = {"kw-small": "economy", "kw-large": "frontier"}.get(PROFILE.get("model"), "standard")
SCORELESS = PROFILE.get("score_kind") == "none"

def send(o):
    sys.stdout.write(json.dumps(o, separators=(",", ":"), sort_keys=True) + "\n")
    sys.stdout.flush()

def decide(p):
    if p.get("task") != "model-route/v2":
        return {"choice": p["options"][0], "scores": {o: (1.0 if i == 0 else 0.0) for i, o in enumerate(p["options"])}, "abstain": False}
    labels = p["rendered"]["option_labels"]
    pick = next((o for o in p["options"] if KEYWORD in labels[o]), p["options"][0])
    body = json.dumps({"prompt": [p["rendered"]["instructions"], p["rendered"]["state"], labels]}, separators=(",", ":"), sort_keys=True)
    scores = None if SCORELESS else {o: (0.8 if o == pick else 0.2 / (len(p["options"]) - 1)) for o in p["options"]}
    return {"choice": pick, "abstain": False, "abstention_reason": "none", "scores": scores,
            "score_kind": "none" if SCORELESS else "candidate_relative",
            "native_confidence": None, "native_confidence_kind": None, "calibration_id": None,
            "call": {"adapter": "org.example/keyword-router@0.1.0", "requested_model": PROFILE.get("model"),
                     "answering_model": PROFILE.get("model"), "checkpoint": None, "identity_kind": "local_declared",
                     "rendered_digest": p["rendered"]["digest"], "wire_bytes": len(body),
                     "usage": {"input_tokens": None, "output_tokens": None, "basis": "unknown"}, "billing": "local"}}

for raw in sys.stdin:
    if not raw.strip():
        continue
    msg = json.loads(raw)
    m, mid = msg.get("method"), msg.get("id")
    if m == "harness/initialize":
        acc = [{"kind": c["kind"], "version": c["version"], "operations": ["evaluate"]} for c in msg["params"]["offered"]]
        send({"jsonrpc": "2.0", "id": mid, "result": {"protocol": PROTOCOL, "accepted": acc}})
    elif m == "harness/invoke":
        req = msg["params"]
        send({"jsonrpc": "2.0", "id": mid, "result": {
            "schema": "semaprax.harness-result.v1", "invocation_id": req["invocation_id"], "project": req["project"],
            "capability": req["capability"], "status": "complete", "payload": decide(req["payload"]), "diagnostics": [],
            "provenance": {"provider_id": "org.example/keyword-router", "adapter_version": "0.1.0", "upstream_version": "none"}}})
    elif m == "harness/shutdown":
        send({"jsonrpc": "2.0", "id": mid, "result": {}})
        break
"#;

fn python() -> PathBuf {
    std::env::var_os("HARNESS_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|d| d.join("python3"))
                .find(|p| p.is_file())
        })
        .expect("python3 not found: the out-of-tree adapter cell cannot run here")
}

fn descriptor() -> serde_json::Value {
    let cap = |v: u32, required: bool| json!({"kind": "decision.evaluate", "version": v, "required": required, "operations": ["evaluate"]});
    json!({
        "schema": "semaprax.harness-provider.v1",
        "provider": {"id": ID, "version": "0.1.0"},
        "adapter": {"runtime": "python", "entry": ["adapter.py"], "version": "0.1.0"},
        "upstream": {"name": "keyword-router", "package": "local:adapter.py",
                     "repository": "https://example.invalid/keyword-router", "versions": ["builtin-0.1.0"],
                     "identity_probe": []},
        "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
        "capabilities": [cap(1, true), cap(2, false)],
        "extensions": [],
        "platforms": [current_platform()],
        "config": {"fields": {"model_profile": {"type": "string"}, "instance_id": {"type": "string"}}},
        "permissions": {"read": [], "write": [], "network": [], "process": [], "secrets": []},
        "resources": {"handshake_timeout_ms": 10_000, "invoke_timeout_ms": 30_000,
                      "max_frame_bytes": 1_048_576, "max_concurrency": 1, "idle_shutdown_ms": 60_000},
        "cancellation": "cooperative",
        "support": {"license": "Apache-2.0", "isolation": "subprocess", "tested": []}
    })
}

struct World {
    root: PathBuf,
    env: Environment,
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

/// Adopt and trust the adapter from a temporary out-of-tree directory.
fn world() -> World {
    let root = fixture_dir("hp-mr15").canonicalize().unwrap();
    write(&root, "vendor/keyword-router/adapter.py", ADAPTER);
    write(
        &root,
        "vendor/keyword-router/harness-provider.json",
        &descriptor().to_string(),
    );
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = Environment {
        harness_home: Some(home),
        compiler: None,
        cwd: root.clone(),
        vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    let desc = root.join("vendor/keyword-router/harness-provider.json");
    let py = python();
    let o = run(
        &s(&[
            "adopt",
            desc.to_str().unwrap(),
            "--runtime",
            py.to_str().unwrap(),
        ]),
        &env,
    );
    assert_eq!(o.code, 0, "adopt: {}", o.stderr);
    let o = run(&s(&["trust", ID]), &env);
    assert_eq!(o.code, 0, "trust: {}", o.stderr);
    World { root, env }
}

/// A project selecting the adapter with one configured model profile.
fn project(w: &World, name: &str, profile: serde_json::Value) -> PathBuf {
    let p = w.root.join(name);
    write(&p, "semaprax.toml", "schema = \"semaprax.manifest.v1\"\n");
    write(&p, "src/lib.spx", "module app\n");
    write(
        &p,
        "semaprax.harness.toml",
        &format!(
            "schema = \"semaprax.harness-config.v1\"\n[capability.\"decision.evaluate\"]\nmode = \"auto\"\nprovider = \"{ID}\"\n[capability.\"decision.evaluate\".config]\nmodel_profile = '{profile}'\ninstance_id = \"{name}\"\n"
        ),
    );
    p
}

fn route(w: &World, project: &Path) -> (RouteDecision, ProviderProfile) {
    let mut o = open_decision(&w.env, project, Some(&python()), &w.root.join("cache")).unwrap();
    let inputs = crate::decision_v2::inputs_with(
        catalog(["alpha", "beta", "gamma"]),
        RouteSignals::default(),
    );
    let mut rctx = ctx();
    rctx.project = o.binding.clone();
    rctx.lock_digest = o.lock_digest.clone();
    let profile = o.profile.clone();
    let mut p = ConfiguredProvider {
        gate: EnablementGate::not_evaluated("model-route/v2", &profile.provider_id),
        profile: profile.clone(),
        invoker: &mut o.invoker,
        mode: ProviderMode::Explicit,
    };
    let d = decide(&inputs, &rctx, Some(&mut p), &|| inputs.clone(), None).unwrap();
    (d, profile)
}

#[test]
fn hp_mr15_out_of_tree_adapter_routes_with_two_profiles_and_a_scoreless_one() {
    let w = world();
    let small = project(
        &w,
        "proj-small",
        json!({"profile_id": "small", "model": "kw-small", "identity_kind": "local_declared", "score_kind": "candidate_relative"}),
    );
    let large = project(
        &w,
        "proj-large",
        json!({"profile_id": "large", "model": "kw-large", "identity_kind": "local_declared", "score_kind": "candidate_relative"}),
    );
    let (ds, ps) = route(&w, &small);
    let (dl, pl) = route(&w, &large);
    for d in [&ds, &dl] {
        assert_eq!(d.source, DecisionSource::Provider, "{:?}", d.wire.note);
        assert_eq!(d.wire.version, 2, "negotiated decision.evaluate v2");
        assert_eq!(d.provider_id, ID);
    }
    // Same adapter code, different configured models, different routes.
    assert_eq!(ds.choice, "beta", "kw-small picks the economy candidate");
    assert_eq!(dl.choice, "alpha", "kw-large picks the frontier candidate");
    assert_eq!(ps.provider_id, pl.provider_id);
    assert_eq!(ps.adapter_version.as_deref(), Some("0.1.0"));
    assert_ne!(ps.scope_digest(), pl.scope_digest());
    assert_eq!(ps.instance.as_ref().unwrap().instance_id, "proj-small");
    let cat = ds.digests.catalog.clone();
    assert_ne!(
        EvidenceKey::live_versioned(&ps, &cat, 2),
        EvidenceKey::live_versioned(&pl, &cat, 2)
    );
    let call = ds.wire.call.as_ref().unwrap();
    assert_eq!(call.answering_model.as_deref(), Some("kw-small"));
    assert_eq!(call.identity_kind, IdentityKind::LocalDeclared);
    assert_eq!(
        Some(call.rendered_digest.as_str()),
        ds.wire.rendered_digest.as_deref()
    );
    assert!(call.wire_bytes <= ds.wire.max_wire_bytes.unwrap());
    let rec = DecisionRecord::from_decision(&ds).to_json();
    assert_eq!(
        rec["identity"]["adapter"],
        "org.example/keyword-router@0.1.0"
    );

    // A scoreless profile: a choice without a fabricated distribution.
    let none = project(
        &w,
        "proj-none",
        json!({"profile_id": "plain", "model": "kw-plain", "identity_kind": "local_declared", "score_kind": "none"}),
    );
    let (dn, pn) = route(&w, &none);
    assert!(pn.scoreless());
    assert_eq!(dn.source, DecisionSource::Provider, "{:?}", dn.wire.note);
    assert_eq!(dn.choice, "gamma", "kw-plain picks the standard candidate");
    assert_eq!(dn.wire.score_kind, Some(ScoreKind::None));

    // A malformed profile is refused before the adapter is started.
    let bad = project(
        &w,
        "proj-bad",
        json!({"profile_id": "x", "model": "m", "renderer": "other.v1"}),
    );
    let e = open_decision(&w.env, &bad, Some(&python()), &w.root.join("cache"))
        .err()
        .expect("malformed profile refused");
    assert_eq!(e.code, "SPX-HPJ020");
}
