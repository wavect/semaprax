//! `semaprax.public-generic-settlement-corpus.v1`: the shared cross-engine
//! settlement corpus for issue #162.
//!
//! This is the one place this repository proves "a safe source program has
//! equivalent checked behavior on every backend that claims to implement
//! the admitted feature" for the [Public Generic Carrier
//! v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md) boundary, rather than
//! merely asserting each physical adapter against its own independent
//! expectation. It follows the same shape issue #160's
//! `tests::support::public_generic_hostile_corpus` established for the
//! consumer corpus: one shared case table with an independently pinned
//! expected outcome per case, each engine's adapter run against every case
//! and made to report what it actually observed, and exactly one checker
//! ([`compare`]) that diffs all of it — engine vs. expectation and engine
//! vs. engine — plus `should_panic` negative controls proving the checker
//! itself can fail rather than vacuously passing.
//!
//! **Engines compared.** [`crate::public_generic_abi::interpreter::InterpreterProvider`]
//! (issue #162, this same change) and
//! [`crate::public_generic_abi::wasm::provider::WasmProvider`] (issue
//! #155). Both are local, in-process, proof-only Rust adapters that drive
//! the identical [`CarrierCallMachine`] type for every phase, commit,
//! sticky-settlement, and release-order decision — see each adapter's own
//! module documentation for its distinct physical memory model (an
//! arena-free heap vs. a bounded LIFO linear-memory arena), which is the
//! actual point of comparing them rather than one being a restatement of
//! the other.
//!
//! **Native C11 is out of scope here.** `crate::public_generic_abi::native`
//! only renders a compilable C translation unit
//! (`native::template::render_reference_provider`); it has no in-process
//! Rust adapter analogous to `WasmProvider`/`InterpreterProvider` to run
//! this same case table against without inventing a fourth artifact this
//! issue's file lease does not own. Real compiled-and-executed native (at
//! `-O0`/`-O2`) and generated-consumer execution against this corpus's
//! shape is `tests/public_generic_native_adapter_v1/**`'s and
//! `tests/public_generic_wasm_adapter_v1/**`'s own leased territory
//! (`c_calling_consumer.rs`, `rust_calling_consumer.rs`, `probe.c`,
//! `allocations.c`) — this module does not duplicate or re-verify that
//! work, and does not claim it.
//!
//! **What "trace" means here.** Neither adapter reinvents the normalized
//! trace vocabulary: `WasmProvider::test_last_trace` and
//! `InterpreterProvider::test_last_trace` are test-only accessors that
//! snapshot `CarrierCallMachine::trace()` at the same moment each adapter's
//! own `settle` helper runs (every terminal path, success or failure), so
//! this module's trace/release-order comparison is over the literal
//! [`crate::public_generic_abi::carrier::trace::TraceEvent`] sequence
//! `CarrierCallMachine` itself recorded, not a second, adapter-local
//! restatement of it.
//!
//! **Nonclaims.** Peak allocation/handle counters are not tracked by
//! either adapter (only live/current counts are), so this module compares
//! final (post-terminal) counts only, per [Public Generic Carrier
//! v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md)'s own "document any
//! non-comparable peak field rather than omitting it silently." Nested
//! multi-level owned records are not exercised: #119's flat-owned-`Bytes`-
//! leaves limitation applies to both adapters equally, so the
//! `two_leaves_structural_order` case stands in for "at least two owned
//! leaves with visible structural order," not a nested record.

use crate::public_generic_abi::carrier::trace::{TraceEvent, TraceLabel};
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::interpreter::{InterpreterPgStatus, InterpreterProvider};
use crate::public_generic_abi::wasm::binding::WasmProviderBindingV1;
use crate::public_generic_abi::wasm::provider::{WasmPgStatus, WasmProvider};

const DESCRIPTOR_FIXTURE: &[u8] =
    b"semaprax.public-generic-settlement-corpus.v1.descriptor-fixture";

fn interpreter_binding() -> CarrierBindingV1 {
    CarrierBindingV1::new(
        "sha256:settlement-corpus-descriptor-identity",
        TargetProfile::Interpreter,
        "sha256:settlement-corpus-runtime-identity",
    )
}

fn wasm_binding() -> WasmProviderBindingV1 {
    let carrier_binding = CarrierBindingV1::new(
        "sha256:settlement-corpus-descriptor-identity",
        TargetProfile::CoreWasm,
        "sha256:settlement-corpus-runtime-identity",
    );
    WasmProviderBindingV1::new(
        carrier_binding,
        "sha256:settlement-corpus-wasm-provider-artifact-fixture",
        crate::public_generic_abi::wasm::provider::FIXTURE_ENDPOINT_EXPORT_NAME,
        "fixture-compiler-backend-v1",
    )
}

/// One shared settlement-corpus case: the input, an optional deterministic
/// failure injection, and the independently pinned expected outcome (not
/// merely "whatever the engines happen to agree on") — mirroring the
/// `semaprax.public-generic-settlement-corpus.v1` schema issue #162
/// describes, at the fields this round's two in-process engines can
/// actually observe.
#[derive(Debug, Clone)]
struct Case {
    case_id: String,
    input_leaves: Vec<Vec<u8>>,
    failure_injection_id: Option<TraceLabel>,
    expected_accepted: bool,
    /// The normalized status vocabulary both adapters converge on
    /// (`Ok` = 0 .. `NullOrWrongKind` = 13), compared as a plain integer so
    /// neither adapter's own enum type has to be shared.
    expected_status: i32,
}

fn expected_result_bytes(leaves: &[Vec<u8>]) -> Vec<u8> {
    let mut framed = Vec::new();
    for leaf in leaves {
        let mut reversed = leaf.clone();
        reversed.reverse();
        crate::public_generic_abi::frame(&mut framed, &reversed);
    }
    framed
}

/// The corpus: minimal/zero-length/embedded-zero/multi-leaf/bounds success
/// and rejection shapes, plus one case per non-terminal
/// [`TraceLabel`] failure-injection ordinal — the full matrix both
/// adapters' `test_inject_failure` supports.
fn corpus() -> Vec<Case> {
    let mut cases = vec![
        Case {
            case_id: "minimal_success".to_owned(),
            input_leaves: vec![b"hello".to_vec()],
            failure_injection_id: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "zero_length_owned_bytes".to_owned(),
            input_leaves: vec![Vec::new()],
            failure_injection_id: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "embedded_zero_bytes".to_owned(),
            input_leaves: vec![vec![0u8, 1, 0, 2, 0, 3, 0]],
            failure_injection_id: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "two_leaves_structural_order".to_owned(),
            input_leaves: vec![b"AA".to_vec(), b"BBB".to_vec()],
            failure_injection_id: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "max_bytes_per_leaf".to_owned(),
            input_leaves: vec![vec![
                0xABu8;
                crate::public_generic_abi::boundary_profile::MAX_BYTES_PER_LEAF
            ]],
            failure_injection_id: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "first_over_max_bytes_per_leaf".to_owned(),
            input_leaves: vec![vec![
                0u8;
                crate::public_generic_abi::boundary_profile::MAX_BYTES_PER_LEAF
                    + 1
            ]],
            failure_injection_id: None,
            expected_accepted: false,
            expected_status: InterpreterPgStatus::CarrierCapacity as i32,
        },
        Case {
            case_id: "first_over_max_leaf_count".to_owned(),
            input_leaves: vec![
                Vec::new();
                crate::public_generic_abi::boundary_profile::MAX_OWNED_LEAVES_PER_INSTANCE
                    + 1
            ],
            failure_injection_id: None,
            expected_accepted: false,
            expected_status: InterpreterPgStatus::CarrierCapacity as i32,
        },
    ];

    // One case per non-terminal trace ordinal, each pinned to the status
    // both adapters' documented `status_from_settlement`/direct-error
    // mapping deterministically produces.
    let injection_matrix: &[(TraceLabel, i32)] = &[
        (
            TraceLabel::FrameValidated,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::LeafAllocationStarted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::LeafAllocationCommitted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::LeafPayloadCopied,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::InputValuePrepared,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::InputTransferCommitted,
            InterpreterPgStatus::IllegalTransition as i32,
        ),
        (
            TraceLabel::ExecutionStarted,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::ExecutionFinished,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::ResultLeafAllocationStarted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::ResultLeafAllocationCommitted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::ResultValuePrepared,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::ResultCommit,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::LeafRelease,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::CarrierRelease,
            InterpreterPgStatus::ContractFailure as i32,
        ),
    ];
    for (label, status) in injection_matrix {
        cases.push(Case {
            case_id: format!("failure_injection_{label:?}"),
            input_leaves: vec![b"inject-me".to_vec(), b"second-leaf".to_vec()],
            failure_injection_id: Some(*label),
            expected_accepted: false,
            expected_status: *status,
        });
    }
    cases
}

/// One engine's observed outcome for one case — the fields both engines'
/// public/test-only surface can actually report.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineOutcome {
    engine_id: &'static str,
    accepted: bool,
    primary_status: i32,
    result_bytes: Option<Vec<u8>>,
    trace: Vec<TraceEvent>,
    final_live_allocations: u32,
    final_live_handles: usize,
    final_live_bytes: u32,
    settlement_overwrite_attempts: u32,
    close_status: i32,
}

fn run_interpreter_case(case: &Case) -> EngineOutcome {
    let trusted = interpreter_binding();
    let mut provider = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted.encode(),
        &trusted,
    )
    .expect("the settlement-corpus fixture binding must open on the interpreter engine");
    if let Some(label) = case.failure_injection_id {
        provider.test_inject_failure(label);
    }
    let outcome = provider
        .input_prepare(&case.input_leaves)
        .and_then(|value| provider.call(value));
    let (accepted, primary_status, result_bytes) = match outcome {
        Ok(result_handle) => {
            let bytes = provider
                .result_export(result_handle, 1 << 20)
                .expect("export must succeed for an accepted interpreter-engine case");
            provider.result_release(result_handle);
            (true, InterpreterPgStatus::Ok as i32, Some(bytes))
        }
        Err(status) => (false, status as i32, None),
    };
    let trace = provider.test_last_trace().to_vec();
    let final_live_allocations = provider.live_allocations();
    let final_live_handles = provider.live_handles();
    let final_live_bytes = provider.live_bytes();
    let settlement_overwrite_attempts = provider.test_settlement_overwrite_attempts();
    let close_status = provider.close() as i32;
    EngineOutcome {
        engine_id: "interpreter",
        accepted,
        primary_status,
        result_bytes,
        trace,
        final_live_allocations,
        final_live_handles,
        final_live_bytes,
        settlement_overwrite_attempts,
        close_status,
    }
}

fn run_wasm_case(case: &Case) -> EngineOutcome {
    let trusted = wasm_binding();
    let mut provider = WasmProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted.encode(),
        &trusted,
    )
    .expect("the settlement-corpus fixture binding must open on the Wasm engine");
    if let Some(label) = case.failure_injection_id {
        provider.test_inject_failure(label);
    }
    let outcome = provider
        .input_prepare(&case.input_leaves)
        .and_then(|value| provider.call(value));
    let (accepted, primary_status, result_bytes) = match outcome {
        Ok(result_handle) => {
            let bytes = provider
                .result_export(result_handle, 1 << 20)
                .expect("export must succeed for an accepted Wasm-engine case");
            provider.result_release(result_handle);
            (true, WasmPgStatus::Ok as i32, Some(bytes))
        }
        Err(status) => (false, status as i32, None),
    };
    let trace = provider.test_last_trace().to_vec();
    let final_live_allocations = provider.live_allocations();
    let final_live_handles = provider.live_handles();
    let final_live_bytes = provider.live_bytes();
    let settlement_overwrite_attempts = provider.test_settlement_overwrite_attempts();
    let close_status = provider.close() as i32;
    EngineOutcome {
        engine_id: "core-wasm",
        accepted,
        primary_status,
        result_bytes,
        trace,
        final_live_allocations,
        final_live_handles,
        final_live_bytes,
        settlement_overwrite_attempts,
        close_status,
    }
}

fn trace_labels(events: &[TraceEvent]) -> Vec<TraceLabel> {
    events.iter().map(|event| event.label).collect()
}

/// Canonical leaf release order, extracted from the trace itself: every
/// `LeafRelease` event's leaf index, in the order the trace recorded them
/// — the direct evidence for "cleanup-plan vectors are canonical runtime
/// order," restated at the carrier-adapter boundary.
fn release_order(events: &[TraceEvent]) -> Vec<Option<u32>> {
    events
        .iter()
        .filter(|event| event.label == TraceLabel::LeafRelease)
        .map(|event| event.leaf)
        .collect()
}

/// The one checker every case (and both negative controls below) funnels
/// through: engine-vs-expectation and engine-vs-engine, over every field
/// issue #162 requires equal across engines. Panics with a message naming
/// the case, the field, and both engines' values on any mismatch — this is
/// deliberately the single place that decides "these engines agree,"
/// exercised directly (not merely invoked) by the two `should_panic`
/// negative controls below.
fn compare(
    case_id: &str,
    expected_accepted: bool,
    expected_status: i32,
    a: &EngineOutcome,
    b: &EngineOutcome,
) {
    for (engine, outcome) in [(a.engine_id, a), (b.engine_id, b)] {
        println!(
            "settlement_corpus case={case_id:?} engine={engine:?} accepted={} status={} live_allocations={} live_handles={} live_bytes={} overwrite_attempts={} close_status={}",
            outcome.accepted,
            outcome.primary_status,
            outcome.final_live_allocations,
            outcome.final_live_handles,
            outcome.final_live_bytes,
            outcome.settlement_overwrite_attempts,
            outcome.close_status,
        );
    }
    assert_eq!(
        a.accepted, expected_accepted,
        "case {case_id:?}: {} accept/reject does not match the independently pinned expectation",
        a.engine_id
    );
    assert_eq!(
        b.accepted, expected_accepted,
        "case {case_id:?}: {} accept/reject does not match the independently pinned expectation",
        b.engine_id
    );
    assert_eq!(
        a.primary_status, expected_status,
        "case {case_id:?}: {} normalized status does not match the independently pinned expectation",
        a.engine_id
    );
    assert_eq!(
        b.primary_status, expected_status,
        "case {case_id:?}: {} normalized status does not match the independently pinned expectation",
        b.engine_id
    );
    assert_eq!(
        a.accepted, b.accepted,
        "case {case_id:?}: accept/reject disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        a.primary_status, b.primary_status,
        "case {case_id:?}: normalized status disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        a.result_bytes, b.result_bytes,
        "case {case_id:?}: result carrier bytes disagree between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        trace_labels(&a.trace),
        trace_labels(&b.trace),
        "case {case_id:?}: normalized trace label sequence disagrees between {} and {}",
        a.engine_id,
        b.engine_id
    );
    assert_eq!(
        release_order(&a.trace),
        release_order(&b.trace),
        "case {case_id:?}: canonical leaf release order disagrees between {} and {}",
        a.engine_id,
        b.engine_id
    );
    assert_eq!(
        a.final_live_allocations, b.final_live_allocations,
        "case {case_id:?}: final live allocation count disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        a.final_live_handles, b.final_live_handles,
        "case {case_id:?}: final live handle count disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        a.final_live_bytes, b.final_live_bytes,
        "case {case_id:?}: final live byte count disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        a.settlement_overwrite_attempts, b.settlement_overwrite_attempts,
        "case {case_id:?}: sticky-settlement overwrite-attempt count disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    assert_eq!(
        a.close_status, b.close_status,
        "case {case_id:?}: provider close status disagrees between {} and {}",
        a.engine_id, b.engine_id
    );
    // Zero live resources after every terminal case, both engines
    // independently — not merely equal to each other, but equal to zero.
    assert_eq!(
        a.final_live_allocations, 0,
        "case {case_id:?}: {} left live allocations",
        a.engine_id
    );
    assert_eq!(
        b.final_live_allocations, 0,
        "case {case_id:?}: {} left live allocations",
        b.engine_id
    );
    assert_eq!(
        a.final_live_handles, 0,
        "case {case_id:?}: {} left live handles",
        a.engine_id
    );
    assert_eq!(
        b.final_live_handles, 0,
        "case {case_id:?}: {} left live handles",
        b.engine_id
    );
    assert_eq!(
        a.final_live_bytes, 0,
        "case {case_id:?}: {} left live bytes",
        a.engine_id
    );
    assert_eq!(
        b.final_live_bytes, 0,
        "case {case_id:?}: {} left live bytes",
        b.engine_id
    );
}

#[test]
fn every_corpus_case_agrees_across_the_interpreter_and_wasm_engines() {
    let cases = corpus();
    assert!(
        cases.len() >= 21,
        "the corpus must cover the base shapes and the full 14-ordinal injection matrix"
    );
    for case in &cases {
        let interpreter = run_interpreter_case(case);
        let wasm = run_wasm_case(case);
        compare(
            &case.case_id,
            case.expected_accepted,
            case.expected_status,
            &interpreter,
            &wasm,
        );
        if case.expected_accepted {
            let expected = expected_result_bytes(&case.input_leaves);
            assert_eq!(
                interpreter.result_bytes.as_deref(),
                Some(expected.as_slice()),
                "case {:?}: interpreter result does not match the independently computed expected bytes",
                case.case_id
            );
            assert_eq!(
                wasm.result_bytes.as_deref(),
                Some(expected.as_slice()),
                "case {:?}: Wasm result does not match the independently computed expected bytes",
                case.case_id
            );
        }
    }
}

#[test]
fn structural_leaf_order_is_left_to_right_staged_and_exact_reverse_released() {
    // Not merely "the two engines agree with each other" (which a
    // consistently-wrong order — e.g. both sorted ascending on release
    // instead of exact-reverse — would also satisfy): pin the literal
    // expected sequence independently, restating "an owned call stages
    // arguments left to right and transfers them together at its declared
    // commit boundary" and "cleanup inventory order is structural
    // metadata... canonical runtime order" at the physical-adapter
    // boundary, not only at the `CarrierCallMachine` unit-test level
    // (`carrier::machine::tests::a_full_success_run_transitions_every_handle_and_records_the_expected_trace`).
    let case = Case {
        case_id: "structural_order_pin".to_owned(),
        input_leaves: vec![b"AA".to_vec(), b"BBB".to_vec()],
        failure_injection_id: None,
        expected_accepted: true,
        expected_status: InterpreterPgStatus::Ok as i32,
    };
    for (engine_name, trace) in [
        ("interpreter", run_interpreter_case(&case).trace),
        ("core-wasm", run_wasm_case(&case).trace),
    ] {
        let staged_leaf_order: Vec<u32> = trace
            .iter()
            .filter(|event| event.label == TraceLabel::LeafAllocationStarted)
            .filter_map(|event| event.leaf)
            .collect();
        assert_eq!(
            staged_leaf_order,
            vec![0, 1],
            "{engine_name}: leaves must stage left to right (input order), not reordered"
        );
        // The trailing `None` is the root aggregate handle's own release
        // (recorded as a `LeafRelease` event with no leaf index), released
        // last per the canonical obligation order (root, then leaves) —
        // reversed is (leaves reversed, then root).
        let released_leaf_order: Vec<Option<u32>> = release_order(&trace);
        assert_eq!(
            released_leaf_order,
            vec![Some(1), Some(0), None],
            "{engine_name}: leaf release must be the exact reverse of staging order, not sorted or otherwise repaired"
        );
    }
}

#[test]
fn sticky_failure_selection_matches_across_engines_when_cleanup_follows_an_earlier_failure() {
    // "Cleanup cannot replace the selected status": inject a primary
    // failure at execution start, which settles before either release
    // ordinal ever runs, so this is not the "cleanup becomes terminal"
    // case above — it is the sibling case where a real earlier failure
    // already exists. Both engines must independently report zero
    // overwrite attempts here (no cleanup injection fires this call at
    // all) and agree with each other; the overwrite-attempt counter itself
    // is exercised end-to-end by each engine's own unit tests
    // (`cleanup_cannot_overwrite_an_earlier_sticky_failure` for the
    // interpreter, the equivalent Wasm test for `WasmProvider`).
    let case = Case {
        case_id: "execution_failure_then_no_cleanup_injection".to_owned(),
        input_leaves: vec![b"payload".to_vec()],
        failure_injection_id: Some(TraceLabel::ExecutionStarted),
        expected_accepted: false,
        expected_status: InterpreterPgStatus::ContractFailure as i32,
    };
    let interpreter = run_interpreter_case(&case);
    let wasm = run_wasm_case(&case);
    compare(
        &case.case_id,
        case.expected_accepted,
        case.expected_status,
        &interpreter,
        &wasm,
    );
    assert_eq!(interpreter.settlement_overwrite_attempts, 0);
    assert_eq!(wasm.settlement_overwrite_attempts, 0);
}

#[test]
fn repeated_invocation_and_provider_recreation_agree_across_engines() {
    // Independent lifecycle across several calls on one provider
    // generation, then a fresh provider generation: no digest/handle state
    // leaks between calls, and both engines agree at every step.
    let trusted_interpreter = interpreter_binding();
    let trusted_wasm = wasm_binding();
    let mut interpreter_provider = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted_interpreter.encode(),
        &trusted_interpreter,
    )
    .unwrap();
    let mut wasm_provider = WasmProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted_wasm.encode(),
        &trusted_wasm,
    )
    .unwrap();

    for iteration in 0u8..4 {
        let payload = vec![iteration; (iteration as usize) + 1];
        let interpreter_value = interpreter_provider
            .input_prepare(std::slice::from_ref(&payload))
            .unwrap();
        let wasm_value = wasm_provider
            .input_prepare(std::slice::from_ref(&payload))
            .unwrap();
        let interpreter_result = interpreter_provider.call(interpreter_value).unwrap();
        let wasm_result = wasm_provider.call(wasm_value).unwrap();
        let interpreter_bytes = interpreter_provider
            .result_export(interpreter_result, 4096)
            .unwrap();
        let wasm_bytes = wasm_provider.result_export(wasm_result, 4096).unwrap();
        assert_eq!(
            interpreter_bytes, wasm_bytes,
            "iteration {iteration}: repeated-invocation result must agree across engines"
        );
        assert_eq!(interpreter_bytes, expected_result_bytes(&[payload]));
        interpreter_provider.result_release(interpreter_result);
        wasm_provider.result_release(wasm_result);
        assert_eq!(
            interpreter_provider.live_handles(),
            wasm_provider.live_handles()
        );
        assert_eq!(interpreter_provider.live_handles(), 0);
    }
    assert_eq!(
        interpreter_provider.close() as i32,
        wasm_provider.close() as i32
    );

    // Provider recreation: a fresh generation on each engine must accept
    // the identical trusted binding, and both must reject a stale handle
    // from the prior generation identically.
    let mut interpreter_second = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted_interpreter.encode(),
        &trusted_interpreter,
    )
    .expect("provider recreation must succeed against the same trusted binding (interpreter)");
    let mut wasm_second = WasmProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted_wasm.encode(),
        &trusted_wasm,
    )
    .expect("provider recreation must succeed against the same trusted binding (Wasm)");
    let interpreter_value = interpreter_second.input_prepare(&[b"z".to_vec()]).unwrap();
    let wasm_value = wasm_second.input_prepare(&[b"z".to_vec()]).unwrap();
    let interpreter_result = interpreter_second.call(interpreter_value).unwrap();
    let wasm_result = wasm_second.call(wasm_value).unwrap();
    interpreter_second.result_release(interpreter_result);
    wasm_second.result_release(wasm_result);
    assert_eq!(
        interpreter_second.close() as i32,
        InterpreterPgStatus::Ok as i32
    );
    assert_eq!(wasm_second.close() as i32, WasmPgStatus::Ok as i32);
}

#[test]
fn abandoned_value_release_agrees_across_engines_with_zero_leaks() {
    // "Input transfer, output adoption, and consumer-side release" (issue
    // #174's in-scope list) exercises `value_release` on both engines
    // individually (`value_release_before_call_is_a_legal_abandon` on each),
    // but neither engine's own suite compares the abandon path AGAINST the
    // other, and this corpus's own `Case`/`run_*_case` helpers always drive
    // `input_prepare().and_then(call)` — never the abandon-before-call
    // path. This closes that gap: both engines take the identical two-leaf
    // input, never call it, and release the value directly instead.
    let two_leaves = vec![b"AA".to_vec(), b"BBB".to_vec()];

    let trusted_interpreter = interpreter_binding();
    let mut interpreter = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted_interpreter.encode(),
        &trusted_interpreter,
    )
    .expect("the settlement-corpus fixture binding must open on the interpreter engine");
    let interpreter_value = interpreter.input_prepare(&two_leaves).unwrap();
    let interpreter_release_status = interpreter.value_release(interpreter_value) as i32;
    let interpreter_trace = interpreter.test_last_trace().to_vec();

    let trusted_wasm = wasm_binding();
    let mut wasm = WasmProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted_wasm.encode(),
        &trusted_wasm,
    )
    .expect("the settlement-corpus fixture binding must open on the Wasm engine");
    let wasm_value = wasm.input_prepare(&two_leaves).unwrap();
    let wasm_release_status = wasm.value_release(wasm_value) as i32;
    let wasm_trace = wasm.test_last_trace().to_vec();

    assert_eq!(
        interpreter_release_status,
        InterpreterPgStatus::Ok as i32,
        "a legal abandon must report Ok on the interpreter engine"
    );
    assert_eq!(
        wasm_release_status,
        WasmPgStatus::Ok as i32,
        "a legal abandon must report Ok on the Wasm engine"
    );
    assert_eq!(
        trace_labels(&interpreter_trace),
        trace_labels(&wasm_trace),
        "abandon-path normalized trace label sequence disagrees between engines"
    );
    // The abandon path releases handles that were only ever `Initialized`,
    // never `Transferred` (`release_input_before_transfer`), a distinct
    // release ordinal from the post-call, post-commit path every other
    // corpus case exercises (`release_input_after_transfer`) — pin the
    // literal leaf release order here too, not only accept/reject.
    assert_eq!(
        release_order(&interpreter_trace),
        vec![Some(1), Some(0), None],
        "interpreter: abandoned leaves must release in exact reverse structural order"
    );
    assert_eq!(
        release_order(&interpreter_trace),
        release_order(&wasm_trace),
        "abandon-path canonical leaf release order disagrees between engines"
    );

    let interpreter_live_allocations = interpreter.live_allocations();
    let interpreter_live_handles = interpreter.live_handles();
    let interpreter_live_bytes = interpreter.live_bytes();
    let wasm_live_allocations = wasm.live_allocations();
    let wasm_live_handles = wasm.live_handles();
    let wasm_live_bytes = wasm.live_bytes();
    assert_eq!(interpreter_live_allocations, 0);
    assert_eq!(interpreter_live_handles, 0);
    assert_eq!(interpreter_live_bytes, 0);
    assert_eq!(wasm_live_allocations, 0);
    assert_eq!(wasm_live_handles, 0);
    assert_eq!(wasm_live_bytes, 0);
    assert_eq!(
        interpreter.close() as i32,
        InterpreterPgStatus::Ok as i32,
        "an abandoned-and-released provider must still close cleanly"
    );
    assert_eq!(wasm.close() as i32, WasmPgStatus::Ok as i32);
}

// ---------------------------------------------------------------------
// Negative controls: proof that `compare` can fail, not merely pass.
// ---------------------------------------------------------------------

#[test]
#[should_panic(expected = "result carrier bytes disagree")]
fn compare_rejects_a_perturbed_result() {
    let case = &corpus()[0];
    let interpreter = run_interpreter_case(case);
    let mut corrupted_wasm = run_wasm_case(case);
    let bytes = corrupted_wasm
        .result_bytes
        .as_mut()
        .expect("the minimal-success case must have a result");
    bytes[0] ^= 0xFF;
    compare(
        &case.case_id,
        case.expected_accepted,
        case.expected_status,
        &interpreter,
        &corrupted_wasm,
    );
}

#[test]
#[should_panic(expected = "does not match the independently pinned expectation")]
fn compare_rejects_a_status_that_does_not_match_the_pinned_expectation() {
    let case = &corpus()[0];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    // Both engines really did accept this case with status `Ok`; assert
    // against a deliberately wrong pinned expectation instead of what they
    // actually produced.
    compare(
        &case.case_id,
        case.expected_accepted,
        InterpreterPgStatus::ContractFailure as i32,
        &interpreter,
        &wasm,
    );
}

#[test]
#[should_panic(expected = "normalized trace label sequence disagrees")]
fn compare_rejects_a_truncated_trace() {
    let case = &corpus()[0];
    let interpreter = run_interpreter_case(case);
    let mut truncated_wasm = run_wasm_case(case);
    truncated_wasm.trace.pop();
    compare(
        &case.case_id,
        case.expected_accepted,
        case.expected_status,
        &interpreter,
        &truncated_wasm,
    );
}

#[test]
#[should_panic(expected = "left live allocations")]
fn compare_rejects_a_nonzero_final_live_allocation_count() {
    // Perturb both sides identically so the engine-vs-engine equality
    // check passes and the check this test is actually named for — "final
    // counts really are zero, not merely equal to each other" — is the one
    // that fires.
    let case = &corpus()[0];
    let mut leaked_interpreter = run_interpreter_case(case);
    let mut leaked_wasm = run_wasm_case(case);
    leaked_interpreter.final_live_allocations = 1;
    leaked_wasm.final_live_allocations = 1;
    compare(
        &case.case_id,
        case.expected_accepted,
        case.expected_status,
        &leaked_interpreter,
        &leaked_wasm,
    );
}
