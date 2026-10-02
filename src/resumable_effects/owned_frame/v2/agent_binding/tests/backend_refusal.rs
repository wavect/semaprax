//! The checked owned Agent wait remains interpreter-only at backend admission.
use super::*;

#[test]
fn owned_agent_wait_profile_refuses_native_c11_and_core_wasm_before_artifacts() {
    // Reuse the source Agent association and owned State/Copy Observation/
    // Proposal fixture that the public journal binding admits, not a scalar
    // resumable stand-in or a fabricated HIR yield.
    let binding = bind(&source()).unwrap();
    let helper = binding.helper();
    assert_eq!(binding.agent().as_str(), "fixture.agent");
    assert_eq!(helper.function().id.as_str(), "fixture.agent.fn.park");
    assert_eq!(binding.signature()["parameters"][0]["mode"], "own");
    assert_eq!(binding.signature()["parameters"][1]["mode"], "copy");
    assert!(!helper.liveness().leaves.is_empty());
    assert!(helper.liveness().completion_cleanup.is_empty());

    // These pure emission APIs return no C source/module bytes on refusal, so
    // no native compiler, Wasm engine, model adapter or target host can run.
    let native = crate::codegen::emit_hir_c(helper.program()).unwrap_err();
    assert_eq!(native.code, "SPX-B116", "{native:?}");
    let wasm = crate::wasm::emit_resolved_module(helper.program()).unwrap_err();
    assert_eq!(wasm.code, "SPX-W126", "{wasm:?}");

    // The scalar resumable artifact projection must not bypass that boundary
    // by accepting this two-argument owned-record helper.
    use crate::resumable_effects::target::{prepare_target_profile, ResumableArtifactTarget};
    for target in [
        ResumableArtifactTarget::NativeC11,
        ResumableArtifactTarget::CoreWasm,
    ] {
        let refused =
            prepare_target_profile(helper.program(), helper.function().id.as_str(), target)
                .unwrap_err();
        assert_eq!(refused.code, "SPX-H006", "{refused:?}");
    }
}
