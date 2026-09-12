//! Local duality/compatibility between two protocol roles (e.g. a client
//! spec and a server spec of one interaction). Two-party only: keeping
//! multi-party global protocols out of scope is an explicit instruction in
//! issue #206, not an oversight here.

use std::collections::BTreeMap;

use super::spec::{Kind, Label, ProtocolSpec, StateId, Transition};

/// Why two specs failed the duality/compatibility check. Each variant names
/// a distinct, stable defect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DualityError {
    /// A transition on one side has no counterpart transition (same state,
    /// same label) on the other side.
    MissingCounterpart { state: StateId, label: Label },
    /// The two sides' transition kinds for the same (state, label) are not
    /// complementary (e.g. both `Send`, instead of one `Send` and one
    /// `Receive`).
    KindNotComplementary {
        state: StateId,
        label: Label,
        a: Kind,
        b: Kind,
    },
    /// The two sides declared different payload tags for the same message.
    PayloadDivergence {
        state: StateId,
        label: Label,
        a: &'static str,
        b: &'static str,
    },
    /// The two sides require different capabilities for the same message.
    /// This is the specific unsoundness the crate doc's failure-case list
    /// names: duality that ignores capability divergence would let one
    /// role's compatibility proof paper over the other role needing
    /// authority the first never has to present.
    CapabilityDivergence {
        state: StateId,
        label: Label,
        a: Option<&'static str>,
        b: Option<&'static str>,
    },
}

fn complementary(a: Kind, b: Kind) -> bool {
    matches!(
        (a, b),
        (Kind::Send, Kind::Receive)
            | (Kind::Receive, Kind::Send)
            | (Kind::Call, Kind::Return)
            | (Kind::Return, Kind::Call)
    ) || (a == b && a.is_escape())
}

/// One-directional check: every transition `a` declares has a compatible
/// counterpart on `b`. This alone does not prove duality -- a transition
/// that exists only on `b`'s side is invisible to this direction. Use
/// [`check_duality`] for the full two-way guarantee.
pub fn check_duality_one_way(a: &ProtocolSpec, b: &ProtocolSpec) -> Result<(), Vec<DualityError>> {
    let mut errors = Vec::new();
    let index_b: BTreeMap<(StateId, Label), &Transition> = b
        .transitions
        .iter()
        .map(|t| ((t.from, t.label), t))
        .collect();

    for ta in &a.transitions {
        match index_b.get(&(ta.from, ta.label)) {
            None => errors.push(DualityError::MissingCounterpart {
                state: ta.from,
                label: ta.label,
            }),
            Some(tb) => {
                if !complementary(ta.kind, tb.kind) {
                    errors.push(DualityError::KindNotComplementary {
                        state: ta.from,
                        label: ta.label,
                        a: ta.kind,
                        b: tb.kind,
                    });
                }
                if ta.payload_type != tb.payload_type {
                    errors.push(DualityError::PayloadDivergence {
                        state: ta.from,
                        label: ta.label,
                        a: ta.payload_type,
                        b: tb.payload_type,
                    });
                }
                if ta.required_capability != tb.required_capability {
                    errors.push(DualityError::CapabilityDivergence {
                        state: ta.from,
                        label: ta.label,
                        a: ta.required_capability,
                        b: tb.required_capability,
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The full two-way duality/compatibility check: `a` is compatible with `b`
/// only if each side's transitions have a complementary counterpart on the
/// other. Errors from both directions are collected so a caller sees every
/// divergence in one pass, never just the first direction checked.
pub fn check_duality(a: &ProtocolSpec, b: &ProtocolSpec) -> Result<(), Vec<DualityError>> {
    let mut errors = Vec::new();
    if let Err(e) = check_duality_one_way(a, b) {
        errors.extend(e);
    }
    if let Err(e) = check_duality_one_way(b, a) {
        errors.extend(e);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
