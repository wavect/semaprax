//! `model.generate` through a real adapter process: the fake-model fixture is
//! spawned by the host, receives a real envelope built by
//! `HostModel::request_payload`, and its reply is interpreted by
//! `HostModel::interpret` (the code `HostModel::propose` runs after the
//! invoke). `propose` itself also checks an adopted-provider grant against
//! local state, which needs the adopt/trust CLI flow and so is not driven here.

use super::*;
use crate::receipt::{Effort, Finish, GenerationControls};
use crate::workflow::lineage::Lineage;
use crate::workflow::stages::{HostModel, ProposalRequest};

fn fake_model(fx: &Fx) -> LaunchSpec {
    let src =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workflow/adapters/fake-model");
    let dir = fx.root.join("fake-model");
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["adapter.py", "harness-provider.json"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    std::fs::copy(
        examples().join("../sdk/python/semaprax_harness_adapter.py"),
        dir.join("semaprax_harness_adapter.py"),
    )
    .unwrap();
    spec_in(
        &dir,
        descriptor_from(&dir, |_| {}),
        fx,
        &[],
        IsolationRequest::None,
    )
}

fn drive(
    goal: &str,
    controls: GenerationControls,
) -> (
    Result<Vec<u8>, crate::workflow::stages::StageFailure>,
    crate::receipt::ProposalReceipt,
) {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, fake_model(&fx)).unwrap();
    let lineage = Lineage::new(
        ProjectBinding {
            id: PROJECT.into(),
            worktree: "w1".into(),
            revision: "r1".into(),
        },
        "sha256:0000000000000000000000000000000000000000000000000000000000000001",
        "task",
    );
    let req = ProposalRequest {
        lineage: &lineage,
        prompt: json!({"goal": goal}),
        model: "m-a".into(),
        controls,
    };
    let envelope = request(
        CapabilityKind::ModelGenerate,
        "generate",
        HostModel::request_payload(&req),
        &lineage.next_invocation(),
    );
    let out = h.invoke(
        &envelope,
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    HostModel::interpret(out, &req)
}

#[test]
fn fake_model_receipt_survives_a_real_adapter_round_trip_and_reports_the_cap() {
    let controls = GenerationControls {
        max_output_tokens: Some(321),
        reasoning: Some(Effort::Low),
        strict: false,
    };
    let (bytes, r) = drive("MODE:receipt MODE:claims", controls);
    let bytes = bytes.expect("proposal bytes");
    assert!(
        String::from_utf8_lossy(&bytes).contains("tests_passed"),
        "the forged claim is in the bytes"
    );
    assert!(r.unavailable.is_none());
    assert_eq!(
        (r.usage.uncached_input, r.usage.cache_read, r.usage.output),
        (Some(40), Some(60), Some(50))
    );
    assert_eq!(r.usage.input_total, Some(100));
    assert_eq!(r.model.as_deref(), Some("fake-model-1"));
    assert_eq!(r.finish, Finish::Complete);
    assert_eq!(
        r.max_output_tokens.effective,
        Some(321),
        "the adapter saw the transmitted cap"
    );
    // The adapter said nothing about reasoning although it was requested.
    assert_eq!(r.reasoning.status.as_str(), "unreported");
}

#[test]
fn fake_model_without_a_receipt_is_explicitly_unavailable_and_truncation_is_not_a_proposal() {
    let (bytes, r) = drive("plain", GenerationControls::default());
    assert!(bytes.is_ok());
    assert_eq!(r.unavailable, Some("adapter_reported_no_receipt"));
    let controls = GenerationControls {
        max_output_tokens: Some(7),
        ..Default::default()
    };
    let (bytes, r) = drive("MODE:truncate", controls);
    assert!(bytes.is_err(), "a length-limited reply is refused");
    assert_eq!(r.finish, Finish::LengthLimited);
    assert_eq!(r.usage.output, Some(7));
    assert_eq!(r.usage.uncached_input, None);
}
