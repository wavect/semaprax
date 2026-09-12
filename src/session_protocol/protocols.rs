//! Two applied example protocols, satisfying issue #206's "apply it to at
//! least two real interaction lifecycles" outcome at this module's Rust
//! reference-kernel layer (see the crate doc `Status` section for exactly
//! what layer that is).
//!
//! [`model_stream_protocol`] models a model/tool streaming session (the
//! issue's own "model streams... database transactions... Agent-to-tool
//! interaction" examples): open, receive chunks, end by branching into a
//! graceful close or an aborted stream, with cancel/timeout/fail escapes at
//! every nonterminal state. It exercises `Send`, `Receive`, `Branch`,
//! `Cancel`, `Timeout`, and `Fail`.
//!
//! [`resource_transaction_protocol`] models a bounded database transaction:
//! begin, then any number of `read`/`write` calls (each a `Call` that opens
//! a pending in-flight operation only its own `Return` or a `Timeout`
//! resolves), then commit (which consumes a [`super::engine::ResourceToken`])
//! or rollback. It exercises `Call`/`Return` and `OwnershipMove::ConsumesResource`.

use std::collections::BTreeSet;

use super::spec::{Kind, Next, OwnershipMove, ProtocolSpec, Transition};

fn states(names: &[&'static str]) -> BTreeSet<&'static str> {
    names.iter().copied().collect()
}

pub fn model_stream_protocol() -> ProtocolSpec {
    ProtocolSpec {
        name: "model-stream-v1",
        states: states(&["Idle", "Streaming", "Closing", "Closed", "Cancelled", "Uncertain", "Failed"]),
        initial: "Idle",
        terminal: states(&["Closed", "Cancelled", "Uncertain", "Failed"]),
        transitions: vec![
            Transition {
                from: "Idle",
                label: "open",
                kind: Kind::Send,
                payload_type: "StreamRequest",
                required_capability: Some("stream.open"),
                ownership: OwnershipMove::None,
                next: Next::Then("Streaming"),
            },
            Transition {
                from: "Idle",
                label: "abandon",
                kind: Kind::Cancel,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Cancelled"),
            },
            Transition {
                from: "Streaming",
                label: "chunk",
                kind: Kind::Receive,
                payload_type: "StreamChunk",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Streaming"),
            },
            Transition {
                from: "Streaming",
                label: "end",
                kind: Kind::Send,
                payload_type: "StreamEnd",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Choice(vec![("graceful", "Closing"), ("aborted", "Cancelled")]),
            },
            Transition {
                from: "Streaming",
                label: "cancel",
                kind: Kind::Cancel,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Cancelled"),
            },
            Transition {
                from: "Streaming",
                label: "timeout",
                kind: Kind::Timeout,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Uncertain"),
            },
            Transition {
                from: "Streaming",
                label: "fail",
                kind: Kind::Fail,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Failed"),
            },
            Transition {
                from: "Closing",
                label: "close",
                kind: Kind::Send,
                payload_type: "StreamClose",
                required_capability: Some("stream.close"),
                ownership: OwnershipMove::None,
                next: Next::Then("Closed"),
            },
            Transition {
                from: "Closing",
                label: "timeout",
                kind: Kind::Timeout,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Uncertain"),
            },
        ],
        cleanup: vec![
            ("Closed", vec!["flush_buffers", "release_socket"]),
            ("Cancelled", vec!["release_socket"]),
            ("Uncertain", vec!["mark_uncertain_for_reconciliation"]),
            ("Failed", vec!["release_socket", "emit_failure_report"]),
        ],
    }
}

pub fn resource_transaction_protocol() -> ProtocolSpec {
    ProtocolSpec {
        name: "resource-transaction-v1",
        states: states(&[
            "Idle",
            "Open",
            "AwaitingRead",
            "AwaitingWrite",
            "Committed",
            "RolledBack",
            "Uncertain",
        ]),
        initial: "Idle",
        terminal: states(&["Committed", "RolledBack", "Uncertain"]),
        transitions: vec![
            Transition {
                from: "Idle",
                label: "begin",
                kind: Kind::Send,
                payload_type: "BeginTxn",
                required_capability: Some("txn.begin"),
                ownership: OwnershipMove::None,
                next: Next::Then("Open"),
            },
            Transition {
                from: "Idle",
                label: "abandon",
                kind: Kind::Cancel,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("RolledBack"),
            },
            Transition {
                from: "Open",
                label: "read",
                kind: Kind::Call,
                payload_type: "ReadOp",
                required_capability: Some("txn.read"),
                ownership: OwnershipMove::None,
                next: Next::Then("AwaitingRead"),
            },
            Transition {
                from: "Open",
                label: "write",
                kind: Kind::Call,
                payload_type: "WriteOp",
                required_capability: Some("txn.write"),
                ownership: OwnershipMove::None,
                next: Next::Then("AwaitingWrite"),
            },
            Transition {
                from: "Open",
                label: "commit",
                kind: Kind::Send,
                payload_type: "Commit",
                required_capability: Some("txn.commit"),
                ownership: OwnershipMove::ConsumesResource,
                next: Next::Then("Committed"),
            },
            Transition {
                from: "Open",
                label: "rollback",
                kind: Kind::Cancel,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("RolledBack"),
            },
            Transition {
                from: "AwaitingRead",
                label: "read_result",
                kind: Kind::Return,
                payload_type: "ReadResult",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Open"),
            },
            Transition {
                from: "AwaitingRead",
                label: "timeout",
                kind: Kind::Timeout,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Uncertain"),
            },
            Transition {
                from: "AwaitingWrite",
                label: "write_result",
                kind: Kind::Return,
                payload_type: "WriteResult",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Open"),
            },
            Transition {
                from: "AwaitingWrite",
                label: "timeout",
                kind: Kind::Timeout,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Uncertain"),
            },
        ],
        cleanup: vec![
            ("Committed", vec!["persist_commit_record", "release_connection"]),
            ("RolledBack", vec!["release_connection"]),
            ("Uncertain", vec!["mark_uncertain_for_reconciliation"]),
        ],
    }
}
