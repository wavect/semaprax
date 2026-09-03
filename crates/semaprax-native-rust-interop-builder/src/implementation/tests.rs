//! Proof-only tests for the private Native Rust interop implementation.
//!
//! The cases are grouped by what they exercise and declared below; this root
//! owns only the shared fixtures, the shared observation helpers, and the
//! concatenated module sources that the source-shape proofs bind. Those
//! proofs read a whole module, so each concatenation lists the module root
//! followed by its submodules in declaration order.

use super::*;

use std::path::Path;
use std::process::Command;

// This module is declared with `#[path]`, so its own children resolve beside
// it rather than inside `tests/`; each child names its file explicitly.
#[path = "tests/artifact_replay.rs"]
mod artifact_replay;
#[path = "tests/cleanup_census.rs"]
mod cleanup_census;
#[path = "tests/cleanup_regions.rs"]
mod cleanup_regions;
#[path = "tests/hir_traversal.rs"]
mod hir_traversal;
#[path = "tests/ledger_capacity.rs"]
mod ledger_capacity;
#[path = "tests/linked_bundle.rs"]
mod linked_bundle;
#[path = "tests/phase_a_contract.rs"]
mod phase_a_contract;
#[path = "tests/phase_b_publication.rs"]
mod phase_b_publication;
#[path = "tests/phase_b_staging.rs"]
mod phase_b_staging;
#[path = "tests/phase_b_toolchain.rs"]
mod phase_b_toolchain;
#[path = "tests/resolved_disposal.rs"]
mod resolved_disposal;
#[path = "tests/source_census.rs"]
mod source_census;

/// The complete private implementation module, root first.
const IMPLEMENTATION_SOURCE: &str = concat!(
    include_str!("../implementation.rs"),
    include_str!("observability.rs"),
    include_str!("facts_capacity.rs"),
    include_str!("bundle_facts.rs"),
    include_str!("ledger.rs"),
    include_str!("disposal.rs"),
    include_str!("toolchain.rs"),
    include_str!("manifest.rs"),
    include_str!("harness.rs"),
    include_str!("phase_b.rs"),
    include_str!("stages.rs"),
    include_str!("authority.rs"),
    include_str!("platform_stage.rs"),
);

/// The complete capacity module, root first.
const CAPACITY_SOURCE: &str = concat!(
    include_str!("capacity.rs"),
    include_str!("capacity/source_budget.rs"),
    include_str!("capacity/ast_walk.rs"),
    include_str!("capacity/ast_census.rs"),
    include_str!("capacity/declaration_dag.rs"),
    include_str!("capacity/cleanup_events.rs"),
    include_str!("capacity/cleanup_retained.rs"),
    include_str!("capacity/hir_pre_resolve.rs"),
    include_str!("capacity/hir_owned.rs"),
);

/// The complete artifact projection module, root first.
const ARTIFACTS_SOURCE: &str = concat!(
    include_str!("artifacts.rs"),
    include_str!("artifacts/descriptor.rs"),
    include_str!("artifacts/descriptor_replay.rs"),
    include_str!("artifacts/header.rs"),
    include_str!("artifacts/c_expression.rs"),
    include_str!("artifacts/c_replay.rs"),
    include_str!("artifacts/c_artifact.rs"),
    include_str!("artifacts/rust_artifact.rs"),
    include_str!("artifacts/generated_replay.rs"),
);

/// The complete proof-only test module, root first.
const TESTS_SOURCE: &str = concat!(
    include_str!("tests.rs"),
    include_str!("tests/phase_a_contract.rs"),
    include_str!("tests/phase_b_publication.rs"),
    include_str!("tests/artifact_replay.rs"),
    include_str!("tests/ledger_capacity.rs"),
    include_str!("tests/phase_b_toolchain.rs"),
    include_str!("tests/phase_b_staging.rs"),
    include_str!("tests/source_census.rs"),
    include_str!("tests/cleanup_census.rs"),
    include_str!("tests/hir_traversal.rs"),
    include_str!("tests/cleanup_regions.rs"),
    include_str!("tests/resolved_disposal.rs"),
    include_str!("tests/linked_bundle.rs"),
);

const SOURCE: &str = r#"module interop.fixture;

permit { host.math }

@id("host.math")
interface HostMath
    permits { host.math }
{
    @id("host.add")
    import rust fn host_add(left: i64, right: i64) -> i64
        effects { host.math }
        failure status "host.math.v1";
}

@id("interop.add")
fn add(left: i64, right: i64) -> i64
    uses { host.math }
{
    host_add(left, right) + right
}

@id("interop.main")
fn main() -> i64
{
    0
}
"#;

fn fixture() -> (Program, String) {
    let program = crate::parse(SOURCE, Path::new("native-rust-interop.spx")).unwrap();
    let source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, source.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["interop.add".to_owned()],
        imports: vec!["host.add".to_owned()],
        capabilities: vec!["host.math".to_owned()],
    };
    let canonical = render_spec(&spec);
    (program, canonical)
}

#[derive(Default)]
struct ObservedCleanupProof {
    slot_payload_bytes: usize,
    call_argument_slot_payload_bytes: usize,
    shape_identity_bytes: usize,
    shape_field_capacity_entries: usize,
    flag_lifecycle_bytes: usize,
    flag_projection_bytes: usize,
    flag_projection_capacity_entries: usize,
    place_storage_bytes: usize,
    place_projection_bytes: usize,
    place_projection_capacity_entries: usize,
    finalizer_storage_bytes: usize,
    finalizer_projection_bytes: usize,
    finalizer_projection_capacity_entries: usize,
    finalizer_lifecycle_bytes: usize,
    inventory_slot_capacity_entries: usize,
    inventory_flag_capacity_entries: usize,
    inventory_entry_capacity_entries: usize,
    plan_slot_capacity_entries: usize,
    plan_entry_capacity_entries: usize,
    finalizer_capacity_entries: usize,
    block_capacity_entries: usize,
    edge_capacity_entries: usize,
    region_capacity_entries: usize,
    exit_capacity_entries: usize,
    status_capacity_entries: usize,
    transition_capacity_entries: usize,
    branch_edge_capacity_entries: usize,
    region_slot_capacity_entries: usize,
    exit_region_capacity_entries: usize,
    status_case_capacity_entries: usize,
}

fn observed_type_bytes(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::ArrayU8(_)
        | ResolvedType::Bytes
        | ResolvedType::Str
        | ResolvedType::SliceU8 => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            declaration.as_str().len()
                + arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + arguments.iter().map(observed_type_bytes).sum::<usize>()
        }
    }
}

fn observe_shape(
    root: &semaprax::cleanup::FieldLivenessShape,
    observed: &mut ObservedCleanupProof,
) -> Option<()> {
    let mut pending = vec![root];
    while let Some(shape) = pending.pop() {
        match shape {
            semaprax::cleanup::FieldLivenessShape::NoDrop => {}
            semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
                observed.shape_identity_bytes += lifecycle.as_str().len();
            }
            semaprax::cleanup::FieldLivenessShape::Record {
                declaration,
                fields,
            } => {
                observed.shape_identity_bytes += declaration.as_str().len();
                observed.shape_field_capacity_entries += fields.capacity();
                for field in fields {
                    observed.shape_identity_bytes += field.field.as_str().len();
                    pending.push(&field.shape);
                }
            }
            _ => return None,
        }
    }
    Some(())
}

fn observed_storage_bytes(storage: &semaprax::cleanup_plan::StorageId) -> usize {
    match storage {
        semaprax::cleanup_plan::StorageId::Value(value) => value.as_str().len(),
        semaprax::cleanup_plan::StorageId::Temporary(expression) => expression.as_str().len(),
        semaprax::cleanup_plan::StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => call.as_str().len() + value_expression.as_str().len(),
        semaprax::cleanup_plan::StorageId::ProvisionalResult => 0,
    }
}

fn observe_place(
    place: &semaprax::cleanup_plan::CleanupPlace,
    finalizer: bool,
    observed: &mut ObservedCleanupProof,
) {
    let storage = observed_storage_bytes(&place.storage);
    let projection_bytes = place
        .projections
        .iter()
        .map(|projection| projection.as_str().len())
        .sum::<usize>();
    if finalizer {
        observed.finalizer_storage_bytes += storage;
        observed.finalizer_projection_bytes += projection_bytes;
        observed.finalizer_projection_capacity_entries += place.projections.capacity();
    } else {
        observed.place_storage_bytes += storage;
        observed.place_projection_bytes += projection_bytes;
        observed.place_projection_capacity_entries += place.projections.capacity();
    }
}

fn observe_cleanup_function(
    function: &ResolvedFunction,
    observed: &mut ObservedCleanupProof,
) -> Option<()> {
    use semaprax::cleanup::CleanupStorageOrigin;
    use semaprax::cleanup_plan::{
        CleanupResultSource, CleanupTerminator, CleanupTransition, ExitContinuation, StatusProducer,
    };

    observed.inventory_slot_capacity_entries += function.cleanup.slots.capacity();
    observed.inventory_flag_capacity_entries += function.cleanup.flags.capacity();
    observed.inventory_entry_capacity_entries += function
        .cleanup
        .entry_state
        .live_owned_parameters
        .capacity();
    for slot in &function.cleanup.slots {
        observed.slot_payload_bytes += match &slot.origin {
            CleanupStorageOrigin::Parameter { value, .. }
            | CleanupStorageOrigin::Binding { value }
            | CleanupStorageOrigin::ProvisionalResult { value } => value.as_str().len(),
            CleanupStorageOrigin::Temporary { expression } => expression.as_str().len(),
            _ => return None,
        };
        observed.slot_payload_bytes += observed_type_bytes(&slot.ty);
        observe_shape(&slot.shape, observed)?;
    }
    for flag in &function.cleanup.flags {
        observed.flag_lifecycle_bytes += flag.lifecycle.as_str().len();
        observed.flag_projection_bytes += flag
            .place
            .projections
            .iter()
            .map(|projection| projection.as_str().len())
            .sum::<usize>();
        observed.flag_projection_capacity_entries += flag.place.projections.capacity();
    }

    let plan = &function.cleanup_plan;
    observed.plan_slot_capacity_entries += plan.slots.capacity();
    observed.plan_entry_capacity_entries += plan.entry_state.live_owned_parameters.capacity();
    observed.block_capacity_entries += plan.blocks.capacity();
    observed.edge_capacity_entries += plan.edges.capacity();
    observed.region_capacity_entries += plan.regions.capacity();
    observed.exit_capacity_entries += plan.exits.capacity();
    observed.status_capacity_entries += plan.status_sources.capacity();
    for slot in &plan.slots {
        let payload = observed_storage_bytes(&slot.storage) + observed_type_bytes(&slot.ty);
        if matches!(
            slot.storage,
            semaprax::cleanup_plan::StorageId::CallArgument { .. }
        ) {
            observed.call_argument_slot_payload_bytes += payload;
        } else {
            observed.slot_payload_bytes += payload;
        }
        observe_shape(&slot.field_liveness_shape, observed)?;
    }
    for place in &plan.entry_state.live_owned_parameters {
        observe_place(place, false, observed);
    }
    for status in &plan.status_sources {
        if let StatusProducer::CheckedArithmetic {
            normalized_cases, ..
        } = &status.producer
        {
            observed.status_case_capacity_entries += normalized_cases.capacity();
        }
    }
    for block in &plan.blocks {
        observed.transition_capacity_entries += block.transitions.capacity();
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { destination, .. } => {
                    observe_place(destination, false, observed);
                }
                CleanupTransition::InitializeVariant { destination, .. } => {
                    observe_place(destination, false, observed);
                }
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } => {
                    observe_place(source, false, observed);
                    observe_place(destination, false, observed);
                }
                CleanupTransition::TransferVariant {
                    source,
                    destination,
                    ..
                } => {
                    observe_place(source, false, observed);
                    observe_place(destination, false, observed);
                }
                CleanupTransition::AuthenticateVariantCase { source, .. } => {
                    observe_place(source, false, observed);
                }
                CleanupTransition::CallCommit { arguments, .. } => {
                    for argument in arguments {
                        observe_place(&argument.source, false, observed);
                    }
                }
                CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        if let CleanupTerminator::Branch(edges) = &block.terminator {
            observed.branch_edge_capacity_entries += edges.capacity();
        }
    }
    for region in &plan.regions {
        observed.region_slot_capacity_entries += region.slots.capacity();
        observed.place_storage_bytes += region
            .slots
            .iter()
            .map(observed_storage_bytes)
            .sum::<usize>();
    }
    for exit in &plan.exits {
        observed.exit_region_capacity_entries += exit.leaves_regions.capacity();
        observed.finalizer_capacity_entries += exit.finalize_in_order.capacity();
        for action in &exit.finalize_in_order {
            observe_place(&action.source, true, observed);
            observed.finalizer_lifecycle_bytes += action.lifecycle_id.as_str().len();
        }
        if let ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage },
        } = &exit.continuation
        {
            observe_place(storage, false, observed);
        }
    }
    Some(())
}

fn prepare_source(
    source: &str,
    exports: &[&str],
    imports: &[&str],
) -> Result<PreparedNativeRustInterop, Vec<Diagnostic>> {
    let program = crate::parse(source, Path::new("native-rust-unit-builder.spx"))
        .map_err(|diagnostic| vec![diagnostic])?;
    hir::resolve(&program)?;
    let canonical = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
        target: current_target().unwrap(),
        exports: exports.iter().map(|value| (*value).to_owned()).collect(),
        imports: imports.iter().map(|value| (*value).to_owned()).collect(),
        capabilities: Vec::new(),
    };
    prepare_native_rust_interop(&program, render_spec(&spec).as_bytes())
}

fn each_byte_edit(label: &str, value: &str, mut reject: impl FnMut(&str) -> bool) {
    let bytes = value.as_bytes();
    for index in 0..bytes.len() {
        let mut mutation = bytes.to_vec();
        mutation[index] = match mutation[index] {
            b'x' => b'y',
            byte if byte.is_ascii() => b'x',
            _ => continue,
        };
        let Ok(mutation) = String::from_utf8(mutation) else {
            continue;
        };
        assert!(
            reject(&mutation),
            "{label} substitution at byte {index} was accepted"
        );
        let mut deletion = bytes.to_vec();
        deletion.remove(index);
        assert!(
            reject(std::str::from_utf8(&deletion).unwrap()),
            "{label} deletion at byte {index} was accepted"
        );
    }
    for index in 0..=bytes.len() {
        let mut insertion = bytes.to_vec();
        insertion.insert(index, b'x');
        assert!(
            reject(std::str::from_utf8(&insertion).unwrap()),
            "{label} insertion at byte {index} was accepted"
        );
    }
    for index in 0..bytes.len() {
        assert!(
            reject(std::str::from_utf8(&bytes[..index]).unwrap()),
            "{label} truncation at byte {index} was accepted"
        );
    }
}

fn assert_resolved_owner_disposes_once_without_growth(
    resolved: ResolvedProgram,
    capacity: usize,
) -> usize {
    RESOLVED_DISPOSE_HIGH_WATER.with(|water| water.set(0));
    RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
    RESOLVED_DISPOSE_CAPACITIES.with(|capacities| capacities.set([0; 2]));
    let frames = Vec::with_capacity(capacity);
    assert_eq!(frames.capacity(), capacity);
    let owner = ResolvedProgramOwner::new(resolved, frames, capacity);
    drop(owner);
    assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
    let high_water = RESOLVED_DISPOSE_HIGH_WATER.with(std::cell::Cell::get);
    assert!(high_water > 0);
    assert!(high_water <= capacity);
    assert_eq!(
        RESOLVED_DISPOSE_CAPACITIES.with(std::cell::Cell::get),
        [capacity; 2]
    );
    high_water
}
