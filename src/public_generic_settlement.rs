//! The owned allocation and failure settlement obligations a public generic
//! boundary would have to meet, derived from the type grammar and bound to the
//! compiler's own cleanup facts.
//!
//! This serves gate PG-7 of the
//! [Public Generic Ownership milestone](../docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
//! at the specification level only. There is no public generic boundary to
//! execute: nothing here allocates, transfers, releases, or observes a runtime,
//! and the gate stays open until a boundary exists and settles on every claimed
//! engine.
//!
//! What it does establish is the thing a boundary must not be free to invent.
//! A consumer that receives an owned generic instance has to know exactly which
//! owned leaves it is accountable for, in which order they are released when a
//! transfer fails part way, and that nobody downstream may sort or repair that
//! order. Two facts the compiler already produces say all of it:
//!
//! - the cleanup **inventory**, which is structural metadata: the leaf tree in
//!   declaration order, and one liveness flag per owned leaf carrying that
//!   leaf's exact projection chain and its drop lifecycle; and
//! - the cleanup **plan**'s entry state, which names the transfer unit: the
//!   owned parameter as a whole, not its leaves.
//!
//! A plan derived here binds all three. The grammar's owned-leaf paths must
//! equal the inventory's leaf paths in structural order *and* the inventory's
//! flag places in flag order, each with its lifecycle; the plan's entry state
//! must carry the parameter as exactly one whole live owned place. Nothing is
//! sorted or repaired: a disagreement is a refusal, because a target-neutral
//! projection that quietly differed from the checked cleanup facts would be
//! worse than no projection at all.
//!
//! The two kinds of disagreement carry distinct codes on purpose, so a
//! caller never has to parse this module's message prose to tell them apart:
//! [`SETTLEMENT_DISAGREEMENT`] for the cleanup **inventory** (the grammar's
//! owned-leaf paths against the inventory's leaf tree and liveness flags),
//! and [`TRANSFER_UNIT_DISAGREEMENT`] for the cleanup **plan**'s entry state
//! (the parameter is not named as exactly one whole live owned place). This
//! is exactly the inventory/plan distinction the module's own opening
//! paragraphs draw, carried through to the diagnostic vocabulary; see
//! `src/public_generic_abi/classifier.rs`, which maps the two to its own
//! distinct `Refusal::CleanupInventoryMismatch` and
//! `Refusal::SettlementObligationMismatch` respectively.

use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};

use crate::cleanup::{CleanupPlace as InventoryPlace, CleanupStorageOrigin};
use crate::cleanup::{FieldLiveness, FieldLivenessShape};
use crate::cleanup_plan::StorageId;
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedFunction, ResolvedType};
use crate::public_generic_type::{self as grammar, TypeInventory};

/// One deterministic settlement obligation list per owned instance parameter.
pub const SETTLEMENT_PLAN_SCHEMA: &str = "semaprax.public-generic-settlement-plan.v1";

const PLAN_DOMAIN: &[u8] = b"semaprax.public-generic-settlement-plan.v1\0";

/// The selected parameter is not an owned admitted instance.
pub const UNSUPPORTED_PARAMETER: &str = "SPX-PG501";
/// The derived owned-leaf order disagrees with the compiler's own cleanup
/// **inventory**: the structural leaf tree and its per-leaf liveness flags.
/// See [`TRANSFER_UNIT_DISAGREEMENT`] for the sibling code covering the
/// cleanup **plan**'s transfer unit instead.
pub const SETTLEMENT_DISAGREEMENT: &str = "SPX-PG502";
/// The derived transfer unit disagrees with the compiler's own cleanup
/// **plan** entry state: the parameter is not named as exactly one live
/// owned place, or it is named projected rather than whole. Distinct from
/// [`SETTLEMENT_DISAGREEMENT`], which covers the cleanup *inventory* (leaf
/// paths and liveness flags) rather than the *plan*'s transfer unit —
/// `src/public_generic_abi/classifier.rs` maps this code to its own
/// `Refusal::SettlementObligationMismatch`, distinct from
/// `Refusal::CleanupInventoryMismatch`, which is exactly what
/// [`SETTLEMENT_DISAGREEMENT`] maps to.
pub const TRANSFER_UNIT_DISAGREEMENT: &str = "SPX-PG503";

fn unsupported(subject: &str) -> Diagnostic {
    Diagnostic::io(
        UNSUPPORTED_PARAMETER,
        format!("{SETTLEMENT_PLAN_SCHEMA} does not describe this parameter: {subject}"),
    )
}

fn disagreement(subject: &str) -> Diagnostic {
    Diagnostic::io(
        SETTLEMENT_DISAGREEMENT,
        format!("{SETTLEMENT_PLAN_SCHEMA} disagrees with the checked cleanup facts: {subject}"),
    )
}

/// Like [`disagreement`], but for a disagreement in the cleanup **plan**'s
/// transfer unit rather than the cleanup **inventory**'s leaf structure. See
/// [`TRANSFER_UNIT_DISAGREEMENT`].
fn transfer_unit_disagreement(subject: &str) -> Diagnostic {
    Diagnostic::io(
        TRANSFER_UNIT_DISAGREEMENT,
        format!("{SETTLEMENT_PLAN_SCHEMA} disagrees with the checked cleanup plan: {subject}"),
    )
}

/// One owned leaf a boundary is accountable for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Obligation {
    /// Position in structural order, equal to the inventory's flag order.
    pub index: u32,
    /// The grammar's identity-framed path to the leaf.
    pub path: String,
    /// The ordered field identities that path projects through.
    pub fields: Vec<String>,
    /// The checked drop lifecycle of the leaf, from the inventory's flag.
    pub lifecycle: String,
}

/// The settlement obligations of one owned instance parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementPlan {
    export: String,
    parameter_index: u32,
    instance_term: String,
    obligations: Vec<Obligation>,
    transfer_unit: String,
    digest: String,
}

impl SettlementPlan {
    /// The export whose parameter this describes.
    pub fn export(&self) -> &str {
        &self.export
    }

    /// The canonical term of the owned instance.
    pub fn instance_term(&self) -> &str {
        &self.instance_term
    }

    /// The owned leaves, in the structural order the grammar and the cleanup
    /// inventory agree on.
    pub fn obligations(&self) -> &[Obligation] {
        &self.obligations
    }

    /// The transfer unit the cleanup plan's entry state names: the owned
    /// parameter's own value identity, transferred whole. A boundary receives
    /// or refuses this one unit; it never transfers a leaf on its own.
    pub fn transfer_unit(&self) -> &str {
        &self.transfer_unit
    }

    /// Release order after a failed transfer: the exact reverse of the
    /// canonical obligation order. Failure selection stays sticky; cleanup
    /// never replaces the selected status, and no result is published.
    pub fn release_order(&self) -> Vec<&str> {
        self.obligations
            .iter()
            .rev()
            .map(|obligation| obligation.path.as_str())
            .collect()
    }

    /// The domain-separated digest of the plan's identity-bearing facts.
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Derive the settlement obligations of one owned instance parameter, and
/// require them to agree with the checked cleanup facts of the same function.
///
/// Fails closed when the parameter is not owned, when its type is not an
/// admitted instance, when the instance owns no leaf (nothing to settle), or
/// when the grammar and the cleanup facts disagree in membership or structural
/// order.
pub fn plan(
    inventory: &TypeInventory<'_>,
    function: &ResolvedFunction,
    parameter_index: u32,
) -> Result<SettlementPlan, Diagnostic> {
    let parameter = function
        .params
        .get(parameter_index as usize)
        .ok_or_else(|| unsupported("no parameter at that index"))?;
    if parameter.ownership != OwnershipMode::Own {
        return Err(unsupported("the parameter is not owned"));
    }
    let ResolvedType::Nominal { .. } = parameter.ty else {
        return Err(unsupported("the parameter is not a record instance"));
    };
    let facts = grammar::describe(inventory, &parameter.ty)?;
    if facts.owned_leaves.is_empty() {
        return Err(unsupported("the instance owns no leaf, so nothing settles"));
    }

    let storage = function
        .cleanup
        .slots
        .iter()
        .find(|slot| {
            matches!(
                slot.origin,
                CleanupStorageOrigin::Parameter { parameter_index: index, .. }
                    if index == parameter_index
            )
        })
        .ok_or_else(|| disagreement("the parameter has no cleanup storage slot"))?;
    if storage.ty != parameter.ty {
        return Err(disagreement(
            "the cleanup storage type is not the parameter type",
        ));
    }

    // Structural agreement: the grammar's owned-leaf paths are the inventory's
    // leaf paths, in the same order.
    let mut structural = Vec::new();
    collect(&storage.shape, &mut Vec::new(), &mut structural)?;
    if structural.len() != facts.owned_leaves.len() {
        return Err(disagreement("the owned-leaf counts differ"));
    }
    // One liveness flag per owned leaf of this storage, in flag order. The
    // flag is where the checked drop lifecycle lives, so an obligation without
    // one would be an obligation with no stated way to discharge it.
    let mut flags = function
        .cleanup
        .flags
        .iter()
        .filter(|flag| flag.place.storage == storage.id)
        .collect::<Vec<_>>();
    flags.sort_by_key(|flag| flag.id.0);
    if flags.len() != structural.len() {
        return Err(disagreement(
            "the liveness flag count is not the owned-leaf count",
        ));
    }

    let mut obligations = Vec::with_capacity(structural.len());
    for (index, ((expected, fields), flag)) in facts
        .owned_leaves
        .iter()
        .zip(&structural)
        .zip(&flags)
        .enumerate()
    {
        let rendered = render_path(fields);
        if &rendered != expected {
            return Err(disagreement(&format!(
                "leaf {index} is {rendered} in the cleanup inventory and {expected} in the grammar"
            )));
        }
        if render_inventory_place(&flag.place) != rendered {
            return Err(disagreement(&format!(
                "liveness flag {index} does not name {rendered}"
            )));
        }
        obligations.push(Obligation {
            index: index as u32,
            path: rendered,
            fields: fields.clone(),
            lifecycle: flag.lifecycle.as_str().to_owned(),
        });
    }

    // The transfer unit. The inventory and the plan number storage
    // separately: the inventory by its own slot identity, the plan by the
    // parameter's checked value identity. Selecting by the value identity is
    // what keeps this bound to one parameter rather than to a slot index.
    //
    // The plan's entry state names the parameter *whole* — one place with no
    // projections — which is the fact a boundary needs: the unit that is
    // transferred or refused. Reading the leaves out of the plan instead would
    // be inventing a per-leaf transfer the compiler does not perform.
    let expected_storage = StorageId::Value(parameter.id.clone());
    let live = function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .iter()
        .filter(|place| place.storage == expected_storage)
        .collect::<Vec<_>>();
    let [whole] = live.as_slice() else {
        return Err(transfer_unit_disagreement(
            "the cleanup plan does not name the owned parameter exactly once",
        ));
    };
    if !whole.projections.is_empty() {
        return Err(transfer_unit_disagreement(
            "the cleanup plan's live owned parameter is projected rather than whole",
        ));
    }
    let transfer_unit = parameter.id.as_str().to_owned();

    let mut preimage = Vec::new();
    frame(&mut preimage, function.id.as_str().as_bytes());
    preimage.extend_from_slice(&parameter_index.to_le_bytes());
    frame(&mut preimage, facts.instance_digest.as_bytes());
    frame(&mut preimage, transfer_unit.as_bytes());
    preimage.extend_from_slice(&(obligations.len() as u64).to_le_bytes());
    for obligation in &obligations {
        preimage.extend_from_slice(&obligation.index.to_le_bytes());
        frame(&mut preimage, obligation.path.as_bytes());
        frame(&mut preimage, obligation.lifecycle.as_bytes());
    }

    Ok(SettlementPlan {
        export: function.id.as_str().to_owned(),
        parameter_index,
        instance_term: facts.term,
        obligations,
        transfer_unit,
        digest: digest(PLAN_DOMAIN, &preimage),
    })
}

/// Walk the cleanup inventory shape, collecting one field-identity chain per
/// owned leaf in structural order.
fn collect(
    shape: &FieldLivenessShape,
    path: &mut Vec<String>,
    output: &mut Vec<Vec<String>>,
) -> Result<(), Diagnostic> {
    match shape {
        FieldLivenessShape::NoDrop => Ok(()),
        FieldLivenessShape::Leaf { .. } => {
            output.push(path.clone());
            Ok(())
        }
        FieldLivenessShape::Record { fields, .. } => {
            for FieldLiveness { field, shape, .. } in fields {
                path.push(field.as_str().to_owned());
                collect(shape, path, output)?;
                path.pop();
            }
            Ok(())
        }
        // A variant leaf is live only for its authenticated case, so its
        // settlement is conditional rather than unconditional. The grammar
        // admits no variant, so reaching one here is a disagreement, not a
        // shape to flatten.
        FieldLivenessShape::Variant { .. } => Err(disagreement(
            "a conditional variant shape has no unconditional owned-leaf order",
        )),
    }
}

fn render_path(fields: &[String]) -> String {
    let mut output = String::new();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push('/');
        }
        let _ = write!(output, "@{}:{field}", field.len());
    }
    output
}

fn render_inventory_place(place: &InventoryPlace) -> String {
    render_path(
        &place
            .projections
            .iter()
            .map(|projection| projection.as_str().to_owned())
            .collect::<Vec<_>>(),
    )
}

fn frame(preimage: &mut Vec<u8>, bytes: &[u8]) {
    preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    preimage.extend_from_slice(bytes);
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

#[cfg(test)]
mod tests;
