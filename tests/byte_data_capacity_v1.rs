#[path = "../src/byte_data_capacity.rs"]
mod byte_data_capacity;

use byte_data_capacity::{
    analyze, ArrayStorageKind, ArrayStorageSlot, CapacityDiagnostic, CapacityFlow,
    FunctionCapacityInput, MAX_STDOUT_TRANSCRIPT_BYTES,
};

fn slot(identity: &str, kind: ArrayStorageKind, length: u32) -> ArrayStorageSlot {
    ArrayStorageSlot {
        identity: identity.to_owned(),
        kind,
        length,
    }
}

fn function(
    identity: &str,
    slots: Vec<ArrayStorageSlot>,
    execution: CapacityFlow,
) -> FunctionCapacityInput {
    FunctionCapacityInput {
        function: identity.to_owned(),
        array_slots: slots,
        execution,
    }
}

fn call(site: &str, callee: &str) -> CapacityFlow {
    CapacityFlow::Call {
        site: site.to_owned(),
        callee: callee.to_owned(),
    }
}

fn copy(site: &str, bytes: u64) -> CapacityFlow {
    CapacityFlow::BytesCopy {
        site: site.to_owned(),
        conservative_payload_bytes: bytes,
    }
}

fn stdout_write(site: &str) -> CapacityFlow {
    CapacityFlow::StdoutWrite {
        site: site.to_owned(),
    }
}

#[test]
fn stdout_transcript_is_single_path_bounded_and_cycle_free() {
    assert_eq!(MAX_STDOUT_TRANSCRIPT_BYTES, 65_536);
    let alternative = analyze(&[function(
        "root",
        vec![],
        CapacityFlow::Alternative(vec![stdout_write("root.left"), stdout_write("root.right")]),
    )])
    .unwrap();
    assert_eq!(alternative.function("root").unwrap().stdout_write_sites, 1);

    let sequence = analyze(&[function(
        "root",
        vec![],
        CapacityFlow::Sequence(vec![
            stdout_write("root.first"),
            stdout_write("root.second"),
        ]),
    )])
    .unwrap_err();
    assert_eq!(sequence.diagnostic, CapacityDiagnostic::Transcript);
    assert!(sequence.detail.contains("reaches 2 sites"));

    let looped = analyze(&[function(
        "root",
        vec![],
        CapacityFlow::Loop {
            condition: Box::new(CapacityFlow::Empty),
            body: Box::new(stdout_write("root.loop")),
        },
    )])
    .unwrap_err();
    assert_eq!(looped.diagnostic, CapacityDiagnostic::Transcript);
    assert!(looped.detail.contains("while"));

    let cycle = analyze(&[
        function("a", vec![], call("a.b", "b")),
        function(
            "b",
            vec![],
            CapacityFlow::Sequence(vec![stdout_write("b.write"), call("b.a", "a")]),
        ),
    ])
    .unwrap_err();
    assert_eq!(cycle.diagnostic, CapacityDiagnostic::Transcript);
    assert!(cycle.detail.contains("cyclic"));
}

#[test]
fn exact_array_frame_and_active_call_path_boundaries_are_checked() {
    let exact = analyze(&[
        function(
            "leaf",
            vec![slot("leaf.param", ArrayStorageKind::Parameter, 32_768)],
            CapacityFlow::Empty,
        ),
        function(
            "root",
            vec![slot("root.stage", ArrayStorageKind::CallStaging, 32_768)],
            call("root.call", "leaf"),
        ),
    ])
    .unwrap();
    assert_eq!(exact.functions().len(), 2);
    assert_eq!(
        exact.function("root").unwrap().inline_array_frame_bytes,
        32_768
    );
    assert_eq!(
        exact.function("root").unwrap().active_array_call_path_bytes,
        65_536
    );

    let overflow = analyze(&[
        function(
            "leaf",
            vec![slot("leaf.param", ArrayStorageKind::Parameter, 32_769)],
            CapacityFlow::Empty,
        ),
        function(
            "root",
            vec![slot("root.stage", ArrayStorageKind::CallStaging, 32_768)],
            call("root.call", "leaf"),
        ),
    ])
    .unwrap_err();
    assert_eq!(overflow.diagnostic, CapacityDiagnostic::Array);
    assert!(overflow.detail.contains("65537"));
}

#[test]
fn alternatives_take_max_and_sequences_conservatively_sum() {
    let summary = analyze(&[
        function(
            "small",
            vec![slot("small.local", ArrayStorageKind::Binding, 10_000)],
            CapacityFlow::Empty,
        ),
        function(
            "large",
            vec![slot("large.local", ArrayStorageKind::Temporary, 20_000)],
            CapacityFlow::Empty,
        ),
        function(
            "root",
            vec![slot(
                "root.result",
                ArrayStorageKind::ProvisionalResult,
                5_000,
            )],
            CapacityFlow::Alternative(vec![
                call("root.small", "small"),
                call("root.large", "large"),
            ]),
        ),
    ])
    .unwrap();
    assert_eq!(
        summary
            .function("root")
            .unwrap()
            .active_array_call_path_bytes,
        25_000
    );

    let sequential = analyze(&[
        function(
            "left",
            vec![slot("left.local", ArrayStorageKind::Binding, 30_000)],
            CapacityFlow::Empty,
        ),
        function(
            "right",
            vec![slot("right.local", ArrayStorageKind::Binding, 30_000)],
            CapacityFlow::Empty,
        ),
        function(
            "root",
            vec![slot("root.local", ArrayStorageKind::Binding, 6_000)],
            CapacityFlow::Sequence(vec![call("root.left", "left"), call("root.right", "right")]),
        ),
    ])
    .unwrap_err();
    assert_eq!(sequential.diagnostic, CapacityDiagnostic::Array);
}

#[test]
fn any_cycle_that_can_reach_nonzero_array_storage_is_rejected() {
    let error = analyze(&[
        function("a", vec![], call("a.b", "b")),
        function(
            "b",
            vec![],
            CapacityFlow::Sequence(vec![call("b.a", "a"), call("b.leaf", "leaf")]),
        ),
        function(
            "leaf",
            vec![slot("leaf.zero", ArrayStorageKind::Binding, 1)],
            CapacityFlow::Empty,
        ),
    ])
    .unwrap_err();
    assert_eq!(error.diagnostic, CapacityDiagnostic::Array);
    assert!(error.detail.contains("cycle"));

    let irrelevant_cycle = analyze(&[
        function("a", vec![], call("a.b", "b")),
        function("b", vec![], call("b.a", "a")),
    ])
    .unwrap();
    assert_eq!(
        irrelevant_cycle
            .function("a")
            .unwrap()
            .active_array_call_path_bytes,
        0
    );
}

#[test]
fn bytes_copy_branch_sequence_and_exact_limits_are_derived() {
    let sixteen = (0..16)
        .map(|index| copy(&format!("copy.{index}"), 65_536))
        .collect();
    let exact = analyze(&[function("exact", vec![], CapacityFlow::Sequence(sixteen))]).unwrap();
    let summary = exact.function("exact").unwrap();
    assert_eq!(summary.bytes_copy_sites, 16);
    assert_eq!(summary.owned_byte_payload_bytes, 1_048_576);

    let too_many = analyze(&[function(
        "too_many",
        vec![],
        CapacityFlow::Sequence(
            (0..17)
                .map(|index| copy(&format!("copy.{index}"), 1))
                .collect(),
        ),
    )])
    .unwrap_err();
    assert_eq!(too_many.diagnostic, CapacityDiagnostic::Allocation);

    let alternative = analyze(&[function(
        "alternative",
        vec![],
        CapacityFlow::Alternative(vec![
            CapacityFlow::Sequence((0..12).map(|i| copy(&format!("a.{i}"), 2)).collect()),
            CapacityFlow::Sequence((0..4).map(|i| copy(&format!("b.{i}"), 20)).collect()),
        ]),
    )])
    .unwrap();
    let summary = alternative.function("alternative").unwrap();
    assert_eq!(summary.bytes_copy_sites, 12);
    assert_eq!(summary.owned_byte_payload_bytes, 80);
}

#[test]
fn bytes_copy_in_loops_and_reachable_cycles_are_rejected() {
    let looped = analyze(&[function(
        "looped",
        vec![],
        CapacityFlow::Loop {
            condition: Box::new(CapacityFlow::Empty),
            body: Box::new(copy("loop.copy", 1)),
        },
    )])
    .unwrap_err();
    assert_eq!(looped.diagnostic, CapacityDiagnostic::Allocation);
    assert!(looped.detail.contains("while"));

    let cyclic = analyze(&[
        function("a", vec![], call("a.b", "b")),
        function(
            "b",
            vec![],
            CapacityFlow::Sequence(vec![call("b.a", "a"), copy("b.copy", 1)]),
        ),
    ])
    .unwrap_err();
    assert_eq!(cyclic.diagnostic, CapacityDiagnostic::Allocation);
    assert!(cyclic.detail.contains("cyclic"));
}

#[test]
fn duplicate_slots_sites_and_unknown_callees_fail_closed() {
    let duplicate_slot = analyze(&[function(
        "f",
        vec![
            slot("same", ArrayStorageKind::Binding, 1),
            slot("same", ArrayStorageKind::Temporary, 1),
        ],
        CapacityFlow::Empty,
    )])
    .unwrap_err();
    assert_eq!(duplicate_slot.diagnostic, CapacityDiagnostic::Invariant);

    let duplicate_site = analyze(&[function(
        "f",
        vec![],
        CapacityFlow::Sequence(vec![copy("same", 1), copy("same", 1)]),
    )])
    .unwrap_err();
    assert_eq!(duplicate_site.diagnostic, CapacityDiagnostic::Invariant);

    let unknown = analyze(&[function("f", vec![], call("f.missing", "missing"))]).unwrap_err();
    assert_eq!(unknown.diagnostic, CapacityDiagnostic::Invariant);
}
