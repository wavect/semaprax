//! HP-01 contract tests (fixture prefix `hp-hp01`).

use semaprax_harness::contract::mock_peer::{respond, MockScript};
use semaprax_harness::contract::*;
use semaprax_harness::json::{canonical, digest, parse_strict, JsonLimits};
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/contract")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn desc(name: &str) -> Descriptor {
    Descriptor::parse(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn code<T: std::fmt::Debug>(
    r: Result<T, semaprax_harness::diag::HarnessDiagnostic>,
) -> &'static str {
    r.unwrap_err().code
}

const D1: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";

fn project() -> ProjectBinding {
    ProjectBinding {
        id: "p1".into(),
        worktree: "w1".into(),
        revision: "r1".into(),
    }
}

fn request(kind: CapabilityKind, op: &str, payload: Value) -> RequestEnvelope {
    RequestEnvelope {
        invocation_id: "inv-000001".into(),
        project: project(),
        lock_digest: D1.into(),
        capability: CapabilityRef { kind, version: 1 },
        operation: op.into(),
        deadline_ms: 30_000,
        max_result_bytes: 65_536,
        remaining_calls: 8,
        lineage: vec!["inv-000000".into()],
        payload,
    }
}

fn context_result() -> Value {
    json!({"items": [{"path": "src/a.rs", "span": {"start_line": 1, "end_line": 4}, "digest": D1,
        "provenance": "structural", "language": "rust", "rank": 0.5}],
        "coverage": {"complete": true, "indexed_files": 3, "skipped": [], "exhaustive": true}})
}

/// One valid `(operation, direction, payload)` per slot shape of each kind.
fn samples(kind: CapabilityKind) -> Vec<(&'static str, Direction, Value)> {
    use Direction::*;
    match kind {
        CapabilityKind::ContextRepository => vec![
            ("orient", Request, json!({"max_items": 5})),
            ("search", Request, json!({"query": "needle"})),
            ("skeleton", Request, json!({"path": "src/a.rs"})),
            ("references", Request, json!({"symbol": "f"})),
            ("search", Result, context_result()),
        ],
        CapabilityKind::CommandView => vec![
            (
                "view",
                Request,
                json!({"form": "post-execution", "argv": ["ls"], "stdout": "a", "stderr": ""}),
            ),
            (
                "wrap",
                Request,
                json!({"form": "wrapper", "argv": ["ls", "-l"]}),
            ),
            (
                "view",
                Result,
                json!({"form": "post-execution", "view": {"text": "a", "lossless": true, "omissions": 0}}),
            ),
            (
                "wrap",
                Result,
                json!({"form": "wrapper", "plan": {"argv": ["ls", "-l"]}}),
            ),
            (
                "plan",
                Request,
                json!({"argv": ["git", "diff"], "cwd_rel": "."}),
            ),
            (
                "plan",
                Result,
                json!({"route": "post-execution", "operation": "view"}),
            ),
        ],
        CapabilityKind::DecisionEvaluate => vec![
            (
                "evaluate",
                Request,
                json!({"task": "model-route/v1", "features": {"size": 3}, "options": ["small", "large"]}),
            ),
            (
                "evaluate",
                Result,
                json!({"choice": "small", "scores": {"small": 0.9, "large": 0.1}, "abstain": false}),
            ),
        ],
        CapabilityKind::ModelGenerate => vec![
            (
                "generate",
                Request,
                json!({"model": "default-large", "input_base64": "aGk=", "max_output_bytes": 100}),
            ),
            (
                "generate",
                Result,
                json!({"model": "default-large", "output_base64": "aGk=", "usage": {"input_bytes": 2, "output_bytes": 2}}),
            ),
        ],
        CapabilityKind::SkillCatalog => vec![
            ("list", Request, json!({"limit": 10})),
            ("load", Request, json!({"digest": D1})),
            (
                "list",
                Result,
                json!({"skills": [{"id": "s1", "name": "S", "description": "d", "digest": D1, "bytes": 10}], "truncated": false}),
            ),
            (
                "load",
                Result,
                json!({"digest": D1, "text": "body", "artifact_refs": []}),
            ),
        ],
    }
}

fn result_frame(req: &RequestEnvelope, payload: &Value) -> Vec<u8> {
    ResultEnvelope::complete(req, payload.clone(), "org.example/p", "1.0.0")
        .to_json()
        .to_string()
        .into_bytes()
}

#[test]
fn hp_hp01_all_five_kinds_validate_through_envelopes() {
    for kind in CapabilityKind::ALL {
        let s = samples(kind);
        for (op, dir, payload) in s.iter().filter(|(_, d, _)| *d == Direction::Request) {
            let req = request(kind, op, payload.clone());
            req.validate()
                .unwrap_or_else(|e| panic!("{} {op}: {e}", kind.as_str()));
            assert_eq!(RequestEnvelope::from_json(&req.to_json()).unwrap(), req);
            let _ = dir;
            // A result for this request, using the kind's result sample for the operation.
            let res = s
                .iter()
                .find(|(o, d, _)| *d == Direction::Result && o == op);
            let res = res
                .or_else(|| s.iter().find(|(_, d, _)| *d == Direction::Result))
                .unwrap();
            // Only run the envelope round trip when the result sample matches the operation.
            if res.0 == *op {
                let parsed = ResultEnvelope::parse_for(&req, &result_frame(&req, &res.2))
                    .unwrap_or_else(|e| panic!("{} {op}: {e}", kind.as_str()));
                assert_eq!(parsed.status, ResultStatus::Complete);
            }
        }
        for (op, _, payload) in s.iter().filter(|(_, d, _)| *d == Direction::Result) {
            // Result sample for `op` pairs with a request of the same kind/op.
            let req_payload = s
                .iter()
                .find(|(o, d, _)| *d == Direction::Request && o == op)
                .map(|x| x.2.clone());
            let req = request(kind, op, req_payload.unwrap_or_else(|| s[0].2.clone()));
            if req.validate().is_ok() {
                ResultEnvelope::parse_for(&req, &result_frame(&req, payload)).unwrap();
            }
        }
    }
}

#[test]
fn hp_hp01_cross_slot_payloads_are_rejected() {
    for a in CapabilityKind::ALL {
        for (_, dir, payload) in samples(a) {
            for b in CapabilityKind::ALL.into_iter().filter(|b| *b != a) {
                for op in b.operations() {
                    let r = validate_payload(b, op, dir, &payload);
                    assert!(
                        r.is_err(),
                        "{} payload accepted as {} {op} ({dir:?})",
                        a.as_str(),
                        b.as_str()
                    );
                }
            }
        }
    }
}

#[test]
fn hp_hp01_payload_specific_refusals() {
    use CapabilityKind::*;
    let v = |k, op, d, p: Value| validate_payload(k, op, d, &p);
    for bad in ["/etc/passwd", "../x", "a/../b", "C:/x", "a\\b", ""] {
        assert_eq!(
            code(v(
                ContextRepository,
                "skeleton",
                Direction::Request,
                json!({"path": bad})
            )),
            "SPX-HPA041",
            "{bad}"
        );
    }
    let mut r = context_result();
    r["items"][0]["path"] = json!("/abs/path.rs");
    assert_eq!(
        code(v(ContextRepository, "search", Direction::Result, r)),
        "SPX-HPA041"
    );
    let mut r = context_result();
    r["coverage"]["exhaustive"] = json!(false);
    r["no_references"] = json!(true);
    assert_eq!(
        code(v(ContextRepository, "references", Direction::Result, r)),
        "SPX-HPA040"
    );
    assert_eq!(
        code(v(
            CommandView,
            "view",
            Direction::Result,
            json!({"form": "post-execution", "view": {"text": "", "lossless": true, "omissions": 0}, "exit_code": 0})
        )),
        "SPX-HPA042"
    );
    assert_eq!(
        code(v(
            CommandView,
            "view",
            Direction::Result,
            json!({"form": "post-execution", "view": {"text": "", "lossless": true, "omissions": 0, "status": 1}})
        )),
        "SPX-HPA042"
    );
    assert_eq!(
        code(v(
            DecisionEvaluate,
            "evaluate",
            Direction::Result,
            json!({"choice": "a", "scores": {"a": 1.5}, "abstain": false})
        )),
        "SPX-HPA044"
    );
    assert_eq!(
        code(v(
            ModelGenerate,
            "generate",
            Direction::Request,
            json!({"model": "Bad Id!", "input_base64": "aGk=", "max_output_bytes": 1})
        )),
        "SPX-HPA045"
    );
    assert_eq!(
        code(v(ModelGenerate, "nope", Direction::Request, json!({}))),
        "SPX-HPA046"
    );
    // Decision choice must be a request option (checked against the request).
    let req = request(
        DecisionEvaluate,
        "evaluate",
        samples(DecisionEvaluate)[0].2.clone(),
    );
    let out = json!({"choice": "huge", "scores": {"small": 0.5}, "abstain": false});
    assert_eq!(
        code(ResultEnvelope::parse_for(&req, &result_frame(&req, &out))),
        "SPX-HPA043"
    );
    // Partial/complete need payload; other statuses must not carry one.
    let mut env = ResultEnvelope::complete(&req, Value::Null, "o/p", "1.0.0");
    env.payload = None;
    assert_eq!(
        code(ResultEnvelope::parse_for(
            &req,
            env.to_json().to_string().as_bytes()
        )),
        "SPX-HPA035"
    );
    env.status = ResultStatus::Refused;
    env.diagnostics = vec![("X".into(), "no".into())];
    assert!(ResultEnvelope::parse_for(&req, env.to_json().to_string().as_bytes()).is_ok());
}

const PLUGIN_MANIFEST_SHA: &str =
    "e3142835b0e006b823fadf1a444e2893fad8e5b80c54fcd7a693169fd5e20274";

#[test]
fn hp_hp01_plugin_manifest_golden_unchanged_and_rejected() {
    let bytes = fixture("plugin-manifest-meaning.v1.json");
    let sum = semaprax_harness::json::sha256_plain(&bytes);
    assert_eq!(sum, format!("sha256:{PLUGIN_MANIFEST_SHA}"));
    assert_eq!(code(Descriptor::parse(&bytes)), "SPX-HPA011");
}

#[test]
fn hp_hp01_negotiation_before_launch() {
    let host = HostSupport::first_wave();
    assert_eq!(
        code(negotiate(&desc("unknown-required.json"), &host)),
        "SPX-HPA019"
    );
    // Required kind at an unsupported version.
    let mut d = desc("context-graft.json");
    d.capabilities[0].version = 2;
    assert_eq!(code(negotiate(&d, &host)), "SPX-HPA023");
    let n = negotiate(&desc("unknown-optional.json"), &host).unwrap();
    assert_eq!(n.active.len(), 1);
    assert_eq!(n.active[0].kind, CapabilityKind::ContextRepository);
    let names: Vec<_> = n.inactive.iter().map(|i| i.kind_name.as_str()).collect();
    assert_eq!(
        names,
        ["telemetry.export", "skill.catalog", "x.example/notes"]
    );
    let n = negotiate(&desc("context-graft.json"), &host).unwrap();
    assert_eq!(n.active.len(), 1);
    assert_eq!(n.inactive[0].kind_name, "x.example/notes");
    let n = negotiate(&desc("builtin-all.json"), &host).unwrap();
    assert_eq!(n.active.len(), 5);
    // Host that lacks a kind: the required declaration fails.
    let none = host.clone().with(CapabilityKind::ContextRepository, &[]);
    assert_eq!(
        code(negotiate(&desc("context-graft.json"), &none)),
        "SPX-HPA023"
    );
}

#[test]
fn hp_hp01_stable_transport_diagnostics() {
    let req = request(CapabilityKind::SkillCatalog, "list", json!({"limit": 3}));
    let ok = result_frame(&req, &json!({"skills": [], "truncated": false}));
    assert!(ResultEnvelope::parse_for(&req, &ok).is_ok());
    let text = String::from_utf8(ok.clone()).unwrap();
    // Oversize.
    let mut small = req.clone();
    small.max_result_bytes = 100;
    assert_eq!(code(ResultEnvelope::parse_for(&small, &ok)), "SPX-HPA002");
    // Duplicate keys.
    let dup = text.replacen(
        "\"status\":\"complete\"",
        "\"status\":\"complete\",\"status\":\"failed\"",
        1,
    );
    assert_eq!(
        code(ResultEnvelope::parse_for(&req, dup.as_bytes())),
        "SPX-HPA005"
    );
    // Invalid numbers.
    let num = text.replacen("\"truncated\":false", "\"truncated\":false,\"n\":1e999", 1);
    assert_eq!(
        code(ResultEnvelope::parse_for(&req, num.as_bytes())),
        "SPX-HPA006"
    );
    let big = text.replacen(
        "\"truncated\":false",
        "\"truncated\":false,\"n\":99999999999999999999999",
        1,
    );
    assert_eq!(
        code(ResultEnvelope::parse_for(&req, big.as_bytes())),
        "SPX-HPA006"
    );
    // Invalid UTF-8.
    let mut bad = ok.clone();
    bad.insert(10, 0xff);
    assert_eq!(code(ResultEnvelope::parse_for(&req, &bad)), "SPX-HPA001");
    // Embedded line break, trailing data.
    assert_eq!(
        code(ResultEnvelope::parse_for(
            &req,
            text.replacen('{', "{\n", 1).as_bytes()
        )),
        "SPX-HPA008"
    );
    assert_eq!(
        code(ResultEnvelope::parse_for(
            &req,
            format!("{text} {{}}").as_bytes()
        )),
        "SPX-HPA007"
    );
    // Spoofed identities.
    let spoof = |f: &dyn Fn(&mut Value)| {
        let mut v: Value = serde_json::from_slice(&ok).unwrap();
        f(&mut v);
        code(ResultEnvelope::parse_for(&req, v.to_string().as_bytes()))
    };
    assert_eq!(
        spoof(&|v| v["invocation_id"] = json!("inv-999999")),
        "SPX-HPA031"
    );
    assert_eq!(
        spoof(&|v| v["project"]["id"] = json!("other")),
        "SPX-HPA032"
    );
    assert_eq!(
        spoof(&|v| v["project"]["revision"] = json!("r2")),
        "SPX-HPA032"
    );
    assert_eq!(
        spoof(&|v| v["capability"]["kind"] = json!("command.view")),
        "SPX-HPA033"
    );
    assert_eq!(
        spoof(&|v| v["payload"]["grants"] = json!({"fs": "write"})),
        "SPX-HPA036"
    );
    assert_eq!(spoof(&|v| v["extra"] = json!(1)), "SPX-HPA037");
    // Depth.
    let deep = format!("{}1{}", "[".repeat(100), "]".repeat(100));
    assert_eq!(
        code(parse_strict(deep.as_bytes(), &JsonLimits::frame(4096))),
        "SPX-HPA003"
    );
}

#[test]
fn hp_hp01_descriptor_permissions_are_requests_only() {
    let d = desc("requests-authority.json");
    assert_eq!(d.permissions.network, ["*"]);
    assert_eq!(d.permissions.write, ["/"]);
    let n = negotiate(&d, &HostSupport::first_wave()).unwrap();
    // Exhaustive destructuring: negotiation output has no grant-bearing field.
    let Negotiation { active, inactive } = n;
    assert_eq!(active.len(), 1);
    assert!(inactive.is_empty());
    // A result cannot smuggle a grant either.
    let req = request(CapabilityKind::SkillCatalog, "list", json!({"limit": 1}));
    let mut env = ResultEnvelope::complete(
        &req,
        json!({"skills": [], "truncated": false}),
        "o/p",
        "1.0.0",
    )
    .to_json();
    env["permissions"] = json!({"network": ["*"]});
    assert_eq!(
        code(ResultEnvelope::parse_for(&req, env.to_string().as_bytes())),
        "SPX-HPA036"
    );
}

#[test]
fn hp_hp01_new_provider_for_existing_kind_is_data_only() {
    let host = HostSupport::first_wave();
    for f in ["context-graft.json", "context-second.json"] {
        let d = desc(f);
        let n = negotiate(&d, &host).unwrap();
        assert_eq!(n.active[0].kind, CapabilityKind::ContextRepository);
    }
    let (a, b) = (desc("context-graft.json"), desc("context-second.json"));
    assert_ne!(a.digest(), b.digest());
    check_duplicate_identities(&[a.clone(), b]).unwrap();
    assert_eq!(
        code(check_duplicate_identities(&[a.clone(), a])),
        "SPX-HPA020"
    );
}

#[test]
fn hp_hp01_descriptor_rejections() {
    let base: Value = serde_json::from_slice(&fixture("context-graft.json")).unwrap();
    let with = |f: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        f(&mut v);
        code(Descriptor::parse(v.to_string().as_bytes()))
    };
    assert_eq!(with(&|v| v["schema"] = json!("other.v1")), "SPX-HPA010");
    assert_eq!(with(&|v| v["surprise"] = json!(1)), "SPX-HPA012");
    assert_eq!(
        with(&|v| v["provider"]["id"] = json!("NoSlash")),
        "SPX-HPA014"
    );
    assert_eq!(
        with(&|v| v["provider"]["id"] = json!("a/b/c")),
        "SPX-HPA014"
    );
    assert_eq!(
        with(&|v| {
            v["adapter"].as_object_mut().unwrap().remove("entry");
        }),
        "SPX-HPA015"
    );
    assert_eq!(
        with(&|v| v["adapter"]["runtime"] = json!("builtin")),
        "SPX-HPA015"
    );
    assert_eq!(
        with(&|v| {
            v["adapter"]["runtime"] = json!("native");
            v["adapter"]["entry"] = json!(["/abs/bin"]);
        }),
        "SPX-HPA015"
    );
    assert_eq!(with(&|v| v["protocol"]["min"] = json!(2)), "SPX-HPA016");
    assert_eq!(with(&|v| v["protocol"]["max"] = json!(0)), "SPX-HPA013");
    assert_eq!(
        with(&|v| {
            let c = v["capabilities"][0].clone();
            v["capabilities"].as_array_mut().unwrap().push(c);
        }),
        "SPX-HPA017"
    );
    assert_eq!(
        with(&|v| v["resources"]["max_frame_bytes"] = json!(8 * 1024 * 1024)),
        "SPX-HPA018"
    );
    assert_eq!(
        with(&|v| v["capabilities"][0]["operations"] = json!(["bogus"])),
        "SPX-HPA022"
    );
    assert_eq!(
        with(&|v| v["resources"]["max_concurrency"] = json!(1.5)),
        "SPX-HPA013"
    );
    // Builtin descriptors carry no entry.
    assert!(desc("builtin-all.json").entry.is_empty());
    assert_eq!(code(Descriptor::parse(b"{\"a\":1,\"a\":2}")), "SPX-HPA005");
}

#[test]
fn hp_hp01_descriptor_digest_and_roundtrip() {
    let d = desc("context-graft.json");
    let again = Descriptor::parse(canonical(&d.to_json()).as_bytes()).unwrap();
    assert_eq!(again, d);
    assert_eq!(again.digest(), d.digest());
    assert_eq!(
        d.digest(),
        digest("semaprax.harness-provider.v1", &d.to_json())
    );
    assert!(d.digest().starts_with("sha256:") && d.digest().len() == 71);
}

#[test]
fn hp_hp01_downgrade_refusal() {
    let d = desc("context-second.json"); // provider 2.0.0 adapter 2.0.1
    let locked = |p: &str, a: &str| LockedIdentity {
        provider_id: d.provider_id.clone(),
        provider_version: p.into(),
        adapter_version: a.into(),
    };
    refuse_downgrade(&locked("2.0.0", "2.0.1"), &d).unwrap();
    refuse_downgrade(&locked("1.9.9", "2.0.0"), &d).unwrap();
    assert_eq!(
        code(refuse_downgrade(&locked("2.1.0", "2.0.1"), &d)),
        "SPX-HPA021"
    );
    assert_eq!(
        code(refuse_downgrade(&locked("2.0.0", "2.0.2"), &d)),
        "SPX-HPA021"
    );
    let other = LockedIdentity {
        provider_id: "x/y".into(),
        provider_version: "1".into(),
        adapter_version: "1".into(),
    };
    assert_eq!(code(refuse_downgrade(&other, &d)), "SPX-HPA021");
}

#[test]
fn hp_hp01_mock_peer_round_trip() {
    let script = MockScript {
        accepted: vec![(CapabilityKind::SkillCatalog, 1, vec!["list".into()])],
        invoke: Box::new(|req| {
            ResultEnvelope::complete(
                req,
                json!({"skills": [], "truncated": false}),
                "o/p",
                "1.0.0",
            )
            .to_json()
        }),
    };
    let init = respond(
        br#"{"jsonrpc":"2.0","id":1,"method":"harness/initialize","params":{}}"#,
        &script,
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&init).unwrap();
    assert_eq!(v["result"]["accepted"][0]["kind"], "skill.catalog");
    let req = request(CapabilityKind::SkillCatalog, "list", json!({"limit": 2}));
    let frame =
        json!({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": req.to_json()})
            .to_string();
    let out = respond(frame.as_bytes(), &script).unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    let result = ResultEnvelope::parse_for(&req, v["result"].to_string().as_bytes()).unwrap();
    assert_eq!(result.status, ResultStatus::Complete);
    let down = respond(
        br#"{"jsonrpc":"2.0","id":3,"method":"harness/shutdown"}"#,
        &script,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&down).unwrap()["result"],
        json!({})
    );
    assert!(respond(
        br#"{"jsonrpc":"2.0","method":"harness/cancel","params":{}}"#,
        &script
    )
    .is_none());
}

#[test]
fn hp_hp09b_command_view_streams_plan_and_bounds() {
    use CapabilityKind::CommandView;
    let v = |op, d, p: Value| validate_payload(CommandView, op, d, &p);
    let view = |extra: Value| {
        let mut m = json!({"form": "post-execution", "argv": ["git", "diff"]});
        for (k, x) in extra.as_object().unwrap() {
            m[k] = x.clone();
        }
        m
    };
    // Each stream by exactly one of text, base64, or a retention-relative path.
    let ok = view(
        json!({"stdout_path": "views/1.out", "stderr_b64": "aGk=", "max_bytes": 10,
        "min_bytes": 0, "recovery_handle": "h", "config": {"allow_wrapper": false, "n": 3, "s": "x"}}),
    );
    v("view", Direction::Request, ok).unwrap();
    for bad in [
        json!({"stdout": "a"}),
        json!({"stdout": "a", "stdout_b64": "", "stderr": ""}),
        json!({"stdout": "a", "stdout_path": "x", "stderr": ""}),
        json!({"stdout_b64": "a b", "stderr": ""}),
        json!({"stdout": "a", "stderr": "", "config": {"x": {"nested": 1}}}),
        json!({"stdout": "a", "stderr": "", "config": {"x": [1]}}),
        json!({"stdout": "a", "stderr": "", "max_bytes": -1}),
        json!({"stdout": "a", "stderr": "", "extra": 1}),
    ] {
        assert_eq!(
            code(v("view", Direction::Request, view(bad.clone()))),
            "SPX-HPA040",
            "{bad}"
        );
    }
    for bad in ["/etc/x", "../x", "a/../b", "a\\b", ""] {
        assert_eq!(
            code(v(
                "view",
                Direction::Request,
                view(json!({"stdout_path": bad, "stderr": ""}))
            )),
            "SPX-HPA041",
            "{bad}"
        );
    }
    // plan request
    let plan = json!({"argv": ["cargo", "test"], "cwd_rel": "crates/a", "estimated_output_bytes": 5,
        "external_hooks": ["rtk"], "lineage": ["exec:1"], "form": "wrapper", "config": {"min_bytes": 1}});
    v("plan", Direction::Request, plan.clone()).unwrap();
    for (k, x, c) in [
        ("cwd_rel", json!("../x"), "SPX-HPA041"),
        ("cwd_rel", json!("/x"), "SPX-HPA041"),
        ("argv", json!([]), "SPX-HPA040"),
        ("argv", json!([1]), "SPX-HPA040"),
        ("form", json!("shell"), "SPX-HPA040"),
        ("lineage", json!([1]), "SPX-HPA040"),
        ("external_hooks", json!("rtk"), "SPX-HPA040"),
        ("estimated_output_bytes", json!("x"), "SPX-HPA040"),
        ("unknown", json!(1), "SPX-HPA040"),
    ] {
        let mut p = plan.clone();
        p[k] = x;
        assert_eq!(code(v("plan", Direction::Request, p)), c, "{k}");
    }
    // plan result: the three routes
    v(
        "plan",
        Direction::Result,
        json!({"route": "bypass", "reason": "small-output"}),
    )
    .unwrap();
    v(
        "plan",
        Direction::Result,
        json!({"route": "post-execution", "family": "git-diff",
        "filter": "git-diff", "operation": "view", "raw_recovery": "host-retained-streams"}),
    )
    .unwrap();
    let wrapped = json!({"route": "wrapped", "argv": ["/opt/rtk", "git", "diff"], "env": {"RTK_X": "1"},
        "resolves_via": "PATH", "recovery": {"kind": "recall-db", "coverage": "complete",
        "retrieve_argv": ["/opt/rtk", "recall", "<handle>"]}});
    v("plan", Direction::Result, wrapped.clone()).unwrap();
    for (k, x) in [
        ("route", json!("teleport")),
        ("argv", json!(["ok", 1])),
        ("env", json!({"A": 1})),
        ("recovery", json!({"a": {"nested": 1}})),
        ("recovery", json!({"a": ["x", 1]})),
    ] {
        let mut p = wrapped.clone();
        p[k] = x;
        assert_eq!(code(v("plan", Direction::Result, p)), "SPX-HPA040", "{k}");
    }
    let mut w = wrapped.clone();
    w.as_object_mut().unwrap().remove("argv");
    assert_eq!(code(v("plan", Direction::Result, w)), "SPX-HPA040");
    assert_eq!(
        code(v("plan", Direction::Result, json!({"route": "bypass"}))),
        "SPX-HPA040"
    );
    // A plan result still can never carry an exit status.
    let mut p = json!({"route": "bypass", "reason": "x"});
    p["exit_code"] = json!(0);
    assert_eq!(code(v("plan", Direction::Result, p)), "SPX-HPA042");
    let mut p = wrapped;
    p["recovery"]["status"] = json!("ok");
    assert_eq!(code(v("plan", Direction::Result, p)), "SPX-HPA042");
}
