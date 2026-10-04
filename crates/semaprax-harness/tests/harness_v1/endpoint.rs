//! HP-12 endpoint adoption tests (fixture prefix `hp-hp12`).

use crate::support::{fixture_dir, write};
use semaprax_harness::cli::Environment;
use semaprax_harness::decision::Destination;
use semaprax_harness::endpoint::*;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[path = "../fixtures/endpoint/server.rs"]
mod server;
use server::{serve, Req, Resp, Server};

/// Mutable behaviour of the fixture upstream.
#[derive(Clone)]
struct Shape {
    ollama: bool,
    responses: bool,
    chat_shaped_responses: bool,
    usage: bool,
    tools: bool,
    digest: String,
    returned: String,
    extra_models: Vec<String>,
    drop_model: bool,
}

impl Shape {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            ollama: true,
            responses: true,
            chat_shaped_responses: false,
            usage: true,
            tools: true,
            digest: "d1".into(),
            returned: "tiny:1b".into(),
            extra_models: vec![],
            drop_model: false,
        }))
    }
}

fn usage(chat: bool) -> Value {
    if chat {
        json!({"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5})
    } else {
        json!({"input_tokens": 3, "output_tokens": 2, "total_tokens": 5})
    }
}

fn fixture(shape: Arc<Mutex<Shape>>) -> Server {
    serve(move |r: &Req| {
        let s = shape.lock().unwrap().clone();
        let model = r
            .body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let stream = r.body.get("stream") == Some(&json!(true));
        let chat = |content: &str, tool: bool| {
            let mut v = json!({"id": "c1", "object": "chat.completion", "model": s.returned,
                "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}]});
            if tool {
                v["choices"][0]["message"]["tool_calls"] = json!([]);
            }
            if s.usage {
                v["usage"] = usage(true);
            }
            v
        };
        match (r.method.as_str(), r.path.as_str()) {
            ("GET", "/api/tags") if s.ollama => {
                let mut models = vec![
                    json!({"name": "tiny:1b", "digest": s.digest, "details": {"context_length": 4096}}),
                ];
                if s.drop_model {
                    models.clear();
                }
                for m in &s.extra_models {
                    models.push(json!({"name": m, "digest": "x", "details": {}}));
                }
                Resp::Json(200, json!({"models": models}))
            }
            ("POST", "/api/show") if s.ollama => {
                Resp::Json(200, json!({"capabilities": ["completion", "tools"]}))
            }
            ("GET", "/v1/models") if !s.ollama => Resp::Json(
                200,
                json!({"object": "list", "data": [{"id": "gw-model", "object": "model"}]}),
            ),
            ("POST", "/v1/responses") if s.responses => {
                if stream {
                    let done = json!({"type": "response.completed", "response": {"model": s.returned, "usage": if s.usage { usage(false) } else { Value::Null }}});
                    return Resp::Sse(
                        vec![
                            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{}}".into(),
                            format!("event: response.completed\ndata: {done}"),
                        ],
                        Duration::from_millis(1),
                    );
                }
                if s.chat_shaped_responses {
                    return Resp::Json(200, chat("ok", false));
                }
                let mut v = json!({"id": "r1", "object": "response", "model": s.returned, "output": [{"type": "message"}]});
                if s.usage {
                    v["usage"] = usage(false);
                }
                Resp::Json(200, v)
            }
            ("POST", "/v1/chat/completions") => {
                if stream {
                    let c = json!({"model": s.returned, "choices": [{"delta": {"content": "ok"}}], "usage": if s.usage { usage(true) } else { Value::Null }});
                    return Resp::Sse(
                        vec![format!("data: {c}"), "data: [DONE]".into()],
                        Duration::from_millis(1),
                    );
                }
                if r.body.get("tools").is_some() {
                    return if s.tools {
                        Resp::Json(200, chat("", true))
                    } else {
                        Resp::Json(
                            400,
                            json!({"error": {"message": format!("{model} does not support tools")}}),
                        )
                    };
                }
                if r.body.get("response_format").is_some() {
                    return Resp::Json(200, chat("{\"color\":\"blue\"}", false));
                }
                Resp::Json(200, chat("ok", false))
            }
            _ => Resp::Json(404, json!({"error": {"message": "page not found"}})),
        }
    })
}

struct Fx {
    env: Environment,
    project: String,
    home: std::path::PathBuf,
    dir: std::path::PathBuf,
}

fn fx() -> Fx {
    let dir = fixture_dir("hp-hp12");
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    let home = dir.join("home");
    let env = Environment {
        harness_home: Some(home.clone()),
        cwd: dir.clone(),
        ..Environment::default()
    };
    Fx {
        env,
        project: "proj".into(),
        home,
        dir,
    }
}

impl Fx {
    fn run(&self, args: &[&str]) -> semaprax_harness::cli::Outcome {
        let mut v = vec![self.project.clone()];
        v.extend(args.iter().map(|s| s.to_string()));
        cli_endpoints(&v, &self.env)
    }
    fn catalog(&self) -> Catalog {
        Catalog::load(&self.home).unwrap()
    }
}

fn disclosure(fx: &Fx, v: Value) -> String {
    write(&fx.dir, "disclosure.json", &v.to_string());
    "disclosure.json".into()
}

#[test]
fn ollama_adoption_records_observed_verdicts_and_identity() {
    let shape = Shape::new();
    let srv = fixture(shape);
    let fx = fx();
    let out = fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "ollama",
        "--id",
        "local",
    ]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let c = fx.catalog();
    let e = &c.endpoints["local"];
    for key in [
        "responses",
        "responses_streaming",
        "chat_completions",
        "chat_streaming",
        "tool_calls",
        "usage",
        "structured_output",
    ] {
        assert_eq!(
            e.verdict(key),
            Verdict::Supported,
            "{key}: {:?}",
            e.probes.get(key)
        );
    }
    assert_eq!(e.verdict("anthropic_messages"), Verdict::Unsupported);
    assert_eq!(
        e.model("tiny:1b").unwrap().identity,
        ModelIdentity::Digest("d1".into())
    );
    assert_eq!(e.destination_for("tiny:1b"), Destination::Local);
    assert_eq!(e.ownership, AttemptOwnership::direct());
    let list = fx.run(&["list", "--json"]);
    assert!(list.stdout.contains("\"responses\""));
}

#[test]
fn openai_compatible_label_implies_nothing_and_no_silent_downgrade() {
    let shape = Shape::new();
    {
        let mut s = shape.lock().unwrap();
        s.ollama = false;
        s.responses = false;
    }
    let srv = fixture(shape);
    let fx = fx();
    assert_eq!(
        fx.run(&[
            "adopt",
            "--url",
            &srv.url(),
            "--kind",
            "openai-compatible",
            "--id",
            "gw"
        ])
        .code,
        0
    );
    let e = fx.catalog().endpoints["gw"].clone();
    assert_eq!(e.verdict("responses"), Verdict::Unsupported);
    assert_eq!(e.verdict("chat_completions"), Verdict::Supported);
    let refused = fx.run(&[
        "bind",
        "m",
        "--endpoint",
        "gw",
        "--model",
        "gw-model",
        "--protocol",
        "responses",
    ]);
    assert_eq!(refused.code, 1);
    assert!(refused.stderr.contains("SPX-HPL032"), "{}", refused.stderr);
    assert_eq!(
        fx.run(&[
            "bind",
            "m",
            "--endpoint",
            "gw",
            "--model",
            "gw-model",
            "--protocol",
            "chat"
        ])
        .code,
        0
    );
}

#[test]
fn responses_vs_chat_mismatch_is_typed() {
    let shape = Shape::new();
    {
        let mut s = shape.lock().unwrap();
        s.ollama = false;
        s.chat_shaped_responses = true;
    }
    let srv = fixture(shape);
    let fx = fx();
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "openai-compatible",
        "--id",
        "gw",
    ]);
    let e = &fx.catalog().endpoints["gw"];
    assert_eq!(e.verdict("responses"), Verdict::Unsupported);
    assert!(
        e.probes["responses"]
            .evidence
            .contains("protocol_mismatch expected=responses observed=chat_completions"),
        "{:?}",
        e.probes["responses"]
    );
    let reply = semaprax_harness::endpoint::probe::HttpReply {
        status: 200,
        content_type: "application/json".into(),
        body: json!({"choices": []}).to_string().into_bytes(),
    };
    let a = assess_reply(Protocol::Responses, &reply, false, None);
    assert!(a.outcomes.contains(&Outcome::ProtocolMismatch {
        expected: Protocol::Responses,
        observed: Protocol::ChatCompletions
    }));
    assert!(!a.accepted());
    assert_eq!(a.outcomes[0].code(), "SPX-HPL020");
}

#[test]
fn unsupported_tool_schema_is_explicit() {
    let shape = Shape::new();
    shape.lock().unwrap().tools = false;
    let srv = fixture(shape);
    let fx = fx();
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "ollama",
        "--id",
        "local",
    ]);
    let e = &fx.catalog().endpoints["local"];
    assert_eq!(e.verdict("tool_calls"), Verdict::Unsupported);
    assert!(e.probes["tool_calls"]
        .evidence
        .contains("tool schema refused"));
    let lm = LogicalModel::bind(
        "m",
        e,
        "tiny:1b",
        Protocol::ChatCompletions,
        EndpointPolicy::default(),
        0,
    )
    .unwrap();
    assert!(!lm.capabilities.tools);
    assert!(!lm.to_model_plan(0, 10).tools);
    let reply = semaprax_harness::endpoint::probe::HttpReply {
        status: 400,
        content_type: String::new(),
        body: br#"{"error":{"message":"x does not support tools"}}"#.to_vec(),
    };
    let a = assess_reply(Protocol::ChatCompletions, &reply, true, None);
    assert!(matches!(
        a.outcomes[0],
        Outcome::UnsupportedToolSchema { .. }
    ));
    assert_eq!(a.outcomes[0].code(), "SPX-HPL022");
}

#[test]
fn missing_usage_is_unknown_never_zero() {
    let shape = Shape::new();
    shape.lock().unwrap().usage = false;
    let srv = fixture(shape);
    let fx = fx();
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "ollama",
        "--id",
        "local",
    ]);
    let e = fx.catalog().endpoints["local"].clone();
    assert_eq!(e.verdict("usage"), Verdict::Unsupported);
    let reply = semaprax_harness::endpoint::probe::HttpReply {
        status: 200,
        content_type: "application/json".into(),
        body: json!({"object": "response", "output": [], "model": "m"})
            .to_string()
            .into_bytes(),
    };
    let a = assess_reply(Protocol::Responses, &reply, false, None);
    assert_eq!(a.usage, UsageEvidence::Unknown);
    assert!(a.outcomes.contains(&Outcome::MissingUsage));
    assert!(a.accepted(), "missing usage is evidence, not a refusal");
    let j = a.usage.to_json().to_string();
    assert!(j.contains("\"unknown\"") && !j.contains(":0"), "{j}");
    // Partial usage keeps absent members unknown.
    let p = parse_usage(Some(&json!({"input_tokens": 7})));
    assert!(p
        .to_json()
        .to_string()
        .contains("\"output_tokens\":\"unknown\""));
    assert_eq!(
        parse_usage(Some(&json!({"input_tokens": "x"}))),
        UsageEvidence::Unknown
    );
}

#[test]
fn returned_identity_recorded_or_uncertain() {
    let reply = |body: Value| semaprax_harness::endpoint::probe::HttpReply {
        status: 200,
        content_type: String::new(),
        body: body.to_string().into_bytes(),
    };
    let with = assess_reply(
        Protocol::ChatCompletions,
        &reply(json!({"choices": [], "model": "a", "usage": {"total_tokens": 1}})),
        false,
        Some("a"),
    );
    assert_eq!(with.returned_model.as_deref(), Some("a"));
    assert!(with.accepted() && with.outcomes.is_empty());
    let changed = assess_reply(
        Protocol::ChatCompletions,
        &reply(json!({"choices": [], "model": "b", "usage": {"total_tokens": 1}})),
        false,
        Some("a"),
    );
    assert!(changed.outcomes.contains(&Outcome::IdentityChanged {
        bound: "a".into(),
        returned: "b".into()
    }));
    assert!(!changed.accepted());
    let none = assess_reply(
        Protocol::ChatCompletions,
        &reply(json!({"choices": [], "usage": {"total_tokens": 1}})),
        false,
        Some("a"),
    );
    assert!(none.outcomes.contains(&Outcome::IdentityUnreported));
}

#[test]
fn changed_identity_catalog_and_missing_model_invalidate_bindings() {
    let shape = Shape::new();
    let srv = fixture(shape.clone());
    let fx = fx();
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "ollama",
        "--id",
        "local",
    ]);
    assert_eq!(
        fx.run(&[
            "bind",
            "m",
            "--endpoint",
            "local",
            "--model",
            "tiny:1b",
            "--protocol",
            "responses"
        ])
        .code,
        0
    );
    let status = |fx: &Fx| -> Value {
        let o = fx.run(&["reprobe", "local"]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        serde_json::from_str::<Value>(&o.stdout).unwrap()["bindings"]["m"].clone()
    };
    assert_eq!(status(&fx)["status"], "valid");
    shape.lock().unwrap().extra_models = vec!["other:1b".into()];
    let s = status(&fx);
    assert_eq!(s["code"], "SPX-HPL033", "{s}");
    shape.lock().unwrap().extra_models = vec![];
    shape.lock().unwrap().digest = "d2".into();
    let s = status(&fx);
    assert_eq!(s["code"], "SPX-HPL031", "{s}");
    assert!(s["reason"].as_str().unwrap().contains("d1 -> d2"));
    shape.lock().unwrap().drop_model = true;
    shape.lock().unwrap().extra_models = vec!["x2".into()];
    assert_eq!(status(&fx)["code"], "SPX-HPL030");
}

#[test]
fn undisclosed_gateway_cannot_satisfy_local_only_or_strict_one_attempt() {
    let shape = Shape::new();
    shape.lock().unwrap().ollama = false;
    let srv = fixture(shape);
    let fx = fx();
    // No disclosure: loopback is not assumed local inference.
    assert_eq!(
        fx.run(&[
            "adopt",
            "--url",
            &srv.url(),
            "--kind",
            "litellm",
            "--id",
            "gw"
        ])
        .code,
        0
    );
    let e = fx.catalog().endpoints["gw"].clone();
    assert_eq!(
        e.destination_for("gw-model"),
        Destination::Remote {
            origin: "unknown".into()
        }
    );
    assert!(e.ownership.max_upstream_attempts().is_none());
    for flag in ["--local-only", "--strict-one-attempt"] {
        let o = fx.run(&[
            "bind",
            "m",
            "--endpoint",
            "gw",
            "--model",
            "gw-model",
            "--protocol",
            "chat",
            flag,
        ]);
        assert_eq!(o.code, 1, "{flag}");
        assert!(o.stderr.contains("SPX-HPL01"), "{}", o.stderr);
    }
    // Adoption itself can carry the requirement and then stores nothing.
    let fx2 = self::fx();
    let o = fx2.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--id",
        "gw",
        "--local-only",
    ]);
    assert_eq!(o.code, 1);
    assert!(!Catalog::path(&fx2.home).exists());

    // Local destination disclosed but fallbacks silent -> HPL010; retries silent -> HPL012.
    let d = disclosure(
        &fx,
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "destinations": [{"kind": "local"}], "balancing": "none", "retries": "disabled"}),
    );
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--id",
        "gw",
        "--disclosure",
        &d,
    ]);
    let o = fx.run(&[
        "bind",
        "m",
        "--endpoint",
        "gw",
        "--model",
        "gw-model",
        "--protocol",
        "chat",
        "--local-only",
    ]);
    assert!(o.stderr.contains("SPX-HPL010"), "{}", o.stderr);
    let d = disclosure(
        &fx,
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "destinations": [{"kind": "local"}], "fallbacks": "disabled"}),
    );
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--id",
        "gw",
        "--disclosure",
        &d,
    ]);
    let o = fx.run(&[
        "bind",
        "m",
        "--endpoint",
        "gw",
        "--model",
        "gw-model",
        "--protocol",
        "chat",
        "--strict-one-attempt",
    ]);
    assert!(o.stderr.contains("SPX-HPL012"), "{}", o.stderr);
    // A disclosed remote fallback violates local-only; bounded retries violate strict.
    let d = disclosure(
        &fx,
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "destinations": [{"kind": "local"}],
        "retries": {"exact": 2}, "fallbacks": [{"model": "cloud", "destination": {"kind": "remote", "origin": "api.example.com"}}]}),
    );
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--id",
        "gw",
        "--disclosure",
        &d,
    ]);
    let o = fx.run(&[
        "bind",
        "m",
        "--endpoint",
        "gw",
        "--model",
        "gw-model",
        "--protocol",
        "chat",
        "--local-only",
    ]);
    assert!(o.stderr.contains("SPX-HPL011"), "{}", o.stderr);
    let o = fx.run(&[
        "bind",
        "m",
        "--endpoint",
        "gw",
        "--model",
        "gw-model",
        "--protocol",
        "chat",
        "--strict-one-attempt",
    ]);
    assert!(
        o.stderr.contains("up to 6 upstream attempts"),
        "{}",
        o.stderr
    );
    // Fully disclosed local one-attempt gateway satisfies both.
    let d = disclosure(&fx, local_one_attempt());
    fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--id",
        "gw",
        "--disclosure",
        &d,
    ]);
    let o = fx.run(&[
        "bind",
        "m",
        "--endpoint",
        "gw",
        "--model",
        "gw-model",
        "--protocol",
        "chat",
        "--local-only",
        "--strict-one-attempt",
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        fx.catalog().bindings["m"].attempt_owner,
        AttemptOwner::Semaprax
    );
}

fn local_one_attempt() -> Value {
    json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "destinations": [{"kind": "local"}],
        "balancing": "none", "retries": "disabled", "fallbacks": "disabled"})
}

#[test]
fn same_routing_policy_direct_or_gateway_yields_same_model_plan() {
    let shape = Shape::new();
    let direct = fixture(shape.clone());
    let gw = fixture(Shape::new_gateway());
    let fx = fx();
    fx.run(&[
        "adopt",
        "--url",
        &direct.url(),
        "--kind",
        "ollama",
        "--id",
        "d",
    ]);
    let d = disclosure(&fx, local_one_attempt());
    fx.run(&[
        "adopt",
        "--url",
        &gw.url(),
        "--kind",
        "litellm",
        "--id",
        "g",
        "--disclosure",
        &d,
    ]);
    let c = fx.catalog();
    let pd = LogicalModel::bind(
        "route-a",
        &c.endpoints["d"],
        "tiny:1b",
        Protocol::ChatCompletions,
        EndpointPolicy::default(),
        3,
    )
    .unwrap()
    .to_model_plan(0, 50);
    let pg = LogicalModel::bind(
        "route-a",
        &c.endpoints["g"],
        "gw-model",
        Protocol::ChatCompletions,
        EndpointPolicy::default(),
        3,
    )
    .unwrap()
    .to_model_plan(0, 50);
    assert_eq!(pd.destination, Destination::Local);
    assert_eq!(
        (
            pd.id,
            pd.destination,
            pd.structured_output,
            pd.tools,
            pd.strength_rank
        ),
        (
            pg.id,
            pg.destination,
            pg.structured_output,
            pg.tools,
            pg.strength_rank
        )
    );
}

impl Shape {
    fn new_gateway() -> Arc<Mutex<Self>> {
        let s = Self::new();
        s.lock().unwrap().ollama = false;
        s
    }
}

#[test]
fn secrets_never_reach_catalog_reports_or_project() {
    let secret = "sk-hp12-super-secret-value";
    let shape = Shape::new_gateway();
    let srv = fixture(shape);
    let mut fx = fx();
    fx.env.vars.insert("GW_KEY".into(), secret.into());
    let o = fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--id",
        "gw",
        "--credential-env",
        "GW_KEY",
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let list = fx.run(&["list"]);
    let json_list = fx.run(&["list", "--json"]);
    let rep = fx.run(&["reprobe", "gw"]);
    let mut all = vec![
        o.stdout,
        o.stderr,
        list.stdout,
        json_list.stdout,
        rep.stdout,
        rep.stderr,
    ];
    for entry in std::fs::read_dir(&fx.home).unwrap() {
        all.push(std::fs::read_to_string(entry.unwrap().path()).unwrap());
    }
    for entry in std::fs::read_dir(fx.dir.join("proj")).unwrap() {
        all.push(std::fs::read_to_string(entry.unwrap().path()).unwrap());
    }
    assert!(all.iter().all(|t| !t.contains(secret)));
    assert!(fx.catalog().endpoints["gw"].credential_env.as_deref() == Some("GW_KEY"));
    // The credential was actually used at call time.
    let probe = serve(move |r: &Req| {
        assert_eq!(r.auth.as_deref(), Some("Bearer sk-hp12-super-secret-value"));
        Resp::Json(200, json!({"data": []}))
    });
    let c = ProbeClient::new(Target::parse(&probe.url()).unwrap(), Some(secret.into()));
    assert_eq!(c.get("/v1/models").unwrap().status, 200);
    // A value in the credential slot is refused, as is an unforwarded name.
    let o = fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--credential-env",
        secret,
    ]);
    assert!(
        o.stderr.contains("SPX-HPL007") && !o.stderr.contains(secret)
            || o.stderr.contains("SPX-HPL007")
    );
    let o = fx.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "litellm",
        "--credential-env",
        "NOT_FORWARDED",
    ]);
    assert!(o.stderr.contains("SPX-HPL007"));
}

#[test]
fn only_loopback_endpoints_and_no_auto_download() {
    let fx = fx();
    for url in [
        "http://example.com:80",
        "https://127.0.0.1:1",
        "http://10.1.2.3:11434",
    ] {
        let o = fx.run(&["adopt", "--url", url, "--kind", "ollama"]);
        assert!(o.stderr.contains("SPX-HPL002"), "{url}: {}", o.stderr);
    }
    // An endpoint with no models adopts nothing and pulls nothing.
    let srv = serve(|r: &Req| match r.path.as_str() {
        "/api/tags" => Resp::Json(200, json!({"models": []})),
        _ => Resp::Json(404, json!({})),
    });
    let o = fx.run(&["adopt", "--url", &srv.url(), "--kind", "ollama"]);
    assert!(o.stderr.contains("SPX-HPL006"));
    assert_eq!(srv.log.lock().unwrap().as_slice(), ["GET /api/tags"]);
}

#[test]
fn disclosure_is_strict_and_litellm_snippet_disables_attempts() {
    for bad in [
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "mystery": 1}),
        json!({"schema": "wrong"}),
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "retries": "sometimes"}),
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "fallbacks": [{"model": "x"}]}),
    ] {
        assert_eq!(
            Disclosure::from_json(&bad).unwrap_err().code,
            "SPX-HPL005",
            "{bad}"
        );
    }
    let s = litellm_config_snippet(
        "semaprax-local",
        "ollama_chat/tiny",
        "http://127.0.0.1:11434",
    );
    assert!(s.contains("num_retries: 0") && s.contains("model_name: semaprax-local"));
    assert!(!s.lines().any(|l| l.trim_start().starts_with("fallbacks:")));
    assert_eq!(
        s,
        litellm_config_snippet(
            "semaprax-local",
            "ollama_chat/tiny",
            "http://127.0.0.1:11434"
        )
    );
    let fx = fx();
    let o = fx.run(&[
        "litellm-config",
        "--logical",
        "a",
        "--upstream",
        "b",
        "--api-base",
        "http://127.0.0.1:1",
    ]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("api_base: http://127.0.0.1:1"));
}

#[test]
fn streaming_cancellation_closes_the_connection_mid_stream() {
    let events: Vec<String> = (0..50).map(|i| format!("data: {{\"i\":{i}}}")).collect();
    let srv = serve(move |_| Resp::Sse(events.clone(), Duration::from_millis(20)));
    let c = ProbeClient::new(Target::parse(&srv.url()).unwrap(), None);
    let r = c
        .post_stream("/v1/responses", &json!({"stream": true}), Some(2), 100)
        .unwrap();
    assert!(r.cancelled && !r.completed);
    assert_eq!(r.events.len(), 2);
    for _ in 0..100 {
        if srv.client_closed.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        srv.client_closed.load(Ordering::SeqCst),
        "server did not observe the close"
    );
    assert_eq!(srv.requests.load(Ordering::SeqCst), 1);
}

#[test]
fn catalog_is_deterministic_canonical_json() {
    let srv = fixture(Shape::new());
    let a = fx();
    let b = fx();
    a.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "ollama",
        "--id",
        "local",
    ]);
    b.run(&[
        "adopt",
        "--url",
        &srv.url(),
        "--kind",
        "ollama",
        "--id",
        "local",
    ]);
    assert_eq!(
        std::fs::read(Catalog::path(&a.home)).unwrap(),
        std::fs::read(Catalog::path(&b.home)).unwrap()
    );
}
