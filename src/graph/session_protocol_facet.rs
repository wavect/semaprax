//! Issue #206 projection: this repository's built-in session-protocol
//! reference kernel (`crate::session_protocol`) surfaced as deterministic,
//! declaration-independent reference data through two routes: the
//! standalone `graph::session_protocol_kernel_json()` Rust API
//! (`session_protocol_kernel`, unconditional -- it takes no `Program`, so
//! there is nothing to gate on, and it is **not** part of `to_json`'s
//! per-program graph document; see that function's own doc comment) and
//! `context`'s v1/v2 envelope (`session_protocol_kernel`, gated on the
//! `AgentContextFilter::SessionProtocol` / `--filters session_protocol`
//! opt-in filter, and CLI-reachable there today -- see
//! `docs/SESSION-PROTOCOL-TYPES-V1.md`'s "Protocol facts" acceptance-
//! criteria row for why no additional CLI verb was added for the former).
//!
//! # What a "protocol fact" is here, and what it deliberately is not
//!
//! `crate::session_protocol` is a Rust-level reference validator, not a
//! `.spx` language feature: four built-in [`ProtocolSpec`] catalog entries
//! (`model_stream_protocol`, `resource_transaction_protocol`,
//! `project_agent_session_protocol`, `database_transaction_protocol`), each checked by that module's own test
//! suite (`ProtocolSpec::validate`, bounded model-checking via
//! [`model_check::check_bounded`]). No `.spx` declaration is bound to any of
//! these specs today: there is no session/protocol source syntax, no HIR
//! node, and no verifier rule
//! (`docs/SESSION-PROTOCOL-TYPES-V1.md`'s "Scope boundary").
//! `project_agent_session_protocol` is a transcription of
//! `project_transport::session`'s real state machine, cited against exact
//! file:line spans in its own doc comment. Since e51226dd that transport and
//! `database_fixture`'s transaction (`database_transaction_protocol`, the
//! fourth catalog entry) both hold live `SessionTable`s over these specs.
//!
//! A "protocol fact," in this projection, is therefore exactly the kernel's
//! own fixed catalog entry for one built-in spec: its name, declared state
//! set, initial/terminal states, transition count, whether
//! `ProtocolSpec::validate` accepts it, and whether bounded model-checking
//! (`check_bounded(spec, spec.states.len())`, the same bound this
//! repository's own `session_protocol` test suite uses) accepts it. It is
//! **not** a fact about the `.spx` program the graph/context command was
//! asked to analyze -- no declaration in that program is consulted to
//! produce it. `docs/SESSION-PROTOCOL-TYPES-V1.md`'s "Acceptance criteria"
//! table named this exact move ("projecting a Rust-only reference kernel")
//! a second, disconnected source of truth; this module does not dispute
//! that characterization -- it makes the disconnection explicit in the
//! projection's own `"note"` field instead of leaving the gap silent, per
//! this lane's brief.
//!
//! Not to be confused with the unrelated, pre-existing `.spx` `protocol`
//! *interface* construct (`crate::protocol_check`, "Protocol Projection
//! v1") -- a body-less method-signature declaration with no relation to
//! message order. This module's JSON key (`session_protocol_kernel`) and
//! the `AgentContextFilter::SessionProtocol` name (`session_protocol`) are
//! deliberately not "protocol" alone, to avoid exactly that collision.

use crate::session_protocol::{model_check, protocols, spec::ProtocolSpec};

use super::quote_json;

/// Shared disclosure text: every emission site (graph and context) carries
/// the same honest scope statement, so an agent reading either output sees
/// the identical non-claim rather than two differently worded summaries.
const NOTE: &str = "Reference catalog of this compiler's built-in session-protocol kernel \
(src/session_protocol), not a fact about the queried .spx source: no catalog entry here is \
bound to a declaration. Session protocols the queried source itself declares appear under \
declared, each bound to its @id, source span, and checked HIR via targets. Two real runtime \
subsystems run on this kernel's SessionTable: project_transport::session \
(project-agent-session-v1) and database_fixture's transaction (database-transaction-v1). \
Legal order is not authority. See docs/SESSION-PROTOCOL-TYPES-V1.md.";

fn catalog() -> [ProtocolSpec; 4] {
    [
        protocols::model_stream_protocol(),
        protocols::resource_transaction_protocol(),
        protocols::project_agent_session_protocol(),
        protocols::database_transaction_protocol(),
    ]
}

fn state_array(states: impl Iterator<Item = &'static str>) -> String {
    let mut rendered = String::from("[");
    for (index, state) in states.enumerate() {
        if index != 0 {
            rendered.push(',');
        }
        rendered.push_str(&quote_json(state));
    }
    rendered.push(']');
    rendered
}

fn kind_text(kind: crate::session_protocol::spec::Kind) -> &'static str {
    use crate::session_protocol::spec::Kind;
    match kind {
        Kind::Send => "send",
        Kind::Receive => "receive",
        Kind::Call => "call",
        Kind::Return => "return",
        Kind::Cancel => "cancel",
        Kind::Timeout => "timeout",
        Kind::Fail => "fail",
    }
}

fn ownership_text(ownership: crate::session_protocol::spec::OwnershipMove) -> &'static str {
    use crate::session_protocol::spec::OwnershipMove;
    match ownership {
        OwnershipMove::None => "none",
        OwnershipMove::ConsumesResource => "consumes_resource",
    }
}

fn next_json(next: &crate::session_protocol::spec::Next) -> String {
    use crate::session_protocol::spec::Next;
    match next {
        Next::Then(state) => format!("{{\"kind\":\"then\",\"state\":{}}}", quote_json(state)),
        Next::Choice(branches) => format!(
            "{{\"kind\":\"choice\",\"branches\":[{}]}}",
            branches
                .iter()
                .map(|(label, state)| format!(
                    "{{\"label\":{},\"state\":{}}}",
                    quote_json(label),
                    quote_json(state)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn transition_json(transition: &crate::session_protocol::spec::Transition) -> String {
    format!(
        "{{\"from\":{},\"label\":{},\"kind\":{},\"payload_type\":{},\"required_capability\":{},\"ownership\":{},\"next\":{}}}",
        quote_json(transition.from),
        quote_json(transition.label),
        quote_json(kind_text(transition.kind)),
        quote_json(transition.payload_type),
        transition
            .required_capability
            .map(quote_json)
            .unwrap_or_else(|| "null".to_owned()),
        quote_json(ownership_text(transition.ownership)),
        next_json(&transition.next)
    )
}

fn spec_header_json(spec: &ProtocolSpec) -> String {
    format!(
        "\"name\":{},\"states\":{},\"initial\":{},\"terminal\":{},\"transition_count\":{},\"well_formed\":{},\"model_checked\":{}",
        quote_json(spec.name),
        state_array(spec.states.iter().copied()),
        quote_json(spec.initial),
        state_array(spec.terminal.iter().copied()),
        spec.transitions.len(),
        spec.validate().is_ok(),
        model_check::check_bounded(spec, spec.states.len()).is_ok(),
    )
}

/// Bounded summary catalog for `context`'s envelope-level
/// `session_protocol` filter: header fields only, no transition detail, so
/// it stays small against `MIN_AGENT_CONTEXT_BYTES` (2048 bytes).
///
/// `declared` is the queried program's bound declaration array
/// (`session_protocol::source::declarations_json`), or empty when the program
/// declares none, in which case the bytes carry no `declared` key.
pub(super) fn summary_catalog_json(declared: &str) -> String {
    let specs = catalog()
        .iter()
        .map(|spec| format!("{{{}}}", spec_header_json(spec)))
        .collect::<Vec<_>>()
        .join(",");
    let declared = if declared.is_empty() {
        String::new()
    } else {
        format!(",\"declared\":{declared}")
    };
    format!(
        "{{\"note\":{},\"specs\":[{}]{}}}",
        quote_json(NOTE),
        specs,
        declared
    )
}

/// Full catalog (header fields plus every declared transition) for the
/// whole-module `graph` command's unconditional `session_protocol_kernel`
/// section.
pub(super) fn full_catalog_json() -> String {
    let specs = catalog()
        .iter()
        .map(|spec| {
            let transitions = spec
                .transitions
                .iter()
                .map(transition_json)
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{{},\"transitions\":[{}]}}",
                spec_header_json(spec),
                transitions
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"note\":{},\"specs\":[{}]}}", quote_json(NOTE), specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_catalog_is_deterministic_across_two_independent_calls() {
        assert_eq!(full_catalog_json(), full_catalog_json());
    }

    #[test]
    fn summary_catalog_is_deterministic_across_two_independent_calls() {
        assert_eq!(summary_catalog_json(""), summary_catalog_json(""));
    }

    #[test]
    fn full_catalog_lists_exactly_the_four_built_in_specs_well_formed_and_model_checked() {
        let json = full_catalog_json();
        for name in [
            "model-stream-v1",
            "resource-transaction-v1",
            "project-agent-session-v1",
            "database-transaction-v1",
        ] {
            assert!(json.contains(&format!("\"name\":\"{name}\"")), "{json}");
        }
        // Every built-in spec validates and model-checks cleanly today
        // (mirrored by `session_protocol::tests::both_applied_protocols_validate`
        // and `..._model_check_cleanly`); this projection reports that
        // verdict rather than asserting it never can flip.
        assert!(!json.contains("\"well_formed\":false"), "{json}");
        assert!(!json.contains("\"model_checked\":false"), "{json}");
    }

    #[test]
    fn summary_catalog_omits_transition_detail_the_full_catalog_carries() {
        let summary = summary_catalog_json("");
        let full = full_catalog_json();
        assert!(!summary.contains("\"transitions\""), "{summary}");
        assert!(full.contains("\"transitions\""), "{full}");
        assert!(full.len() > summary.len());
    }

    #[test]
    fn the_note_field_names_both_live_subsystems_and_discloses_no_declaration_is_bound() {
        // Factual correction (issue #297): since e51226dd both real
        // subsystems run on the kernel, so the note must say so, and must
        // still disclose that this catalog is not bound to queried source.
        for json in [summary_catalog_json(""), full_catalog_json()] {
            assert!(json.contains("not a fact about the queried"), "{json}");
            assert!(json.contains("no catalog entry here is bound"), "{json}");
            assert!(json.contains("appear under declared"), "{json}");
            assert!(
                json.contains("Two real runtime subsystems run on this kernel"),
                "{json}"
            );
            assert!(json.contains("project_transport::session"), "{json}");
            assert!(json.contains("database_fixture"), "{json}");
            assert!(!json.contains("no runtime subsystem calls into"), "{json}");
            assert!(!json.contains("hand-rolled"), "{json}");
        }
    }
}
