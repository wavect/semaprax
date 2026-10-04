//! Provisioned real-tool evidence for HP-12 (fixture prefix `hp-hp12r`).
//!
//! Needs a running Ollama with one small model and a LiteLLM proxy whose
//! `semaprax-local` model routes to a counting proxy on 127.0.0.1:11435 (which
//! this test starts and which forwards to Ollama) and whose `semaprax-missing`
//! model routes to a nonexistent Ollama model, both with `num_retries: 0`.
//!   HARNESS_OLLAMA_URL   e.g. http://127.0.0.1:11434
//!   HARNESS_LITELLM_URL  e.g. http://127.0.0.1:4000
//!   HARNESS_LITELLM_KEY  the proxy master key (forwarded by name only)
//!   HARNESS_OLLAMA_MODEL (default qwen2.5:0.5b)  HARNESS_COUNTING_PORT (default 11435)

use crate::support::fixture_dir;
use semaprax_harness::cli::Environment;
use semaprax_harness::decision::Destination;
use semaprax_harness::endpoint::*;
use serde_json::{json, Value};
use std::time::Duration;

#[path = "../fixtures/endpoint/server.rs"]
mod server;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("provisioned test requires {name}"))
}

fn settle(proxy: &server::Server) -> usize {
    // Wait for the request count to stop moving for 1.5 s (retry storms would show here).
    let mut last = proxy.requests.load(std::sync::atomic::Ordering::SeqCst);
    let mut quiet = 0;
    while quiet < 6 {
        std::thread::sleep(Duration::from_millis(250));
        let now = proxy.requests.load(std::sync::atomic::Ordering::SeqCst);
        quiet = if now == last { quiet + 1 } else { 0 };
        last = now;
    }
    last
}

fn show(label: &str, e: &EndpointRecord) {
    println!(
        "== {label}: {} {} catalog={}",
        e.id,
        e.url,
        e.catalog_digest()
    );
    for (k, p) in &e.probes {
        println!("  {k}: {} | {}", p.verdict.as_str(), p.evidence);
    }
}

#[test]
#[ignore = "provisioned: needs HARNESS_OLLAMA_URL HARNESS_LITELLM_URL HARNESS_LITELLM_KEY"]
fn ollama_and_litellm_gateway_reuse_with_cancellation_and_single_attempt() {
    let (ollama_url, litellm_url, key) = (
        var("HARNESS_OLLAMA_URL"),
        var("HARNESS_LITELLM_URL"),
        var("HARNESS_LITELLM_KEY"),
    );
    let model = std::env::var("HARNESS_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:0.5b".into());
    let cport: u16 = std::env::var("HARNESS_COUNTING_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(11435);
    let proxy = server::counting_proxy(cport, Target::parse(&ollama_url).unwrap().port);

    let dir = fixture_dir("hp-hp12r");
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    std::fs::write(
        dir.join("gw.json"),
        json!({"schema": "semaprax.harness-endpoint-disclosure.v1", "destinations": [{"kind": "local"}],
            "balancing": "none", "retries": "disabled", "fallbacks": "disabled"})
        .to_string(),
    )
    .unwrap();
    let mut env = Environment::default();
    env.harness_home = Some(dir.join("home"));
    env.cwd = dir.clone();
    env.vars.insert("LITELLM_MASTER_KEY".into(), key.clone());
    let run = |args: &[&str]| {
        let mut v = vec!["proj".to_string()];
        v.extend(args.iter().map(|s| s.to_string()));
        cli_endpoints(&v, &env)
    };

    // 1. Adopt the existing local Ollama (no model is installed by adoption).
    let o = run(&[
        "adopt",
        "--url",
        &ollama_url,
        "--kind",
        "ollama",
        "--id",
        "ollama",
        "--model",
        &model,
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    // 2. Adopt the explicitly configured LiteLLM gateway with its disclosure.
    let o = run(&[
        "adopt",
        "--url",
        &litellm_url,
        "--kind",
        "litellm",
        "--id",
        "litellm",
        "--disclosure",
        "gw.json",
        "--credential-env",
        "LITELLM_MASTER_KEY",
        "--model",
        "semaprax-local",
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let catalog = Catalog::load(&dir.join("home")).unwrap();
    let (ol, gw) = (&catalog.endpoints["ollama"], &catalog.endpoints["litellm"]);
    show("ollama", ol);
    show("litellm", gw);
    assert_eq!(ol.verdict("chat_completions"), Verdict::Supported);
    assert_eq!(gw.verdict("chat_completions"), Verdict::Supported);
    assert_eq!(ol.verdict("structured_output"), Verdict::Supported);
    assert_eq!(gw.verdict("structured_output"), Verdict::Supported);
    assert!(matches!(
        ol.model(&model).unwrap().identity,
        ModelIdentity::Digest(_)
    ));
    let secret_free = std::fs::read_to_string(Catalog::path(&dir.join("home"))).unwrap();
    assert!(
        !secret_free.contains(&key),
        "master key leaked into the catalog"
    );

    // 3. Bindings: same logical policy, direct and gateway; undisclosed gateway is refused.
    let strict = EndpointPolicy {
        local_only: true,
        strict_one_attempt: true,
    };
    let proto = if ol.verdict("responses") == Verdict::Supported
        && gw.verdict("responses") == Verdict::Supported
    {
        Protocol::Responses
    } else {
        Protocol::ChatCompletions
    };
    let d = LogicalModel::bind("local-small", ol, &model, proto, strict, 1).unwrap();
    let g = LogicalModel::bind("local-small", gw, "semaprax-local", proto, strict, 1).unwrap();
    assert_eq!(d.destination, Destination::Local);
    assert_eq!(g.destination, Destination::Local);
    assert_eq!(g.attempt_owner, AttemptOwner::Semaprax);
    println!("bridge protocol: {}", proto.as_str());
    println!("plan direct : {}", d.to_model_plan(0, 2000).to_json());
    println!("plan gateway: {}", g.to_model_plan(0, 2000).to_json());
    let o = run(&[
        "adopt",
        "--url",
        &litellm_url,
        "--kind",
        "litellm",
        "--id",
        "undisclosed",
        "--credential-env",
        "LITELLM_MASTER_KEY",
        "--model",
        "semaprax-local",
        "--local-only",
    ]);
    assert_eq!(o.code, 1);
    println!("undisclosed gateway + local-only: {}", o.stderr.trim());

    // 4. Streaming with cancellation (close mid-stream) on both.
    for (name, url, m, cred) in [
        ("ollama", &ollama_url, model.as_str(), None),
        ("litellm", &litellm_url, "semaprax-local", Some(key.clone())),
    ] {
        let c = ProbeClient::new(Target::parse(url).unwrap(), cred);
        let body = json!({"model": m, "input": "Count from one to forty, one number per line.", "max_output_tokens": 200, "stream": true});
        let r = c
            .post_stream("/v1/responses", &body, Some(3), 1000)
            .unwrap();
        println!(
            "{name}: cancel after 3 events: status={} cancelled={} events={} first={:?}",
            r.status,
            r.cancelled,
            r.events.len(),
            r.events.first().and_then(|e| e.event.clone())
        );
        assert_eq!(r.status, 200);
        assert!(r.cancelled && !r.completed && r.events.len() >= 3);
    }

    // 5. Failure through the counting proxy: one upstream attempt, no retry storm.
    let before = settle(&proxy);
    let c = ProbeClient::new(Target::parse(&litellm_url).unwrap(), Some(key.clone()));
    let r = c.post_json("/v1/chat/completions", &json!({"model": "semaprax-missing", "messages": [{"role": "user", "content": "hi"}], "max_tokens": 4})).unwrap();
    let a = assess_reply(Protocol::ChatCompletions, &r, false, None);
    let after = settle(&proxy);
    println!(
        "litellm failure: status={} outcomes={:?} upstream_attempts={}",
        r.status,
        a.outcomes.iter().map(Outcome::code).collect::<Vec<_>>(),
        after - before
    );
    println!("proxy log tail: {:?}", &proxy.log.lock().unwrap()[before..]);
    assert!(!(200..300).contains(&r.status));
    assert_eq!(
        after - before,
        1,
        "gateway made more than one upstream attempt"
    );
    let direct = ProbeClient::new(Target::parse(&proxy.url()).unwrap(), None);
    let r = direct
        .post_json(
            "/v1/chat/completions",
            &json!({"model": "does-not-exist:1b", "messages": [{"role": "user", "content": "hi"}]}),
        )
        .unwrap();
    assert_eq!(r.status, 404);
    assert_eq!(settle(&proxy) - after, 1);

    // 6. Structured-output negotiation, executed, plus usage evidence.
    for (name, url, m, cred) in [
        ("ollama", &ollama_url, model.as_str(), None),
        ("litellm", &litellm_url, "semaprax-local", Some(key.clone())),
    ] {
        let c = ProbeClient::new(Target::parse(url).unwrap(), cred);
        let body = json!({"model": m, "max_tokens": 48, "messages": [{"role": "user", "content": "Name one color."}],
            "response_format": {"type": "json_schema", "json_schema": {"name": "color", "strict": true,
                "schema": {"type": "object", "properties": {"color": {"type": "string"}}, "required": ["color"], "additionalProperties": false}}}});
        let r = c.post_json("/v1/chat/completions", &body).unwrap();
        let v = r.json().unwrap();
        let content: Value = serde_json::from_str(
            v.pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .unwrap(),
        )
        .unwrap();
        let a = assess_reply(Protocol::ChatCompletions, &r, false, None);
        println!(
            "{name}: structured={content} usage={} returned_model={:?}",
            a.usage.to_json(),
            a.returned_model
        );
        assert!(content["color"].is_string());
        assert_ne!(a.usage, UsageEvidence::Unknown);
    }

    // 7. Re-probe keeps bindings valid while nothing changed.
    let o = run(&[
        "bind",
        "gw-local",
        "--endpoint",
        "litellm",
        "--model",
        "semaprax-local",
        "--protocol",
        "chat",
        "--local-only",
        "--strict-one-attempt",
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let o = run(&["reprobe"]);
    println!("reprobe: {}", o.stdout.trim());
    assert!(o.stdout.contains("\"valid\""));
}
