//! `protocol_order_bound` (issue #297): a declared session protocol's `via`
//! realizations, checked against one `ProjectRevision`'s direct call graph.
//!
//! Declarations are read from the revision's own authenticated source bytes
//! (`ProjectRevision::sources`), parsed with the ordinary parser, and checked
//! with the same `SPX-K1xx` source rules the single-file verifier applies.
//! No caller-authored edge enters: the claim names only a protocol identity,
//! and each `via` edge (declaration to realizing function) must resolve to a
//! node the compiler itself retained in checked HIR.
//!
//! Status vocabulary is the existing one. `held`: every `via` resolves.
//! `violated`: at least one `via` names no checked call-graph node; `missing`
//! lists them in declaration order. `unevaluable`: the declaration binds no
//! `via` at all, so there is nothing checked to hold. None of these grants
//! any authority.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::ast::SessionProtocolDeclaration;
use crate::project::ProjectRevision;

use super::{invalid, Result};

#[derive(Default)]
pub(super) struct DeclaredProtocols {
    by_id: BTreeMap<String, SessionProtocolDeclaration>,
}

impl DeclaredProtocols {
    pub(super) fn from_revision(revision: &ProjectRevision) -> Result<Self> {
        let mut by_id = BTreeMap::new();
        for source in revision.sources() {
            if !source.source().contains("session") {
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
            for declaration in program.session_protocols.iter() {
                if by_id
                    .insert(declaration.stable_id.clone(), declaration.clone())
                    .is_some()
                {
                    return Err(invalid(
                        "architecture claim found a duplicate session protocol identity",
                    ));
                }
            }
        }
        Ok(Self { by_id })
    }
}

pub(super) fn evaluate(
    claim_id: &str,
    protocol: &str,
    protocols: &DeclaredProtocols,
    is_checked_node: impl Fn(&str) -> bool,
) -> Result<Value> {
    let declaration = protocols.by_id.get(protocol).ok_or_else(|| {
        invalid("architecture claim protocol_order_bound `protocol` is not a declared session protocol in this revision")
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
    let status = if via.is_empty() {
        "unevaluable"
    } else if missing.is_empty() {
        "held"
    } else {
        "violated"
    };
    Ok(json!({
        "claim_id": claim_id,
        "operator": "protocol_order_bound",
        "protocol": protocol,
        "protocol_name": declaration.name,
        "via": via,
        "missing": if missing.is_empty() { Value::Null } else { Value::Array(missing) },
        "authority": "none",
        "status": status,
    }))
}
