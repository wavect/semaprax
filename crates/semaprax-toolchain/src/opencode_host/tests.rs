use super::*;
use std::path::PathBuf;
use std::time::Duration;

struct FixtureRunner {
    events: Vec<u8>,
    export: Vec<u8>,
    cancelled: bool,
}
impl OpenCodeRunner for FixtureRunner {
    fn run(&mut self, _: &OpenCodeHostConfig, _: &str) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        Ok(self.events.clone())
    }
    fn export(
        &mut self,
        _: &OpenCodeHostConfig,
        _: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        Ok(self.export.clone())
    }
    fn cancelled(&self, _: &OpenCodeHostConfig) -> bool {
        self.cancelled
    }
}

fn config(digest: &str) -> OpenCodeHostConfig {
    let sandbox =
        std::env::temp_dir().join(format!("semaprax-opencode-host-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir(&sandbox).unwrap();
    let config = OpenCodeHostConfig::new(
        PathBuf::from("/bin/true"),
        sandbox.clone(),
        Duration::from_secs(1),
        OpenCodeGrammar {
            digest: digest.into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    std::fs::remove_dir(sandbox).unwrap();
    config
}

#[test]
fn configured_handler_returns_only_validated_raw_text_for_the_existing_decoder() {
    let request = ModelInvocationRequest {
        turn: 0,
        task: b"task".to_vec(),
        observation: b"obs".to_vec(),
        proposal_grammar_digest: "sha256:grammar".into(),
        deployment_binding: "sha256:deployment".into(),
        max_response_bytes: 128,
        effective_budget: 1,
    };
    let prompt = wire_prompt(
        &request,
        &OpenCodeGrammar {
            digest: request.proposal_grammar_digest.clone(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    let events = b"{\"type\":\"step_start\",\"sessionID\":\"s\",\"part\":{\"sessionID\":\"s\",\"messageID\":\"m\"}}\n{\"type\":\"text\",\"sessionID\":\"s\",\"part\":{\"sessionID\":\"s\",\"messageID\":\"m\",\"text\":\"proposal\"}}\n{\"type\":\"step_finish\",\"sessionID\":\"s\",\"part\":{\"sessionID\":\"s\",\"messageID\":\"m\",\"reason\":\"stop\"}}\n".to_vec();
    let export = serde_json::json!({"info":{"id":"s","model":{"providerID":"opencode","id":"muse-spark-1.3-contributor-free"}},"messages":[{"info":{"id":"u","role":"user","sessionID":"s"},"parts":[{"text":prompt}]},{"info":{"id":"m","parentID":"u","role":"assistant","sessionID":"s","tokens":{"total":7}},"parts":[{"text":"proposal"}]}]}).to_string().into_bytes();
    let mut handler = OpenCodeModelHandler::new(
        config("sha256:grammar"),
        FixtureRunner {
            events,
            export,
            cancelled: false,
        },
    );
    assert_eq!(
        handler.invoke(&ModelInvokeCapability::grant("test"), &request),
        ModelInvocationOutcome::Settled(b"proposal".to_vec())
    );
    assert_eq!(handler.last_receipt.unwrap().usage_total, Some(7));
}

#[test]
fn post_start_transport_failure_is_provider_error_unless_cancellation_was_observed() {
    struct FailedRunner {
        cancelled: bool,
    }
    impl OpenCodeRunner for FailedRunner {
        fn run(
            &mut self,
            _: &OpenCodeHostConfig,
            _: &str,
        ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
            Err(OpenCodeRunnerFailure::Provider)
        }
        fn export(
            &mut self,
            _: &OpenCodeHostConfig,
            _: &str,
        ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
            unreachable!("run failure cannot export")
        }
        fn cancelled(&self, _: &OpenCodeHostConfig) -> bool {
            self.cancelled
        }
    }
    let request = ModelInvocationRequest {
        turn: 0,
        task: vec![],
        observation: vec![],
        proposal_grammar_digest: "g".into(),
        deployment_binding: "d".into(),
        max_response_bytes: 128,
        effective_budget: 1,
    };
    let mut uncertain = OpenCodeModelHandler::new(config("g"), FailedRunner { cancelled: false });
    assert_eq!(
        uncertain.invoke(&ModelInvokeCapability::grant("test"), &request),
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::ProviderError,
            attempted_bytes: 0
        }
    );
    let mut cancelled = OpenCodeModelHandler::new(config("g"), FailedRunner { cancelled: true });
    assert_eq!(
        cancelled.invoke(&ModelInvokeCapability::grant("test"), &request),
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::Cancelled,
            attempted_bytes: 0
        }
    );
}

#[test]
fn handler_bytes_pass_the_real_compiler_derived_proposal_decoder() {
    use semaprax::agent_interaction_schema::compile_agent_interaction_schema;
    use semaprax::agent_interaction_schema::live_bridge::SourceInteractionProposalDecoder;
    use semaprax::live_invocation::ProposalDecoder;

    let source = std::env::temp_dir().join(format!(
        "semaprax-opencode-host-schema-{}.spx",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
module host.test;

@id("answer.type")
record Answer {
    @id("answer.note")
    note: string,
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    )
    .unwrap();
    let schema = compile_agent_interaction_schema(&source, "answer.type").unwrap();
    std::fs::remove_file(source).unwrap();
    let document = format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"answer.type\",\"schema_digest\":{},\"value\":{{\"fields\":{{\"answer.note\":\"accepted\"}}}}}}}\n",
        semaprax::diagnostic::quote_json(schema.schema().digest())
    );
    let request = ModelInvocationRequest {
        turn: 0,
        task: vec![],
        observation: vec![],
        proposal_grammar_digest: schema.schema().digest().to_owned(),
        deployment_binding: "d".into(),
        max_response_bytes: document.len() + 1,
        effective_budget: 1,
    };
    let grammar = OpenCodeGrammar::from_compiled(&schema).unwrap();
    let prompt = wire_prompt(&request, &grammar).unwrap();
    let events = format!(
        "{{\"type\":\"step_start\",\"sessionID\":\"s\",\"part\":{{\"sessionID\":\"s\",\"messageID\":\"m\"}}}}\n{{\"type\":\"text\",\"sessionID\":\"s\",\"part\":{{\"sessionID\":\"s\",\"messageID\":\"m\",\"text\":{}}}}}\n{{\"type\":\"step_finish\",\"sessionID\":\"s\",\"part\":{{\"sessionID\":\"s\",\"messageID\":\"m\",\"reason\":\"stop\"}}}}\n",
        semaprax::diagnostic::quote_json(&document)
    ).into_bytes();
    let export = serde_json::json!({"info":{"id":"s","model":{"providerID":"opencode","id":"muse-spark-1.3-contributor-free"}},"messages":[{"info":{"id":"u","role":"user","sessionID":"s"},"parts":[{"text":prompt}]},{"info":{"id":"m","parentID":"u","role":"assistant","sessionID":"s"},"parts":[{"text":document}]}]}).to_string().into_bytes();
    let mut handler = OpenCodeModelHandler::new(
        OpenCodeHostConfig::new(
            PathBuf::from("/bin/true"),
            {
                let sandbox = std::env::temp_dir().join(format!(
                    "semaprax-opencode-host-compiled-{}",
                    std::process::id()
                ));
                let _ = std::fs::remove_dir_all(&sandbox);
                std::fs::create_dir(&sandbox).unwrap();
                sandbox
            },
            Duration::from_secs(1),
            grammar,
        )
        .unwrap(),
        FixtureRunner {
            events,
            export,
            cancelled: false,
        },
    );
    let ModelInvocationOutcome::Settled(bytes) =
        handler.invoke(&ModelInvokeCapability::grant("test"), &request)
    else {
        panic!("fixture must settle");
    };
    let mut decoder = SourceInteractionProposalDecoder::new(schema);
    assert!(matches!(
        decoder.decode(0, &bytes),
        semaprax::live_invocation::ProposalOutcome::Admitted(_)
    ));
}

#[cfg(unix)]
#[test]
fn process_runner_uses_a_local_stub_without_provider_access() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("semaprax-opencode-stub-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let stub = root.join("stub");
    std::fs::write(&stub, "#!/bin/sh\nprintf 'stub-output'\n").unwrap();
    let mut permissions = std::fs::metadata(&stub).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&stub, permissions).unwrap();
    let sandbox = root.join("sandbox");
    std::fs::create_dir(&sandbox).unwrap();
    let config = OpenCodeHostConfig::new(
        stub,
        sandbox,
        Duration::from_secs(1),
        OpenCodeGrammar {
            digest: "g".into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    assert_eq!(
        ProcessOpenCodeRunner::capture(&config, &[], 64).unwrap(),
        b"stub-output"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn export_rejects_user_or_assistant_session_substitution() {
    let export = serde_json::json!({
        "info":{"id":"s","model":{"providerID":"opencode","id":"muse-spark-1.3-contributor-free"}},
        "messages":[
            {"info":{"id":"u","role":"user","sessionID":"other"},"parts":[{"text":"prompt"}]},
            {"info":{"id":"m","parentID":"u","role":"assistant","sessionID":"s"},"parts":[{"text":"answer"}]}
        ]
    }).to_string();
    assert_eq!(
        validate_export(export.as_bytes(), "s", "m", "prompt", "answer"),
        Err(ModelFailure::MalformedResponse)
    );
}

#[cfg(unix)]
#[test]
fn process_runner_reaps_an_overflowing_local_stub_without_waiting_for_deadline() {
    use std::os::unix::fs::PermissionsExt;
    let root =
        std::env::temp_dir().join(format!("semaprax-opencode-overflow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let stub = root.join("stub");
    std::fs::write(&stub, "#!/bin/sh\nhead -c 1024 /dev/zero\nsleep 5\n").unwrap();
    let mut permissions = std::fs::metadata(&stub).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&stub, permissions).unwrap();
    let sandbox = root.join("sandbox");
    std::fs::create_dir(&sandbox).unwrap();
    let config = OpenCodeHostConfig::new(
        stub,
        sandbox,
        Duration::from_secs(2),
        OpenCodeGrammar {
            digest: "g".into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    let started = std::time::Instant::now();
    assert_eq!(
        ProcessOpenCodeRunner::capture(&config, &[], 8),
        Err(OpenCodeRunnerFailure::Malformed)
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn process_runner_cancels_and_reaps_a_sleeping_local_stub() {
    use std::os::unix::fs::PermissionsExt;
    let root =
        std::env::temp_dir().join(format!("semaprax-opencode-cancel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let stub = root.join("stub");
    std::fs::write(&stub, "#!/bin/sh\nsleep 5\n").unwrap();
    let mut permissions = std::fs::metadata(&stub).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&stub, permissions).unwrap();
    let sandbox = root.join("sandbox");
    std::fs::create_dir(&sandbox).unwrap();
    let config = OpenCodeHostConfig::new(
        stub,
        sandbox,
        Duration::from_secs(2),
        OpenCodeGrammar {
            digest: "g".into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    let cancellation = config.cancellation();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        cancellation.cancel();
    });
    assert_eq!(
        ProcessOpenCodeRunner::capture(&config, &[], 8),
        Err(OpenCodeRunnerFailure::Cancelled)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn handler_refuses_before_dispatch_when_compiler_guidance_digest_does_not_match_request() {
    let request = ModelInvocationRequest {
        turn: 0,
        task: vec![],
        observation: vec![],
        proposal_grammar_digest: "other".into(),
        deployment_binding: "d".into(),
        max_response_bytes: 128,
        effective_budget: 1,
    };
    let mut handler = OpenCodeModelHandler::new(
        config("bound"),
        FixtureRunner {
            events: vec![],
            export: vec![],
            cancelled: false,
        },
    );
    assert_eq!(
        handler.invoke(&ModelInvokeCapability::grant("test"), &request),
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::Refused,
            attempted_bytes: 0
        }
    );
}

#[cfg(unix)]
#[test]
fn host_config_rejects_a_sandbox_symlink() {
    use std::os::unix::fs::symlink;
    let root =
        std::env::temp_dir().join(format!("semaprax-opencode-symlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let actual = root.join("actual");
    std::fs::create_dir(&actual).unwrap();
    let linked = root.join("linked");
    symlink(&actual, &linked).unwrap();
    assert!(OpenCodeHostConfig::new(
        PathBuf::from("/bin/true"),
        linked,
        Duration::from_secs(1),
        OpenCodeGrammar {
            digest: "g".into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into()
        }
    )
    .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
