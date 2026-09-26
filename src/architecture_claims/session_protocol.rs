//! `protocol_realizers_bound` (issue #297): every `via` target of one
//! declared session protocol is a checked function node of one
//! `ProjectRevision`'s direct call graph.
//!
//! # What this attests, and what it does not
//!
//! It attests only realizer binding: each function a declaration names in a
//! `via` clause is retained, as a checked function, in the call graph of the
//! exact revision the claim is evaluated against. It does **not** attest
//! message order, call order, that any caller respects the protocol, or that
//! the realizers are ever called; and it grants no capability, effect, or
//! execution authority.
//!
//! Declarations are read from the revision's own authenticated source bytes
//! (`ProjectRevision::sources`), parsed with the ordinary parser, and checked
//! with the same `SPX-K1xx` source rules the single-file verifier applies.
//! Only sources whose text contains the `session protocol` keyword pair are
//! reparsed, and only when the claim set contains this operator. The claim
//! names only a protocol identity; no edge is caller-authored.
//!
//! # Status
//!
//! - `held`: at least one `via`, and every `via` target is a checked node.
//! - There is no `violated` outcome. A built revision already refuses a `via`
//!   naming no function of its declaring module (`SPX-K104`), project modules
//!   are never pruned, and bundled-dependency pruning keeps every `via`
//!   target, so from built source every `via` target is a checked node. A
//!   `via` target absent from the evaluated graph would therefore mean an
//!   inconsistent revision, and it fails closed (`SPX-AC601`) rather than
//!   reporting a claim outcome.
//! - `unevaluable`: the declaration binds no `via`
//!   (`reason: "no_via_binding"`), or two retained sources declare the same
//!   protocol identity (`reason: "duplicate_session_protocol_identity"`).
//!   Only claims naming that identity are affected.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ast::SessionProtocolDeclaration;
use crate::project::ProjectRevision;

use super::{invalid, Result};

#[derive(Default)]
pub(super) struct DeclaredProtocols {
    by_id: BTreeMap<String, SessionProtocolDeclaration>,
    duplicates: BTreeSet<String>,
}

/// Whether `source` can contain a `session protocol` declaration: the
/// keyword `session` followed by whitespace and `protocol`. A cheap
/// over-approximation (a comment can match); a false positive only costs
/// one reparse.
fn may_declare(source: &str) -> bool {
    source.match_indices("session").any(|(index, word)| {
        let rest = &source[index + word.len()..];
        let trimmed = rest.trim_start();
        trimmed.len() < rest.len() && trimmed.starts_with("protocol")
    })
}

impl DeclaredProtocols {
    pub(super) fn from_revision(revision: &ProjectRevision) -> Result<Self> {
        let mut protocols = Self::default();
        for source in revision.sources() {
            if !may_declare(source.source()) {
                continue;
            }
            let program = crate::parse(source.source(), source.path())
                .map_err(|_| invalid("architecture claim could not reparse a retained source"))?;
            if program.session_protocols.is_empty() {
                continue;
            }
            if crate::session_protocol::source::check(&program)
                .iter()
                .any(|diagnostic| diagnostic.severity.is_error())
            {
                return Err(invalid(
                    "architecture claim found a retained session protocol that fails its source checks",
                ));
            }
            for declaration in &program.session_protocols {
                if protocols
                    .by_id
                    .insert(declaration.stable_id.clone(), declaration.clone())
                    .is_some()
                {
                    protocols.duplicates.insert(declaration.stable_id.clone());
                }
            }
        }
        Ok(protocols)
    }
}

pub(super) fn evaluate(
    claim_id: &str,
    protocol: &str,
    protocols: &DeclaredProtocols,
    is_checked_node: impl Fn(&str) -> bool,
) -> Result<Value> {
    let base = |status: &str| {
        json!({
            "claim_id": claim_id,
            "operator": "protocol_realizers_bound",
            "protocol": protocol,
            "attests": "via_targets_are_checked_call_graph_nodes_not_ordering",
            "authority": "none",
            "status": status,
        })
    };
    if protocols.duplicates.contains(protocol) {
        let mut value = base("unevaluable");
        value["reason"] = json!("duplicate_session_protocol_identity");
        value["protocol_name"] = Value::Null;
        value["via"] = json!([]);
        return Ok(value);
    }
    let declaration = protocols.by_id.get(protocol).ok_or_else(|| {
        invalid("architecture claim protocol_realizers_bound `protocol` is not a declared session protocol in this revision")
    })?;
    let via = declaration
        .transitions
        .iter()
        .filter_map(|transition| {
            transition.via.as_ref().map(|via| {
                json!({
                    "from": transition.from.name,
                    "label": transition.label.name,
                    "via": via.name,
                })
            })
        })
        .collect::<Vec<_>>();
    let missing = via
        .iter()
        .filter(|edge| !is_checked_node(edge["via"].as_str().unwrap_or_default()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(invalid(
            "architecture claim protocol_realizers_bound found a via target that is not a checked call-graph node of this revision",
        ));
    }
    let (status, reason) = if via.is_empty() {
        ("unevaluable", json!("no_via_binding"))
    } else {
        ("held", Value::Null)
    };
    let mut value = base(status);
    value["reason"] = reason;
    value["protocol_name"] = json!(declaration.name);
    value["via"] = Value::Array(via);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARED: &str = include_str!("../session_protocol/tests/fixtures/declared.spx");

    fn protocols() -> DeclaredProtocols {
        let program = crate::parse(DECLARED, "declared.spx").unwrap();
        let mut protocols = DeclaredProtocols::default();
        for declaration in &program.session_protocols {
            protocols
                .by_id
                .insert(declaration.stable_id.clone(), declaration.clone());
        }
        protocols
    }

    #[test]
    fn a_via_target_absent_from_the_evaluated_graph_fails_closed() {
        let error = evaluate("c", "fixture.session.transaction", &protocols(), |id| {
            id == "fixture.session.begin"
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-AC601");
        let held = evaluate("c", "fixture.session.transaction", &protocols(), |_| true).unwrap();
        assert_eq!(held["status"], "held");
        assert_eq!(
            held["attests"],
            "via_targets_are_checked_call_graph_nodes_not_ordering"
        );
    }

    #[test]
    fn a_duplicate_identity_affects_only_claims_that_name_it() {
        let mut protocols = protocols();
        protocols
            .duplicates
            .insert("fixture.session.transaction".to_owned());
        let value = evaluate("c", "fixture.session.transaction", &protocols, |_| true).unwrap();
        assert_eq!(value["status"], "unevaluable");
        assert_eq!(value["reason"], "duplicate_session_protocol_identity");
    }

    #[test]
    fn the_prefilter_matches_only_the_keyword_pair() {
        assert!(may_declare("x\nsession protocol \"a\" {"));
        assert!(may_declare("session\n    protocol"));
        assert!(!may_declare("fn session_protocol() -> i64 { 0 }"));
        assert!(!may_declare(
            "@id(\"a.session\") fn protocolize() -> i64 { 0 }"
        ));
    }
}
