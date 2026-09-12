use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct Runner {
    events: Vec<u8>,
    export: Vec<u8>,
    failure: Option<OpenCodeRunnerFailure>,
    calls: usize,
}

impl OpenCodeRunner for Runner {
    fn run(&mut self, _: &OpenCodeHostConfig, _: &str) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        self.calls += 1;
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        Ok(self.events.clone())
    }

    fn export(
        &mut self,
        _: &OpenCodeHostConfig,
        _: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        Ok(self.export.clone())
    }
}

fn config(digest: &str) -> OpenCodeHostConfig {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let sandbox = std::env::temp_dir().join(format!(
        "semaprax-opencode-bounds-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
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

fn request(max_response_bytes: usize) -> ModelInvocationRequest {
    ModelInvocationRequest {
        turn: 0,
        task: b"task".to_vec(),
        observation: b"observation".to_vec(),
        proposal_grammar_digest: "sha256:grammar".into(),
        deployment_binding: "sha256:deployment".into(),
        max_response_bytes,
        effective_budget: 1,
    }
}

fn fixture(prompt: &str, answer: &str) -> (Vec<u8>, Vec<u8>) {
    let mut export: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../scripts/fixtures/opencode-provider-smoke-v1/session.json"
    ))
    .unwrap();
    export["messages"][0]["parts"][0]["text"] = serde_json::json!(prompt);
    export["messages"][1]["parts"][2]["text"] = serde_json::json!(answer);
    let parts = export["messages"][1]["parts"].as_array().unwrap();
    let events = [
        ("step_start", &parts[0]),
        ("text", &parts[2]),
        ("step_finish", &parts[3]),
    ]
    .iter()
    .map(|(kind, part)| {
        serde_json::json!({"type":kind,"sessionID":"ses_fixture","part":part}).to_string()
    })
    .collect::<Vec<_>>()
    .join("\n")
    .into_bytes();
    (events, serde_json::to_vec(&export).unwrap())
}

fn valid_runner(prompt: &str, answer: &str) -> Runner {
    let (events, export) = fixture(prompt, answer);
    Runner {
        events,
        export,
        failure: None,
        calls: 0,
    }
}

#[test]
fn oversized_wire_prompt_refuses_before_runner() {
    let mut req = request(128);
    req.task = vec![b'x'; MAX_PROMPT_BYTES];
    let runner = Runner {
        events: Vec::new(),
        export: Vec::new(),
        failure: None,
        calls: 0,
    };
    let mut handler = OpenCodeModelHandler::new(config("sha256:grammar"), runner);
    assert_eq!(
        handler.invoke(&ModelInvokeCapability::grant("bounds"), &req),
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::Refused,
            attempted_bytes: 0,
        }
    );
    assert_eq!(handler.runner.calls, 0);
}

#[test]
fn response_exactly_at_cap_settles_and_one_over_fails() {
    let cap = 32;
    let answer = "x".repeat(cap);
    let req = request(cap);
    let prompt = wire_prompt(
        &req,
        &OpenCodeGrammar {
            digest: "sha256:grammar".into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    let mut exact =
        OpenCodeModelHandler::new(config("sha256:grammar"), valid_runner(&prompt, &answer));
    assert_eq!(
        exact.invoke(&ModelInvokeCapability::grant("bounds"), &req),
        ModelInvocationOutcome::Settled(answer.clone().into_bytes())
    );

    let over = format!("{answer}x");
    let mut too_large =
        OpenCodeModelHandler::new(config("sha256:grammar"), valid_runner(&prompt, &over));
    assert_eq!(
        too_large.invoke(&ModelInvokeCapability::grant("bounds"), &req),
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::MalformedResponse,
            attempted_bytes: req.max_response_bytes,
        }
    );
}

#[test]
fn failed_followup_clears_previous_receipt() {
    let req = request(128);
    let grammar = OpenCodeGrammar {
        digest: "sha256:grammar".into(),
        canonical_schema: "{}".into(),
        provider_schema: "{}".into(),
    };
    let prompt = wire_prompt(&req, &grammar).unwrap();
    let (events, export) = fixture(&prompt, "ok");
    struct Twice {
        events: Vec<u8>,
        export: Vec<u8>,
        calls: usize,
    }
    impl OpenCodeRunner for Twice {
        fn run(
            &mut self,
            _: &OpenCodeHostConfig,
            _: &str,
        ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
            self.calls += 1;
            if self.calls == 1 {
                Ok(self.events.clone())
            } else {
                Err(OpenCodeRunnerFailure::Provider)
            }
        }
        fn export(
            &mut self,
            _: &OpenCodeHostConfig,
            _: &str,
        ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
            Ok(self.export.clone())
        }
    }
    let mut handler = OpenCodeModelHandler::new(
        config("sha256:grammar"),
        Twice {
            events,
            export,
            calls: 0,
        },
    );
    assert!(matches!(
        handler.invoke(&ModelInvokeCapability::grant("bounds"), &req),
        ModelInvocationOutcome::Settled(_)
    ));
    assert!(handler.last_receipt.is_some());
    assert!(matches!(
        handler.invoke(&ModelInvokeCapability::grant("bounds"), &req),
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::ProviderError,
            ..
        }
    ));
    assert!(handler.last_receipt.is_none());
}

#[test]
fn timeout_and_capacity_are_closed_failures() {
    for (runner_failure, model_failure) in [
        (OpenCodeRunnerFailure::Timeout, ModelFailure::Timeout),
        (
            OpenCodeRunnerFailure::Capacity,
            ModelFailure::CapacityExceeded,
        ),
    ] {
        let req = request(128);
        let mut handler = OpenCodeModelHandler::new(
            config("sha256:grammar"),
            Runner {
                events: Vec::new(),
                export: Vec::new(),
                failure: Some(runner_failure),
                calls: 0,
            },
        );
        assert_eq!(
            handler.invoke(&ModelInvokeCapability::grant("bounds"), &req),
            ModelInvocationOutcome::Failed {
                failure: model_failure,
                attempted_bytes: 0
            }
        );
    }
}

#[test]
fn policy_refusal_does_not_return_policy_contents() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let sandbox = std::env::temp_dir().join(format!(
        "semaprax-opencode-policy-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir(&sandbox).unwrap();
    let sentinel = "SECRET_POLICY_SENTINEL";
    std::fs::write(sandbox.join("opencode.json"), sentinel).unwrap();
    let cfg = OpenCodeHostConfig::new(
        PathBuf::from("/bin/true"),
        sandbox.clone(),
        Duration::from_secs(1),
        OpenCodeGrammar {
            digest: "sha256:grammar".into(),
            canonical_schema: "{}".into(),
            provider_schema: "{}".into(),
        },
    )
    .unwrap();
    let result = OpenCodeRunner::run(&mut ProcessOpenCodeRunner, &cfg, "prompt");
    assert_eq!(result, Err(OpenCodeRunnerFailure::Refused));
    assert!(!format!("{result:?}").contains(sentinel));
    std::fs::remove_dir_all(sandbox).unwrap();
}
