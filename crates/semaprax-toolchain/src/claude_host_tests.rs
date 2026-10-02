use super::*;
use serde_json::{json, Value};

pub(crate) fn envelope(result: &str) -> Value {
    json!({"type":"result", "subtype":"success", "is_error":false, "num_turns":1,
        "stop_reason":"end_turn", "terminal_reason":"completed", "queued_turn_count":0,
        "result_index":0, "permission_denials":[], "subagent_stats":{"spawned":0},
        "result":result, "usage":{"input_tokens":17,"output_tokens":23},
        "modelUsage":{MODEL:{"canonicalModel":MODEL,"provider":"firstParty","webSearchRequests":0}}})
}
#[test]
fn native_claude_result_preserves_exact_bytes_and_reported_usage() {
    let settled = parse(
        &serde_json::to_vec(&envelope("{\"proposal\":1}\n")).unwrap(),
        1024,
    )
    .unwrap();
    assert_eq!(settled.response_bytes, b"{\"proposal\":1}\n");
    assert_eq!(settled.usage.tokens_in, Some(17));
    assert_eq!(settled.usage.tokens_out, Some(23));
    assert_eq!(settled.usage.cost_micros, None);
}
#[test]
fn native_claude_refuses_wrong_model_auxiliary_calls_and_malformed_usage() {
    for (path, replacement) in [
        ("/type", json!("text")),
        ("/subtype", json!("error")),
        ("/is_error", json!(true)),
        ("/num_turns", json!(2)),
        ("/stop_reason", json!("max_tokens")),
        ("/terminal_reason", json!("error")),
        ("/queued_turn_count", json!(1)),
        ("/permission_denials", json!([{}])),
        ("/subagent_stats/spawned", json!(1)),
        ("/usage/input_tokens", json!(-1)),
        ("/usage/output_tokens", json!("23")),
        ("/result", json!(null)),
        (
            "/modelUsage/claude-haiku-4-5/canonicalModel",
            json!("other"),
        ),
        ("/modelUsage/claude-haiku-4-5/provider", json!("bedrock")),
        ("/modelUsage/claude-haiku-4-5/webSearchRequests", json!(1)),
    ] {
        let mut value = envelope("ok");
        *value.pointer_mut(path).unwrap() = replacement;
        assert_eq!(
            parse(&serde_json::to_vec(&value).unwrap(), 1024),
            Err(ModelFailure::MalformedResponse),
            "{path}"
        );
    }
    let mut value = envelope("ok");
    value["modelUsage"]["other"] = json!({});
    assert!(parse(&serde_json::to_vec(&value).unwrap(), 1024).is_err());
    assert!(parse(&serde_json::to_vec(&envelope("too long")).unwrap(), 1).is_err());
    assert!(parse(b"{}\n{}", 1024).is_err());
}

#[cfg(unix)]
mod process {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(script: &str, deadline: Duration) -> (Self, Config) {
            let root = std::env::temp_dir().join(format!(
                "semaprax-claude-host-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            let executable = root.join("fixture");
            std::fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            let scratch = root.join("scratch");
            std::fs::create_dir(&scratch).unwrap();
            let config = Config::new(executable, scratch, deadline).unwrap();
            (Self(root), config)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn claude_capture_drains_more_than_pipe_capacity_before_child_exit() {
        let mut value = envelope("unchanged\n");
        value["padding"] = json!("x".repeat(200_000));
        let script = format!("test \"$1\" = --print || exit 91\ntest \"$4\" = --tools || exit 92\ntest -z \"$5\" || exit 93\ntest -z \"${{ANTHROPIC_API_KEY+x}}\" || exit 94\ntest -n \"$USER\" || exit 95\ntest \"$LOGNAME\" = \"$USER\" || exit 96\nprintf '%s' '{}'", value);
        let (_fixture, config) = Fixture::new(&script, Duration::from_secs(5));
        let bytes = invoke(&config, "bounded fixture prompt").unwrap();
        assert!(bytes.len() > 200_000);
        assert_eq!(parse(&bytes, 1024).unwrap().response_bytes, b"unchanged\n");
    }
    #[test]
    fn claude_capture_enforces_deadline_output_cap_and_cancellation() {
        for (script, duration, expected) in [
            ("sleep 60", Duration::from_millis(80), ModelFailure::Timeout),
            ("while :; do printf '0123456789012345678901234567890123456789012345678901234567890123456789'; done", Duration::from_secs(5), ModelFailure::MalformedResponse),
        ] {
            let (_fixture, config) = Fixture::new(script, duration);
            let start = Instant::now(); assert_eq!(invoke(&config, "fixture"), Err(expected));
            assert!(start.elapsed() < Duration::from_secs(6));
            assert_eq!(std::fs::read_dir(&config.host.sandbox).unwrap().count(), 0);
        }
        let (_fixture, config) = Fixture::new("exit 99", Duration::from_secs(5));
        config.host.cancellation().cancel();
        assert_eq!(invoke(&config, "fixture"), Err(ModelFailure::Cancelled));
    }
    #[test]
    fn claude_identity_drift_and_second_start_refuse() {
        let (_fixture, config) = Fixture::new("printf bad", Duration::from_secs(2));
        let identity = config.identity().adapter_identity;
        std::fs::write(&config.host.executable, b"changed").unwrap();
        assert_eq!(invoke(&config, "fixture"), Err(ModelFailure::Refused));
        let mut adapter = ClaudeAdapter::new(config, identity);
        let capability = AdapterInvocationCapability::grant("offline fixture only");
        let request = AdapterRequest {
            request_bytes: b"fixture".to_vec(),
            max_response_bytes: 1024,
        };
        adapter.start(&capability, &request).unwrap();
        assert!(adapter.start(&capability, &request).is_err());
    }
}

#[test]
fn compiler_derived_guidance_supplies_an_exact_decoder_valid_envelope() {
    let source = include_str!("../../../examples/offline-repair-project/src/app.spx");
    let compiled = semaprax::agent_lifecycle::iterative::compile_source_agent_lifecycle_v2(
        source,
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    let schema = compiled.proposal_schema();
    let prefix = proposal_prefix(schema);
    let guidance = proposal_guidance(schema);
    assert!(guidance.contains(&prefix));
    assert!(guidance.contains(schema.schema().digest()));
    // Values remain chosen by the provider. These test-only values exercise
    // the unchanged production decoder against the actual supplied prefix.
    let body = r#"{"fields":{"fixture.agent.type.proposal.budget":"9","fixture.agent.type.proposal.urgent":false,"fixture.agent.type.proposal.sequence":"0"}}"#;
    let document = format!("{prefix}{body}}}\n");
    assert!(schema.decode(&document).is_ok());
    assert!(schema.decode(&format!("{body}\n")).is_err());
    assert!(schema.decode(&document.replace("\"9\"", "9")).is_err());
    assert!(schema.decode(document.trim_end()).is_err());
}
