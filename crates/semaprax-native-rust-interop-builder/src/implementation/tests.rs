use super::*;

use std::path::Path;
use std::process::Command;

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
        source_revision: domain_digest(SOURCE_DOMAIN, source.as_bytes()),
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
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String => 0,
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
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } => {
                    observe_place(source, false, observed);
                    observe_place(destination, false, observed);
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

#[test]
fn build_race_hooks_reject_each_pre_effect_mutation_and_preserve_foreign_bytes() {
    use std::io::Write as _;

    let points = [
        NativeRustBuildPoint::BeforeClang,
        NativeRustBuildPoint::BeforeRustLink,
        NativeRustBuildPoint::BeforeExecutableAuthentication,
        NativeRustBuildPoint::BeforeExecute,
        NativeRustBuildPoint::BeforeObjectRead,
        NativeRustBuildPoint::BeforeManifestPublish,
        NativeRustBuildPoint::BeforeBundlePublish,
    ];
    let complete_order = [
        NativeRustBuildPoint::BeforeClang,
        NativeRustBuildPoint::BeforeRustLink,
        NativeRustBuildPoint::BeforeExecutableAuthentication,
        NativeRustBuildPoint::BeforeExecute,
        NativeRustBuildPoint::BeforeExecutableAuthentication,
        NativeRustBuildPoint::BeforeExecute,
        NativeRustBuildPoint::BeforeObjectRead,
        NativeRustBuildPoint::BeforeManifestPublish,
        NativeRustBuildPoint::BeforeBundlePublish,
    ];
    let (program, spec) = fixture();
    for (index, selected) in points.into_iter().enumerate() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-race-hook-{}-{index}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let output = root.join("bundle");
        let mut fired = false;
        let mut observed = Vec::with_capacity(complete_order.len());
        PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
        PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
        PHASE_B_INVENTORY_EXACT_SCANS.with(|count| count.set(0));
        PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
        PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(0));
        reset_phase_b_object_authority_observer();
        reset_phase_b_manifest_authority_observer();
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &output,
            |point, publish, run, final_output| {
                observed.push(point);
                if point != selected {
                    return;
                }
                assert!(!fired, "hook {selected:?} fired more than once");
                fired = true;
                match point {
                    NativeRustBuildPoint::BeforeClang => {
                        append_hostile(&publish.join("module.c"));
                    }
                    NativeRustBuildPoint::BeforeRustLink => {
                        append_hostile(&run.join("__semaprax_native_rust_link.rs"));
                    }
                    NativeRustBuildPoint::BeforeExecutableAuthentication => {
                        append_hostile(&run.join("__semaprax_native_rust_main.o"));
                    }
                    NativeRustBuildPoint::BeforeExecute => {
                        append_hostile(&run.join(if cfg!(windows) {
                            "__semaprax_native_rust_link_O0.exe"
                        } else {
                            "__semaprax_native_rust_link_O0"
                        }));
                    }
                    NativeRustBuildPoint::BeforeObjectRead => {
                        append_hostile(&publish.join(if cfg!(windows) {
                            "module.obj"
                        } else {
                            "module.o"
                        }));
                    }
                    NativeRustBuildPoint::BeforeManifestPublish => {
                        std::fs::write(publish.join("foreign-sentinel"), b"foreign").unwrap();
                    }
                    NativeRustBuildPoint::BeforeBundlePublish => {
                        std::fs::create_dir(final_output).unwrap();
                        std::fs::write(final_output.join("foreign-sentinel"), b"foreign").unwrap();
                    }
                }
            },
        );
        assert!(fired, "hook {selected:?} was not reached");
        let selected_index = complete_order
            .iter()
            .position(|point| *point == selected)
            .unwrap();
        assert_eq!(
            observed,
            complete_order[..=selected_index],
            "hook {selected:?} allowed a later action or skipped an earlier action",
        );
        let error = match result {
            Ok(_) => panic!("hostile hook {selected:?} unexpectedly published"),
            Err(error) => error,
        };
        assert_eq!(error.len(), 1);
        assert_eq!(
            error[0].code,
            if selected == NativeRustBuildPoint::BeforeExecute {
                "SPX-I231"
            } else {
                "SPX-I232"
            }
        );
        let carrier = if selected == NativeRustBuildPoint::BeforeExecute {
            PhaseBLocalError::Link
        } else {
            PhaseBLocalError::Publication
        };
        assert_eq!(
            error[0].message.as_ptr() as usize,
            PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)[carrier.index()],
            "hook {selected:?} did not return its pre-effect carrier",
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "hook {selected:?} materialized an error after effects",
        );
        assert_eq!(
            PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get),
            2,
            "hook {selected:?} did not attempt run and publish settlement exactly once",
        );
        assert_eq!(PHASE_B_INVENTORY_EXACT_PLANS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PHASE_B_INVENTORY_EXACT_SCANS.with(std::cell::Cell::get),
            match selected {
                NativeRustBuildPoint::BeforeManifestPublish => 1,
                NativeRustBuildPoint::BeforeBundlePublish => 2,
                _ => 0,
            },
            "hook {selected:?} crossed an unexpected exact-inventory scan boundary",
        );
        assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PHASE_B_PUBLISH_CONSUMPTIONS.with(std::cell::Cell::get),
            usize::from(selected == NativeRustBuildPoint::BeforeBundlePublish),
            "hook {selected:?} consumed final publication unexpectedly",
        );
        let expected_transfer = usize::from(selected != NativeRustBuildPoint::BeforeClang);
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
            expected_transfer,
            "hook {selected:?} transferred the O2 authority at the wrong boundary",
        );
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get),
            expected_transfer,
            "hook {selected:?} did not release the O2 authority exactly once",
        );
        let expected_outer_observation =
            usize::from(selected == NativeRustBuildPoint::BeforeBundlePublish);
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(std::cell::Cell::get),
            expected_outer_observation,
        );
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(std::cell::Cell::get),
            expected_outer_observation,
        );
        assert_phase_b_object_drop_order(expected_transfer);
        assert_eq!(
            PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
            1
        );
        assert_eq!(PHASE_B_MANIFEST_ARENA_GROWTHS.with(std::cell::Cell::get), 0);
        assert_eq!(
            PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
            1
        );
        assert_phase_b_manifest_drop_order(1);
        if selected == NativeRustBuildPoint::BeforeBundlePublish {
            assert_eq!(
                std::fs::read(output.join("foreign-sentinel")).unwrap(),
                b"foreign"
            );
        } else {
            assert!(!output.exists());
        }
        if selected == NativeRustBuildPoint::BeforeManifestPublish {
            let sentinel = std::fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("foreign-sentinel"))
                .find(|path| path.is_file())
                .unwrap();
            assert_eq!(std::fs::read(sentinel).unwrap(), b"foreign");
        }
        let stages = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".semaprax-native-rust-interop-")
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        match selected {
            NativeRustBuildPoint::BeforeManifestPublish => {
                assert_eq!(
                    stages.len(),
                    1,
                    "foreign inventory uncertainty must leave one inert stage"
                );
            }
            NativeRustBuildPoint::BeforeClang
            | NativeRustBuildPoint::BeforeRustLink
            | NativeRustBuildPoint::BeforeExecutableAuthentication
            | NativeRustBuildPoint::BeforeExecute => {
                assert_eq!(stages.len(), 1, "mutated held bytes must leave one inert stage rather than deleting uncertain data");
            }
            NativeRustBuildPoint::BeforeObjectRead => {
                let expected = if cfg!(any(target_os = "linux", windows)) {
                    2
                } else {
                    1
                };
                assert_eq!(
                    stages.len(),
                    expected,
                    "mutating a hard-linked object must preserve both uncertain stages"
                );
            }
            NativeRustBuildPoint::BeforeBundlePublish => {
                assert!(
                    stages.is_empty(),
                    "owned publish stage must settle when only the foreign final output conflicts"
                );
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn append_hostile(path: &Path) {
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"hostile")
            .unwrap();
    }
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_prepared_publish_open_information_and_rename_failures_are_sticky() {
    let (program, spec) = fixture();
    for point in [1_u8, 2, 4] {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-publish-failure-{}-{point}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let output = root.join("bundle");
        PHASE_B_PUBLISH_FAILURE.with(|selected| selected.set(point));
        PHASE_B_DISCARD_ATTEMPTS.with(|count| count.set(0));
        PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
        PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(0));
        let mut reached_publish = false;
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &output,
            |boundary, _, _, _| {
                if boundary == NativeRustBuildPoint::BeforeBundlePublish {
                    reached_publish = true;
                }
            },
        );
        PHASE_B_PUBLISH_FAILURE.with(|selected| selected.set(0));
        assert!(reached_publish);
        let diagnostics = match result {
            Ok(_) => panic!("injected publish failure {point} unexpectedly succeeded"),
            Err(diagnostics) => diagnostics,
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-I232");
        assert_eq!(
            diagnostics[0].message.as_ptr() as usize,
            PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
                [PhaseBLocalError::Publication.index()]
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0
        );
        assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
        assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
        assert_eq!(PHASE_B_PUBLISH_CONSUMPTIONS.with(std::cell::Cell::get), 1);
        assert!(!output.exists());
        assert!(
            std::fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".semaprax-native-rust-interop-")),
            "owned stages must settle after injected publish failure {point}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_prepared_publish_close_failure_child() {
    let Ok(root) = std::env::var("SEMAPRAX_PUBLISH_CLOSE_FAILURE_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    let (program, spec) = fixture();
    PHASE_B_PUBLISH_FAILURE.with(|selected| selected.set(3));
    let _ = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| {},
    );
    std::fs::write(root.join("later-action"), b"must not exist").unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_prepared_publish_close_uncertainty_is_fail_stop() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-publish-close-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("implementation::tests::phase_b_prepared_publish_close_failure_child")
        .arg("--nocapture")
        .env("SEMAPRAX_PUBLISH_CLOSE_FAILURE_ROOT", &root)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!root.join("later-action").exists());
    std::fs::remove_dir_all(root).unwrap();
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
        source_revision: domain_digest(SOURCE_DOMAIN, canonical.as_bytes()),
        target: current_target().unwrap(),
        exports: exports.iter().map(|value| (*value).to_owned()).collect(),
        imports: imports.iter().map(|value| (*value).to_owned()).collect(),
        capabilities: Vec::new(),
    };
    prepare_native_rust_interop(&program, render_spec(&spec).as_bytes())
}

#[test]
fn native_unit_import_is_exact_direct_unused_let_and_resolved_identity_scoped() {
    const UNIT_SOURCE: &str = r#"module interop.unit;

@id("host.unit")
interface HostUnit
    permits {  }
{
    @id("host.unit.ping")
    import rust fn ping(value: i64) -> unit
        effects {  }
        failure infallible;
}

@id("interop.unit.selected")
fn selected(value: i64) -> i64
{
    let acknowledged = ping(value);
    let outcome = value + 1;
    outcome
}

@id("interop.unit.unselected")
fn unselected(value: i64) -> i64
{
    let acknowledged = ping(value);
    let outcome = 7;
    outcome
}

@id("interop.unit.main")
fn main() -> i64
{
    0
}
"#;
    let prepared = prepare_source(UNIT_SOURCE, &["interop.unit.selected"], &["host.unit.ping"])
        .unwrap_or_else(|errors| panic!("unit prepare: {errors:?}"));
    assert_eq!(prepared.exports.len(), 1);
    assert_eq!(prepared.imports.len(), 1);
    assert!(prepared.imports[0].result == ScalarType::Unit);

    for hostile in [
            UNIT_SOURCE.replacen(
                "    let outcome = value + 1;\n    outcome",
                "    let outcome = value + 1;\n    acknowledged",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let outcome = value + 1;\n    outcome",
                "    let outcome = selected(acknowledged);\n    outcome",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let acknowledged = ping(value);",
                "    let acknowledged = { ping(value) };",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let acknowledged = ping(value);\n    let outcome = value + 1;",
                "    let acknowledged = 0;\n    let outcome = if ping(value) { 1 } else { 2 };",
                1,
            ),
            UNIT_SOURCE.replacen(
                "    let acknowledged = ping(value);\n    let outcome = value + 1;",
                "    let acknowledged = 0;\n    let outcome = if true { ping(value) } else { ping(value) };",
                1,
            ),
            UNIT_SOURCE
                .replacen(
                    "@id(\"interop.unit.selected\")",
                    "@id(\"interop.unit.helper\")\nfn helper(value: i64) -> unit\n{\n    ping(value)\n}\n\n@id(\"interop.unit.selected\")",
                    1,
                )
                .replacen(
                    "    let acknowledged = ping(value);",
                    "    let acknowledged = helper(value);",
                    1,
                ),
        ] {
            let errors =
                match prepare_source(&hostile, &["interop.unit.selected"], &["host.unit.ping"]) {
                    Ok(_) => panic!("hostile Unit use was accepted"),
                    Err(errors) => errors,
                };
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code, "SPX-B107");
            assert_eq!(
                errors[0].message,
                "Native Rust Interop declaration set is unsupported: scalar value signature required"
            );
        }

    let unit_export = UNIT_SOURCE.replacen(
            "fn selected(value: i64) -> i64\n{\n    let acknowledged = ping(value);\n    let outcome = value + 1;\n    outcome",
            "fn selected(value: i64) -> unit\n{\n    let acknowledged = ping(value);\n    acknowledged",
            1,
        );
    let errors = match prepare_source(
        &unit_export,
        &["interop.unit.selected"],
        &["host.unit.ping"],
    ) {
        Ok(_) => panic!("Unit export was accepted"),
        Err(errors) => errors,
    };
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "SPX-B107");
    assert_eq!(
        errors[0].message,
        "Native Rust Interop declaration set is unsupported: scalar value signature required"
    );
}

#[test]
fn multi_export_contract_binds_global_capabilities_and_import_table_exactly() {
    assert_eq!(
        replay_symbol_hash("interop.add"),
        "ee967df46a76c68f1e8650d38ddb6886c897b34a82c4ea48ed3f70788e911326"
    );
    assert_eq!(
        replay_capabilities_digest(&["host.math".to_owned()]),
        "sha256:d510605f56f47934126eeac931a6b363d7da36f492af90bb36ff573b00fb7d84"
    );
    let source = r#"module interop.disjoint;

permit { cap.a, cap.b }

@id("host.a")
interface HostA permits { cap.a } {
    @id("host.a.call")
    import rust fn call_a(value: i64) -> i64
        effects { cap.a }
        failure infallible;
}

@id("host.b")
interface HostB permits { cap.b } {
    @id("host.b.call")
    import rust fn call_b(value: i64) -> i64
        effects { cap.b }
        failure infallible;
}

@id("export.a")
fn export_a(value: i64) -> i64 uses { cap.a } { call_a(value) }

@id("export.b")
fn export_b(value: i64) -> i64 uses { cap.b } { call_b(value) }

@id("interop.disjoint.main")
fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("native-rust-disjoint.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical.as_bytes()),
        target: current_target().unwrap(),
        exports: vec!["export.a".to_owned(), "export.b".to_owned()],
        imports: vec!["host.a.call".to_owned(), "host.b.call".to_owned()],
        capabilities: vec!["cap.a".to_owned(), "cap.b".to_owned()],
    };
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    for export in &prepared.exports {
        assert_eq!(export.capabilities, spec.capabilities);
        assert_eq!(export.required_imports, spec.imports);
    }
    assert_eq!(
        prepared
            .descriptor
            .matches("\"required_imports\":[\"host.a.call\",\"host.b.call\"]")
            .count(),
        2
    );
    assert!(prepared
        .generated_rust
        .contains("const EXPECTED_CAPABILITIES:&[&str]=&[\"cap.a\",\"cap.b\"]"));
    assert!(prepared.generated_c.contains("spxnr_validate_import_"));

    let first = call_digest(
        "export",
        "delimiter.test",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["a,b".to_owned(), "c".to_owned()],
        &[("a,b".to_owned(), "sha256:first".to_owned())],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    let second = call_digest(
        "export",
        "delimiter.test",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["a".to_owned(), "b,c".to_owned()],
        &[("a".to_owned(), "sha256:first".to_owned())],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    assert_ne!(first, second);

    let import_i64 = call_digest(
        "import",
        "same.id",
        &[ParameterFact {
            name: "value".to_owned(),
            ty: ScalarType::I64,
        }],
        ScalarType::I64,
        &[],
        &[],
        &[],
        &[],
        "infallible",
        0,
        &spec.target,
    )
    .unwrap();
    let import_bool = call_digest(
        "import",
        "same.id",
        &[ParameterFact {
            name: "value".to_owned(),
            ty: ScalarType::Bool,
        }],
        ScalarType::I64,
        &[],
        &[],
        &[],
        &[],
        "infallible",
        0,
        &spec.target,
    )
    .unwrap();
    assert_ne!(import_i64, import_bool);
    let export_for_i64 = call_digest(
        "export",
        "export.same",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["same.id".to_owned()],
        &[("same.id".to_owned(), import_i64)],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    let export_for_bool = call_digest(
        "export",
        "export.same",
        &[],
        ScalarType::I64,
        &[],
        &[],
        &["same.id".to_owned()],
        &[("same.id".to_owned(), import_bool)],
        "status",
        0,
        &spec.target,
    )
    .unwrap();
    assert_ne!(export_for_i64, export_for_bool);
}

#[test]
fn private_a_is_canonical_and_pure() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    assert_eq!(prepared.canonical_spec, spec);
    assert!(prepared.descriptor.ends_with('\n'));
    assert!(prepared.generated_c.contains("spxnr1_i_"));
    assert!(prepared.generated_header.contains("spxnr_context_v1"));
    assert!(prepared
        .generated_rust
        .starts_with("mod api{#![forbid(unsafe_code)]"));
    assert!(prepared
        .private_ffi_source
        .starts_with("#![allow(unsafe_code)]"));
}

#[test]
fn source_descriptor_and_generated_views_reconstruct_from_authenticated_facts() {
    let (program, spec_source) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
    let spec = parse_spec(&program, spec_source.as_bytes()).unwrap();
    let status_domains = prepared
        .imports
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let reconstructed = render_descriptor(
        &spec,
        &prepared.hir_digest,
        &status_domains,
        &prepared.exports,
        &prepared.imports,
    )
    .unwrap();
    assert_eq!(reconstructed, prepared.descriptor);
    assert!(reconstructed.contains(
            "\"status_domains\":[{\"ordinal\":0,\"domain_id\":\"success\"},{\"ordinal\":1,\"domain_id\":\"host.math.v1\"},{\"ordinal\":65533,\"domain_id\":\"semaprax.native-rust-semantics.v1\"},{\"ordinal\":65534,\"domain_id\":\"semaprax.native-rust-host.v1\"},{\"ordinal\":65535,\"domain_id\":\"semaprax.native-rust-adapter.v1\"}]"
        ));
    assert!(reconstructed.contains("\"status_domain_ordinals\":[1,65533,65534,65535]"));
    assert_eq!(
        domain_digest(DESCRIPTOR_DIGEST_DOMAIN, reconstructed.as_bytes()),
        prepared.descriptor_digest
    );
    assert_eq!(
        domain_digest(SOURCE_DOMAIN, crate::format::canonical(&program).as_bytes()),
        prepared.source_revision
    );
    replay_descriptor(
        &reconstructed,
        &spec,
        &prepared.hir_digest,
        &prepared.exports,
        &prepared.imports,
    )
    .unwrap();
    replay_generated(
        &prepared.generated_header,
        &prepared.generated_c,
        &prepared.generated_rust,
        &prepared.private_ffi_source,
    )
    .unwrap();

    let changed_source = SOURCE.replacen("host_add(left, right)", "host_add(right, left)", 1);
    let changed = crate::parse(
        &changed_source,
        Path::new("native-rust-interop-changed.spx"),
    )
    .unwrap();
    let stale = match prepare_native_rust_interop(&changed, spec_source.as_bytes()) {
        Ok(_) => panic!("stale source binding was accepted"),
        Err(error) => error,
    };
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].code, "SPX-B107");
    assert_eq!(
        stale[0].message,
        "Native Rust Interop declaration set is unsupported: selected identity missing"
    );

    let mut changed_spec = spec;
    changed_spec.source_revision =
        domain_digest(SOURCE_DOMAIN, crate::format::canonical(&changed).as_bytes());
    let changed_prepared =
        prepare_native_rust_interop(&changed, render_spec(&changed_spec).as_bytes()).unwrap();
    assert_ne!(changed_prepared.source_revision, prepared.source_revision);
    assert_ne!(changed_prepared.hir_digest, prepared.hir_digest);
    assert_ne!(
        changed_prepared.descriptor_digest,
        prepared.descriptor_digest
    );
    assert_ne!(changed_prepared.generated_c, prepared.generated_c);
}

#[test]
fn descriptor_and_generated_source_replay_reject_every_bound_family() {
    let (program, spec_source) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
    let spec = parse_spec(&program, spec_source.as_bytes()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();

    let descriptor_mutations = [
        prepared.descriptor.replacen(
            "\"module\":\"interop.fixture\"",
            "\"module\":\"interop.forgery\"",
            1,
        ),
        prepared
            .descriptor
            .replacen(&prepared.source_revision, "sha256:forged-source", 1),
        prepared
            .descriptor
            .replacen(&prepared.hir_digest, "sha256:forged-hir", 1),
        prepared
            .descriptor
            .replacen("\"pointer_width\":64", "\"pointer_width\":32", 1),
        prepared
            .descriptor
            .replacen("\"ordinal\":65533", "\"ordinal\":65532", 1),
        prepared.descriptor.replacen(
            "\"calling_convention\":\"C\"",
            "\"calling_convention\":\"X\"",
            1,
        ),
        prepared
            .descriptor
            .replacen("\"id\":\"interop.add\"", "\"id\":\"interop.bad\"", 1),
        prepared
            .descriptor
            .replacen("\"id\":\"host.add\"", "\"id\":\"host.bad\"", 1),
        prepared
            .descriptor
            .replacen("\"max_exports\":32", "\"max_exports\":31", 1),
        prepared.descriptor.replacen(
            "no_resource_owned_borrow_shared_or_aggregate_abi",
            "xo_resource_owned_borrow_shared_or_aggregate_abi",
            1,
        ),
        prepared.descriptor.trim_end().to_owned(),
    ];
    for (index, mutation) in descriptor_mutations.into_iter().enumerate() {
        let error = replay_descriptor(
            &mutation,
            &spec,
            &prepared.hir_digest,
            &prepared.exports,
            &prepared.imports,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-B108", "mutation {index}");
        assert_eq!(
            error.message, "Native Rust Interop descriptor disagrees with validated source and HIR",
            "mutation {index}"
        );
    }

    let generated_mutations = [
        (
            prepared.generated_header.replacen("#ifndef", "#ifndez", 1),
            prepared.generated_c.clone(),
            prepared.generated_rust.clone(),
            prepared.private_ffi_source.clone(),
        ),
        (
            prepared.generated_header.clone(),
            prepared.generated_c.replacen("#include", "#includx", 1),
            prepared.generated_rust.clone(),
            prepared.private_ffi_source.clone(),
        ),
        (
            prepared.generated_header.clone(),
            prepared.generated_c.clone(),
            prepared.generated_rust.replacen("forbid", "forbia", 1),
            prepared.private_ffi_source.clone(),
        ),
        (
            prepared.generated_header.clone(),
            prepared.generated_c.clone(),
            prepared.generated_rust.clone(),
            prepared.private_ffi_source.replacen("allow", "allox", 1),
        ),
    ];
    for (index, (header, c, rust, ffi)) in generated_mutations.into_iter().enumerate() {
        let error = replay_generated_exact(
            &spec,
            &closure,
            &prepared.exports,
            &prepared.imports,
            &header,
            &c,
            &rust,
            &ffi,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-B111", "generated mutation {index}");
        assert_eq!(
            error.message, "Native Rust Interop generated artifact replay failed",
            "generated mutation {index}"
        );
    }
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

#[test]
fn exact_replayers_reject_every_generated_and_descriptor_byte_substitution() {
    let (program, spec_source) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
    let spec = parse_spec(&program, spec_source.as_bytes()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();

    each_byte_edit("spec", &spec_source, |mutation| {
        !replay_spec_bytes_exact(mutation, &spec)
    });

    each_byte_edit("descriptor", &prepared.descriptor, |mutation| {
        replay_descriptor(
            mutation,
            &spec,
            &prepared.hir_digest,
            &prepared.exports,
            &prepared.imports,
        )
        .is_err()
    });

    let artifacts = [
        (0, prepared.generated_header.as_str()),
        (1, prepared.generated_c.as_str()),
        (2, prepared.generated_rust.as_str()),
        (3, prepared.private_ffi_source.as_str()),
    ];
    for (selected, artifact) in artifacts {
        each_byte_edit("generated", artifact, |mutation| {
            let mut values = [
                prepared.generated_header.as_str(),
                prepared.generated_c.as_str(),
                prepared.generated_rust.as_str(),
                prepared.private_ffi_source.as_str(),
            ];
            values[selected] = mutation;
            replay_generated_exact(
                &spec,
                &closure,
                &prepared.exports,
                &prepared.imports,
                values[0],
                values[1],
                values[2],
                values[3],
            )
            .is_err()
        });
    }

    let files = [("descriptor.json", prepared.descriptor.as_bytes())];
    let rustc = RustcVersion::from_fields([
        "1.0.0",
        "0123456789abcdef",
        &prepared.target.triple,
        "20.0.0",
    ]);
    let manifest = render_manifest(
        &prepared,
        &files,
        "/held/clang",
        "clang version 20.0.0",
        &rustc,
        &prepared.target.triple,
    );
    each_byte_edit("manifest", &manifest, |mutation| {
        !replay_manifest_bytes_exact(
            mutation,
            &prepared,
            &files,
            "/held/clang",
            "clang version 20.0.0",
            &rustc,
            &prepared.target.triple,
        )
    });
}

#[test]
fn manifest_fixed_names_and_streaming_cursor_work_are_exact() {
    assert_eq!(
        canonical_manifest_file_names(),
        [
            "descriptor.json",
            "module.c",
            if cfg!(windows) {
                "module.obj"
            } else {
                "module.o"
            },
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
        ]
    );

    let assert_linear = |encoded: &str, decoded: &str| {
        let mut cursor = ManifestCursor::new(encoded).unwrap();
        cursor.string_eq(decoded).unwrap();
        let work = cursor.finish().unwrap();
        assert_eq!(work, encoded.len());
        assert!(work <= encoded.len().checked_mul(2).unwrap());
    };
    {
        let decoded = "a".repeat(MAX_MANIFEST_BYTES - 2);
        let mut encoded = String::with_capacity(MAX_MANIFEST_BYTES);
        encoded.push('"');
        encoded.push_str(&decoded);
        encoded.push('"');
        assert_linear(&encoded, &decoded);
    }
    {
        let decoded = "é".repeat((MAX_MANIFEST_BYTES - 2) / 2);
        let mut encoded = String::with_capacity(MAX_MANIFEST_BYTES);
        encoded.push('"');
        encoded.push_str(&decoded);
        encoded.push('"');
        assert_linear(&encoded, &decoded);
    }
    {
        let characters = (MAX_MANIFEST_BYTES - 2) / 6;
        let decoded = "a".repeat(characters);
        let mut encoded = String::with_capacity(characters * 6 + 2);
        encoded.push('"');
        for _ in 0..characters {
            encoded.push_str("\\u0061");
        }
        encoded.push('"');
        assert_linear(&encoded, &decoded);
    }

    for malformed in [
        "\"\\ud800\"",
        "\"\\udc00\"",
        "\"\\ud800\\u0000\"",
        "\"\\x\"",
    ] {
        let mut cursor = ManifestCursor::new(malformed).unwrap();
        assert!(cursor.string_eq("x").is_err());
    }
    let mut leading_zero = ManifestCursor::new("01").unwrap();
    assert!(leading_zero.usize_eq(1).is_err());
    let overflow = "9".repeat(usize::BITS as usize + 2);
    let mut overflow = ManifestCursor::new(&overflow).unwrap();
    assert!(overflow.usize_eq(usize::MAX).is_err());
}

#[test]
fn six_output_artifact_known_answer_vectors_are_frozen() {
    fn independent_sha256(bytes: &[u8]) -> String {
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
        )
    }

    fn independent_domain_sha256(domain: &[u8], bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(bytes);
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hasher.finalize())
        )
    }

    fn assert_raw_kat(name: &str, bytes: &[u8], length: usize, digest: &str) {
        assert_eq!(bytes.len(), length, "{name} byte length changed");
        assert_eq!(
            independent_sha256(bytes),
            digest,
            "{name} raw SHA-256 changed"
        );
    }

    with_test_target(
        Target {
            triple: "x86_64-unknown-linux-gnu".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, spec_source) = fixture();
            let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
            let object = b"semaprax-native-rust-interop-kat-object-v1";
            let files = [
                ("descriptor.json", prepared.descriptor.as_bytes()),
                ("module.c", prepared.generated_c.as_bytes()),
                ("module.o", object.as_slice()),
                (
                    "semaprax_native_rust_interop.h",
                    prepared.generated_header.as_bytes(),
                ),
                (
                    "semaprax_native_rust_interop.rs",
                    prepared.generated_rust.as_bytes(),
                ),
                (
                    "semaprax_native_rust_interop_ffi.rs",
                    prepared.private_ffi_source.as_bytes(),
                ),
            ];
            let rustc = RustcVersion::from_fields([
                "1.88.0",
                "0123456789abcdef",
                &prepared.target.triple,
                "20.1.0",
            ]);
            let manifest = render_manifest(
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            );
            assert_raw_kat(
                "descriptor.json",
                prepared.descriptor.as_bytes(),
                4_498,
                "sha256:603c609409a2e35ee524481aa2225c6f0c6557dbff7d9650df8057daebcf173c",
            );
            assert_raw_kat(
                "semaprax.native-rust-interop.json",
                manifest.as_bytes(),
                3_493,
                "sha256:e8652276a9ea4489c5758aaa1e24456c9b3be96538384a125c8f41fb714b73ca",
            );
            assert_raw_kat(
                "semaprax_native_rust_interop.h",
                prepared.generated_header.as_bytes(),
                870,
                "sha256:3ebdf5567d93b9e24ccdea5a0bb76d83b7bdcc44721e2a65846f83f1c92ace3b",
            );
            assert_raw_kat(
                "module.c",
                prepared.generated_c.as_bytes(),
                4_124,
                "sha256:1f1640553fe746b0c2baef87b76ce1013ee2ac9f8ee1bc9209dae6b4ccbb3e61",
            );
            assert_raw_kat(
                "semaprax_native_rust_interop.rs",
                prepared.generated_rust.as_bytes(),
                2_100,
                "sha256:b75eb57f911ea274cd1ae5fb1a4b789f58008613d027c94d904c90d3085e2d62",
            );
            assert_raw_kat(
                "semaprax_native_rust_interop_ffi.rs",
                prepared.private_ffi_source.as_bytes(),
                4_719,
                "sha256:f317bef66a0ac44ba4ba89862ae645383f7d48668277f6a8fa559ada8fc4ff9a",
            );

            let descriptor_domain =
                "sha256:d10e85e8fefed377df137ac22791099a702b460ed31ea3c65a6061b222e0c7ba";
            assert_eq!(prepared.descriptor_digest, descriptor_domain);
            assert_eq!(
                independent_domain_sha256(DESCRIPTOR_DIGEST_DOMAIN, prepared.descriptor.as_bytes()),
                descriptor_domain
            );
            assert_eq!(
                independent_domain_sha256(BUNDLE_DIGEST_DOMAIN, manifest.as_bytes()),
                "sha256:4fbab384e26a272eb02166bc02aeb59f03cabfc92d5d547854b124d7eaf813bf"
            );
            assert!(replay_manifest_bytes_exact(
                &manifest,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            ));
            assert!(replay_manifest_semantic(
                &manifest,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            )
            .is_ok());

            let escaped =
                manifest.replacen("clang version 20.1.0", "clang\\u0020version 20.1.0", 1);
            assert!(replay_manifest_semantic(
                &escaped,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            )
            .is_ok());
            assert!(!replay_manifest_bytes_exact(
                &escaped,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            ));

            let malformed = [
                manifest.replacen("{\"schema\":", "{\"schema\":\"duplicate\",\"schema\":", 1),
                manifest.replacen("{\"schema\":", "{\"unknown\":0,\"schema\":", 1),
                manifest.replacen("{\"schema\":", "{\"missing_schema\":", 1),
                manifest.replacen("\"bytes\":4498", "\"bytes\":\"4498\"", 1),
                manifest.replacen("\"descriptor\":{", "\"descriptor\":[{", 1),
                format!("{manifest}trailing"),
            ];
            for hostile in malformed {
                assert!(replay_manifest_semantic(
                    &hostile,
                    &prepared,
                    &files,
                    "/authenticated/clang",
                    "clang version 20.1.0",
                    &rustc,
                    &prepared.target.triple,
                )
                .is_err());
            }
        },
    );
}

#[test]
fn cumulative_builder_limit_is_exact_and_cannot_be_widened() {
    let (program, spec) = fixture();
    let (mut low, mut high) = (0_usize, MAX_BUILDER_BYTES);
    while low < high {
        let middle = low + (high - low) / 2;
        if prepare_native_rust_interop_with_test_limit(&program, spec.as_bytes(), middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let minimum = low;
    assert!(minimum > 0 && minimum <= MAX_BUILDER_BYTES);
    let prepared =
        prepare_native_rust_interop_with_test_limit(&program, spec.as_bytes(), minimum).unwrap();
    assert_eq!(prepared.canonical_spec, spec);
    let error =
        match prepare_native_rust_interop_with_test_limit(&program, spec.as_bytes(), minimum - 1) {
            Ok(_) => panic!("one-under builder limit was accepted"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(
        error[0].message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );

    let widened = std::panic::catch_unwind(|| {
        let _ = prepare_native_rust_interop_with_test_limit(
            &program,
            spec.as_bytes(),
            MAX_BUILDER_BYTES + 1,
        );
    });
    assert!(widened.is_err());
}

#[test]
fn full_bundle_builder_limit_is_cumulative_exact_and_cannot_be_widened() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-builder-exact-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    let output = root.join("bundle");
    let prepare_probe =
        |limit: usize| prepare_phase_b_with_test_limit(&program, spec.as_bytes(), &output, limit);
    let build_probe = |limit: usize| {
        std::fs::create_dir(&root).unwrap();
        let result = build_native_rust_interop_bundle_with_test_limit(
            &program,
            spec.as_bytes(),
            &output,
            limit,
        );
        std::fs::remove_dir_all(&root).unwrap();
        result.map(|_| ())
    };

    let (mut low, mut high) = (0_usize, MAX_BUILDER_BYTES);
    while low < high {
        let middle = low + (high - low) / 2;
        match prepare_probe(middle) {
            Ok(()) => high = middle,
            Err(error) => {
                assert_eq!(error.len(), 1);
                assert_eq!(error[0].code, "SPX-B109");
                assert_eq!(
                    error[0].message,
                    "Native Rust Interop max_builder_bytes exceeds 33554432"
                );
                low = middle + 1;
            }
        }
    }
    let minimum = low;
    assert!(minimum > 0 && minimum <= MAX_BUILDER_BYTES);
    prepare_probe(minimum).unwrap();
    let error = prepare_probe(minimum - 1).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(
        error[0].message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
    build_probe(minimum).unwrap();

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-builder-widen-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let widened = std::panic::catch_unwind(|| {
        let _ = build_native_rust_interop_bundle_with_test_limit(
            &program,
            spec.as_bytes(),
            &root.join("bundle"),
            MAX_BUILDER_BYTES + 1,
        );
    });
    std::fs::remove_dir_all(&root).unwrap();
    assert!(widened.is_err());
}

#[test]
fn phase_b_local_paths_digest_and_stage_names_are_frozen_before_effects() {
    let output = Path::new("phase-b-bundle");
    let mut pending = PendingBundleFacts::new(output, "module.o").unwrap();
    pending.bind_manifest_digest(b"manifest\n").unwrap();
    let facts = pending.finish();
    assert_eq!(facts.output_directory, output);
    assert_eq!(facts.object_path, output.join("module.o"));
    assert_eq!(facts.descriptor_path, output.join("descriptor.json"));
    assert_eq!(
        facts.manifest_path,
        output.join("semaprax.native-rust-interop.json")
    );
    assert_eq!(
        facts.manifest_digest,
        domain_digest(BUNDLE_DIGEST_DOMAIN, b"manifest\n")
    );

    let parent = Path::new("phase-b-parent");
    let descriptor = "sha256:phase-b-descriptor";
    let mut slot = StageSlot::new(parent, descriptor, "publish").unwrap();
    let name_capacity = slot.name.capacity();
    let path_capacity = slot.path.capacity();
    slot.prepare(parent, 0).unwrap();
    assert_eq!(
        slot.name,
        format!(
            ".semaprax-native-rust-interop-publish-{}-{}-0",
            std::process::id(),
            &full_hash(descriptor)[..16]
        )
    );
    assert_eq!(slot.path, parent.join(&slot.name));
    slot.prepare(parent, 1023).unwrap();
    assert!(slot.name.ends_with("-1023"));
    assert_eq!(slot.name.capacity(), name_capacity);
    assert_eq!(slot.path.capacity(), path_capacity);
}

#[cfg(windows)]
#[test]
fn phase_b_windows_absolute_precarrier_topology_is_cumulatively_bounded() {
    let synthetic_parent = Path::new(r"\\?\C:\semaprax-δ");
    let synthetic_capacity =
        exact_child_path_capacity(synthetic_parent, "artifact.obj".len()).unwrap();
    let synthetic = exact_child_path(synthetic_parent, "artifact.obj", synthetic_capacity).unwrap();
    assert_eq!(synthetic.capacity(), synthetic_capacity);
    assert!(exact_child_path_matches(
        &synthetic,
        synthetic_parent,
        OsStr::new("artifact.obj"),
    ));
    for drive_relative in [Path::new(r"C:"), Path::new(r"\\?\C:")] {
        let capacity = exact_child_path_capacity(drive_relative, "artifact.obj".len()).unwrap();
        assert_eq!(
            capacity,
            drive_relative.as_os_str().len() + "artifact.obj".len(),
        );
        let child = exact_child_path(drive_relative, "artifact.obj", capacity).unwrap();
        assert_eq!(child.capacity(), capacity);
        let child_bytes = child.as_os_str().as_encoded_bytes();
        let parent_bytes = drive_relative.as_os_str().as_encoded_bytes();
        assert_eq!(&child_bytes[..parent_bytes.len()], parent_bytes);
        assert_eq!(&child_bytes[parent_bytes.len()..], b"artifact.obj");
    }

    let ((), synthetic_overflowed, _) =
        crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
            let mut slot =
                StageSlot::new(synthetic_parent, "sha256:windows-verbatim-stage", "publish")
                    .unwrap();
            slot.prepare(synthetic_parent, 0).unwrap();
            let allocation = slot.path.as_os_str().as_encoded_bytes().as_ptr();
            let capacity = slot.path.capacity();
            slot.prepare(synthetic_parent, 1023).unwrap();
            assert_eq!(
                slot.path.as_os_str().as_encoded_bytes().as_ptr(),
                allocation,
            );
            assert_eq!(slot.path.capacity(), capacity);
        });
    assert!(!synthetic_overflowed);

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-precarrier-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    PHASE_B_PREPARED_CARRIER_IDENTITIES.with(|identities| identities.set([0; 7]));

    let ((), overflowed, _) = crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
        let (program, canonical_spec) = fixture();
        let prepared = prepare_native_rust_interop_bounded(&program, canonical_spec.as_bytes())
            .unwrap_or_else(|error| {
                panic!(
                    "prepare_native failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let parent = output.parent().unwrap();
        let pending = PendingBundleFacts::new(&output, "module.obj").unwrap_or_else(|error| {
            panic!(
                "pending facts failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let publish_slot = StageSlot::new(parent, &prepared.descriptor_digest, "publish")
            .unwrap_or_else(|error| {
                panic!(
                    "publish slot failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let run_slot =
            StageSlot::new(parent, &prepared.descriptor_digest, "run").unwrap_or_else(|error| {
                panic!(
                    "run slot failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let publish_files = prepare_publish_discard_inventory().unwrap_or_else(|error| {
            panic!(
                "publish inventory failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let run_files = prepare_run_discard_inventory().unwrap_or_else(|error| {
            panic!(
                "run inventory failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let parent_capacity = parent.as_os_str().as_encoded_bytes().len();
        let parent_budget = reserve_temporary_exact(parent_capacity).unwrap_or_else(|error| {
            panic!(
                "parent reservation failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let parent_path = exact_path_copy(parent, parent_capacity).unwrap_or_else(|error| {
            panic!(
                "parent copy failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        parent_budget
            .retain(parent_capacity)
            .unwrap_or_else(|error| {
                panic!(
                    "parent retain failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let carriers = PhaseBErrorCarriers::prepare().unwrap_or_else(|error| {
            panic!(
                "carrier preparation failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let identities = PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get);
        assert!(identities.into_iter().all(|identity| identity != 0));
        for (index, carrier) in carriers.carriers.iter().enumerate() {
            assert_eq!(
                identities[index],
                carrier.errors.as_ref().unwrap()[0].message.as_ptr() as usize,
            );
        }
        drop((
            carriers,
            parent_path,
            run_files,
            publish_files,
            run_slot,
            publish_slot,
            pending,
            prepared,
        ));
    });
    assert!(!overflowed);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_rejects_non_component_output_before_build_hooks() {
    let (program, spec) = fixture();
    let mut hooks = 0usize;
    let error = match build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        Path::new("/"),
        |_, _, _, _| hooks += 1,
    ) {
        Ok(_) => panic!("non-component output unexpectedly reached Phase B"),
        Err(error) => error,
    };
    assert_eq!(hooks, 0);
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(
        error[0].message,
        "Native Rust Interop output publication failed"
    );
}

#[test]
fn phase_b_output_exists_precedes_invalid_tool_environment_without_tool_activity() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-output-precedes-tools-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("sentinel"), b"foreign").unwrap();
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(true));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &output,
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("existing output unexpectedly allowed the invalid tool environment"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(error[0].message, PHASE_B_PUBLICATION_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::Publication.index()],
    );
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(hooks, 0);
    assert_eq!(std::fs::read(output.join("sentinel")).unwrap(), b"foreign");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_invalid_frozen_tool_environment_fails_after_stages_before_hold_or_spawn() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-invalid-frozen-tools-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(true));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("invalid frozen tool environment unexpectedly authenticated"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B110");
    assert_eq!(error[0].message, PHASE_B_UNSUPPORTED_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::Unsupported.index()],
    );
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(hooks, 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_direct_rustc_fixed_point_mismatch_is_b110_before_artifact_processes() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-direct-fixed-point-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION.with(|injection| injection.set(true));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("mismatched direct rustc sysroot unexpectedly admitted"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B110");
    assert_eq!(error[0].message, PHASE_B_UNSUPPORTED_MESSAGE);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 2);
    assert_eq!(
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(hooks, 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_fixed_rustc_version_parser_is_no_growth_at_representative_and_maximum() {
    for source in [
            String::from(
                "rustc 1.88.0 (012345678 2026-01-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\ncommit-date: 2026-01-01\nhost: x86_64-unknown-linux-gnu\nrelease: 1.88.0\nLLVM version: 20.1.0",
            ),
            format!(
                "rustc 1.88.0 (012345678 2026-01-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\ncommit-date: 2026-01-01\nhost: x86_64-unknown-linux-gnu\nrelease: 1.88.0\nLLVM version: {}",
                "1".repeat(PHASE_B_TOOL_VERSION_CAPACITY - 200)
            ),
        ] {
            assert!(source.len() <= PHASE_B_TOOL_VERSION_CAPACITY);
            let mut parsed = RustcVersion::prepared().unwrap();
            let capacity = parsed.capacity();
            parse_rustc_version(&source, &mut parsed).unwrap();
            assert_eq!(parsed.capacity(), capacity);
            assert_eq!(parsed.release(), "1.88.0");
            assert_eq!(parsed.commit_hash(), "0123456789abcdef");
            assert_eq!(parsed.host(), "x86_64-unknown-linux-gnu");
        }

    let exact_first = "r".repeat(PHASE_B_TOOL_VERSION_CAPACITY - 3);
    let mut exact = RustcVersion::prepared().unwrap();
    let exact_pointer = exact.storage.as_ptr();
    exact.store([&exact_first, "c", "h", "l"]).unwrap();
    assert_eq!(exact.storage.len(), PHASE_B_TOOL_VERSION_CAPACITY);
    assert_eq!(exact.storage.capacity(), PHASE_B_TOOL_VERSION_CAPACITY);
    assert_eq!(exact.storage.as_ptr(), exact_pointer);

    let overflow_first = "r".repeat(PHASE_B_TOOL_VERSION_CAPACITY - 2);
    let mut overflow = RustcVersion::prepared().unwrap();
    let overflow_pointer = overflow.storage.as_ptr();
    assert_eq!(
        overflow.store([&overflow_first, "c", "h", "l"]),
        Err(PhaseBLocalError::Unsupported),
    );
    assert!(overflow.storage.is_empty());
    assert_eq!(overflow.boundaries, [0; 5]);
    assert_eq!(overflow.storage.capacity(), PHASE_B_TOOL_VERSION_CAPACITY);
    assert_eq!(overflow.storage.as_ptr(), overflow_pointer);

    for invalid in [
            "rustc 1.88.0\nrelease: 1.88.0\nrelease: 1.88.0\ncommit-hash: 0123456\nhost: h\nLLVM version: 1",
            "rustc 1.88.0\nrelease: 1.88.0\ncommit-hash: 0123456\nhost: h\nunknown: value\nLLVM version: 1",
            "rustc 1.88.0\nrelease: 1.88.0\ncommit-hash: 0123456\nhost: h",
        ] {
            let mut parsed = RustcVersion::prepared().unwrap();
            assert_eq!(
                parse_rustc_version(invalid, &mut parsed),
                Err(PhaseBLocalError::Unsupported),
            );
        }
}

#[test]
fn capacity_module_has_no_physical_or_platform_authority() {
    let source = include_str!("capacity.rs");
    for forbidden in [
        "platform::",
        "std::fs",
        "std::process",
        "create_directory_new_prepared",
        "write_file_new_prepared",
        "discard_owned_stage_prepared",
        "compile_c_prepared",
        "compile_rust_prepared",
        "link_or_copy_prepared",
        "run_prepared",
        "archive_tool_prepared",
        "publish_directory_new_prepared",
    ] {
        assert!(
            !source.contains(forbidden),
            "capacity-only implementation admitted `{forbidden}`"
        );
    }
}

#[test]
fn artifact_projection_module_has_no_physical_authority_or_replay_generator_shortcut() {
    let artifacts = include_str!("artifacts.rs");
    let cursor = include_str!("exact_replay.rs");
    for forbidden in [
        "platform::",
        "std::fs",
        "std::process",
        "create_directory_new_prepared",
        "write_file_new_prepared",
        "archive_tool_prepared",
        "archive_prepared",
        "compile_c_tool_prepared",
        "compile_rust_tool_prepared",
        "link_tool_prepared",
        "execute_tool_prepared",
        "publish_directory_new_prepared",
        "discard_owned_stage_prepared",
        "settle_for_publish",
        "settle_regular_file_for_publish",
    ] {
        assert!(
            !artifacts.contains(forbidden) && !cursor.contains(forbidden),
            "pure artifact boundary admitted `{forbidden}`"
        );
    }
    let start = artifacts
        .find("fn replay_c_expression_linear_independent(")
        .unwrap();
    let end = artifacts[start..]
        .find("\nfn replay_c_expression(")
        .map(|offset| start + offset)
        .unwrap();
    let independent = &artifacts[start..end];
    for generator in [
        "c_expression_linear(",
        "c_expr_iterative(",
        "generate_c_into(",
        "c_expression_hash(",
        "c_expression_scalar(",
        "c_expression_resolved_scalar(",
    ] {
        assert!(
            !independent.contains(generator),
            "independent C replay called generator helper `{generator}`"
        );
    }
    assert!(artifacts.contains("enum CExpressionFrame<'a>"));
    assert!(artifacts.contains("enum ReplayCExpressionFrame<'a>"));
    assert!(cursor.contains("pub(super) struct ExactReplay<'a>"));
}

#[test]
fn phase_b_process_arena_reservation_precedes_materialization_source_contract() {
    let source = include_str!("../implementation.rs");
    let toolchain_start = source.find("fn prepare_toolchain_plan()").unwrap();
    let toolchain_end = source[toolchain_start..]
        .find("fn authenticate_toolchain(")
        .map(|offset| toolchain_start + offset)
        .unwrap();
    let toolchain = &source[toolchain_start..toolchain_end];
    let arena = toolchain.find("prepare_process_arena_authorized(").unwrap();
    let include_drop = toolchain.find("drop(include)").unwrap();
    let libraries_drop = toolchain.find("drop(libraries)").unwrap();
    let environment_shrink = toolchain
        .find("shrink_phase_b(&mut environment.budget")
        .unwrap();
    let resolver_reservation = toolchain
        .find("reserve_phase_b(PHASE_B_TOOL_RESOLVER_CAPACITY)")
        .unwrap();
    assert!(
        arena < include_drop
            && include_drop < libraries_drop
            && libraries_drop < environment_shrink
            && environment_shrink < resolver_reservation
    );

    let start = source.find("fn prepare_process_arena_authorized(").unwrap();
    let end = source[start..]
        .find("#[cfg(test)]\nfn reset_phase_b_error_materialization_observer")
        .map(|offset| start + offset)
        .unwrap();
    let helper = &source[start..end];
    let plan = helper
        .find("platform::prepare_process_arena_plan_with_environment(")
        .unwrap();
    let required = helper
        .find("platform::prepared_process_arena_plan_capacity(&plan)")
        .unwrap();
    let reserve = helper.find("reserve_phase_b(required)?").unwrap();
    let allocate = helper
        .find("platform::materialize_process_arena_with_environment(")
        .unwrap();
    assert!(plan < required && required < reserve && reserve < allocate);
    assert!(helper.contains("required > PHASE_B_PROCESS_ARENA_MAX_CAPACITY"));
    assert!(helper.contains("platform::prepared_process_arena_owned_capacity(&arena) != required"));

    let wrapper_start = source.find("struct AuthorizedProcessArena {").unwrap();
    let wrapper_end = source[wrapper_start..]
        .find("struct PreparedToolchainPlan {")
        .map(|offset| wrapper_start + offset)
        .unwrap();
    let wrapper = &source[wrapper_start..wrapper_end];
    assert!(wrapper.find("arena:").unwrap() < wrapper.find("budget:").unwrap());
    assert!(wrapper.find("drop(arena)").unwrap() < wrapper.find("drop(budget)").unwrap());
    assert!(
        !source[source.find("struct PreparedToolchainPlan {").unwrap()..]
            .split("struct ToolchainFacts {")
            .next()
            .unwrap()
            .contains("process_arena_budget")
    );
}

#[test]
fn windows_direct_rustc_tests_use_the_frozen_native_linker() {
    let source = include_str!("../implementation.rs");
    let tests = include_str!("tests.rs");
    let linker = source.find("fn bind_test_rust_linker(").unwrap();
    let linker_end = source[linker..]
        .find("\n}\n\nstruct RustcVersion")
        .map(|offset| linker + offset)
        .unwrap();
    let helper = &source[linker..linker_end];
    assert!(helper.contains("std::env::var_os(\"SEMAPRAX_LINKER\")"));
    assert!(helper.contains("std::fs::canonicalize(configured)"));
    let callsites = tests
        .match_indices(
            "fn phase_b_process_arena_drops_bytes_before_authority_on_early_plan_failure(",
        )
        .nth(1)
        .map(|(offset, _)| offset)
        .unwrap();
    assert_eq!(
        tests[callsites..]
            .matches("bind_test_rust_linker(&mut ")
            .count(),
        5
    );
    assert!(!source.contains("format!(\"linker={}\", clang.path.display())"));
}

#[test]
fn phase_b_process_arena_drops_bytes_before_authority_on_early_plan_failure() {
    reset_phase_b_process_arena_drop_observer();
    let (plan, overflowed) =
        crate::bounded_output::with_limit(MAX_BUILDER_BYTES, prepare_toolchain_plan);
    assert!(!overflowed);
    let plan = plan.unwrap();
    assert_eq!(PHASE_B_PROCESS_ARENA_DROPS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(std::cell::Cell::get),
        0
    );
    drop(plan);
    assert_eq!(PHASE_B_PROCESS_ARENA_DROPS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(
        PHASE_B_PROCESS_ARENA_DROP_ORDER.with(std::cell::Cell::get),
        [1, 2]
    );
    assert_eq!(
        PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH.with(std::cell::Cell::get),
        2
    );
}

#[cfg(windows)]
#[test]
fn phase_b_process_arena_exact_and_one_less_is_zero_effect() {
    let include = OsStr::new(r"C:\sdk\include");
    let libraries = OsStr::new(r"C:\sdk\lib");
    let sizing = platform::prepare_process_arena_plan_with_environment(
        PHASE_B_PROCESS_INVOCATIONS,
        Some(include),
        Some(libraries),
    )
    .unwrap();
    let required = platform::prepared_process_arena_plan_capacity(&sizing);
    assert!(required > 0 && required <= PHASE_B_PROCESS_ARENA_MAX_CAPACITY);

    let (exact, overflowed, used) = crate::bounded_output::with_limit_usage(required, || {
        prepare_process_arena_authorized(Some(include), Some(libraries))
    });
    assert!(!overflowed);
    let arena = exact.unwrap();
    assert_eq!(used, required);
    assert_eq!(arena.authorized_capacity().unwrap(), required);
    assert_eq!(
        platform::prepared_process_arena_owned_capacity(arena.arena().unwrap()),
        required
    );
    assert_eq!(
        platform::prepared_process_arena_remaining(arena.arena().unwrap()),
        PHASE_B_PROCESS_INVOCATIONS
    );
    drop(arena);

    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    reset_phase_b_error_materialization_observer();
    let (one_less, overflowed) = crate::bounded_output::with_limit(required - 1, || {
        prepare_process_arena_authorized(Some(include), Some(libraries))
    });
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    assert!(!PHASE_B_EFFECT_STARTED.with(std::cell::Cell::get));
}

#[test]
fn phase_b_harness_is_exact_capacity_at_representative_and_maximum_and_one_less_is_pre_effect() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let (representative, representative_budget) = prepare_rust_harness(&prepared).unwrap();
    assert_eq!(representative.len(), representative.capacity());
    assert_eq!(representative.len(), representative_budget.maximum());
    drop((representative, representative_budget));

    let mut maximum = prepared;
    let mut import = maximum.imports[0].clone();
    import.parameters = (0..MAX_PARAMETERS)
        .map(|index| ParameterFact {
            name: format!("p{index}"),
            ty: ScalarType::I64,
        })
        .collect();
    import.capabilities = (0..MAX_EFFECTS)
        .map(|index| format!("capability.{index:02}"))
        .collect();
    maximum.imports = vec![import; MAX_IMPORTS];
    let mut export = maximum.exports[0].clone();
    export.parameters = (0..MAX_PARAMETERS)
        .map(|index| ParameterFact {
            name: format!("p{index}"),
            ty: ScalarType::I64,
        })
        .collect();
    maximum.exports = vec![export; MAX_EXPORTS];
    let (harness, overflowed, exact) =
        crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
            prepare_rust_harness(&maximum)
        });
    assert!(!overflowed);
    let (harness, budget) = harness.unwrap();
    assert_eq!(harness.len(), harness.capacity());
    assert_eq!(harness.len(), budget.maximum());
    assert_eq!(exact, harness.len());
    drop((harness, budget));

    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    let (one_less, overflowed) =
        crate::bounded_output::with_limit(exact - 1, || prepare_rust_harness(&maximum));
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
}

#[test]
fn phase_b_prepared_invocations_admit_linux_underscore_and_reject_other_punctuation_as_b110() {
    let mut prepared = with_test_target(
        Target {
            triple: "x86_64-unknown-linux-gnu".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, spec) = fixture();
            prepare_native_rust_interop(&program, spec.as_bytes()).unwrap()
        },
    );
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
    let linker = std::env::var_os("SEMAPRAX_LINKER");
    let linker = if cfg!(windows) {
        Some(
            linker
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_LINKER)),
        )
    } else {
        None
    };
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS");
    let vctools = if cfg!(windows) {
        Some(
            vctools
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_VCTOOLS)),
        )
    } else {
        None
    };
    let plans = prepare_build_invocations(&prepared, false, linker, vctools).unwrap();
    assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
    drop(plans);

    prepared.target.triple = "x86_64-unknown/linux-gnu".to_owned();
    let error = match prepare_build_invocations(&prepared, false, linker, vctools) {
        Ok(_) => panic!("noncanonical target punctuation was admitted"),
        Err(error) => error,
    };
    assert_eq!(error, PhaseBLocalError::Unsupported);
}

#[test]
fn phase_b_all_eight_build_invocations_are_prepared_bounded_and_consumed_once() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
    PHASE_B_INVENTORY_EXACT_SCANS.with(|count| count.set(0));
    PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
    PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(0));
    reset_phase_b_object_authority_observer();
    reset_phase_b_manifest_authority_observer();
    let linker = std::env::var_os("SEMAPRAX_LINKER");
    let linker = if cfg!(windows) {
        Some(
            linker
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_LINKER)),
        )
    } else {
        None
    };
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS");
    let vctools = if cfg!(windows) {
        Some(
            vctools
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_VCTOOLS)),
        )
    } else {
        None
    };
    let plans = prepare_build_invocations(&prepared, false, linker, vctools).unwrap();
    assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
    assert_eq!(
        platform::prepared_c_compile_owned_capacity(&plans.c_o0.0),
        plans.c_o0.1.maximum()
    );
    assert_eq!(
        platform::prepared_c_compile_owned_capacity(&plans.c_o2.0),
        plans.c_o2.1.maximum()
    );
    assert_eq!(
        platform::prepared_rust_compile_owned_capacity(&plans.rust.0),
        plans.rust.1.maximum()
    );
    assert_eq!(
        platform::prepared_c_compile_owned_capacity(&plans.c_main.0),
        plans.c_main.1.maximum()
    );
    assert_eq!(
        platform::prepared_link_owned_capacity(&plans.link_o0.0),
        plans.link_o0.1.maximum()
    );
    assert_eq!(
        platform::prepared_run_owned_capacity(&plans.run_o0.0),
        plans.run_o0.1.maximum()
    );
    assert_eq!(
        platform::prepared_link_owned_capacity(&plans.link_o2.0),
        plans.link_o2.1.maximum()
    );
    assert_eq!(
        platform::prepared_run_owned_capacity(&plans.run_o2.0),
        plans.run_o2.1.maximum()
    );
    drop(plans);

    let publish = prepare_publish_discard_inventory().unwrap();
    let run = prepare_run_discard_inventory().unwrap();
    let copies = prepare_link_copies(
        &publish,
        &run,
        if cfg!(windows) {
            "module.obj"
        } else {
            "module.o"
        },
    )
    .unwrap();
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 3);
    for (prepared, budget) in [
        &copies.safe_rust,
        &copies.private_ffi,
        &copies.optimized_object,
    ] {
        assert_eq!(
            platform::prepared_link_or_copy_owned_capacity(prepared),
            budget.maximum()
        );
    }
    drop((copies, publish, run));

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-build-plans-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(0));
    build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| {},
    )
    .unwrap_or_else(|errors| {
        panic!(
            "prepared build failed after {} invocation consumptions: {errors:?}",
            PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        )
    });
    assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
    assert_eq!(
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        8
    );
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 3);
    assert_eq!(PHASE_B_LINK_COPY_CONSUMPTIONS.with(std::cell::Cell::get), 3);
    assert_eq!(PHASE_B_INVENTORY_EXACT_PLANS.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_INVENTORY_EXACT_SCANS.with(std::cell::Cell::get), 2);
    assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_PUBLISH_CONSUMPTIONS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(std::cell::Cell::get),
        1
    );
    assert_phase_b_object_drop_order(1);
    assert_eq!(
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_MANIFEST_ARENA_GROWTHS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_phase_b_manifest_drop_order(1);
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn phase_b_prepared_tool_resolver_covers_path_positions_symlink_and_capacity_minus_one() {
    use std::os::unix::fs::symlink;

    let real = configured_tool("CLANG").unwrap().path;
    let canonical = std::fs::canonicalize(&real).unwrap();
    let canonical_text = canonical.to_str().unwrap();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-tool-resolver-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let directories = [root.join("first"), root.join("middle"), root.join("last")];
    for directory in &directories {
        std::fs::create_dir(directory).unwrap();
    }

    for position in 0..directories.len() {
        let link = directories[position].join("clang");
        symlink(&real, &link).unwrap();
        let paths = std::env::join_paths(&directories).unwrap();
        let resolver =
            platform::prepare_tool_resolver("clang", PHASE_B_TOOL_PATH_CAPACITY).unwrap();
        assert!(
            platform::prepared_tool_resolver_owned_capacity(&resolver)
                <= PHASE_B_TOOL_RESOLVER_CAPACITY
        );
        reset_phase_b_error_materialization_observer();
        mark_phase_b_effect_started();
        let held =
            platform::resolve_and_hold_tool_prepared(resolver, None, Some(paths.as_os_str()))
                .unwrap();
        assert_eq!(platform::tool_path(&held), canonical_text);
        assert_eq!(
            platform::tool_path_capacity(&held),
            PHASE_B_TOOL_PATH_CAPACITY
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
        );
        std::fs::remove_file(link).unwrap();
    }

    let configured_link = root.join("configured-clang");
    symlink(&real, &configured_link).unwrap();
    let resolver = platform::prepare_tool_resolver("clang", PHASE_B_TOOL_PATH_CAPACITY).unwrap();
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    let held =
        platform::resolve_and_hold_tool_prepared(resolver, Some(configured_link.as_os_str()), None)
            .unwrap();
    assert_eq!(platform::tool_path(&held), canonical_text);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );

    let missing_paths = std::env::join_paths(&directories).unwrap();
    let resolver = platform::prepare_tool_resolver("clang", PHASE_B_TOOL_PATH_CAPACITY).unwrap();
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    assert!(platform::resolve_and_hold_tool_prepared(
        resolver,
        None,
        Some(missing_paths.as_os_str()),
    )
    .is_err());
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );

    let exact = canonical.as_os_str().as_encoded_bytes().len() + 1;
    let resolver = platform::prepare_tool_resolver("clang", exact).unwrap();
    let held =
        platform::resolve_and_hold_tool_prepared(resolver, Some(canonical.as_os_str()), None)
            .unwrap();
    assert_eq!(platform::tool_path(&held), canonical_text);
    assert_eq!(platform::tool_path_capacity(&held), exact);
    let resolver = platform::prepare_tool_resolver("clang", exact - 1).unwrap();
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    assert!(
        platform::resolve_and_hold_tool_prepared(resolver, Some(canonical.as_os_str()), None,)
            .is_err()
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_discard_inventory_capacity_minus_one_rejects_without_effects() {
    let ((publish, run), overflowed, exact) =
        crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
            (
                prepare_publish_discard_inventory(),
                prepare_run_discard_inventory(),
            )
        });
    assert!(!overflowed);
    let publish = publish.unwrap();
    let run = run.unwrap();
    assert_eq!((publish.capacity(), publish.attached()), (7, 0));
    assert_eq!((run.capacity(), run.attached()), (10, 0));
    assert!(exact > 0);

    let ((publish, run), overflowed) = crate::bounded_output::with_limit(exact - 1, || {
        (
            prepare_publish_discard_inventory(),
            prepare_run_discard_inventory(),
        )
    });
    assert!(!overflowed);
    assert!(publish.is_ok());
    let error = match run {
        Ok(_) => panic!("capacity-minus-one admitted the run discard plan"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
}

#[test]
fn phase_b_created_directory_auth_disagreement_attempts_one_discard_and_stops() {
    for mode in [
        CreateAuthDisagreement::Clean,
        CreateAuthDisagreement::Substituted,
    ] {
        let (program, spec) = fixture();
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-auth-disagreement-{}-{mode:?}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        CREATE_AUTH_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
        CREATE_AUTH_DISAGREEMENT.with(|injection| injection.set(Some(mode)));
        let mut hooks = 0usize;
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &root.join("bundle"),
            |_, _, _, _| hooks += 1,
        );
        CREATE_AUTH_DISAGREEMENT.with(|injection| injection.set(None));
        let error = match result {
            Ok(_) => panic!("created-directory authentication disagreement was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-I232");
        assert_eq!(CREATE_AUTH_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "authentication disagreement materialized its sticky diagnostic after effects",
        );
        assert_eq!(hooks, 0, "later build action followed sticky I232");
        assert!(!root.join("bundle").exists());
        match mode {
            CreateAuthDisagreement::Clean => {
                assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            }
            CreateAuthDisagreement::Substituted => {
                assert!(root.join("auth-displaced").is_dir());
                let substitute = std::fs::read_dir(&root)
                    .unwrap()
                    .map(Result::unwrap)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".semaprax-native-rust-interop-publish-")
                    })
                    .expect("substituted stage remains inert");
                assert_eq!(
                    std::fs::read(substitute.path().join("foreign-sentinel")).unwrap(),
                    b"foreign"
                );
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn phase_b_all_local_failures_move_exact_prebuilt_carrier_and_settle_without_later_action() {
    let kinds = [
        PhaseBLocalError::BuilderBudget,
        PhaseBLocalError::ManifestBudget,
        PhaseBLocalError::Unsupported,
        PhaseBLocalError::Replay,
        PhaseBLocalError::Compile,
        PhaseBLocalError::Link,
        PhaseBLocalError::Publication,
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        let (program, spec) = fixture();
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-local-carrier-{}-{index}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
        PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
        PHASE_B_LOCAL_FAILURE_INJECTION.with(|injection| injection.set(Some(kind)));
        let mut hooks = 0usize;
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &root.join("bundle"),
            |_, _, _, _| hooks += 1,
        );
        PHASE_B_LOCAL_FAILURE_INJECTION.with(|injection| injection.set(None));
        let error = match result {
            Ok(_) => panic!("{kind:?} failure injection unexpectedly succeeded"),
            Err(error) => error,
        };
        let (code, message) = kind.diagnostic();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, code);
        assert_eq!(error[0].message, message);
        assert_eq!(
            error[0].message.as_ptr() as usize,
            PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)[kind.index()],
            "{kind:?} did not move its pre-effect String allocation",
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "{kind:?} materialized a Diagnostic, String, or Vec after effects",
        );
        assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
        assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
        assert_eq!(
            PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
            0,
        );
        assert_eq!(hooks, 0, "{kind:?} allowed a later build action");
        assert!(!root.join("bundle").exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn phase_b_real_oversize_manifest_uses_exact_manifest_carrier_and_settles() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-oversize-manifest-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    reset_phase_b_object_authority_observer();
    reset_phase_b_manifest_authority_observer();
    PHASE_B_OVERSIZE_MANIFEST_INJECTION.with(|injection| injection.set(true));
    let mut observed = Vec::new();
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |point, _, _, _| observed.push(point),
    );
    PHASE_B_OVERSIZE_MANIFEST_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("oversize manifest unexpectedly published"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(error[0].message, PHASE_B_MANIFEST_BUDGET_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::ManifestBudget.index()],
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(std::cell::Cell::get),
        0
    );
    assert_phase_b_object_drop_order(1);
    assert_eq!(
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_MANIFEST_ARENA_GROWTHS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_phase_b_manifest_drop_order(1);
    assert_eq!(
        observed,
        [
            NativeRustBuildPoint::BeforeClang,
            NativeRustBuildPoint::BeforeRustLink,
            NativeRustBuildPoint::BeforeExecutableAuthentication,
            NativeRustBuildPoint::BeforeExecute,
            NativeRustBuildPoint::BeforeExecutableAuthentication,
            NativeRustBuildPoint::BeforeExecute,
            NativeRustBuildPoint::BeforeObjectRead,
        ],
        "oversize manifest allowed a later manifest-write or publish hook",
    );
    assert!(!root.join("bundle").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_manifest_capacity_minus_one_fails_before_any_effect() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-manifest-capacity-minus-one-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    reset_phase_b_manifest_authority_observer();
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_DISCARD_ATTEMPTS.with(|count| count.set(0));
    PHASE_B_MANIFEST_PLAN_CAPACITY.with(|capacity| capacity.set(MAX_MANIFEST_BYTES - 1));
    let mut hooks = 0_usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_MANIFEST_PLAN_CAPACITY.with(|capacity| capacity.set(MAX_MANIFEST_BYTES));
    let error = match result {
        Ok(_) => panic!("capacity-minus-one manifest plan unexpectedly reached effects"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(error[0].message, PHASE_B_MANIFEST_BUDGET_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::ManifestBudget.index()]
    );
    assert_eq!(hooks, 0);
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        0
    );
    assert_phase_b_manifest_drop_order(0);
    assert!(!root.join("bundle").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_primary_failure_is_sticky_over_mid_cleanup_failure_without_materialization() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-sticky-cleanup-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    PHASE_B_DISCARD_FAILURE_AFTER_DELETE.with(|failure| failure.set(Some(0)));
    PHASE_B_LOCAL_FAILURE_INJECTION
        .with(|injection| injection.set(Some(PhaseBLocalError::Compile)));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_LOCAL_FAILURE_INJECTION.with(|injection| injection.set(None));
    PHASE_B_DISCARD_FAILURE_AFTER_DELETE.with(|failure| failure.set(None));
    let error = match result {
        Ok(_) => panic!("compile failure injection unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I230");
    assert_eq!(error[0].message, PHASE_B_COMPILE_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::Compile.index()],
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(hooks, 0);
    assert!(!root.join("bundle").exists());
    assert_eq!(
        std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".semaprax-native-rust-interop-run-"))
            .count(),
        1,
        "failed run-stage cleanup must leave one inert owned residue",
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_fixed_discard_plans_remove_every_partial_prefix_without_growth() {
    fn exercise<const N: usize>(label: &str, names: [&'static str; N]) {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-prefix-{label}-{}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let parent = platform::hold_directory(&root).unwrap();
        for prefix in 0..=N {
            let stage_name = format!("s{prefix}");
            let prepared_stage = platform::prepare_stage_name(stage_name.as_ref()).unwrap();
            let stage =
                platform::create_directory_new(&parent, stage_name.as_ref(), 0o700).unwrap();
            let os_names = names.map(OsStr::new);
            let mut inventory = platform::prepare_discard_inventory(os_names).unwrap();
            let native_capacity = platform::prepared_discard_inventory_owned_capacity(&inventory);
            assert_eq!(inventory.capacity(), N);
            for (index, name) in names.iter().take(prefix).enumerate() {
                platform::write_file_new_prepared(&stage, &mut inventory, name, b"owned", 0o600)
                    .unwrap();
                assert_eq!(inventory.attached(), index + 1);
                assert_eq!(inventory.capacity(), N);
                assert_eq!(
                    platform::prepared_discard_inventory_owned_capacity(&inventory),
                    native_capacity
                );
            }
            assert_eq!(inventory.attached(), prefix);
            platform::discard_owned_stage_prepared(&parent, &stage, &prepared_stage, &inventory)
                .unwrap();
            // Windows finalizes the delete disposition only after the
            // last authenticated directory handle closes.
            drop(stage);
            assert!(!root.join(stage_name).exists());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    exercise("publish", ["p0", "p1", "p2", "p3", "p4", "p5", "p6"]);
    exercise(
        "run",
        ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9"],
    );
}

#[test]
fn phase_b_prepared_file_names_reject_invalid_duplicate_and_wrong_order_before_create() {
    for invalid in ["", ".", "..", "slash/name", "nul\0name"] {
        assert!(platform::prepare_discard_inventory([OsStr::new(invalid)]).is_err());
    }
    assert!(platform::prepare_discard_inventory([
        OsStr::new("duplicate"),
        OsStr::new("duplicate")
    ])
    .is_err());
    let bounded_names = [OsStr::new("first"), OsStr::new("second")];
    let bounded = platform::prepare_discard_inventory(bounded_names).unwrap();
    let exact_native = platform::prepared_discard_inventory_owned_capacity(&bounded);
    assert!(exact_native > 0);
    drop(bounded);
    assert!(platform::prepare_discard_inventory_bounded(bounded_names, exact_native).is_ok());
    assert!(platform::prepare_discard_inventory_bounded(bounded_names, exact_native - 1).is_err());
    #[cfg(windows)]
    {
        for invalid in ["back\\slash", "CON", "com1.txt", "trailing.", "trailing "] {
            assert!(platform::prepare_discard_inventory([OsStr::new(invalid)]).is_err());
        }
        assert!(
            platform::prepare_discard_inventory([OsStr::new("Case"), OsStr::new("case")]).is_err()
        );
    }

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-file-order-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let directory = platform::hold_directory(&root).unwrap();
    let mut inventory =
        platform::prepare_discard_inventory([OsStr::new("first"), OsStr::new("second")]).unwrap();
    reset_phase_b_error_materialization_observer();
    assert!(platform::write_file_new_prepared(
        &directory,
        &mut inventory,
        "second",
        b"must-not-exist",
        0o600,
    )
    .is_err());
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "second").is_err());
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "unknown").is_err());
    assert_eq!(inventory.attached(), 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    platform::write_file_new_prepared(&directory, &mut inventory, "first", b"first", 0o600)
        .unwrap();
    assert!(platform::write_file_new_prepared(
        &directory,
        &mut inventory,
        "first",
        b"duplicate",
        0o600,
    )
    .is_err());
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "second").is_err());
    assert_eq!(inventory.attached(), 1);
    assert!(!root.join("second").exists());
    platform::write_file_new_prepared(&directory, &mut inventory, "second", b"second", 0o600)
        .unwrap();
    let held = platform::hold_regular_file_prepared(&directory, &inventory, "second").unwrap();
    platform::recheck_regular_file(&held).unwrap();
    drop(held);
    std::fs::rename(root.join("second"), root.join("tracked-original")).unwrap();
    std::fs::write(root.join("second"), b"foreign").unwrap();
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "second").is_err());
    assert_eq!(
        platform::read_exact(inventory.file("second").unwrap(), b"second".len()).unwrap(),
        b"second"
    );
    assert_eq!(std::fs::read(root.join("second")).unwrap(), b"foreign");
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    drop(inventory);
    drop(directory);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_prepared_link_copy_binds_tracked_source_and_exact_next_destination() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-link-copy-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let directory = platform::hold_directory(&root).unwrap();
    let mut source = platform::prepare_discard_inventory([OsStr::new("source")]).unwrap();
    let mut destination =
        platform::prepare_discard_inventory([OsStr::new("first"), OsStr::new("copy")]).unwrap();
    let future = platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    let unattached =
        platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    let substituted =
        platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    let duplicate =
        platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    assert!(platform::prepare_link_or_copy(&source, "unknown", &destination, "copy").is_err());
    assert!(platform::prepare_link_or_copy(&source, "source", &destination, "unknown").is_err());

    reset_phase_b_error_materialization_observer();
    assert!(platform::link_or_copy_new_prepared(
        unattached,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .is_err());
    assert!(platform::link_or_copy_new_prepared(
        future,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .is_err());
    assert_eq!(destination.attached(), 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);

    platform::write_file_new_prepared(&directory, &mut source, "source", b"original", 0o600)
        .unwrap();
    platform::write_file_new_prepared(&directory, &mut destination, "first", b"first", 0o600)
        .unwrap();
    std::fs::rename(root.join("source"), root.join("tracked-source")).unwrap();
    std::fs::write(root.join("source"), b"foreign").unwrap();
    platform::link_or_copy_new_prepared(
        substituted,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .unwrap();
    assert_eq!(destination.attached(), 2);
    assert_eq!(
        platform::read_exact(destination.file("copy").unwrap(), b"original".len()).unwrap(),
        b"original"
    );
    assert_eq!(std::fs::read(root.join("source")).unwrap(), b"foreign");
    assert!(platform::link_or_copy_new_prepared(
        duplicate,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .is_err());
    assert_eq!(destination.attached(), 2);

    let mut exists_destination = platform::prepare_discard_inventory([
        OsStr::new("exists-first"),
        OsStr::new("exists-copy"),
    ])
    .unwrap();
    let exists =
        platform::prepare_link_or_copy(&source, "source", &exists_destination, "exists-copy")
            .unwrap();
    platform::write_file_new_prepared(
        &directory,
        &mut exists_destination,
        "exists-first",
        b"first",
        0o600,
    )
    .unwrap();
    std::fs::write(root.join("exists-copy"), b"foreign-destination").unwrap();
    assert!(platform::link_or_copy_new_prepared(
        exists,
        &source,
        &directory,
        &mut exists_destination,
        b"original",
    )
    .is_err());
    assert_eq!(exists_destination.attached(), 1);
    assert_eq!(
        std::fs::read(root.join("exists-copy")).unwrap(),
        b"foreign-destination"
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    drop((exists_destination, destination, source, directory));
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_post_link_pre_auth_failure_is_sticky_and_preserves_inert_stage() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-post-link-pre-auth-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    let mut run_stage = None;
    let mut hooks = 0_usize;
    let mut unexpected_hook = false;
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_DISCARD_ATTEMPTS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION.with(|enabled| enabled.set(true));
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &output,
        |point, _, run, _| {
            hooks += 1;
            unexpected_hook |= point != NativeRustBuildPoint::BeforeClang;
            if point == NativeRustBuildPoint::BeforeClang {
                run_stage = Some(run.to_path_buf());
                std::fs::write(run.join("foreign-sentinel"), b"foreign").unwrap();
            }
        },
    );
    PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION.with(|enabled| enabled.set(false));
    let error = match result {
        Ok(_) => panic!("post-link authentication failure unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(error[0].message, PHASE_B_PUBLICATION_MESSAGE);
    assert_eq!(hooks, 1);
    assert!(!unexpected_hook);
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 3);
    assert_eq!(PHASE_B_LINK_COPY_CONSUMPTIONS.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    let run_stage = run_stage.expect("BeforeClang records the run stage");
    assert!(run_stage.is_dir());
    assert_eq!(
        std::fs::read(run_stage.join("foreign-sentinel")).unwrap(),
        b"foreign"
    );
    assert!(run_stage.join("semaprax_native_rust_interop.rs").is_file());
    assert!(!output.exists());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_link_copy_capacity_minus_one_is_pre_effect() {
    let publish = prepare_publish_discard_inventory().unwrap();
    let run = prepare_run_discard_inventory().unwrap();
    let required = platform::link_or_copy_required_capacity(
        &publish,
        "semaprax_native_rust_interop.rs",
        &run,
        "semaprax_native_rust_interop.rs",
    )
    .unwrap();
    assert!(required > 0);

    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    let (plan, overflowed) = crate::bounded_output::with_limit(required, || {
        prepare_link_copy(
            &publish,
            "semaprax_native_rust_interop.rs",
            &run,
            "semaprax_native_rust_interop.rs",
        )
    });
    assert!(!overflowed);
    let (prepared, budget) = plan.unwrap();
    assert_eq!(
        platform::prepared_link_or_copy_owned_capacity(&prepared),
        required
    );
    assert_eq!(budget.maximum(), required);
    drop((prepared, budget));

    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    let (one_less, overflowed) = crate::bounded_output::with_limit(required - 1, || {
        prepare_link_copy(
            &publish,
            "semaprax_native_rust_interop.rs",
            &run,
            "semaprax_native_rust_interop.rs",
        )
    });
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
}

#[test]
fn phase_b_inventory_exact_plan_is_exact_capacity_and_one_less_is_pre_effect() {
    let publish = prepare_publish_discard_inventory().unwrap();
    let required = platform::inventory_exact_required_capacity(&publish).unwrap();
    assert!(required > 0);
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
    let (exact, overflowed) =
        crate::bounded_output::with_limit(required, || prepare_publish_inventory_exact(&publish));
    assert!(!overflowed);
    let (prepared, budget) = exact.unwrap();
    assert_eq!(
        platform::prepared_inventory_exact_owned_capacity(&prepared),
        required
    );
    assert_eq!(budget.maximum(), required);
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 2);
    drop((prepared, budget));

    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
    let (one_less, overflowed) = crate::bounded_output::with_limit(required - 1, || {
        prepare_publish_inventory_exact(&publish)
    });
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_INVENTORY_EXACT_PLANS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
}

#[test]
fn phase_b_final_publish_plan_is_exact_capacity_and_one_less_is_pre_effect() {
    let output = Path::new("bundle");
    let required = platform::publish_directory_required_capacity(OsStr::new("bundle")).unwrap();
    assert!(required > 0);
    PHASE_B_EFFECT_STARTED.with(|started| started.set(false));
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
    let (exact, overflowed) =
        crate::bounded_output::with_limit(required, || prepare_final_publish(output));
    assert!(!overflowed);
    let (prepared, budget) = exact.unwrap();
    assert_eq!(
        platform::prepared_publish_directory_owned_capacity(&prepared),
        required
    );
    assert_eq!(platform::prepared_publish_directory_remaining(&prepared), 1);
    assert_eq!(budget.maximum(), required);
    assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
    drop((prepared, budget));

    PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
    let (one_less, overflowed) =
        crate::bounded_output::with_limit(required - 1, || prepare_final_publish(output));
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 0);
    assert!(!PHASE_B_EFFECT_STARTED.with(std::cell::Cell::get));
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
}

#[test]
fn phase_b_final_comparisons_precede_scan_and_allocate_no_file_buffers() {
    let source = include_str!("../implementation.rs");
    let start = source.find("fn publish_stage_platform").unwrap();
    let publish = &source[start..];
    let comparison = publish.find("platform::compare_exact").unwrap();
    let scan = publish.find("scan_publish_inventory_exact").unwrap();
    let settle = publish.find(".settle_for_publish()").unwrap();
    let rename = publish
        .find("platform::publish_directory_new_prepared")
        .unwrap();
    assert!(comparison < scan && scan < settle && settle < rename);
    assert!(publish.contains("platform::FILE_COMPARE_SCRATCH_BYTES"));
    assert!(publish.contains(".discard_name"));
    assert!(!publish.contains("platform::read_exact"));
    assert!(!publish.contains("debit_phase_b"));
    assert!(!publish.contains("try_clone"));
}

#[cfg(unix)]
#[test]
fn phase_b_inventory_exact_rejects_every_missing_and_substituted_slot_and_extra() {
    const NAMES: [&str; 7] = [
        "descriptor.json",
        "module.c",
        "semaprax_native_rust_interop.h",
        "semaprax_native_rust_interop.rs",
        "semaprax_native_rust_interop_ffi.rs",
        "module.o",
        "semaprax.native-rust-interop.json",
    ];

    fn fixture(
        root: &Path,
    ) -> (
        platform::HeldDirectory,
        PublishDiscardInventory,
        platform::PreparedInventoryExact<7>,
    ) {
        std::fs::create_dir(root).unwrap();
        let directory = platform::hold_directory(root).unwrap();
        let mut inventory = prepare_publish_discard_inventory().unwrap();
        let prepared = platform::prepare_inventory_exact(&inventory).unwrap();
        for (index, name) in NAMES.iter().enumerate() {
            platform::write_file_new_prepared(
                &directory,
                &mut inventory,
                name,
                &[u8::try_from(index).unwrap()],
                0o600,
            )
            .unwrap();
        }
        (directory, inventory, prepared)
    }

    let base = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-inventory-exact-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir(&base).unwrap();

    for (index, name) in NAMES.iter().enumerate() {
        let root = base.join(format!("missing-{index}"));
        let (directory, inventory, mut prepared) = fixture(&root);
        std::fs::remove_file(root.join(name)).unwrap();
        assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
        drop((prepared, inventory, directory));
        std::fs::remove_dir_all(&root).unwrap();
    }

    for (index, name) in NAMES.iter().enumerate() {
        let root = base.join(format!("substituted-{index}"));
        let (directory, inventory, mut prepared) = fixture(&root);
        std::fs::rename(root.join(name), root.join(format!("tracked-{index}"))).unwrap();
        std::fs::write(root.join(name), b"foreign").unwrap();
        assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
        assert_eq!(std::fs::read(root.join(name)).unwrap(), b"foreign");
        drop((prepared, inventory, directory));
        std::fs::remove_dir_all(&root).unwrap();
    }

    let root = base.join("extra");
    let (directory, inventory, mut prepared) = fixture(&root);
    std::fs::write(root.join("foreign-extra"), b"foreign").unwrap();
    assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
    drop((prepared, inventory, directory));
    std::fs::remove_dir_all(&root).unwrap();

    #[cfg(target_os = "linux")]
    {
        let root = base.join("invalid-encoding");
        let (directory, inventory, mut prepared) = fixture(&root);
        use std::os::unix::ffi::OsStringExt as _;
        std::fs::write(
            root.join(std::ffi::OsString::from_vec(vec![0xff])),
            b"foreign",
        )
        .unwrap();
        assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
        drop((prepared, inventory, directory));
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::remove_dir_all(&base).unwrap();
}

#[cfg(unix)]
#[test]
fn phase_b_inventory_exact_is_bound_to_one_inventory_and_exactly_two_scans() {
    const NAMES: [&str; 7] = [
        "descriptor.json",
        "module.c",
        "semaprax_native_rust_interop.h",
        "semaprax_native_rust_interop.rs",
        "semaprax_native_rust_interop_ffi.rs",
        "module.o",
        "semaprax.native-rust-interop.json",
    ];
    let base = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-inventory-binding-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir(&base).unwrap();
    let mut fixtures = Vec::with_capacity(2);
    for suffix in ["a", "b"] {
        let root = base.join(suffix);
        std::fs::create_dir(&root).unwrap();
        let directory = platform::hold_directory(&root).unwrap();
        let mut inventory = prepare_publish_discard_inventory().unwrap();
        for (index, name) in NAMES.iter().enumerate() {
            platform::write_file_new_prepared(
                &directory,
                &mut inventory,
                name,
                &[u8::try_from(index).unwrap()],
                0o600,
            )
            .unwrap();
        }
        fixtures.push((root, directory, inventory));
    }
    let mut prepared = platform::prepare_inventory_exact(&fixtures[0].2).unwrap();
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[1].1, &fixtures[1].2).is_err()
    );
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 2);
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[0].1, &fixtures[0].2).is_ok()
    );
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 1);
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[0].1, &fixtures[0].2).is_ok()
    );
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 0);
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[0].1, &fixtures[0].2).is_err()
    );
    let hardlink_root = base.join("hardlink-second-directory");
    std::fs::create_dir(&hardlink_root).unwrap();
    for name in NAMES {
        std::fs::hard_link(fixtures[0].0.join(name), hardlink_root.join(name)).unwrap();
    }
    let hardlink_directory = platform::hold_directory(&hardlink_root).unwrap();
    let mut hardlink_prepared = platform::prepare_inventory_exact(&fixtures[0].2).unwrap();
    assert!(platform::inventory_exact_prepared(
        &mut hardlink_prepared,
        &fixtures[0].1,
        &fixtures[0].2
    )
    .is_ok());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&hardlink_prepared),
        1
    );
    assert!(platform::inventory_exact_prepared(
        &mut hardlink_prepared,
        &hardlink_directory,
        &fixtures[0].2
    )
    .is_err());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&hardlink_prepared),
        1
    );
    let mut consumed_failure = platform::prepare_inventory_exact(&fixtures[1].2).unwrap();
    assert!(platform::inventory_exact_prepared(
        &mut consumed_failure,
        &fixtures[1].1,
        &fixtures[1].2
    )
    .is_ok());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&consumed_failure),
        1
    );
    for name in NAMES {
        std::fs::remove_file(fixtures[1].0.join(name)).unwrap();
    }
    assert!(platform::inventory_exact_prepared(
        &mut consumed_failure,
        &fixtures[1].1,
        &fixtures[1].2
    )
    .is_err());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&consumed_failure),
        0
    );
    drop((
        consumed_failure,
        hardlink_prepared,
        hardlink_directory,
        prepared,
        fixtures,
    ));
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn phase_b_nonce_exists_retries_reuse_native_stage_name_arena_without_materialization() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-nonce-retry-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let parent = hold_stage(root.clone()).unwrap();
    let digest = "sha256:nonce-retry";
    reset_phase_b_native_stage_arena_observer();
    let mut collision = StageSlot::new(&root, digest, "retry").unwrap();
    let native_capacity = collision.native_name.capacity();
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1,
    );
    collision.prepare(&root, 0).unwrap();
    assert_eq!(collision.native_name.capacity(), native_capacity);
    std::fs::create_dir(&collision.path).unwrap();
    let inventory = platform::prepare_discard_inventory([]).unwrap();
    let slot = StageSlot::new(&root, digest, "retry").unwrap();
    assert_eq!(slot.native_name.capacity(), native_capacity);
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        2,
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_SETS.with(std::cell::Cell::get),
        1,
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(std::cell::Cell::get),
        0,
    );
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    let stage = create_stage(&parent, slot, &inventory).unwrap();
    assert!(stage
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with("-1"));
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        2,
        "nonce retries allocated a new native name after effects",
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_SETS.with(std::cell::Cell::get),
        3,
        "the collision probe plus nonce zero and nonce one must set one arena",
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(std::cell::Cell::get),
        2,
        "both create attempts must consume the prepared native arena",
    );
    assert_eq!(
        stage.discard_name.as_ref().unwrap().capacity(),
        native_capacity,
    );
    discard_run_stage(&parent, &stage, &inventory).unwrap();
    drop(stage);
    drop(parent);
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_every_mid_discard_failure_moves_prebuilt_i232_without_materialization() {
    const NAMES: [&str; 3] = ["first", "second", "third"];
    for failure_after_delete in 0..=NAMES.len() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-mid-discard-{}-{failure_after_delete}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let parent = hold_stage(root.clone()).unwrap();
        let mut inventory = platform::prepare_discard_inventory(NAMES.map(OsStr::new)).unwrap();
        let slot = StageSlot::new(&root, "sha256:mid-discard", "discard").unwrap();
        let stage = create_stage(&parent, slot, &inventory).unwrap();
        for name in NAMES {
            platform::write_file_new_prepared(
                stage.authority.held(),
                &mut inventory,
                name,
                b"owned",
                0o600,
            )
            .unwrap();
        }
        inventory.inject_discard_failure_after_delete(Some(failure_after_delete));
        reset_phase_b_error_materialization_observer();
        let mut carriers = PhaseBErrorCarriers::prepare().unwrap();
        mark_phase_b_effect_started();
        assert!(discard_run_stage(&parent, &stage, &inventory).is_err());
        let error = carriers.take(PhaseBLocalError::Publication);
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-I232");
        assert_eq!(error[0].message, PHASE_B_PUBLICATION_MESSAGE);
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "delete boundary {failure_after_delete} materialized an error after effects",
        );
        drop(error);
        drop(inventory);
        drop(stage);
        drop(parent);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn noncanonical_spec_is_b106_before_target_admission() {
    let (program, spec) = fixture();
    let noncanonical = spec.replacen("{\"schema\"", "{ \"schema\"", 1);
    let error = match prepare_native_rust_interop(&program, noncanonical.as_bytes()) {
        Ok(_) => panic!("noncanonical spec was accepted"),
        Err(error) => error,
    };
    assert_eq!(error[0].code, "SPX-B106");
}

#[test]
fn specification_parser_is_canonical_bounded_and_intent_bound() {
    fn assert_spec_error(program: &Program, source: &[u8], code: &str, message: &str) {
        let error = match parse_spec(program, source) {
            Ok(_) => panic!("hostile specification was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
    }

    let (program, canonical) = fixture();
    parse_spec(&program, canonical.as_bytes()).unwrap();
    let b106_message =
            "Native Rust Interop specification is not canonical semaprax.native-rust-interop-spec.v1 JSON";
    let schema_prefix = format!("\"schema\":{},", quote_json(SPEC_SCHEMA));
    let malformed = [
        format!(" \n{canonical}"),
        canonical.trim_end().to_owned(),
        canonical.replace('\n', "\r\n"),
        format!("\u{feff}{canonical}"),
        canonical.replacen(&schema_prefix, "", 1),
        canonical.replacen(
            &schema_prefix,
            &format!("{schema_prefix}{schema_prefix}"),
            1,
        ),
        canonical.replacen(&schema_prefix, &format!("{schema_prefix}\"extra\":0,"), 1),
        canonical.replacen(
            &format!("\"schema\":{}", quote_json(SPEC_SCHEMA)),
            "\"schema\":1",
            1,
        ),
        canonical.replacen(
            "\"exports\":[\"interop.add\"]",
            "\"exports\":[\"interop.add\",\"interop.add\"]",
            1,
        ),
        canonical.replacen("\"max_exports\":32", "\"max_exports\":31", 1),
        canonical.replacen(
            "no_resource_owned_borrow_shared_or_aggregate_abi",
            "xo_resource_owned_borrow_shared_or_aggregate_abi",
            1,
        ),
    ];
    for mutation in malformed {
        assert_spec_error(&program, mutation.as_bytes(), "SPX-B106", b106_message);
    }

    let exact_depth = format!("[[[[[[{canonical}]]]]]]");
    assert_eq!(json_depth(exact_depth.as_bytes()).unwrap(), MAX_JSON_DEPTH);
    assert_spec_error(&program, exact_depth.as_bytes(), "SPX-B106", b106_message);
    let over_depth = format!("[{exact_depth}]");
    assert_spec_error(
        &program,
        over_depth.as_bytes(),
        "SPX-B109",
        "Native Rust Interop max_json_depth exceeds 8",
    );

    let exact_cap = vec![b' '; MAX_SPEC_BYTES];
    assert_spec_error(&program, &exact_cap, "SPX-B106", b106_message);
    let over_cap = vec![b' '; MAX_SPEC_BYTES + 1];
    assert_spec_error(
        &program,
        &over_cap,
        "SPX-B109",
        "Native Rust Interop max_spec_bytes exceeds 1048576",
    );

    let mut exact_source_program = program.clone();
    exact_source_program.functions[0].name.clear();
    let source_overhead = crate::format::canonical(&exact_source_program).len();
    exact_source_program.functions[0].name = "a".repeat(MAX_SOURCE_BYTES - source_overhead);
    let exact_source = crate::format::canonical(&exact_source_program);
    assert_eq!(exact_source.len(), MAX_SOURCE_BYTES);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (scratch_error, overflowed, consumed) = crate::bounded_output::with_limit_usage(
        canonical_format_scratch_capacity(&exact_source_program)
            .unwrap()
            .bytes()
            - 1,
        || canonical_source_bounded(&exact_source_program),
    );
    let scratch_error = scratch_error.unwrap_err();
    assert!(!overflowed);
    assert_eq!(consumed, 0, "rejected scratch reservation leaked budget");
    assert_eq!(scratch_error.code, "SPX-B109");
    assert_eq!(
        scratch_error.message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
    let exact_peak = canonical_format_scratch_capacity(&exact_source_program)
        .unwrap()
        .bytes()
        .checked_add(MAX_SOURCE_BYTES)
        .unwrap();
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (bounded_source, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak, || {
            canonical_source_bounded(&exact_source_program)
        });
    let bounded_source = bounded_source.unwrap();
    assert!(!overflowed);
    assert_eq!(consumed, MAX_SOURCE_BYTES);
    assert_eq!(bounded_source.len(), MAX_SOURCE_BYTES);
    assert_eq!(bounded_source.capacity(), MAX_SOURCE_BYTES);
    assert_eq!(bounded_source, exact_source);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 2));
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (peak_error, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak - 1, || {
            canonical_source_bounded(&exact_source_program)
        });
    let peak_error = peak_error.unwrap_err();
    assert!(!overflowed);
    assert_eq!(consumed, 0, "failed materialization leaked scratch budget");
    assert_eq!(peak_error.code, "SPX-B109");
    assert_eq!(
        peak_error.message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
    let mut exact_source_spec = parse_spec(&program, canonical.as_bytes()).unwrap();
    exact_source_spec.source_revision = domain_digest(SOURCE_DOMAIN, exact_source.as_bytes());
    parse_spec(
        &exact_source_program,
        render_spec(&exact_source_spec).as_bytes(),
    )
    .unwrap();

    let mut over_program = exact_source_program;
    over_program.functions[0].name.push('a');
    let over_source = crate::format::canonical(&over_program);
    assert_eq!(over_source.len(), MAX_SOURCE_BYTES + 1);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (bounded_source, overflowed, consumed) = crate::bounded_output::with_limit_usage(
        canonical_format_scratch_capacity(&over_program)
            .unwrap()
            .bytes(),
        || canonical_source_bounded(&over_program),
    );
    let bounded_source = bounded_source.unwrap_err();
    assert!(!overflowed);
    assert_eq!(consumed, 0, "over-limit counting pass allocated output");
    assert_eq!(bounded_source.code, "SPX-B109");
    assert_eq!(
        bounded_source.message,
        "Native Rust Interop max_source_bytes exceeds 16777216"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
    assert_eq!(
        crate::format::canonical(&over_program),
        over_source,
        "bounded formatting mutated the source program"
    );
    let mut over_source_spec = exact_source_spec;
    over_source_spec.source_revision = domain_digest(SOURCE_DOMAIN, over_source.as_bytes());
    let error = match parse_spec(&over_program, render_spec(&over_source_spec).as_bytes()) {
        Ok(_) => panic!("over-limit source was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_source_bytes exceeds 16777216"
    );

    let spec = parse_spec(&program, canonical.as_bytes()).unwrap();
    for mutation in [
        {
            let mut value = spec.clone();
            value.module = "forged.module".to_owned();
            value
        },
        {
            let mut value = spec.clone();
            value.source_revision = "sha256:forged-source".to_owned();
            value
        },
    ] {
        assert_spec_error(
            &program,
            render_spec(&mutation).as_bytes(),
            "SPX-B107",
            "Native Rust Interop declaration set is unsupported: selected identity missing",
        );
    }
    let mut wrong_target = spec.clone();
    wrong_target.target.triple = "forged-unknown-target".to_owned();
    assert_spec_error(
        &program,
        render_spec(&wrong_target).as_bytes(),
        "SPX-B107",
        "Native Rust Interop declaration set is unsupported: target profile mismatch",
    );
    let mut wrong_capability = spec.clone();
    wrong_capability.capabilities = vec!["forged.capability".to_owned()];
    let error =
        match prepare_native_rust_interop(&program, render_spec(&wrong_capability).as_bytes()) {
            Ok(_) => panic!("forged capability was accepted"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B107");
    assert_eq!(
        error[0].message,
        "Native Rust Interop declaration set is unsupported: effect or capability mismatch"
    );

    let automatic_source = SOURCE.replacen("@id(\"interop.add\")\n", "", 1);
    let automatic_program = crate::parse(
        &automatic_source,
        Path::new("native-rust-interop-automatic-export.spx"),
    )
    .unwrap();
    let automatic_id = automatic_program
        .functions
        .iter()
        .find(|function| function.name == "add")
        .unwrap()
        .stable_id
        .clone();
    let mut automatic_spec = spec;
    automatic_spec.source_revision = domain_digest(
        SOURCE_DOMAIN,
        crate::format::canonical(&automatic_program).as_bytes(),
    );
    automatic_spec.exports = vec![automatic_id];
    let error = match prepare_native_rust_interop(
        &automatic_program,
        render_spec(&automatic_spec).as_bytes(),
    ) {
        Ok(_) => panic!("automatic export identity was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B107");
    assert_eq!(
        error[0].message,
        "Native Rust Interop declaration set is unsupported: explicit persistent ID required"
    );
}

#[test]
fn specification_shape_rejects_flat_container_and_scalar_explosions_before_decode() {
    let (program, _) = fixture();
    for element in ["[]", "0", "\"\""] {
        let mut hostile = String::with_capacity(MAX_SPEC_BYTES);
        hostile.push('[');
        let mut first = true;
        while hostile
            .len()
            .checked_add(usize::from(!first))
            .and_then(|length| length.checked_add(element.len()))
            .is_some_and(|length| length < MAX_SPEC_BYTES)
        {
            if !first {
                hostile.push(',');
            }
            hostile.push_str(element);
            first = false;
        }
        while hostile.len() + 1 < MAX_SPEC_BYTES {
            hostile.push(' ');
        }
        hostile.push(']');
        assert_eq!(hostile.len(), MAX_SPEC_BYTES);
        let error = match parse_spec(&program, hostile.as_bytes()) {
            Ok(_) => panic!("hostile generic JSON shape was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code, "SPX-B106");
        assert_eq!(
                error.message,
                "Native Rust Interop specification is not canonical semaprax.native-rust-interop-spec.v1 JSON"
            );
    }
}

#[test]
fn export_import_and_parameter_count_limits_are_exact() {
    let mut source = String::from(
        "module interop.limit;\n\n@id(\"host.limit\")\ninterface HostLimit\n    permits {  }\n{\n",
    );
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    for index in 0..MAX_IMPORTS {
        write!(
                source,
                "    @id(\"host.{index:02}\")\n    import rust fn import_{index:02}({parameters}) -> i64\n        effects {{  }}\n        failure infallible;\n"
            )
            .unwrap();
    }
    source.push_str("}\n\n");
    for index in 0..MAX_EXPORTS {
        write!(
                source,
                "@id(\"export.{index:02}\")\nfn export_{index:02}({parameters}) -> i64\n{{\n    import_{index:02}({arguments})\n}}\n\n"
            )
            .unwrap();
    }
    source.push_str("@id(\"interop.limit.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let program = crate::parse(&source, Path::new("native-rust-limits.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes()),
        target: current_target().unwrap(),
        exports: (0..MAX_EXPORTS)
            .map(|index| format!("export.{index:02}"))
            .collect(),
        imports: (0..MAX_IMPORTS)
            .map(|index| format!("host.{index:02}"))
            .collect(),
        capabilities: Vec::new(),
    };
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    assert_eq!(prepared.exports.len(), MAX_EXPORTS);
    assert_eq!(prepared.imports.len(), MAX_IMPORTS);
    assert_eq!(prepared.closure.len(), MAX_EXPORTS);
    assert!(prepared
        .exports
        .iter()
        .all(|export| export.parameters.len() == MAX_PARAMETERS));
    assert!(prepared
        .imports
        .iter()
        .all(|import| import.parameters.len() == MAX_PARAMETERS));

    let mut over_exports = spec.clone();
    over_exports.exports.push("export.over".to_owned());
    let error = match parse_spec(&program, render_spec(&over_exports).as_bytes()) {
        Ok(_) => panic!("over-limit export set was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(error.message, "Native Rust Interop max_exports exceeds 32");

    let mut over_imports = spec;
    over_imports.imports.push("host.over".to_owned());
    let error = match parse_spec(&program, render_spec(&over_imports).as_bytes()) {
        Ok(_) => panic!("over-limit import set was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(error.message, "Native Rust Interop max_imports exceeds 32");
}

#[test]
fn closure_effect_and_identifier_limits_are_exact() {
    let effects = (0..MAX_EFFECTS)
        .map(|index| {
            let first = char::from(b'a' + u8::try_from(index / 26).unwrap());
            let second = char::from(b'a' + u8::try_from(index % 26).unwrap());
            format!("effect.e{first}{second}")
        })
        .collect::<Vec<_>>();
    let effect_list = effects.join(", ");
    let source = format!(
            "module interop.effects;\n\npermit {{ {effect_list} }}\n\n@id(\"host.effects\")\ninterface HostEffects\n    permits {{ {effect_list} }}\n{{\n    @id(\"host.effects.call\")\n    import rust fn host_call(value: i64) -> i64\n        effects {{ {effect_list} }}\n        failure infallible;\n}}\n\n@id(\"export.effects\")\nfn export_effects(value: i64) -> i64\n    uses {{ {effect_list} }}\n{{\n    host_call(value)\n}}\n\n@id(\"interop.effects.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        );
    let program = crate::parse(&source, Path::new("native-rust-effects.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes()),
        target: current_target().unwrap(),
        exports: vec!["export.effects".to_owned()],
        imports: vec!["host.effects.call".to_owned()],
        capabilities: effects,
    };
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    assert_eq!(prepared.exports[0].effects.len(), MAX_EFFECTS);
    assert_eq!(prepared.imports[0].effects.len(), MAX_EFFECTS);

    let mut over_effects = spec.clone();
    over_effects.capabilities.push("effect.over".to_owned());
    let error = match parse_spec(&program, render_spec(&over_effects).as_bytes()) {
        Ok(_) => panic!("over-limit capability set was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(error.message, "Native Rust Interop max_effects exceeds 64");

    for (length, code, message) in [
        (
            MAX_IDENTIFIER_BYTES,
            "SPX-B107",
            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
        ),
        (
            MAX_IDENTIFIER_BYTES + 1,
            "SPX-B109",
            "Native Rust Interop max_identifier_bytes exceeds 128",
        ),
    ] {
        let mut identifier_spec = spec.clone();
        identifier_spec.capabilities = vec!["a".repeat(length)];
        let error =
            match prepare_native_rust_interop(&program, render_spec(&identifier_spec).as_bytes()) {
                Ok(_) => panic!("hostile identifier was accepted"),
                Err(error) => error,
            };
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, code);
        assert_eq!(error[0].message, message);
    }

    fn closure_fixture(count: usize) -> (Program, Spec) {
        let mut source = String::from(
                "module interop.closure;\n\n@id(\"host.closure\")\ninterface HostClosure\n    permits {  }\n{\n    @id(\"host.closure.leaf\")\n    import rust fn host_leaf(value: i64) -> i64\n        effects {  }\n        failure infallible;\n}\n\n",
            );
        for index in 0..count {
            let body = if index + 1 == count {
                "host_leaf(value)".to_owned()
            } else {
                format!("closure_{:03}(value)", index + 1)
            };
            write!(
                    source,
                    "@id(\"closure.{index:03}\")\nfn closure_{index:03}(value: i64) -> i64\n{{\n    {body}\n}}\n\n"
                )
                .unwrap();
        }
        source.push_str("@id(\"interop.closure.main\")\nfn main() -> i64\n{\n    0\n}\n");
        let program = crate::parse(&source, Path::new("native-rust-closure.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        let spec = Spec {
            module: program.module.clone(),
            source_revision: domain_digest(SOURCE_DOMAIN, canonical.as_bytes()),
            target: current_target().unwrap(),
            exports: vec!["closure.000".to_owned()],
            imports: vec!["host.closure.leaf".to_owned()],
            capabilities: Vec::new(),
        };
        (program, spec)
    }

    let (program, spec) = closure_fixture(MAX_CALL_DEPTH);
    let canonical_source = crate::format::canonical(&program);
    let mut hir_scan_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let closure_phase =
        hir_pre_resolve_capacity(&program, canonical_source.len(), &mut hir_scan_stack)
            .unwrap()
            .phase_peaks()[6];
    let terms = hir_capacity_terms_for_test(&program, canonical_source.len()).unwrap();
    assert_eq!(terms.2, 0, "scalar closure has no retained cleanup payload");
    reset_closure_capacity_high_water();
    let prepared = prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()).unwrap();
    assert_eq!(prepared.closure.len(), MAX_CALL_DEPTH);
    let observed_closure_peak = closure_capacity_high_water();
    assert!(observed_closure_peak <= closure_phase);
    assert_eq!(observed_closure_peak, 8_220);
    let (program, spec) = closure_fixture(MAX_CALL_DEPTH + 1);
    let error = match prepare_native_rust_interop(&program, render_spec(&spec).as_bytes()) {
        Ok(_) => panic!("over-limit call depth was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(
        error[0].message,
        "Native Rust Interop max_call_depth exceeds 32"
    );

    let cycle_source = "module interop.closure_cycle; @id(\"cycle.a\") fn a(value: i64) -> i64 { b(value) } @id(\"cycle.b\") fn b(value: i64) -> i64 { a(value) } @id(\"app.main\") fn main() -> i64 { 0 }";
    let cycle_program =
        crate::parse(cycle_source, Path::new("native-rust-closure-cycle.spx")).unwrap();
    let cycle_spec = Spec {
        module: cycle_program.module.clone(),
        source_revision: domain_digest(
            SOURCE_DOMAIN,
            crate::format::canonical(&cycle_program).as_bytes(),
        ),
        target: current_target().unwrap(),
        exports: vec!["cycle.a".to_owned()],
        imports: Vec::new(),
        capabilities: Vec::new(),
    };
    let error =
        match prepare_native_rust_interop(&cycle_program, render_spec(&cycle_spec).as_bytes()) {
            Ok(_) => panic!("cyclic closure was accepted"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B107");
    assert_eq!(
        error[0].message,
        "Native Rust Interop declaration set is unsupported: selected closure is cyclic"
    );
}

#[test]
fn deeply_forged_hir_fails_iteratively_without_stack_growth() {
    let (program, _) = fixture();
    let mut resolved = hir::resolve(&program).unwrap();
    let function_index = resolved
        .functions
        .iter()
        .position(|function| function.id.as_str() == "interop.add")
        .unwrap();
    let mut expression = resolved.functions[function_index].body.clone();
    for _ in 0..MAX_SEMANTIC_EXPRESSION_DEPTH {
        let id = expression.id.clone();
        let ty = expression.ty.clone();
        let ownership = expression.ownership;
        let span = expression.span;
        expression = ResolvedExpr {
            id,
            ty,
            ownership,
            kind: ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: Box::new(expression),
            },
            span,
        };
    }
    resolved.functions[function_index].body = expression;
    let capacity = MAX_SEMANTIC_EXPRESSION_DEPTH * 4 + 32;
    let owner = ResolvedProgramOwner::new(resolved, Vec::with_capacity(capacity), capacity);
    let error = validate_native_rust_expression_budget(owner.program()).unwrap_err();
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_semantic_expression_depth exceeds 512"
    );
    drop(owner);
}

#[test]
fn semantic_expression_depth_512_is_exact_for_source_and_hir() {
    fn wrap_source(mut expression: crate::ast::Expr, count: usize) -> crate::ast::Expr {
        for _ in 0..count {
            let span = expression.span;
            expression = crate::ast::Expr {
                kind: crate::ast::ExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: Box::new(expression),
                },
                span,
            };
        }
        expression
    }

    fn wrap_hir(mut expression: ResolvedExpr, count: usize) -> ResolvedExpr {
        for _ in 0..count {
            let id = expression.id.clone();
            let ty = expression.ty.clone();
            let ownership = expression.ownership;
            let span = expression.span;
            expression = ResolvedExpr {
                id,
                ty,
                ownership,
                kind: ResolvedExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: Box::new(expression),
                },
                span,
            };
        }
        expression
    }

    // The fixture body has depth four: block -> addition -> import call -> argument.
    const EXACT_WRAPPERS: usize = MAX_SEMANTIC_EXPRESSION_DEPTH - 4;
    let (program, _) = fixture();
    let mut exact_source = program.clone();
    let function = exact_source
        .functions
        .iter_mut()
        .find(|function| function.stable_id == "interop.add")
        .unwrap();
    function.body = wrap_source(function.body.clone(), EXACT_WRAPPERS);
    validate_native_rust_source_expression_budget(&exact_source).unwrap();
    let canonical_exact = canonical_source_bounded(&exact_source).unwrap();
    let mut hir_scan_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let hir_upper =
        hir_pre_resolve_capacity(&exact_source, canonical_exact.len(), &mut hir_scan_stack)
            .unwrap();
    assert!(hir_upper.complete().unwrap() >= canonical_exact.len());
    let resolved_exact_source = hir::resolve(&exact_source).unwrap();
    validate_native_rust_expression_budget(&resolved_exact_source).unwrap();
    let mut over_source = exact_source;
    let function = over_source
        .functions
        .iter_mut()
        .find(|function| function.stable_id == "interop.add")
        .unwrap();
    function.body = wrap_source(function.body.clone(), 1);
    let error = validate_native_rust_source_expression_budget(&over_source).unwrap_err();
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_semantic_expression_depth exceeds 512"
    );

    let mut exact_hir = hir::resolve(&program).unwrap();
    let function = exact_hir
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "interop.add")
        .unwrap();
    function.body = wrap_hir(function.body.clone(), EXACT_WRAPPERS);
    validate_native_rust_expression_budget(&exact_hir).unwrap();
    let function = exact_hir
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "interop.add")
        .unwrap();
    function.body = wrap_hir(function.body.clone(), 1);
    let error = validate_native_rust_expression_budget(&exact_hir).unwrap_err();
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_semantic_expression_depth exceeds 512"
    );
}

#[test]
fn canonical_formatter_census_admits_shallow_wide_types_and_patterns() {
    const WIDTH: usize = 128;
    #[allow(clippy::format_collect)]
    let fields = (0..WIDTH)
        .map(|index| format!("    @id(\"wide.record.f{index:03}\")\n    f{index:03}: i64,\n"))
        .collect::<String>();
    let pattern = (0..WIDTH)
        .map(|index| format!("f{index:03}: _"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
            "module formatter.wide;\n\n@id(\"wide.record\")\nrecord Wide {{\n{fields}}}\n\n@id(\"wide.read\")\nfn read(value: Wide) -> i64\n{{\n    match value {{\n        Wide {{ {pattern} }} => 0,\n    }}\n}}\n\n@id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        );
    let program = crate::parse(&source, Path::new("formatter-shallow-wide.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let scratch = canonical_format_scratch_capacity(&program).unwrap();
    assert_eq!(
        scratch.bytes(),
        crate::private_format::private_scratch_capacity(3, 1, 1)
            .unwrap()
            .bytes(),
        "width must not be mistaken for recursive formatter depth"
    );
    let exact_peak = scratch.bytes().checked_add(canonical.len()).unwrap();
    let (bounded, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak, || canonical_source_bounded(&program));
    let bounded = bounded.unwrap();
    assert!(!overflowed);
    assert_eq!(bounded, canonical);
    assert_eq!(consumed, canonical.len());

    let mut deep_program = crate::parse(
        "module formatter.deep; @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("formatter-deep-types.spx"),
    )
    .unwrap();
    let mut deep_type = crate::ast::Type::I64;
    for index in 0..32 {
        deep_type = crate::ast::Type::Named {
            name: format!("T{index}"),
            arguments: vec![deep_type],
        };
    }
    let call = |index| crate::ast::Expr {
        kind: crate::ast::ExprKind::Call {
            name: format!("callee_{index}"),
            type_arguments: vec![deep_type.clone()],
            args: vec![],
        },
        span: crate::ast::Span::default(),
    };
    deep_program.functions[0].body = crate::ast::Expr {
        kind: crate::ast::ExprKind::Block {
            statements: (0..64)
                .map(|index| crate::ast::Statement::Let {
                    name: format!("value_{index}"),
                    name_span: crate::ast::Span::default(),
                    mutable: false,
                    declared: None,
                    value: call(index),
                    span: crate::ast::Span::default(),
                })
                .collect(),
            tail: Box::new(call(64)),
        },
        span: crate::ast::Span::default(),
    };
    let canonical = crate::format::canonical(&deep_program);
    let scratch = canonical_format_scratch_capacity(&deep_program).unwrap();
    assert_eq!(
        scratch.bytes(),
        crate::private_format::private_scratch_capacity(2, 33, 1)
            .unwrap()
            .bytes(),
        "statement and type width must not inflate nesting, but embedded type depth must count"
    );
    let exact_peak = scratch.bytes().checked_add(canonical.len()).unwrap();
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (bounded, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak, || {
            canonical_source_bounded(&deep_program)
        });
    assert_eq!(bounded.unwrap(), canonical);
    assert!(!overflowed);
    assert_eq!(
        consumed,
        canonical.len(),
        "private formatting charged legacy temporaries"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 2));
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (error, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak - 1, || {
            canonical_source_bounded(&deep_program)
        });
    assert_eq!(error.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn formatter_frame_capacity_covers_nested_delimiters_and_helper_stacks() {
    use crate::ast::{Expr, ExprKind, MatchArm, MatchPattern, RecordMatchFieldPattern};

    let span = crate::ast::Span::default();
    let mut ty = crate::ast::Type::I64;
    for index in 0..31 {
        ty = crate::ast::Type::Named {
            name: format!("T{index}"),
            arguments: vec![ty],
        };
    }
    let mut scrutinee = Expr {
        kind: ExprKind::ConstructRecord {
            type_name: "Leaf".into(),
            type_span: span,
            type_arguments: vec![ty.clone()],
            fields: vec![],
        },
        span,
    };
    for _ in 0..64 {
        scrutinee = Expr {
            kind: ExprKind::If {
                condition: Box::new(scrutinee),
                then_branch: Box::new(Expr {
                    kind: ExprKind::Int(1),
                    span,
                }),
                else_branch: Box::new(Expr {
                    kind: ExprKind::Int(0),
                    span,
                }),
            },
            span,
        };
    }
    let mut nested_pattern = RecordMatchFieldPattern::Binding {
        name: "value".into(),
        span,
    };
    for index in 0..31 {
        nested_pattern = RecordMatchFieldPattern::Record {
            type_name: format!("P{index}"),
            type_span: span,
            fields: vec![crate::ast::RecordMatchPatternField {
                name: "next".into(),
                name_span: span,
                pattern: nested_pattern,
                span,
            }],
            span,
        };
    }
    let mut program = crate::parse(
        "module formatter.frames; @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("formatter-frames.spx"),
    )
    .unwrap();
    program.functions[0].body = Expr {
        kind: ExprKind::Match {
            scrutinee: Box::new(scrutinee),
            arms: vec![MatchArm {
                pattern: MatchPattern::Record {
                    type_name: "Root".into(),
                    type_span: span,
                    fields: vec![crate::ast::RecordMatchPatternField {
                        name: "next".into(),
                        name_span: span,
                        pattern: nested_pattern,
                        span,
                    }],
                    span,
                },
                value: Expr {
                    kind: ExprKind::Call {
                        name: "typed".into(),
                        type_arguments: vec![ty],
                        args: vec![],
                    },
                    span,
                },
                span,
            }],
        },
        span,
    };

    let capacity = canonical_format_scratch_capacity(&program).unwrap();
    crate::private_format::reset_private_scratch_high_water();
    let mut sink = String::new();
    crate::private_format::write_canonical_with_scratch(&program, &mut sink, capacity);
    let water = crate::private_format::private_scratch_high_water();
    let slots = capacity.slots();
    for (index, ((length, allocated), admitted)) in water.into_iter().zip(slots).enumerate() {
        assert!(length > 0, "formatter helper {index} was not exercised");
        assert!(
            length <= admitted,
            "formatter helper {index} exceeded census"
        );
        assert_eq!(allocated, admitted, "formatter helper {index} grew its Vec");
    }
    assert!(
        water[0].0 > 120,
        "nested delimiter continuations were not retained"
    );
    assert!(
        water[1].0 > 60,
        "nested contains-record traversal was not retained"
    );
    assert!(water[2].0 > 30, "nested type traversal was not retained");
    assert!(water[3].0 > 30, "nested pattern traversal was not retained");
}

#[test]
fn declaration_dag_capacity_counts_layered_leaf_and_layout_expansion_once() {
    fn layered(resource: bool, levels: usize) -> Program {
        let mut source = String::from("module capacity.layers;\n\n");
        if resource {
            source.push_str("@id(\"layer.r0\")\nresource R0 {\n    @id(\"layer.r0.drop\")\n    drop trivial;\n}\n\n");
        } else {
            source.push_str("@id(\"layer.r0\")\nrecord R0 {\n    @id(\"layer.r0.value\")\n    value: i64,\n}\n\n");
        }
        for level in 1..=levels {
            writeln!(
                    source,
                    "@id(\"layer.r{level}\")\nrecord R{level} {{\n    @id(\"layer.r{level}.a\")\n    a: R{},\n    @id(\"layer.r{level}.b\")\n    b: R{},\n}}\n",
                    level - 1,
                    level - 1
                )
                .unwrap();
        }
        source.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
        crate::parse(&source, Path::new("layered-capacity.spx")).unwrap()
    }

    let resource = declaration_dag_expansion(&layered(true, 12), 0).unwrap();
    assert_eq!(resource.maximum_resource_leaves, 1 << 12);
    assert_eq!(resource.maximum_type_occurrences, (1 << 13) - 1);
    assert!(resource.maximum_shape_fields >= (1 << 13) - 2);
    assert!(resource.maximum_projection_segments >= 12 * (1 << 12));
    assert!(resource.maximum_shape_identity_bytes > 0);
    assert!(resource.maximum_lifecycle_identity_bytes > 0);
    assert!(resource.maximum_projection_identity_bytes > 0);
    let scalar = declaration_dag_expansion(&layered(false, 12), 0).unwrap();
    assert_eq!(scalar.maximum_resource_leaves, 0);
    assert_eq!(scalar.maximum_type_occurrences, 3 * (1 << 12) - 1);
    assert!(scalar.maximum_shape_fields >= (1 << 13) - 1);
    assert_eq!(scalar.maximum_projection_segments, 0);

    let long = "x".repeat(128);
    let long_source = format!(
            "module capacity.long; @id(\"life.{long}\") resource Leaf {{ @id(\"drop.{long}\") drop trivial; }} @id(\"outer.{long}\") record Outer {{ @id(\"field.{long}\") leaf: Leaf, }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
        );
    let long_program = crate::parse(&long_source, Path::new("long-capacity.spx")).unwrap();
    let long_expansion = declaration_dag_expansion(&long_program, 0).unwrap();
    assert_eq!(long_expansion.maximum_resource_leaves, 1);
    assert_eq!(long_expansion.maximum_shape_fields, 1);
    assert_eq!(long_expansion.maximum_projection_segments, 1);
    assert!(long_expansion.maximum_shape_identity_bytes >= 3 * 128);
    assert!(long_expansion.maximum_lifecycle_identity_bytes >= 128);
    assert!(long_expansion.maximum_projection_identity_bytes >= 128);

    let cyclic = crate::parse(
            "module capacity.cycle;\n\n@id(\"cycle.a\")\nrecord A {\n    @id(\"cycle.a.next\")\n    next: A,\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
            Path::new("cycle-capacity.spx"),
        )
        .unwrap();
    let error = declaration_dag_expansion(&cyclic, 0).unwrap_err();
    assert_eq!(error.code, "SPX-B107");

    let mut shallow = String::from("module capacity.shallow;\n\n");
    for index in 0..514 {
        writeln!(
                shallow,
                "@id(\"shallow.r{index}\")\nrecord R{index} {{\n    @id(\"shallow.r{index}.value\")\n    value: i64,\n}}\n"
            )
            .unwrap();
    }
    shallow.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let shallow = crate::parse(&shallow, Path::new("shallow-declarations.spx")).unwrap();
    let expansion = declaration_dag_expansion(&shallow, 0).unwrap();
    assert_eq!(expansion.maximum_resource_leaves, 0);
    assert_eq!(expansion.maximum_type_occurrences, 2);

    let mut chain = String::from(
            "module capacity.chain;\n\n@id(\"chain.r0\")\nrecord R0 {\n    @id(\"chain.r0.value\")\n    value: i64,\n}\n\n",
        );
    for index in 1..514 {
        writeln!(
                chain,
                "@id(\"chain.r{index}\")\nrecord R{index} {{\n    @id(\"chain.r{index}.next\")\n    next: R{},\n}}\n",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let chain = crate::parse(&chain, Path::new("long-chain.spx")).unwrap();
    let expansion = declaration_dag_expansion(&chain, 0).unwrap();
    assert_eq!(expansion.maximum_resource_leaves, 0);
    assert_eq!(expansion.maximum_type_occurrences, 515);
}

#[test]
fn typed_cleanup_retained_census_covers_long_ids_and_many_owned_roots() {
    let long = "x".repeat(128);
    let mut source = String::from("module capacity.cleanup_typed;\n\n");
    writeln!(
        source,
        "@id(\"resource.{long}\") resource R0 {{ @id(\"lifecycle.{long}\") drop trivial; }}"
    )
    .unwrap();
    for index in 1..=64 {
        writeln!(
                source,
                "@id(\"record.{index:03}.{long}\") record R{index} {{ @id(\"field.{index:03}.{long}\") next: R{}, }}",
                index - 1
            )
            .unwrap();
    }
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: own R64"))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        source,
        "@id(\"consume.typed\") fn consume({parameters}) -> i64 {{ 0 }}"
    )
    .unwrap();
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");

    let program = crate::parse(&source, Path::new("typed-cleanup-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual > MAX_PARAMETERS * 64 * 128);
    assert!(
        actual <= capacity.cleanup_retained_upper,
        "actual cleanup {actual} exceeds derived {}",
        capacity.cleanup_retained_upper
    );
}

#[test]
fn cleanup_retained_census_admits_depth_by_live_roots_with_long_identities() {
    let long = "x".repeat(128);
    let mut source = String::from("module capacity.cleanup_depth_live;\n\n");
    writeln!(
        source,
        "@id(\"resource.{long}\") resource R0 {{ @id(\"lifecycle.{long}\") drop trivial; }}"
    )
    .unwrap();
    source.push_str("@id(\"identity\") fn identity(value: own R0) -> R0 { value }\n");
    source.push_str("@id(\"consume\") fn consume(value: own R0) -> i64 { 1 }\n");
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: own R0"))
        .chain(std::iter::once("value: i64".to_owned()))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(source, "@id(\"stress\") fn stress({parameters}) -> i64 {{").unwrap();
    for index in 0..MAX_PARAMETERS {
        writeln!(source, "let live{index} = identity(p{index});").unwrap();
    }
    source.push_str("let checked = value");
    for _ in 0..510 {
        source.push_str(" + 1");
    }
    source.push_str(";\nchecked + ");
    for index in 0..MAX_PARAMETERS {
        if index != 0 {
            source.push_str(" + ");
        }
        write!(source, "consume(live{index})").unwrap();
    }
    source.push_str("\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");

    let program = crate::parse(&source, Path::new("cleanup-depth-live.spx")).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    let mut depth_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let depth = scan_ast_capacity(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
        &program,
        false,
        &mut depth_scan,
    )
    .unwrap()
    .max_depth;
    assert_eq!(depth, MAX_SEMANTIC_EXPRESSION_DEPTH);
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "actual cleanup {actual} exceeds authority {}",
        capacity.cleanup_authority_upper
    );
    assert!(
        capacity.complete().unwrap() <= MAX_BUILDER_BYTES,
        "depth×live capacity terms: {:?}; actual cleanup: {actual}",
        hir_capacity_terms_for_test(&program, canonical.len()).unwrap()
    );
}

#[test]
fn cleanup_retained_census_releases_sequential_early_move_epochs() {
    fn measure(delayed_moves: bool) -> (HirPreResolveCapacity, usize) {
        let long = "x".repeat(128);
        let mut source = String::from("module capacity.cleanup_sequential_moves;\n\n");
        writeln!(
            source,
            "@id(\"resource.{long}\") resource R0 {{ @id(\"lifecycle.{long}\") drop trivial; }}"
        )
        .unwrap();
        source.push_str("@id(\"identity\") fn identity(value: own R0) -> R0 { value }\n");
        source.push_str("@id(\"consume\") fn consume(value: own R0) -> i64 { 1 }\n");
        let parameters = (0..MAX_PARAMETERS)
            .map(|index| format!("p{index}: own R0"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(source, "@id(\"stress\") fn stress({parameters}) -> i64 {{").unwrap();
        for index in 0..MAX_PARAMETERS {
            writeln!(source, "let epoch{index} = identity(p{index});").unwrap();
            if !delayed_moves {
                writeln!(source, "let consumed{index} = consume(epoch{index});").unwrap();
            }
        }
        if delayed_moves {
            for index in 0..MAX_PARAMETERS {
                writeln!(source, "let consumed{index} = consume(epoch{index});").unwrap();
            }
        }
        source.push('0');
        for _ in 0..256 {
            source.push_str(" + 1");
        }
        source.push_str("\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");

        let program = crate::parse(&source, Path::new("cleanup-sequential-moves.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let actual = resolved
            .functions
            .iter()
            .try_fold(0usize, |bytes, function| {
                bytes
                    .checked_add(
                        crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                            &function.cleanup,
                        )?,
                    )?
                    .checked_add(
                        crate::private_capacity_contract::cleanup_plan_owned_capacity(
                            &function.cleanup_plan,
                        )?,
                    )
            })
            .unwrap();
        assert!(actual <= capacity.cleanup_authority_upper);
        (capacity, actual)
    }

    let (early, early_actual) = measure(false);
    let (delayed, delayed_actual) = measure(true);
    assert!(early.cleanup_authority_upper < delayed.cleanup_authority_upper);
    assert!(early_actual < delayed_actual);
    for (arrangement, capacity) in [("early", early), ("delayed", delayed)] {
        assert!(
            capacity.complete().unwrap() <= MAX_BUILDER_BYTES,
            "sequential {arrangement}-move capacity {} exceeds {MAX_BUILDER_BYTES}",
            capacity.complete().unwrap()
        );
    }
}

#[test]
fn cleanup_binding_flow_releases_nested_moves_and_preserves_partial_projection() {
    fn measure(source: &str) -> (usize, HirPreResolveCapacity, usize, usize) {
        let program = crate::parse(source, Path::new("cleanup-nested-move.spx")).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "stress")
            .unwrap();
        let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let events =
            cleanup_parameter_finalizer_events(function, "value", &program, &mut traversal)
                .unwrap();
        let nodes = scan_ast_capacity(
            std::iter::once(&function.body),
            &program,
            false,
            &mut traversal,
        )
        .unwrap()
        .nodes;
        let canonical = crate::format::canonical(&program);
        let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut traversal).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let actual = resolved
            .functions
            .iter()
            .try_fold(0usize, |bytes, function| {
                bytes
                    .checked_add(
                        crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                            &function.cleanup,
                        )?,
                    )?
                    .checked_add(
                        crate::private_capacity_contract::cleanup_plan_owned_capacity(
                            &function.cleanup_plan,
                        )?,
                    )
            })
            .unwrap();
        assert!(actual <= capacity.cleanup_authority_upper);
        assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
        (events, capacity, actual, nodes)
    }

    let definitions = r#"
@id("flow.r") resource R { @id("flow.r.drop") drop trivial; }
@id("flow.consume") fn consume(value: own R) -> i64 { 1 }
"#;
    let cases = [
            (
                "block",
                "{ let moved = { consume(value) }; let observed = checked + 1; moved + observed }",
                "{ let observed = checked + 1; let moved = { consume(value) }; moved + observed }",
                false,
                true,
                "",
                "value: own R, checked: i64",
            ),
            (
                "if",
                "{ let moved = if condition { consume(value) } else { consume(value) }; let observed = checked + 1; moved + observed }",
                "{ let observed = checked + 1; let moved = if condition { consume(value) } else { consume(value) }; moved + observed }",
                false,
                true,
                "",
                "value: own R, checked: i64, condition: bool",
            ),
            (
                "match",
                "{ let moved = match choice { Choice::A {} => consume(value), Choice::B {} => consume(value), }; let observed = checked + 1; moved + observed }",
                "{ let observed = checked + 1; let moved = match choice { Choice::A {} => consume(value), Choice::B {} => consume(value), }; moved + observed }",
                false,
                true,
                "@id(\"flow.choice\") variant Choice { @id(\"flow.choice.a\") A {}, @id(\"flow.choice.b\") B {}, }",
                "value: own R, checked: i64, choice: Choice",
            ),
            (
                "construct",
                "{ let moved = consume_box(Box { value: value }); let observed = checked + 1; moved + observed }",
                "{ let observed = checked + 1; let moved = consume_box(Box { value: value }); moved + observed }",
                false,
                false,
                "@id(\"flow.box\") record Box { @id(\"flow.box.value\") value: R, } @id(\"flow.consume_box\") fn consume_box(value: own Box) -> i64 { 1 }",
                "value: own R, checked: i64",
            ),
            (
                "update",
                "{ let moved = consume_box(value with { item: replacement }); let observed = checked + 1; moved + observed }",
                "{ let observed = checked + 1; let moved = consume_box(value with { item: replacement }); moved + observed }",
                false,
                false,
                "@id(\"flow.box\") record Box { @id(\"flow.box.item\") item: R, } @id(\"flow.consume_box\") fn consume_box(value: own Box) -> i64 { 1 }",
                "value: own Box, replacement: own R, checked: i64",
            ),
            (
                "projection",
                "{ let moved = consume(value.left); let observed = checked + 1; moved + observed }",
                "{ let observed = checked + 1; let moved = consume(value.left); moved + observed }",
                true,
                false,
                "@id(\"flow.pair\") record Pair { @id(\"flow.pair.left\") left: R, @id(\"flow.pair.right\") right: R, }",
                "value: own Pair, checked: i64",
            ),
        ];
    for (shape, early_body, delayed_body, conservative, authority_drop, extra, parameters) in cases
    {
        let source = |body: &str| {
            format!(
                    "module capacity.flow_{shape};\n{definitions}\n{extra}\n@id(\"flow.stress\") fn stress({parameters}) -> i64 {body}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
                )
        };
        let (early_events, early, _, early_nodes) = measure(&source(early_body));
        let (delayed_events, delayed, _, delayed_nodes) = measure(&source(delayed_body));
        assert_eq!(early_nodes, delayed_nodes, "{shape}");
        if conservative {
            assert_eq!(early_events, delayed_events, "{shape}");
        } else {
            assert!(early_events < delayed_events, "{shape}");
        }
        if authority_drop {
            assert!(
                early.cleanup_authority_upper < delayed.cleanup_authority_upper,
                "{shape}"
            );
        }
    }
}

#[test]
fn cleanup_retained_census_joins_mutually_exclusive_owned_branches() {
    let long = "x".repeat(128);
    let mut source = format!(
            "module capacity.cleanup_branch_live;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"identity\") fn identity(value: own R) -> R {{ value }}\n@id(\"consume\") fn consume(value: own R) -> i64 {{ 1 }}\n"
        );
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: own R"))
        .chain(["condition: bool".to_owned(), "value: i64".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        source,
        "@id(\"branch.stress\") fn stress({parameters}) -> i64 {{ if condition {{"
    )
    .unwrap();
    for index in 0..4 {
        writeln!(source, "let live{index} = identity(p{index});").unwrap();
    }
    source.push_str("let checked = value");
    for _ in 0..508 {
        source.push_str(" + 1");
    }
    source.push_str("; checked");
    for index in 0..4 {
        write!(source, " + consume(live{index})").unwrap();
    }
    source.push_str(" } else { ");
    for index in 4..8 {
        writeln!(source, "let live{index} = identity(p{index});").unwrap();
    }
    source.push_str("let checked = value");
    for _ in 0..508 {
        source.push_str(" + 1");
    }
    source.push_str("; checked");
    for index in 4..8 {
        write!(source, " + consume(live{index})").unwrap();
    }
    source.push_str(" } }\n@id(\"app.main\") fn main() -> i64 { 0 }\n");

    let program = crate::parse(&source, Path::new("cleanup-branch-live.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let stress = program
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert_eq!(
        scan_ast_capacity(std::iter::once(&stress.body), &program, false, &mut scan)
            .unwrap()
            .max_depth,
        MAX_SEMANTIC_EXPRESSION_DEPTH
    );
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert!(
        capacity.complete().unwrap() <= MAX_BUILDER_BYTES,
        "branch capacity terms {:?}, plan structural {}, complete {}",
        hir_capacity_terms_for_test(&program, canonical.len()).unwrap(),
        capacity.cleanup_plan_structural_upper,
        capacity.complete().unwrap()
    );
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "branch actual cleanup {actual} exceeds authority {} (retained {}, structural {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper
    );
}

#[test]
fn type_facts_hostile_envelopes_are_bound_to_canonical_fixtures() {
    fn layered(resource: bool, levels: usize) -> String {
        let mut source = String::from("module capacity.typefacts.layers;\n\n");
        if resource {
            source.push_str(
                    "@id(\"layer.r0\")\nresource R0 {\n    @id(\"layer.r0.drop\")\n    drop trivial;\n}\n\n",
                );
        } else {
            source.push_str(
                    "@id(\"layer.r0\")\nrecord R0 {\n    @id(\"layer.r0.value\")\n    value: i64,\n}\n\n",
                );
        }
        for level in 1..=levels {
            writeln!(
                    source,
                    "@id(\"layer.r{level}\")\nrecord R{level} {{\n    @id(\"layer.r{level}.a\")\n    a: R{},\n    @id(\"layer.r{level}.b\")\n    b: R{},\n}}\n",
                    level - 1,
                    level - 1
                )
                .unwrap();
        }
        source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
        source
    }

    fn envelope(source: &str, name: &str) -> (String, usize, usize, usize) {
        let program = crate::parse(source, Path::new(name)).unwrap();
        let canonical = crate::format::canonical(&program);
        let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack).unwrap();
        let type_facts_phase = capacity.phase_peaks()[7];
        (
            raw_digest(canonical.as_bytes()),
            capacity.retained_upper,
            type_facts_phase,
            capacity
                .retained_upper
                .checked_add(type_facts_phase)
                .unwrap(),
        )
    }

    let scalar = layered(false, 12);
    let resource = layered(true, 12);
    let mut wide = String::from("module capacity.typefacts.wide;\n\n");
    for index in 0..514 {
        writeln!(
                wide,
                "@id(\"wide.r{index}\")\nrecord R{index} {{\n    @id(\"wide.r{index}.value\")\n    value: i64,\n}}\n"
            )
            .unwrap();
    }
    wide.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
    let mut chain = String::from(
            "module capacity.typefacts.chain;\n\n@id(\"chain.r0\")\nrecord R0 {\n    @id(\"chain.r0.value\")\n    value: i64,\n}\n\n",
        );
    for index in 1..514 {
        writeln!(
                chain,
                "@id(\"chain.r{index}\")\nrecord R{index} {{\n    @id(\"chain.r{index}.next\")\n    next: R{},\n}}\n",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");

    assert_eq!(
        [
            envelope(&scalar, "typefacts-layered-scalar.spx"),
            envelope(&resource, "typefacts-layered-resource.spx"),
            envelope(&wide, "typefacts-wide.spx"),
            envelope(&chain, "typefacts-chain.spx"),
        ],
        [
            (
                "sha256:cfa16985be87d169c3fb81d5958126347ec82b4c1afed878e2d98d1fbfe72c80"
                    .to_owned(),
                220_110_854,
                438_720_350,
                658_831_204,
            ),
            (
                "sha256:461611e4315e312330af0285273568e5d09cd8e5770a35dcf66a82783aa15ae6"
                    .to_owned(),
                147_075_460,
                293_107_472,
                440_182_932,
            ),
            (
                "sha256:dc19474b86def3eaf6e3c60cc2224694e6aa7cf2811cca6115943c11102f95fc"
                    .to_owned(),
                42_048_403,
                80_965_504,
                123_013_907,
            ),
            (
                "sha256:d2692d4883957575ee95df8f9ee7057343599e1da945c386cedea714c716f66d"
                    .to_owned(),
                10_529_688_603,
                21_056_178_704,
                31_585_867_307,
            ),
        ],
        "canonical fixture or independently computed envelope terms drifted"
    );
}

#[test]
fn cleanup_retained_census_covers_shared_transition_and_staging_families() {
    let source = include_str!("../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .chain(
            resolved
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    let actual_exits = resolved
        .functions
        .iter()
        .chain(
            resolved
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .try_fold(0usize, |count, function| {
            count.checked_add(function.cleanup_plan.exits.len())
        })
        .unwrap();
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup_plan.blocks.iter().any(|block| {
            block.transitions.iter().any(|transition| {
                matches!(
                    transition,
                    semaprax::cleanup_plan::CleanupTransition::CallCommit { .. }
                )
            })
        })
    }));
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup_plan.blocks.iter().any(|block| {
            block.transitions.iter().any(|transition| {
                matches!(
                    transition,
                    semaprax::cleanup_plan::CleanupTransition::StageCopyResult { .. }
                )
            })
        })
    }));
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup_plan.edges.iter().any(|edge| {
            matches!(
                edge.condition,
                semaprax::cleanup_plan::EdgeCondition::VariantCase { .. }
            )
        })
    }));
    assert!(actual_exits <= capacity.cleanup_exit_events_upper);
    assert!(actual <= capacity.cleanup_retained_upper);
}

#[test]
fn cleanup_fieldwise_payload_and_vec_floors_are_covered() {
    let source = include_str!("../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let proof = capacity.cleanup_proof;
    let resolved = hir::resolve(&program).unwrap();
    let mut observed = ObservedCleanupProof::default();
    for function in resolved.functions.iter().chain(
        resolved
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        assert!(
            observe_cleanup_function(function, &mut observed).is_some(),
            "cleanup proof observer encountered an unadmitted non-exhaustive family"
        );
    }

    let stats = proof.stats;
    assert!(observed.slot_payload_bytes <= stats.ordinary_slot_payload_bytes);
    assert!(observed.call_argument_slot_payload_bytes <= stats.call_argument_owned_bytes);
    assert!(observed.shape_identity_bytes <= stats.shape_ids * 2);
    assert!(observed.flag_lifecycle_bytes <= stats.lifecycle_ids);
    assert!(observed.flag_projection_bytes <= stats.projection_ids);
    assert!(
        observed.place_storage_bytes
            <= stats.ordinary_place_storage_bytes + stats.call_argument_owned_bytes
    );
    assert!(observed.place_projection_bytes <= stats.place_projection_ids);
    assert!(observed.finalizer_storage_bytes <= stats.ordinary_finalizer_storage_bytes);
    assert!(observed.finalizer_projection_bytes <= stats.finalizer_projection_ids);
    assert!(observed.finalizer_lifecycle_bytes <= stats.finalizer_lifecycle_ids);

    for (observed, derived, family) in [
        (
            observed.inventory_slot_capacity_entries,
            proof.inventory_slot_capacity_entries,
            "inventory slots",
        ),
        (
            observed.inventory_flag_capacity_entries,
            proof.inventory_flag_capacity_entries,
            "inventory flags",
        ),
        (
            observed.inventory_entry_capacity_entries,
            proof.inventory_entry_capacity_entries,
            "inventory entry state",
        ),
        (
            observed.plan_slot_capacity_entries,
            proof.plan_slot_capacity_entries,
            "plan slots",
        ),
        (
            observed.plan_entry_capacity_entries,
            proof.plan_entry_capacity_entries,
            "plan entry state",
        ),
        (
            observed.shape_field_capacity_entries,
            proof.shape_field_capacity_entries,
            "shape fields",
        ),
        (
            observed.flag_projection_capacity_entries,
            proof.flag_projection_capacity_entries,
            "flag projections",
        ),
        (
            observed.place_projection_capacity_entries,
            proof.place_projection_capacity_entries,
            "plan-place projections",
        ),
        (
            observed.finalizer_projection_capacity_entries,
            proof.finalizer_projection_capacity_entries,
            "finalizer projections",
        ),
        (
            observed.finalizer_capacity_entries,
            proof.finalizer_capacity_entries,
            "finalizers",
        ),
        (
            observed.block_capacity_entries,
            proof.block_capacity_entries,
            "blocks",
        ),
        (
            observed.edge_capacity_entries,
            proof.edge_capacity_entries,
            "edges",
        ),
        (
            observed.region_capacity_entries,
            proof.region_capacity_entries,
            "regions",
        ),
        (
            observed.exit_capacity_entries,
            proof.exit_capacity_entries,
            "exits",
        ),
        (
            observed.status_capacity_entries,
            proof.status_capacity_entries,
            "status sources",
        ),
        (
            observed.transition_capacity_entries,
            proof.transition_capacity_entries,
            "transitions",
        ),
        (
            observed.branch_edge_capacity_entries,
            proof.branch_edge_capacity_entries,
            "branch edge vectors",
        ),
        (
            observed.region_slot_capacity_entries,
            proof.region_slot_capacity_entries,
            "region slots",
        ),
        (
            observed.exit_region_capacity_entries,
            proof.exit_region_capacity_entries,
            "exit region vectors",
        ),
        (
            observed.status_case_capacity_entries,
            proof.status_case_capacity_entries,
            "status case vectors",
        ),
    ] {
        assert!(
            observed <= derived,
            "observed {family} capacity {observed} exceeds derived {derived}"
        );
    }
}

#[test]
fn cleanup_generic_arity_two_checked_call_includes_exact_instance_identities() {
    let long = "x".repeat(128);
    let source = format!(
        r#"
module capacity.cleanup_generic_checked;
@id("checked.{long}")
fn checked<T, U>(left: T, right: U, value: i64) -> i64 {{ value + 1 }}
@id("app.main")
fn main() -> i64 {{
    checked<i64, bool>(1, true, 1)
}}
"#
    );
    let program = crate::parse(&source, Path::new("cleanup-generic-checked.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let template = program
        .functions
        .iter()
        .find(|function| function.name == "checked")
        .unwrap();
    let expected_instance_len = generic_function_instance_identity_upper(&program, template)
        .expect("valid concrete arity-two arguments have an identity upper");
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let instance = resolved
        .function_instances
        .iter()
        .find(|instance| instance.template.as_str() == template.stable_id)
        .expect("checked call materializes its generic instance");
    assert_eq!(instance.type_arguments.len(), 2);
    assert_eq!(expected_instance_len, instance.id.as_str().len());
    let checked_expression = &instance
        .function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|status| {
            matches!(
                status.producer,
                semaprax::cleanup_plan::StatusProducer::CheckedArithmetic { .. }
            )
        })
        .expect("generic checked body has an arithmetic status source")
        .id
        .expression;
    let mut checked_clones = 0usize;
    let mut checked_clone_bytes = 0usize;
    let mut note = |expression: &crate::hir::ExpressionId| {
        if expression == checked_expression {
            checked_clones += 1;
            checked_clone_bytes += expression.as_str().len();
        }
    };
    for status in &instance.function.cleanup_plan.status_sources {
        note(&status.id.expression);
    }
    for block in &instance.function.cleanup_plan.blocks {
        for transition in &block.transitions {
            match transition {
                semaprax::cleanup_plan::CleanupTransition::Initialize { at, .. }
                | semaprax::cleanup_plan::CleanupTransition::Transfer { at, .. } => note(at),
                semaprax::cleanup_plan::CleanupTransition::CallCommit { call, .. } => note(call),
                semaprax::cleanup_plan::CleanupTransition::SelectFailure { source } => {
                    note(&source.expression)
                }
                semaprax::cleanup_plan::CleanupTransition::StageCopyResult { source } => {
                    match source {
                        semaprax::cleanup_plan::StagedCopyResultSource::Body {
                            expression, ..
                        } => note(expression),
                        semaprax::cleanup_plan::StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            ..
                        }
                        | semaprax::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            ..
                        } => {
                            note(expression);
                            note(operand);
                        }
                    }
                }
            }
        }
    }
    for edge in &instance.function.cleanup_plan.edges {
        match &edge.condition {
            semaprax::cleanup_plan::EdgeCondition::BooleanResult(expression, _) => note(expression),
            semaprax::cleanup_plan::EdgeCondition::VariantCase { scrutinee, .. } => note(scrutinee),
            semaprax::cleanup_plan::EdgeCondition::StatusZero(source)
            | semaprax::cleanup_plan::EdgeCondition::StatusNonzero(source) => {
                note(&source.expression)
            }
            semaprax::cleanup_plan::EdgeCondition::Always => {}
        }
    }
    for exit in &instance.function.cleanup_plan.exits {
        match &exit.continuation {
            semaprax::cleanup_plan::ExitContinuation::CommitResult {
                source: semaprax::cleanup_plan::CleanupResultSource::Scalar { expression },
            } => note(expression),
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { source } => {
                note(&source.expression)
            }
            _ => {}
        }
    }
    // StatusSource, SelectFailure, two status edges, ReturnFailure.
    assert_eq!(checked_clones, 5);
    assert_eq!(
        checked_clone_bytes,
        checked_clones * checked_expression.as_str().len()
    );
    let actual = resolved
        .functions
        .iter()
        .chain(
            resolved
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "generic arity-two cleanup {actual} exceeds authority {}",
        capacity.cleanup_authority_upper
    );
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_source_exit_events_upper_bounds_lowerer_families() {
    let source = r#"
module capacity.cleanup_exit_events;
@id("exit.resource") resource R { @id("exit.resource.drop") drop trivial; }
@id("exit.box") record Box { @id("exit.box.value") value: R, }
@id("exit.choice") variant Choice {
    @id("exit.choice.first") First,
    @id("exit.choice.second") Second,
}
@id("exit.helper") fn helper(value: i64) -> i64 { value }
@id("exit.call") fn call_case(value: i64) -> i64 { helper(value) }
@id("exit.neg") fn neg_case(value: i64) -> i64 { -value }
@id("exit.add") fn add_case(value: i64) -> i64 { value + 1 }
@id("exit.lazy") fn lazy_case(condition: bool) -> bool { condition && true }
@id("exit.if") fn if_case(condition: bool) -> i64 { if condition { 1 } else { 2 } }
@id("exit.match") fn match_case(value: Choice) -> i64 {
    match value {
        Choice::First {} => 0,
        Choice::Second {} => 1,
    }
}
@id("exit.update") fn update_case(base: own Box, replacement: own R) -> Box {
    base with { value: replacement }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("cleanup-exit-events.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let expected_tail_events = [
        ("helper", 0usize),
        ("call_case", 1),
        ("neg_case", 1),
        ("add_case", 1),
        ("lazy_case", 0),
        ("if_case", 0),
        ("match_case", 0),
        ("update_case", 1),
        ("main", 0),
    ];
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for (name, expected_tail) in expected_tail_events {
        let function = program
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap();
        let crate::ast::ExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("function body must retain its authored block");
        };
        assert_eq!(cleanup_source_exit_events(tail), expected_tail, "{name}");
        let source_events = cleanup_function_exit_events(function, &mut traversal).unwrap();
        let actual_exits = resolved
            .functions
            .iter()
            .find(|candidate| candidate.name == name)
            .unwrap()
            .cleanup_plan
            .exits
            .len();
        assert_eq!(source_events, actual_exits, "{name}");
    }
}

#[test]
fn cleanup_retained_census_covers_update_region_with_live_long_id_roots() {
    let long = "x".repeat(128);
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("live{index}: own R"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
            "module capacity.cleanup_update;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"box.{long}\") record Box {{ @id(\"box.value.{long}\") value: R, }}\n@id(\"update.stress\") fn stress(base: own Box, replacement: own R, {parameters}) -> Box {{ base with {{ value: replacement }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
    let program = crate::parse(&source, Path::new("cleanup-update-live.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(
        stress
            .cleanup_plan
            .exits
            .iter()
            .map(|exit| exit.finalize_in_order.len())
            .max()
            .unwrap_or(0)
            >= MAX_PARAMETERS
    );
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    let actual_exits = resolved
        .functions
        .iter()
        .try_fold(0usize, |count, function| {
            count.checked_add(function.cleanup_plan.exits.len())
        })
        .unwrap();
    assert!(actual_exits <= capacity.cleanup_exit_events_upper);
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "update actual {actual} exceeds authority {} (retained {}, structural {}, call epoch {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper,
        capacity.cleanup_call_argument_owned_upper
    );
    assert_eq!(capacity.cleanup_fallback_roots, 0);
}

#[test]
fn cleanup_update_staged_base_survives_replacement_failure() {
    let long = "x".repeat(128);
    let source = format!(
            "module capacity.cleanup_update_failure;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"box.{long}\") record Box {{ @id(\"box.value.{long}\") value: R, }}\n@id(\"update.failure\") fn stress(base: own Box, replacement: own R, checked: i64) -> Box {{ base with {{ value: {{ let observed = checked + 1; replacement }} }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
    let program = crate::parse(&source, Path::new("cleanup-update-failure.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
        ) && exit.finalize_in_order.iter().any(|action| {
            matches!(
                &action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(expression)
                    if expression.as_str().contains(".base")
            ) && action.lifecycle_id.as_str() == format!("lifecycle.{long}")
                && action
                    .source
                    .projections
                    .iter()
                    .any(|projection| projection.as_str() == format!("box.value.{long}"))
        })
    }));
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "update staged-base cleanup {actual} exceeds authority {}",
        capacity.cleanup_authority_upper
    );
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_parent_local_update_prefix_survives_later_replacement_failure() {
    let long = "x".repeat(128);
    let left_field_id = format!("pair.left.{long}");
    let lifecycle_id = format!("resource.drop.{long}");
    let source = format!(
            "module capacity.cleanup_update_prefix;\n@id(\"resource.{long}\") resource R {{ @id(\"{lifecycle_id}\") drop trivial; }}\n@id(\"pair.{long}\") record Pair {{ @id(\"{left_field_id}\") left: R, @id(\"pair.right.{long}\") right: R, }}\n@id(\"update.prefix.stress.{long}\") fn stress(base: own Pair, new_left: own R, new_right: own R, checked: i64) -> Pair {{ base with {{ left: new_left, right: {{ let observed = checked + 1; new_right }}, }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
    let program = crate::parse(&source, Path::new("cleanup-update-prefix.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let stats = capacity.cleanup_proof.stats;
    assert_eq!(stats.parent_local_update_prefix_fields, 1);
    assert_eq!(stats.parent_local_update_prefix_exit_groups, 1);
    assert_eq!(stats.parent_local_update_prefix_finalizer_copies, 1);
    assert_eq!(
        stats.parent_local_update_prefix_finalizer_projection_segments,
        1
    );
    assert_eq!(
        stats.parent_local_update_prefix_finalizer_lifecycle_ids,
        lifecycle_id.len()
    );
    assert_eq!(
        stats.parent_local_update_prefix_finalizer_projection_ids,
        left_field_id.len()
    );

    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    let is_left = |action: &semaprax::cleanup_plan::FinalizeAction| {
        action.lifecycle_id.as_str() == lifecycle_id
            && action
                .source
                .projections
                .iter()
                .any(|projection| projection.as_str() == left_field_id)
    };
    let is_destination = |action: &semaprax::cleanup_plan::FinalizeAction| {
        is_left(action)
            && matches!(
                &action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(expression)
                    if !expression.as_str().ends_with(".base")
            )
    };
    let is_staged_base = |action: &semaprax::cleanup_plan::FinalizeAction| {
        is_left(action)
            && matches!(
                &action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(expression)
                    if expression.as_str().ends_with(".base")
            )
    };
    let failure = stress
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                exit.continuation,
                semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
            ) && exit.finalize_in_order.iter().any(&is_destination)
                && exit.finalize_in_order.iter().any(&is_staged_base)
        })
        .expect("later replacement failure retains new destination and staged old base");
    let destination_actions = failure
        .finalize_in_order
        .iter()
        .filter(|action| is_destination(action))
        .collect::<Vec<_>>();
    assert_eq!(destination_actions.len(), 1);
    let observed_named = destination_actions
        .iter()
        .try_fold(0usize, |bytes, action| {
            let storage_bytes = match &action.source.storage {
                semaprax::cleanup_plan::StorageId::Temporary(expression) => {
                    expression.as_str().len()
                }
                _ => 0,
            };
            bytes
                .checked_add(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?
                .checked_add(storage_bytes)?
                .checked_add(
                    action
                        .source
                        .projections
                        .capacity()
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )?
                .checked_add(
                    action
                        .source
                        .projections
                        .iter()
                        .try_fold(0usize, |bytes, projection| {
                            bytes.checked_add(projection.as_str().len())
                        })?,
                )?
                .checked_add(action.lifecycle_id.as_str().len())
        })
        .unwrap();
    assert!(
        observed_named <= capacity.cleanup_parent_local_update_prefix_lifetime_upper,
        "update-prefix actual {observed_named} exceeds named authority {}",
        capacity.cleanup_parent_local_update_prefix_lifetime_upper
    );
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual <= capacity.cleanup_authority_upper);
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_parent_local_record_prefix_survives_later_field_failure() {
    let long = "x".repeat(128);
    let first_field_id = format!("pair.first.{long}");
    let lifecycle_id = format!("resource.drop.{long}");
    let source = format!(
            "module capacity.cleanup_record_prefix;\n@id(\"resource.{long}\") resource R {{ @id(\"{lifecycle_id}\") drop trivial; }}\n@id(\"pair.{long}\") record Pair {{ @id(\"{first_field_id}\") first: R, @id(\"pair.second.{long}\") second: R, }}\n@id(\"record.prefix.stress.{long}\") fn stress(first: own R, second: own R, checked: i64) -> Pair {{ Pair {{ first: first, second: {{ let observed = checked + 1; second }}, }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
    let program = crate::parse(&source, Path::new("cleanup-record-prefix.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let stats = capacity.cleanup_proof.stats;
    assert_eq!(stats.parent_local_partial_fields, 1);
    assert_eq!(stats.parent_local_finalizer_copies, 1);
    assert_eq!(stats.parent_local_finalizer_projection_segments, 1);
    assert_eq!(
        stats.parent_local_finalizer_lifecycle_ids,
        lifecycle_id.len()
    );
    assert_eq!(
        stats.parent_local_finalizer_projection_ids,
        first_field_id.len()
    );
    assert!(capacity.cleanup_parent_local_lifetime_upper > 0);

    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
        ) && exit.finalize_in_order.iter().any(|action| {
            matches!(
                action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(_)
            ) && action.lifecycle_id.as_str() == lifecycle_id
                && action
                    .source
                    .projections
                    .iter()
                    .any(|projection| projection.as_str() == first_field_id)
        })
    }));
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual <= capacity.cleanup_authority_upper);
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_parent_local_projection_residual_survives_failure_and_success() {
    let long = "x".repeat(128);
    let right_field_id = format!("pair.right.{long}");
    let lifecycle_id = format!("resource.drop.{long}");
    let source = format!(
            "module capacity.cleanup_projection_residual;\n@id(\"resource.{long}\") resource R {{ @id(\"{lifecycle_id}\") drop trivial; }}\n@id(\"pair.{long}\") record Pair {{ @id(\"pair.left.{long}\") left: R, @id(\"{right_field_id}\") right: R, }}\n@id(\"projection.residual.stress.{long}\") fn stress(left: own R, right: own R, checked: i64) -> R {{ let selected = Pair {{ left: left, right: right, }}.left; let observed = checked + 1; selected }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
    let program = crate::parse(&source, Path::new("cleanup-projection-residual.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let stats = capacity.cleanup_proof.stats;
    assert_eq!(stats.parent_local_projection_epochs, 1);
    assert_eq!(stats.parent_local_projection_exit_groups, 2);
    assert_eq!(stats.parent_local_projection_finalizer_copies, 2);
    assert_eq!(
        stats.parent_local_projection_finalizer_projection_segments,
        2
    );
    assert_eq!(
        stats.parent_local_projection_finalizer_lifecycle_ids,
        lifecycle_id.len() * 2
    );
    assert_eq!(
        stats.parent_local_projection_finalizer_projection_ids,
        right_field_id.len() * 2
    );

    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    let is_residual = |action: &semaprax::cleanup_plan::FinalizeAction| {
        matches!(
            action.source.storage,
            semaprax::cleanup_plan::StorageId::Temporary(_)
        ) && action.lifecycle_id.as_str() == lifecycle_id
            && action
                .source
                .projections
                .iter()
                .any(|projection| projection.as_str() == right_field_id)
    };
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
        ) && exit.finalize_in_order.iter().any(&is_residual)
    }));
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::CommitResult { .. }
        ) && exit.finalize_in_order.iter().any(&is_residual)
    }));
    let residual_actions = stress
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .filter(|action| is_residual(action))
        .collect::<Vec<_>>();
    assert_eq!(residual_actions.len(), 2);
    let observed_named = residual_actions
        .iter()
        .try_fold(0usize, |bytes, action| {
            let storage_bytes = match &action.source.storage {
                semaprax::cleanup_plan::StorageId::Temporary(expression) => {
                    expression.as_str().len()
                }
                _ => 0,
            };
            bytes
                .checked_add(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?
                .checked_add(storage_bytes)?
                .checked_add(
                    action
                        .source
                        .projections
                        .capacity()
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )?
                .checked_add(
                    action
                        .source
                        .projections
                        .iter()
                        .try_fold(0usize, |bytes, projection| {
                            bytes.checked_add(projection.as_str().len())
                        })?,
                )?
                .checked_add(action.lifecycle_id.as_str().len())
        })
        .unwrap();
    assert!(
        observed_named <= capacity.cleanup_parent_local_projection_lifetime_upper,
        "projection-residual actual {observed_named} exceeds named authority {}",
        capacity.cleanup_parent_local_projection_lifetime_upper
    );
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual <= capacity.cleanup_authority_upper);
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_typed_roots_resolve_nested_and_later_arm_bindings() {
    let source = r#"
module capacity.cleanup_lexical_types;
@id("lexical.resource") resource R { @id("lexical.resource.drop") drop trivial; }
@id("lexical.choice") variant Choice {
    @id("lexical.choice.first") First { @id("lexical.choice.first.value") value: i64, },
    @id("lexical.choice.second") Second { @id("lexical.choice.second.value") value: i64, },
}
@id("lexical.identity") fn identity(value: own R) -> R { value }
@id("lexical.consume") fn consume(value: own R) -> i64 { 1 }
@id("lexical.stress") fn stress(value: own R, choice: Choice) -> i64 {
    let outer = identity(value);
    let nested = {
        let inner = identity(outer);
        consume(inner)
    };
    nested + match choice {
        Choice::First { value: first } => first,
        Choice::Second { value: second } => second,
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("cleanup-lexical-types.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert_eq!(capacity.cleanup_fallback_roots, 0);
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "lexical actual cleanup {actual} exceeds authority {} (retained {}, structural {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper
    );
}

#[test]
fn cleanup_typed_roots_treat_generic_and_prelude_copy_types_as_no_drop() {
    let source = r#"
module capacity.cleanup_copy_types;
@id("copy.resource") resource R { @id("copy.resource.drop") drop trivial; }
@id("copy.generic") fn generic<T>(value: T) -> T { value }
@id("copy.option") fn option(value: i64) -> Option<i64> {
    Option<i64>::Some { value: value }
}
@id("copy.result") fn make_result(value: i64) -> Result<i64, bool> {
    Result<i64, bool>::Ok { value: value }
}
@id("copy.outer") fn outer(value: own R) -> R { { value } }
@id("app.main") fn main() -> i64 {
    let first = generic<i64>(1);
    let second = option(first);
    let third = make_result(first);
    first
}
"#;
    let program = crate::parse(source, Path::new("cleanup-copy-types.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert_eq!(capacity.cleanup_fallback_roots, 0);
    hir::resolve(&program).unwrap();

    // Same-name shadowing is rejected by the language, but the
    // pre-resolution census must still resolve the outer initializer and
    // remain conservative without falling back to an unrelated resource.
    let shadow = crate::parse(
        r#"
module capacity.cleanup_shadow;
@id("shadow.resource") resource R { @id("shadow.resource.drop") drop trivial; }
@id("shadow.invalid") fn invalid(value: own R) -> R {
    let outer = value;
    { let outer = outer; outer }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("cleanup-shadow.spx"),
    )
    .unwrap();
    let shadow_canonical = crate::format::canonical(&shadow);
    let shadow_capacity =
        hir_pre_resolve_capacity(&shadow, shadow_canonical.len(), &mut scan).unwrap();
    assert_eq!(shadow_capacity.cleanup_fallback_roots, 0);
    let diagnostics = hir::resolve(&shadow).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T209"));
}

#[test]
fn cleanup_pattern_binding_lookup_is_iterative_at_exact_depth() {
    use crate::ast::{
        Expr, ExprKind, FieldDeclaration, MatchArm, MatchPattern, Param, ParamMode,
        RecordMatchFieldPattern, RecordMatchPatternField, Type, TypeDeclaration,
        TypeDeclarationKind,
    };

    fn program_with_pattern_depth(depth: usize) -> Program {
        let span = crate::ast::Span::default();
        let mut program = crate::parse(
                "module cleanup.pattern.depth; @id(\"app.inspect\") fn inspect(scrutinee: R0) -> i64 { 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
                Path::new("cleanup-pattern-depth.spx"),
            )
            .unwrap();
        let mut pattern = RecordMatchFieldPattern::Binding {
            name: "value".into(),
            span,
        };
        for index in (1..depth).rev() {
            pattern = RecordMatchFieldPattern::Record {
                type_name: format!("R{index}"),
                type_span: span,
                fields: vec![RecordMatchPatternField {
                    name: "next".into(),
                    name_span: span,
                    pattern,
                    span,
                }],
                span,
            };
        }
        program.types = (0..depth)
            .map(|index| TypeDeclaration {
                stable_id: format!("cleanup.pattern.r{index}"),
                explicit_id: true,
                name: format!("R{index}"),
                name_span: span,
                type_parameters: Vec::new(),
                kind: TypeDeclarationKind::Record {
                    fields: vec![FieldDeclaration {
                        stable_id: format!("cleanup.pattern.r{index}.next"),
                        explicit_id: true,
                        name: "next".into(),
                        name_span: span,
                        ty: if index + 1 == depth {
                            Type::I64
                        } else {
                            Type::Named {
                                name: format!("R{}", index + 1),
                                arguments: Vec::new(),
                            }
                        },
                        span,
                    }],
                },
                extends: None,
                span,
            })
            .collect();
        program.functions[0].params = vec![Param {
            name: "scrutinee".into(),
            mode: ParamMode::Value,
            ty: Type::Named {
                name: "R0".into(),
                arguments: Vec::new(),
            },
            span,
        }];
        program.functions[0].body = Expr {
            kind: ExprKind::Match {
                scrutinee: Box::new(Expr {
                    kind: ExprKind::Var("scrutinee".into()),
                    span,
                }),
                arms: vec![MatchArm {
                    pattern: MatchPattern::Record {
                        type_name: "R0".into(),
                        type_span: span,
                        fields: vec![RecordMatchPatternField {
                            name: "next".into(),
                            name_span: span,
                            pattern,
                            span,
                        }],
                        span,
                    },
                    value: Expr {
                        kind: ExprKind::Var("value".into()),
                        span,
                    },
                    span,
                }],
            },
            span,
        };
        program
    }

    const CHILD_ENV: &str = "SEMAPRAX_TEST_CLEANUP_PATTERN_DEPTH";
    if let Some(depth) = std::env::var_os(CHILD_ENV) {
        let depth = depth.to_string_lossy().parse::<usize>().unwrap();
        let program = program_with_pattern_depth(depth);
        let canonical = crate::format::canonical(&program);
        HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
        POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
        RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
        if depth == MAX_SEMANTIC_EXPRESSION_DEPTH {
            let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
            let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack)
                .expect("depth-512 pattern capacity");
            assert_eq!(capacity.cleanup_fallback_roots, 0);
            note_hir_resolve_pass();
            let resolved = hir::resolve(&program).unwrap();
            let frames = Vec::with_capacity(capacity.disposal_frames);
            assert_eq!(frames.capacity(), capacity.disposal_frames);
            drop(ResolvedProgramOwner::new(
                resolved,
                frames,
                capacity.disposal_frames,
            ));
            assert_eq!(HIR_RESOLVE_PASS_COUNT.with(std::cell::Cell::get), 1);
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
        } else {
            let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
            let diagnostic = match hir_pre_resolve_capacity(&program, canonical.len(), &mut stack) {
                Err(diagnostic) => diagnostic,
                Ok(_) => panic!("depth-513 nested record pattern was admitted"),
            };
            assert_eq!(diagnostic.code, "SPX-B109");
            assert_eq!(HIR_RESOLVE_PASS_COUNT.with(std::cell::Cell::get), 0);
            assert_eq!(POST_HIR_FACTS_ENTRY_COUNT.with(std::cell::Cell::get), 0);
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 0);
        }
        std::mem::forget(program);
        std::process::exit(0);
    }

    for depth in [
        MAX_SEMANTIC_EXPRESSION_DEPTH,
        MAX_SEMANTIC_EXPRESSION_DEPTH + 1,
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .arg(
                "implementation::tests::cleanup_pattern_binding_lookup_is_iterative_at_exact_depth",
            )
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, depth.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "pattern depth {depth}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cleanup_call_argument_epoch_covers_later_argument_failure() {
    let long = "x".repeat(128);
    let source = format!(
            "module capacity.cleanup_call_epoch;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"identity\") fn identity(value: own R) -> R {{ value }}\n@id(\"consume\") fn consume(value: own R) -> i64 {{ 1 }}\n@id(\"combine\") fn combine(first: own R, second: own R) -> i64 {{ let left = consume(first); let right = consume(second); left + right }}\n@id(\"stress\") fn stress(first: own R, second: own R, checked: i64) -> i64 {{ combine(identity(first), {{ let observed = checked + 1; let staged = identity(second); staged }}) }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
    let program = crate::parse(&source, Path::new("cleanup-call-epoch.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert_eq!(capacity.cleanup_fallback_roots, 0);
    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        exit.finalize_in_order.iter().any(|action| {
            matches!(
                action.source.storage,
                semaprax::cleanup_plan::StorageId::CallArgument { .. }
            )
        })
    }));
    let actual_inventory = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes.checked_add(
                crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                    &function.cleanup,
                )?,
            )
        })
        .unwrap();
    let actual_plan = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes.checked_add(
                crate::private_capacity_contract::cleanup_plan_owned_capacity(
                    &function.cleanup_plan,
                )?,
            )
        })
        .unwrap();
    let actual = actual_inventory.checked_add(actual_plan).unwrap();
    assert!(
            actual <= capacity.cleanup_authority_upper,
            "call-epoch inventory {actual_inventory} + plan {actual_plan} = {actual} exceeds authority {} (retained {}, structural {}, call epoch {})",
            capacity.cleanup_authority_upper,
            capacity.cleanup_retained_upper,
            capacity.cleanup_authority_upper - capacity.cleanup_retained_upper,
            capacity.cleanup_call_argument_owned_upper
        );
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn inventory_and_cleanup_hostile_envelopes_bind_the_shared_fixture() {
    let source = include_str!("../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    assert_eq!(
        raw_digest(canonical.as_bytes()),
        "sha256:2a012464bb1bdb624a79972d558fe837f6d55a9cd9f40d2ead16bfbba615f316"
    );
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack).unwrap();
    let peaks = capacity.phase_peaks();
    assert_eq!(
        [
            capacity.retained_upper,
            peaks[3],
            capacity.retained_upper.checked_add(peaks[3]).unwrap(),
            peaks[4],
            capacity.retained_upper.checked_add(peaks[4]).unwrap(),
        ],
        [2_803_527, 38_736, 2_842_263, 299_312, 3_102_839],
        "retained/inventory/cleanup envelope terms drifted"
    );
    let complete = capacity.complete().unwrap();
    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(complete - 1, || {
            let _budget = reserve_temporary_exact(complete)?;
            note_hir_resolve_pass();
            Ok::<_, Diagnostic>(())
        });
    assert_eq!(result.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
}

#[test]
fn hir_capacity_layout_constants_are_bound_to_root_const_assertions() {
    let hir_resolver = include_str!("../../../../src/hir.rs");
    let hir_validator = include_str!("../../../../src/hir/validation.rs");
    let verifier = include_str!("../../../../src/source_verify.rs");
    let cleanup = include_str!("../../../../src/cleanup.rs");
    let lower = include_str!("../../../../src/cleanup_plan/build.rs");
    let calls = include_str!("../../../../src/call_index.rs");
    for (source, expected) in [
        (hir_resolver, "size_of::<Frame<'static>>() == 552"),
        (hir_validator, "size_of::<Frame<'static>>() == 288"),
        (verifier, "size_of::<VerifierFrame<'static>>() == 320"),
        (verifier, "size_of::<VariantMatchState<'static>>() == 312"),
        (cleanup, "size_of::<Frame<'static>>() == 40"),
        (cleanup, "size_of::<Frame<'static>>() == 24"),
        (lower, "size_of::<Frame<'static>>() == 344"),
        (calls, "size_of::<Frame<'static>>() == 16"),
    ] {
        assert!(
            source.contains(expected),
            "missing root layout pin `{expected}`"
        );
    }
}

#[test]
fn hir_complete_reservation_is_exact_and_one_less_prevents_resolution() {
    let (program, _) = fixture();
    let canonical = crate::format::canonical(&program);
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack).unwrap();
    assert_eq!(capacity.retained_upper, 49_075);
    assert_eq!(capacity.scratch_upper, 16_170);
    assert_eq!(
        capacity.phase_peaks(),
        [5_028, 15_492, 4_900, 3_488, 5_792, 3_456, 16_170, 1_032]
    );
    assert_eq!(capacity.complete().unwrap(), 65_245);
    assert_eq!(
        capacity.scratch_upper,
        capacity.phase_peaks().into_iter().max().unwrap(),
        "scratch must equal the largest sequential phase"
    );
    let complete = capacity.complete().unwrap();
    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| counts.set([0; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| water.set([0; 3]));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(complete - 1, || {
            let budget = reserve_temporary_exact(complete)?;
            note_hir_resolve_pass();
            let _ = hir::resolve(&program).map_err(|_| b107("selected identity missing"))?;
            drop(budget);
            Ok::<_, Diagnostic>(())
        });
    assert_eq!(result.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| assert_eq!(counts.get(), [0; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| assert_eq!(water.get(), [0; 3]));

    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| counts.set([0; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| water.set([0; 3]));
    let (result, overflowed, _) = crate::bounded_output::with_limit_usage(complete, || {
        let budget = reserve_temporary_exact(complete)?;
        note_hir_resolve_pass();
        let resolved = hir::resolve(&program).map_err(|_| b107("selected identity missing"))?;
        reset_closure_capacity_high_water();
        let (closure, _) = selected_closure(&resolved, &["interop.add".to_owned()])?;
        validate_native_rust_expression_budget_for_closure(&closure, true)?;
        validate_selected_scalar_closure(&closure)?;
        validate_native_unit_discard_bindings(&closure)?;
        assert!(closure_capacity_high_water() <= capacity.phase_peaks()[6]);
        drop(budget);
        Ok::<_, Diagnostic>(())
    });
    result.unwrap();
    assert!(!overflowed);
    HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| assert_eq!(counts.get(), [1; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| {
        assert!(water.get().into_iter().all(|bytes| bytes > 0));
    });
}

#[test]
fn post_hir_nontransfer_reservation_precedes_all_fact_and_render_work() {
    let (program, canonical_spec) = fixture();
    let canonical_source = crate::format::canonical(&program);
    let spec =
        parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let complete = capacity.complete().unwrap();
    let transfer = prepared_spec_transfer_capacity(&spec).unwrap();
    let reservation = complete.checked_sub(transfer).unwrap();

    POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(reservation - 1, || {
            let _budget = reserve_temporary_exact(reservation)?;
            note_post_hir_facts_entry();
            Ok::<_, Diagnostic>(())
        });
    assert_eq!(result.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    POST_HIR_FACTS_ENTRY_COUNT.with(|count| assert_eq!(count.get(), 0));
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| assert_eq!(water.get(), 0));

    POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(reservation, || {
            let budget = reserve_temporary_exact(reservation)?;
            note_post_hir_facts_entry();
            drop(budget);
            Ok::<_, Diagnostic>(())
        });
    result.unwrap();
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    POST_HIR_FACTS_ENTRY_COUNT.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn post_hir_spec_transfer_is_single_charged_across_target_triple_lengths() {
    fn terms(triple: &str) -> [usize; 5] {
        with_test_target(
            Target {
                triple: triple.to_owned(),
                pointer_width: 64,
                endian: "little".to_owned(),
                panic_strategy: "unwind".to_owned(),
                thread_policy: "same_thread".to_owned(),
            },
            || {
                let (program, canonical_spec) = fixture();
                POST_HIR_AUTHORITY_TRANSFER_TERMS.with(|terms| terms.set([0; 5]));
                prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
                POST_HIR_AUTHORITY_TRANSFER_TERMS.with(std::cell::Cell::get)
            },
        )
    }

    // [complete formula, moved Spec ownership, net facts reservation,
    //  new persistent facts, total persistent Prepared ownership]
    let apple = terms("aarch64-apple-darwin");
    let linux = terms("x86_64-unknown-linux-gnu");
    for observed in [apple, linux] {
        assert!(observed.into_iter().all(|value| value > 0));
        assert_eq!(observed[0] - observed[1], observed[2]);
        assert_eq!(observed[4] - observed[1], observed[3]);
    }
    assert_eq!(linux[0] - apple[0], 4);
    assert_eq!(linux[1] - apple[1], 4);
    assert_eq!(linux[2], apple[2]);
    assert_eq!(linux[3], apple[3]);
    assert_eq!(linux[4] - apple[4], 4);
}

#[test]
fn windows_target_phase_a_preparation_stays_inside_the_builder_ledger() {
    with_test_target(
        Target {
            triple: "x86_64-pc-windows-msvc".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, canonical_spec) = fixture();
            let (result, overflowed, consumed) =
                crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
                    prepare_native_rust_interop_bounded(&program, canonical_spec.as_bytes())
                });
            assert!(
                !overflowed,
                "Windows-target phase A overflowed; consumed={consumed}",
            );
            result.unwrap();
        },
    );
}

#[test]
fn post_hir_spec_transfer_capacity_slack_does_not_consume_scratch_authority() {
    let (program, canonical_spec) = fixture();
    let canonical_source = crate::format::canonical(&program);
    let mut spec =
        parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();

    let base = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let base_transfer = prepared_spec_transfer_capacity(&spec).unwrap();
    let digest = spec.source_revision.clone();
    let requested_capacity = digest.len() + 37;
    let mut over_capacity_digest = String::with_capacity(requested_capacity);
    over_capacity_digest.push_str(&digest);
    assert!(over_capacity_digest.capacity() > over_capacity_digest.len());
    spec.source_revision = over_capacity_digest;

    let hostile = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let hostile_transfer = prepared_spec_transfer_capacity(&spec).unwrap();
    let transfer_delta = hostile_transfer.checked_sub(base_transfer).unwrap();
    assert!(transfer_delta > 0);
    assert_eq!(
        hostile.complete().unwrap() - base.complete().unwrap(),
        transfer_delta,
    );
    assert_eq!(
        hostile.complete().unwrap() - hostile_transfer,
        base.complete().unwrap() - base_transfer,
    );
}

#[test]
fn final_artifact_sinks_reject_one_less_before_output_allocation() {
    let (program, canonical_spec) = fixture();
    let canonical_source = crate::format::canonical(&program);
    let spec =
        parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source).unwrap();
    let prepared = prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let status_domains = prepared
        .imports
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| count.set(0));
    let descriptor = render_descriptor_with_limit(
        &spec,
        &prepared.hir_digest,
        &status_domains,
        &prepared.exports,
        &prepared.imports,
        prepared.descriptor.len() - 1,
    )
    .unwrap_err();
    let header = generate_header_with_limit(
        &prepared.exports,
        &prepared.imports,
        prepared.generated_header.len() - 1,
    )
    .unwrap_err();
    let generated_c = render_exact_artifact(
        "max_generated_c_bytes",
        prepared.generated_c.len() - 1,
        |sink| generate_c_into(sink, &spec, &closure, &prepared.exports, &prepared.imports),
    )
    .unwrap_err();
    let rust_combined = prepared
        .generated_rust
        .len()
        .checked_add(prepared.private_ffi_source.len())
        .unwrap();
    let rust_aggregate_one_less = generate_rust_artifacts_with_limit(
        &spec,
        &prepared.exports,
        &prepared.imports,
        rust_combined - 1,
    )
    .unwrap_err();
    let rust_first_sink_one_less = generate_rust_artifacts_with_limit(
        &spec,
        &prepared.exports,
        &prepared.imports,
        prepared.generated_rust.len() - 1,
    )
    .unwrap_err();
    for diagnostic in [
        descriptor,
        header,
        generated_c,
        rust_aggregate_one_less,
        rust_first_sink_one_less,
    ] {
        assert_eq!(diagnostic.code, "SPX-B109");
    }
    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| assert_eq!(count.get(), 0));

    assert_eq!(
        render_descriptor_with_limit(
            &spec,
            &prepared.hir_digest,
            &status_domains,
            &prepared.exports,
            &prepared.imports,
            prepared.descriptor.len(),
        )
        .unwrap(),
        prepared.descriptor
    );
    assert_eq!(
        generate_header_with_limit(
            &prepared.exports,
            &prepared.imports,
            prepared.generated_header.len(),
        )
        .unwrap(),
        prepared.generated_header
    );
    assert_eq!(
        render_exact_artifact(
            "max_generated_c_bytes",
            prepared.generated_c.len(),
            |sink| generate_c_into(sink, &spec, &closure, &prepared.exports, &prepared.imports,),
        )
        .unwrap(),
        prepared.generated_c
    );
    let exact_rust = generate_rust_artifacts_with_limit(
        &spec,
        &prepared.exports,
        &prepared.imports,
        rust_combined,
    )
    .unwrap();
    assert_eq!(exact_rust.0, prepared.generated_rust);
    assert_eq!(exact_rust.1, prepared.private_ffi_source);
    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| assert_eq!(count.get(), 5));
}

#[test]
fn post_hir_named_phase_envelopes_cover_representative_and_depth_512_c() {
    fn measure(program: &Program, spec: &Spec) -> ([usize; 4], [usize; 3]) {
        let canonical_source = crate::format::canonical(program);
        let canonical_spec = render_spec(spec);
        let resolved = hir::resolve(program).unwrap();
        let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
        let capacity = post_hir_facts_capacity(
            canonical_source.len(),
            canonical_spec.len(),
            &resolved,
            &closure,
            spec,
        )
        .unwrap();
        POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        prepare_native_rust_interop(program, canonical_spec.as_bytes()).unwrap();
        let actual = [
            POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
            POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
            POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
        ];
        assert!(
            actual[0]
                <= capacity
                    .retained_upper
                    .checked_add(capacity.facts_scratch_upper)
                    .unwrap()
        );
        assert!(actual[1] <= capacity.render_scratch_upper);
        assert!(actual[2] <= capacity.replay_scratch_upper);
        (
            [
                capacity.retained_upper,
                capacity.facts_scratch_upper,
                capacity.render_scratch_upper,
                capacity.replay_scratch_upper,
            ],
            actual,
        )
    }

    // This historical evidence tuple was authorized for the Apple-arm
    // target. Freeze that target explicitly so host triple length cannot
    // silently repin a target-specific retained-allocation census.
    with_test_target(
        Target {
            triple: "aarch64-apple-darwin".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, canonical_spec) = fixture();
            let canonical_source = crate::format::canonical(&program);
            let spec =
                parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source)
                    .unwrap();
            let representative = measure(&program, &spec);

            let mut deep = program;
            let function = deep
                .functions
                .iter_mut()
                .find(|function| function.stable_id == "interop.add")
                .unwrap();
            for _ in 0..MAX_SEMANTIC_EXPRESSION_DEPTH - 4 {
                let expression = function.body.clone();
                function.body = crate::ast::Expr {
                    span: expression.span,
                    kind: crate::ast::ExprKind::Unary {
                        op: crate::ast::UnaryOp::Neg,
                        value: Box::new(expression),
                    },
                };
            }
            validate_native_rust_source_expression_budget(&deep).unwrap();
            let deep_source = crate::format::canonical(&deep);
            let deep_spec = Spec {
                module: deep.module.clone(),
                source_revision: domain_digest(SOURCE_DOMAIN, deep_source.as_bytes()),
                target: current_target().unwrap(),
                exports: vec!["interop.add".to_owned()],
                imports: vec!["host.add".to_owned()],
                capabilities: vec!["host.math".to_owned()],
            };
            let deep = measure(&deep, &deep_spec);

            assert_eq!(
                [representative, deep],
                [
                    (
                        [1_630, 115_266, 8_390_881, 8_390_881],
                        [116_499, 4_195_020, 4_195_020]
                    ),
                    (
                        [1_630, 115_266, 8_447_777, 8_447_777],
                        [116_499, 4_251_916, 4_251_916]
                    ),
                ],
                "named phase formula or observed high-water pins drifted"
            );
        },
    );
}

#[test]
fn post_hir_facts_cross_product_maxima_stay_inside_named_scratch() {
    let capabilities = (0..MAX_IMPORTS)
        .map(|index| format!("cap.c{index:02}"))
        .collect::<Vec<_>>();
    let capability_list = capabilities.join(", ");
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = format!(
            "module post.cross_product; permit {{ {capability_list} }} @id(\"host.cross\") interface HostCross permits {{ {capability_list} }} {{ "
        );
    for index in 0..MAX_IMPORTS {
        write!(
                source,
                "@id(\"import.{index:02}\") import rust fn import_{index:02}({parameters}) -> i64 effects {{ cap.c{index:02} }} failure status \"status.{index:02}\"; "
            )
            .unwrap();
    }
    source.push_str("} ");
    let call_sum = (0..MAX_IMPORTS)
        .map(|index| format!("import_{index:02}({arguments})"))
        .collect::<Vec<_>>()
        .join(" + ");
    write!(
            source,
            "@id(\"bridge.all\") fn bridge_all({parameters}) -> i64 uses {{ {capability_list} }} {{ {call_sum} }} "
        )
        .unwrap();
    for index in 0..MAX_EXPORTS {
        write!(
                source,
                "@id(\"export.{index:02}\") fn export_{index:02}({parameters}) -> i64 uses {{ {capability_list} }} {{ bridge_all({arguments}) }} "
            )
            .unwrap();
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }");
    let program = crate::parse(&source, Path::new("post-hir-cross-product.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes()),
        target: current_target().unwrap(),
        exports: (0..MAX_EXPORTS)
            .map(|index| format!("export.{index:02}"))
            .collect(),
        imports: (0..MAX_IMPORTS)
            .map(|index| format!("import.{index:02}"))
            .collect(),
        capabilities,
    };
    let canonical_spec = render_spec(&spec);
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let mut hir_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let hir_capacity =
        hir_pre_resolve_capacity(&program, canonical_source.len(), &mut hir_scan).unwrap();
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "cross-product prepare failed: {diagnostics:?}; source={}, spec={}, hir={}, retained={}, facts={}, render={}, replay={}, complete={}",
                    canonical_source.len(),
                    canonical_spec.len(),
                    hir_capacity.complete().unwrap(),
                    capacity.retained_upper,
                    capacity.facts_scratch_upper,
                    capacity.render_scratch_upper,
                    capacity.replay_scratch_upper,
                    capacity.complete().unwrap()
                )
            },
        );
    let actual = [
        POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
        POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
        POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
    ];
    let facts_scratch_actual = POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(std::cell::Cell::get);
    assert!(
        actual[0]
            <= capacity
                .retained_upper
                .checked_add(capacity.facts_scratch_upper)
                .unwrap(),
        "facts total-live / retained+scratch: {}/{}, all={actual:?}",
        actual[0],
        capacity.retained_upper + capacity.facts_scratch_upper
    );
    assert!(facts_scratch_actual > 0);
    assert!(facts_scratch_actual <= capacity.facts_scratch_upper);
    assert!(
        actual[1] <= capacity.render_scratch_upper,
        "render actual/formula: {}/{}, all={actual:?}",
        actual[1],
        capacity.render_scratch_upper
    );
    assert!(
        actual[2] <= capacity.replay_scratch_upper,
        "replay actual/formula: {}/{}, all={actual:?}",
        actual[2],
        capacity.replay_scratch_upper
    );
}

#[test]
fn post_hir_facts_zero_entry_collections_have_zero_backing_and_stay_bounded() {
    let empty_strings = Vec::<String>::new();
    let empty_pairs = Vec::<(String, String)>::new();
    let empty_ordinals = Vec::<u16>::new();
    let empty_set = BTreeSet::<String>::new();
    assert_eq!(
        checked_owned_string_vec(&empty_strings, empty_strings.capacity()),
        Some(0)
    );
    assert_eq!(checked_owned_string_pairs(&empty_pairs), Some(0));
    assert_eq!(checked_u16_vec(&empty_ordinals), Some(0));
    assert_eq!(checked_owned_string_set(&empty_set), Some(0));

    let source = "module post.zero; @id(\"zero.export\") fn export(value: i64) -> i64 { value } @id(\"app.main\") fn main() -> i64 { 0 }";
    let program = crate::parse(source, Path::new("post-hir-zero.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes()),
        target: current_target().unwrap(),
        exports: vec!["zero.export".to_owned()],
        imports: Vec::new(),
        capabilities: Vec::new(),
    };
    let canonical_spec = render_spec(&spec);
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
    let total = POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get);
    let scratch = POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(std::cell::Cell::get);
    assert!(total <= capacity.retained_upper + capacity.facts_scratch_upper);
    assert!(scratch <= capacity.facts_scratch_upper);

    // Unselected source text does not multiply any post-HIR owned
    // collection. A near-limit source with this same one-function closure
    // therefore has the same fieldwise facts authority and stays admitted.
    let near_max_source = post_hir_facts_capacity(
        MAX_SOURCE_BYTES,
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    assert_eq!(near_max_source.retained_upper, capacity.retained_upper);
    assert_eq!(
        near_max_source.facts_scratch_upper,
        capacity.facts_scratch_upper
    );
    assert!(near_max_source.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn post_hir_dense_fan_in_duplicates_and_all_interface_imports_stay_bounded() {
    let mut source = String::from(
            "module post.fanin; permit { cap.fan } @id(\"host.fan\") interface HostFan permits { cap.fan } { @id(\"import.fan\") import rust fn host_fan() -> i64 effects { cap.fan } failure status \"status.fan\"; } @id(\"host.unused\") interface HostUnused permits { cap.fan } { ",
        );
    for index in 0..24 {
        write!(source, "@id(\"unused.{index:02}\") import rust fn unused_{index:02}() -> i64 effects {{ cap.fan }} failure status \"status.unused.{index:02}\"; ").unwrap();
    }
    source
        .push_str("} @id(\"fanin.leaf\") fn fanin_leaf() -> i64 uses { cap.fan } { host_fan() } ");
    for index in 0..16 {
        write!(source, "@id(\"fanin.mid.{index:02}\") fn fanin_mid_{index:02}() -> i64 uses {{ cap.fan }} {{ fanin_leaf() + fanin_leaf() + fanin_leaf() }} ").unwrap();
    }
    let fan_in = (0..16)
        .map(|index| format!("fanin_mid_{index:02}()"))
        .collect::<Vec<_>>()
        .join(" + ");
    write!(source, "@id(\"fanin.export\") fn fanin_export() -> i64 uses {{ cap.fan }} {{ {fan_in} }} @id(\"app.main\") fn main() -> i64 {{ 0 }}").unwrap();
    let program = crate::parse(&source, Path::new("post-hir-fanin.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes()),
        target: current_target().unwrap(),
        exports: vec!["fanin.export".to_owned()],
        imports: vec!["import.fan".to_owned()],
        capabilities: vec!["cap.fan".to_owned()],
    };
    let canonical_spec = render_spec(&spec);
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let census = traversal_call_site_census(&closure).unwrap();
    assert!(census.function_sites > closure.len());
    assert_eq!(
        capacity.traversal_pending_capacity,
        census.function_sites + 1
    );
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
    let total = POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get);
    let scratch = POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(std::cell::Cell::get);
    assert!(total <= capacity.retained_upper + capacity.facts_scratch_upper);
    assert!(scratch <= capacity.facts_scratch_upper);
}

#[test]
fn serde_json_lock_and_near_max_escaped_payload_match_parser_contract() {
    assert!(include_str!("../../Cargo.toml").contains("serde_json = \"=1.0.151\""));
    let serde_package = include_str!("../../../../Cargo.lock")
        .split("[[package]]")
        .find(|package| package.lines().any(|line| line == "name = \"serde_json\""))
        .expect("serde_json package is locked");
    for expected in [
        "version = \"1.0.151\"",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"",
        "checksum = \"c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14\"",
    ] {
        assert!(serde_package.lines().any(|line| line == expected));
    }
    let mut encoded = String::with_capacity(MAX_DESCRIPTOR_BYTES);
    encoded.push_str("{\"escaped\":\"");
    while encoded.len() + "\\u0061\"}".len() <= MAX_DESCRIPTOR_BYTES {
        encoded.push_str("\\u0061");
    }
    encoded.push_str("\"}");
    assert!(encoded.len() >= MAX_DESCRIPTOR_BYTES - 6);
    let value: Value = serde_json::from_str(&encoded).unwrap();
    let string_payload = checked_json_string_payload(&value).unwrap();
    assert!(string_payload <= encoded.len());
    assert!(encoded.len().checked_mul(2).unwrap() <= MAX_DESCRIPTOR_BYTES * 2);
}

#[test]
fn hir_fingerprint_admits_exact_depth_result_and_option_try_chains() {
    let (program, _) = fixture();
    let resolved = hir::resolve(&program).unwrap();
    let seed_id = resolved.functions[0].body.id.clone();
    for option in [false, true] {
        let leaf = ResolvedExpr {
            id: seed_id.clone(),
            ty: ResolvedType::I64,
            ownership: OwnershipMode::Value,
            span: crate::ast::Span::default(),
            kind: ResolvedExprKind::Int(1),
        };
        let wrap = |operand: ResolvedExpr, _index: usize| ResolvedExpr {
            // Fingerprinting does not validate expression identity uniqueness. Reusing a
            // resolver-issued ID keeps this forged, parser-independent depth fixture within
            // the public HIR construction surface.
            id: seed_id.clone(),
            ty: ResolvedType::I64,
            ownership: OwnershipMode::Value,
            span: crate::ast::Span::default(),
            kind: if option {
                ResolvedExprKind::TryOption {
                    operand: Box::new(operand),
                    option: DeclarationId::new("prelude.option".to_owned()),
                    some_case: DeclarationId::new("prelude.option.some".to_owned()),
                    some_field: DeclarationId::new("prelude.option.some.value".to_owned()),
                    none_case: DeclarationId::new("prelude.option.none".to_owned()),
                    residual_type: ResolvedType::I64,
                }
            } else {
                ResolvedExprKind::Try {
                    operand: Box::new(operand),
                    result: DeclarationId::new("prelude.result".to_owned()),
                    ok_case: DeclarationId::new("prelude.result.ok".to_owned()),
                    ok_field: DeclarationId::new("prelude.result.ok.value".to_owned()),
                    err_case: DeclarationId::new("prelude.result.err".to_owned()),
                    err_field: DeclarationId::new("prelude.result.err.error".to_owned()),
                    residual_type: ResolvedType::I64,
                }
            },
        };
        let mut exact = leaf;
        for index in 1..MAX_SEMANTIC_EXPRESSION_DEPTH {
            exact = wrap(exact, index);
        }
        let mut hasher = Sha256::new();
        hash_expr(&mut hasher, &exact, 0).unwrap();
        assert_eq!(
            format!(
                "sha256:{:x}",
                semaprax::digest_hex::LowerHex(hasher.finalize())
            )
            .len(),
            71
        );

        let over = wrap(exact, MAX_SEMANTIC_EXPRESSION_DEPTH);
        let mut hasher = Sha256::new();
        assert_eq!(
            hash_expr(&mut hasher, &over, 0).unwrap_err().code,
            "SPX-B109"
        );

        // Iteratively dismantle this deliberately forged test tree; the
        // production builder receives validated HIR through `resolve`.
        let mut current = over;
        loop {
            current = match current.kind {
                ResolvedExprKind::Try { operand, .. }
                | ResolvedExprKind::TryOption { operand, .. } => *operand,
                _ => break,
            };
        }
    }
}

#[test]
fn fingerprint_type_identity_exact_writer_matches_hir_and_named_topology() {
    for depth in [0usize, 1, 32, MAX_SEMANTIC_EXPRESSION_DEPTH - 1] {
        let mut ty = ResolvedType::TypeParameter {
            owner: DeclarationId::new("type.owner".to_owned()),
            index: u32::MAX,
        };
        for index in 0..depth {
            ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(format!("type.layer.{index}")),
                arguments: vec![ty, ResolvedType::Bool],
            };
        }
        let expected = ty.identity_key();
        let upper = type_identity_scratch_upper(&ty).unwrap();
        POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        let actual = fingerprint_type_identity(&ty, 0, 0).unwrap();
        let observed = POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get);
        assert_eq!(actual, expected);
        assert!(
            observed <= upper,
            "depth {depth} identity scratch actual/formula: {observed}/{upper}"
        );
    }
    let over_work = ResolvedType::Nominal {
        declaration: DeclarationId::new("type.too-wide".to_owned()),
        arguments: vec![ResolvedType::Bool; FINGERPRINT_ACTION_SLOTS],
    };
    assert_eq!(
        type_identity_metrics(&over_work, 1).unwrap_err().code,
        "SPX-B109"
    );
}

#[test]
fn resolved_owner_disposal_is_preallocated_and_depth_bounded() {
    let source = include_str!("../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan)
        .unwrap()
        .disposal_frames;
    let resolved = hir::resolve(&program).unwrap();
    assert!(
        !resolved.function_instances.is_empty(),
        "generic instances are required"
    );
    assert!(resolved.interfaces.iter().any(|interface| {
        interface
            .imports
            .iter()
            .any(|import| !import.parameters.is_empty())
    }));
    assert!(resolved.functions.iter().any(|function| {
        function.cleanup.slots.iter().any(|slot| {
            matches!(
                slot.shape,
                semaprax::cleanup::FieldLivenessShape::Leaf { .. }
                    | semaprax::cleanup::FieldLivenessShape::Record { .. }
            )
        })
    }));
    let mut staged_sources = [false; 3];
    for transition in resolved.functions.iter().flat_map(|function| {
        function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
    }) {
        if let crate::cleanup_plan::CleanupTransition::StageCopyResult { source } = transition {
            match source {
                crate::cleanup_plan::StagedCopyResultSource::Body { .. } => {
                    staged_sources[0] = true
                }
                crate::cleanup_plan::StagedCopyResultSource::TryResidual { .. } => {
                    staged_sources[1] = true
                }
                crate::cleanup_plan::StagedCopyResultSource::TryOptionNone { .. } => {
                    staged_sources[2] = true
                }
            }
        }
    }
    assert_eq!(staged_sources, [true; 3]);
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
    assert_eq!(std::mem::size_of::<ResolvedDisposeFrame>(), 56);
}

#[test]
fn resolved_owner_disposes_nested_patterns_and_514_level_resource_cleanup() {
    let pattern_source = "module disposal.patterns; @id(\"disposal.inner\") record Inner { @id(\"disposal.inner.value\") value: i64, } @id(\"disposal.outer\") record Outer { @id(\"disposal.outer.inner\") inner: Inner, } @id(\"disposal.choice\") variant Choice { @id(\"disposal.choice.value\") Value { @id(\"disposal.choice.value.payload\") payload: i64, }, @id(\"disposal.choice.empty\") Empty, } @id(\"disposal.record.match\") fn record_match(input: Outer) -> i64 { match input { Outer { inner: Inner { value } } => value, } } @id(\"disposal.variant.match\") fn variant_match(input: Choice) -> i64 { match input { Choice::Value { payload } => payload, Choice::Empty {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }";
    let pattern_program = crate::parse(pattern_source, Path::new("disposal-patterns.spx")).unwrap();
    let pattern_canonical = crate::format::canonical(&pattern_program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let pattern_capacity =
        hir_pre_resolve_capacity(&pattern_program, pattern_canonical.len(), &mut scan)
            .unwrap()
            .disposal_frames;
    let pattern_resolved = hir::resolve(&pattern_program).unwrap();
    assert_resolved_owner_disposes_once_without_growth(pattern_resolved, pattern_capacity);

    let mut chain = String::from(
            "module disposal.cleanup_chain; @id(\"cleanup.r0\") resource R0 { @id(\"cleanup.r0.drop\") drop trivial; } ",
        );
    for index in 1..514 {
        use std::fmt::Write as _;
        write!(
                chain,
                "@id(\"cleanup.r{index}\") record R{index} {{ @id(\"cleanup.r{index}.value\") value: R{}, }} ",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"cleanup.consume\") fn consume(value: own R513) -> i64 { 1 } @id(\"app.main\") fn main() -> i64 { 0 }");
    let chain_program = crate::parse(&chain, Path::new("disposal-cleanup-chain.spx")).unwrap();
    let chain_canonical = crate::format::canonical(&chain_program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let chain_capacity = hir_pre_resolve_capacity(&chain_program, chain_canonical.len(), &mut scan)
        .unwrap()
        .disposal_frames;
    let chain_resolved = hir::resolve(&chain_program).unwrap();
    let consume = chain_resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "cleanup.consume")
        .unwrap();
    let mut maximum_shape_depth = 0usize;
    let mut pending = consume
        .cleanup_plan
        .slots
        .iter()
        .map(|slot| (&slot.field_liveness_shape, 1usize))
        .collect::<Vec<_>>();
    while let Some((shape, depth)) = pending.pop() {
        maximum_shape_depth = maximum_shape_depth.max(depth);
        if let semaprax::cleanup::FieldLivenessShape::Record { fields, .. } = shape {
            pending.extend(fields.iter().map(|field| (&field.shape, depth + 1)));
        }
    }
    assert_eq!(maximum_shape_depth, 514);
    assert_resolved_owner_disposes_once_without_growth(chain_resolved, chain_capacity);
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

#[test]
fn resolved_owner_undersized_workspace_aborts_before_post_drop_marker() {
    const CHILD_ENV: &str = "SEMAPRAX_TEST_UNDERSIZED_RESOLVED_DISPOSE";
    const BEFORE_MARKER: &str = "before-drop";
    const FORBIDDEN_MARKER: &str = "after-drop";

    if let Some(marker_root) = std::env::var_os(CHILD_ENV) {
        let marker_root = std::path::PathBuf::from(marker_root);
        let source = include_str!("../../../../tests/fixtures/native_rust_hir_capacity.spx");
        let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx"))
            .expect("child fixture parses");
        let resolved = hir::resolve(&program).expect("child fixture resolves");
        let owner = ResolvedProgramOwner::new(resolved, Vec::with_capacity(1), 1);
        std::fs::write(marker_root.join(BEFORE_MARKER), b"entered drop")
            .expect("write pre-drop marker");
        drop(owner);
        std::fs::write(marker_root.join(FORBIDDEN_MARKER), b"drop returned")
            .expect("write forbidden post-drop marker");
        return;
    }

    let marker_root =
        std::env::temp_dir().join(format!("semaprax-resolved-dispose-{}", std::process::id()));
    std::fs::create_dir(&marker_root).expect("create child marker directory");
    let output = Command::new(std::env::current_exe().expect("test executable path"))
            .arg("implementation::tests::resolved_owner_undersized_workspace_aborts_before_post_drop_marker")
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, &marker_root)
            .output()
            .expect("undersized disposal child starts");
    assert!(!output.status.success());
    assert!(marker_root.join(BEFORE_MARKER).is_file());
    assert!(!marker_root.join(FORBIDDEN_MARKER).exists());
    std::fs::remove_file(marker_root.join(BEFORE_MARKER)).expect("remove child marker");
    std::fs::remove_dir(&marker_root).expect("remove child marker directory");
}

#[test]
fn resolved_owner_disposes_on_every_late_prepare_failure() {
    let (program, spec) = fixture();
    for point in [
        PrepareFailurePoint::Closure,
        PrepareFailurePoint::Facts,
        PrepareFailurePoint::Render,
        PrepareFailurePoint::Replay,
    ] {
        RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
        RESOLVED_DISPOSE_CAPACITIES.with(|capacities| capacities.set([0; 2]));
        PREPARE_FAILURE_INJECTION.with(|selected| selected.set(Some(point)));
        let result = prepare_native_rust_interop(&program, spec.as_bytes());
        PREPARE_FAILURE_INJECTION.with(|selected| selected.set(None));
        let diagnostic = result.err().expect("injected stage must fail");
        assert_eq!(diagnostic.len(), 1, "{point:?}");
        assert_eq!(diagnostic[0].code, "SPX-B107", "{point:?}");
        assert_eq!(
            RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get),
            1,
            "{point:?}"
        );
        let capacities = RESOLVED_DISPOSE_CAPACITIES.with(std::cell::Cell::get);
        assert!(capacities[0] > 0, "{point:?}");
        assert_eq!(capacities[0], capacities[1], "{point:?}");
    }
}

#[test]
fn prebuilt_exact_depth_program_prepares_and_disposes_in_child() {
    const CHILD_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_DISPOSE";
    const CHILD_SHAPE_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_SHAPE";
    const CHILD_DEPTH_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_VALUE";
    const CHILD_MARKER_ENV: &str = "SEMAPRAX_TEST_PREBUILT_DEPTH_MARKERS";
    const READY: &str = "ready";
    const DONE: &str = "done";
    const REJECTED: &str = "rejected";

    if std::env::var_os(CHILD_ENV).is_some() {
        let shape_value = std::env::var(CHILD_SHAPE_ENV).expect("child shape");
        let shape = shape_value.as_str();
        let over = std::env::var(CHILD_DEPTH_ENV).as_deref() == Ok("513");
        let marker_root =
            std::path::PathBuf::from(std::env::var_os(CHILD_MARKER_ENV).expect("marker root"));
        let source = format!(
                "module prebuilt.{shape}; @id(\"prebuilt.{shape}.deep\") fn deep(value: bool) -> bool {{ value }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
            );
        let mut program = crate::parse(&source, Path::new("prebuilt-depth.spx")).unwrap();
        let mut serial = 1usize;
        loop {
            let function = program
                .functions
                .iter_mut()
                .find(|function| function.stable_id.ends_with(".deep"))
                .expect("selected function exists");
            let body = std::mem::replace(
                &mut function.body,
                crate::ast::Expr {
                    span: crate::ast::Span::default(),
                    kind: crate::ast::ExprKind::Bool(false),
                },
            );
            let span = crate::ast::Span {
                start: serial,
                end: serial + 1,
                line: serial + 1,
                column: 1,
            };
            serial += 2;
            function.body = crate::ast::Expr {
                span,
                kind: if shape == "if" {
                    crate::ast::ExprKind::If {
                        condition: Box::new(crate::ast::Expr {
                            span: crate::ast::Span {
                                start: serial,
                                end: serial + 1,
                                line: serial + 1,
                                column: 1,
                            },
                            kind: crate::ast::ExprKind::Bool(true),
                        }),
                        then_branch: Box::new(body),
                        else_branch: Box::new(crate::ast::Expr {
                            span: crate::ast::Span {
                                start: serial + 2,
                                end: serial + 3,
                                line: serial + 3,
                                column: 1,
                            },
                            kind: crate::ast::ExprKind::Bool(false),
                        }),
                    }
                } else {
                    crate::ast::ExprKind::Binary {
                        op: crate::ast::BinaryOp::And,
                        left: Box::new(crate::ast::Expr {
                            span: crate::ast::Span {
                                start: serial,
                                end: serial + 1,
                                line: serial + 1,
                                column: 1,
                            },
                            kind: crate::ast::ExprKind::Bool(true),
                        }),
                        right: Box::new(body),
                    }
                },
            };
            serial += 4;
            let _ = function;
            if validate_native_rust_source_expression_budget(&program).is_err() {
                if !over {
                    let function = program
                        .functions
                        .iter_mut()
                        .find(|function| function.stable_id.ends_with(".deep"))
                        .expect("selected function exists");
                    let wrapper = std::mem::replace(
                        &mut function.body,
                        crate::ast::Expr {
                            span,
                            kind: crate::ast::ExprKind::Bool(false),
                        },
                    );
                    function.body = match wrapper.kind {
                        crate::ast::ExprKind::If { then_branch, .. } => *then_branch,
                        crate::ast::ExprKind::Binary { right, .. } => *right,
                        _ => unreachable!(),
                    };
                }
                break;
            }
        }
        let canonical = crate::format::canonical(&program);
        let spec = render_spec(&Spec {
            module: program.module.clone(),
            source_revision: domain_digest(SOURCE_DOMAIN, canonical.as_bytes()),
            target: current_target().unwrap(),
            exports: vec![format!("prebuilt.{shape}.deep")],
            imports: Vec::new(),
            capabilities: Vec::new(),
        });
        std::fs::write(marker_root.join(READY), b"ready").unwrap();
        RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
        HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
        POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
        let result = prepare_native_rust_interop(&program, spec.as_bytes());
        if over {
            let diagnostics = match result {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("depth 513 unexpectedly prepared"),
            };
            assert_eq!(diagnostics[0].code, "SPX-B109");
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 0);
            HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
            POST_HIR_FACTS_ENTRY_COUNT.with(|count| assert_eq!(count.get(), 0));
            std::fs::write(marker_root.join(REJECTED), b"rejected").unwrap();
        } else {
            let prepared = result.unwrap();
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
            drop(prepared);
            std::fs::write(marker_root.join(DONE), b"done").unwrap();
        }
        std::mem::forget(program);
        std::process::exit(0);
    }

    let marker_root = std::env::temp_dir().join(format!(
        "semaprax-prebuilt-depth-dispose-{}",
        std::process::id()
    ));
    std::fs::create_dir(&marker_root).expect("create hosted marker directory");
    for (shape, depth, marker) in [
        ("if", "512", DONE),
        ("if", "513", REJECTED),
        ("lazy", "512", DONE),
        ("lazy", "513", REJECTED),
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
                .arg("implementation::tests::prebuilt_exact_depth_program_prepares_and_disposes_in_child")
                .arg("--exact")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .env(CHILD_SHAPE_ENV, shape)
                .env(CHILD_DEPTH_ENV, depth)
                .env(CHILD_MARKER_ENV, &marker_root)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{shape}/{depth}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(marker_root.join(READY).is_file());
        std::fs::remove_file(marker_root.join(READY)).unwrap();
        assert!(marker_root.join(marker).is_file());
        std::fs::remove_file(marker_root.join(marker)).unwrap();
    }
    std::fs::remove_dir(&marker_root).expect("remove hosted marker directory");
}

#[test]
fn every_expression_shape_resolves_at_exact_depth_512_and_rejects_513() {
    fn wrap_source(mut expression: crate::ast::Expr, count: usize) -> crate::ast::Expr {
        for _ in 0..count {
            let span = expression.span;
            expression = crate::ast::Expr {
                kind: crate::ast::ExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: Box::new(expression),
                },
                span,
            };
        }
        expression
    }

    fn replace_payload(
        expression: &mut crate::ast::Expr,
        replacement: &mut Option<crate::ast::Expr>,
    ) -> bool {
        use crate::ast::ExprKind;

        if matches!(&expression.kind, ExprKind::Var(name) if name == "payload") {
            *expression = replacement.take().expect("payload replacement is unique");
            return true;
        }
        match &mut expression.kind {
            ExprKind::Call { args, .. } => args
                .iter_mut()
                .any(|child| replace_payload(child, replacement)),
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => replace_payload(value, replacement),
            ExprKind::SuperMethod { args, .. } => args
                .iter_mut()
                .any(|child| replace_payload(child, replacement)),
            ExprKind::Binary { left, right, .. } => {
                replace_payload(left, replacement) || replace_payload(right, replacement)
            }
            ExprKind::Block { statements, tail } => {
                statements
                    .iter_mut()
                    .any(|statement| replace_payload(statement.value_mut(), replacement))
                    || replace_payload(tail, replacement)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                replace_payload(condition, replacement)
                    || replace_payload(then_branch, replacement)
                    || replace_payload(else_branch, replacement)
            }
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. } => fields
                .iter_mut()
                .any(|field| replace_payload(&mut field.value, replacement)),
            ExprKind::Match { scrutinee, arms } => {
                replace_payload(scrutinee, replacement)
                    || arms
                        .iter_mut()
                        .any(|arm| replace_payload(&mut arm.value, replacement))
            }
            ExprKind::UpdateRecord { base, fields } => {
                replace_payload(base, replacement)
                    || fields
                        .iter_mut()
                        .any(|field| replace_payload(&mut field.value, replacement))
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                replace_payload(receiver, replacement)
                    || args
                        .iter_mut()
                        .any(|child| replace_payload(child, replacement))
            }
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => false,
        }
    }

    fn source_depth(program: &Program) -> usize {
        let mut maximum = 0;
        let mut pending = program
            .functions
            .iter()
            .flat_map(|function| {
                function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures)
            })
            .map(|expression| (expression, 1_usize))
            .collect::<Vec<_>>();
        while let Some((expression, depth)) = pending.pop() {
            maximum = maximum.max(depth);
            let mut index = 0;
            while let Some(child) = ast_child(expression, index) {
                pending.push((child, depth + 1));
                index += 1;
            }
        }
        maximum
    }

    fn payload_depth(program: &Program) -> usize {
        let deep = program
            .functions
            .iter()
            .find(|function| function.stable_id.ends_with(".deep"))
            .expect("fixture deep function must exist");
        let mut pending = vec![(&deep.body, 1_usize)];
        while let Some((expression, depth)) = pending.pop() {
            if matches!(&expression.kind, crate::ast::ExprKind::Var(name) if name == "payload") {
                return depth;
            }
            let mut index = 0;
            while let Some(child) = ast_child(expression, index) {
                pending.push((child, depth + 1));
                index += 1;
            }
        }
        panic!("fixture payload must be present")
    }

    fn wrap_hir_body_once(program: &mut ResolvedProgram) {
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str().ends_with(".deep"))
            .expect("fixture deep function must resolve");
        let body = function.body.clone();
        function.body = ResolvedExpr {
            id: body.id.clone(),
            ty: body.ty.clone(),
            ownership: body.ownership,
            span: body.span,
            kind: ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: Box::new(body),
            },
        };
    }

    let cases = [
            (
                "unary",
                "module depth.unary; @id(\"depth.unary.deep\") fn deep(payload: i64) -> i64 { -payload } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "binary",
                "module depth.binary; @id(\"depth.binary.deep\") fn deep(payload: i64) -> i64 { payload + 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "binary-right",
                "module depth.binary_right; @id(\"depth.binary_right.deep\") fn deep(payload: i64) -> i64 { 0 + payload } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "if",
                "module depth.if_shape; @id(\"depth.if.deep\") fn deep(payload: i64) -> i64 { if true { payload } else { 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "if-condition",
                "module depth.if_condition; @id(\"depth.if_condition.deep\") fn deep(payload: i64) -> i64 { if payload > 0 { 1 } else { 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "if-else",
                "module depth.if_else; @id(\"depth.if_else.deep\") fn deep(payload: i64) -> i64 { if true { 0 } else { payload } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "block",
                "module depth.block; @id(\"depth.block.deep\") fn deep(payload: i64) -> i64 { let before = 0; payload } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "block-let-rhs",
                "module depth.block_let; @id(\"depth.block_let.deep\") fn deep(payload: i64) -> i64 { let value = payload; value } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "call",
                "module depth.call; @id(\"depth.call.id\") fn id(value: i64) -> i64 { value } @id(\"depth.call.deep\") fn deep(payload: i64) -> i64 { id(payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "call-first",
                "module depth.call_first; @id(\"depth.call_first.sum\") fn sum(a: i64, b: i64, c: i64) -> i64 { a + b + c } @id(\"depth.call_first.deep\") fn deep(payload: i64) -> i64 { sum(payload, 0, 0) } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "call-middle",
                "module depth.call_middle; @id(\"depth.call_middle.sum\") fn sum(a: i64, b: i64, c: i64) -> i64 { a + b + c } @id(\"depth.call_middle.deep\") fn deep(payload: i64) -> i64 { sum(0, payload, 0) } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "call-last",
                "module depth.call_last; @id(\"depth.call_last.sum\") fn sum(a: i64, b: i64, c: i64) -> i64 { a + b + c } @id(\"depth.call_last.deep\") fn deep(payload: i64) -> i64 { sum(0, 0, payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "native-call",
                "module depth.native_call; permit { host.math } @id(\"host.math\") interface HostMath permits { host.math } { @id(\"host.add\") import rust fn host_add(left: i64, right: i64) -> i64 effects { host.math } failure status \"host.math.v1\"; } @id(\"depth.native_call.deep\") fn deep(payload: i64) -> i64 uses { host.math } { host_add(0, payload) } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "try",
                "module depth.try_shape; @id(\"depth.try.ok\") fn ok(value: i64) -> Result<i64, bool> { Result<i64, bool>::Ok { value: value } } @id(\"depth.try.deep\") fn deep(payload: i64) -> Result<i64, bool> { Result<i64, bool>::Ok { value: ok(payload)? } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "try-option",
                "module depth.try_option; @id(\"depth.try_option.some\") fn some(value: i64) -> Option<i64> { Option<i64>::Some { value: value } } @id(\"depth.try_option.deep\") fn deep(payload: i64) -> Option<i64> { Option<i64>::Some { value: some(payload)? } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "record-project",
                "module depth.record_project; @id(\"depth.pair\") record Pair { @id(\"depth.pair.x\") x: i64, } @id(\"depth.record_project.deep\") fn deep(payload: i64) -> i64 { Pair { x: payload }.x } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "record-field-first",
                "module depth.record_first; @id(\"depth.record_first.triple\") record Triple { @id(\"depth.record_first.triple.a\") a: i64, @id(\"depth.record_first.triple.b\") b: i64, @id(\"depth.record_first.triple.c\") c: i64, } @id(\"depth.record_first.deep\") fn deep(payload: i64) -> Triple { Triple { a: payload, b: 0, c: 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "record-field-middle",
                "module depth.record_middle; @id(\"depth.record_middle.triple\") record Triple { @id(\"depth.record_middle.triple.a\") a: i64, @id(\"depth.record_middle.triple.b\") b: i64, @id(\"depth.record_middle.triple.c\") c: i64, } @id(\"depth.record_middle.deep\") fn deep(payload: i64) -> Triple { Triple { a: 0, b: payload, c: 0 } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "record-field-last",
                "module depth.record_last; @id(\"depth.record_last.triple\") record Triple { @id(\"depth.record_last.triple.a\") a: i64, @id(\"depth.record_last.triple.b\") b: i64, @id(\"depth.record_last.triple.c\") c: i64, } @id(\"depth.record_last.deep\") fn deep(payload: i64) -> Triple { Triple { a: 0, b: 0, c: payload } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "variant",
                "module depth.variant; @id(\"depth.choice\") variant Choice { @id(\"depth.choice.value\") Value { @id(\"depth.choice.value.value\") value: i64, }, } @id(\"depth.variant.deep\") fn deep(payload: i64) -> Choice { Choice::Value { value: payload } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "match",
                "module depth.match_shape; @id(\"depth.match.choice\") variant Choice { @id(\"depth.match.choice.none\") None, @id(\"depth.match.choice.value\") Value { @id(\"depth.match.choice.value.value\") value: i64, }, } @id(\"depth.match.deep\") fn deep(payload: i64) -> i64 { match Choice::Value { value: 0 } { Choice::Value { value } => payload, Choice::None {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "match-scrutinee",
                "module depth.match_scrutinee; @id(\"depth.match_scrutinee.choice\") variant Choice { @id(\"depth.match_scrutinee.choice.none\") None, @id(\"depth.match_scrutinee.choice.value\") Value { @id(\"depth.match_scrutinee.choice.value.value\") value: i64, }, } @id(\"depth.match_scrutinee.deep\") fn deep(payload: i64) -> i64 { match Choice::Value { value: payload } { Choice::Value { value } => value, Choice::None {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "match-later-arm",
                "module depth.match_later; @id(\"depth.match_later.choice\") variant Choice { @id(\"depth.match_later.choice.a\") A, @id(\"depth.match_later.choice.b\") B, @id(\"depth.match_later.choice.c\") C, } @id(\"depth.match_later.deep\") fn deep(choice: Choice, payload: i64) -> i64 { match choice { Choice::A {} => 0, Choice::B {} => payload, Choice::C {} => 0, } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "match-nested-record-pattern",
                "module depth.match_nested; @id(\"depth.match_nested.inner\") record Inner { @id(\"depth.match_nested.inner.value\") value: i64, } @id(\"depth.match_nested.outer\") record Outer { @id(\"depth.match_nested.outer.inner\") inner: Inner, @id(\"depth.match_nested.outer.other\") other: i64, } @id(\"depth.match_nested.deep\") fn deep(input: Outer, payload: i64) -> i64 { match input { Outer { inner: Inner { value }, other: _ } => payload, } } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "update",
                "module depth.update; @id(\"depth.update.pair\") record Pair { @id(\"depth.update.pair.x\") x: i64, } @id(\"depth.update.deep\") fn deep(payload: i64) -> i64 { let pair = Pair { x: 0 }; (pair with { x: payload }).x } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
            (
                "update-base",
                "module depth.update_base; @id(\"depth.update_base.pair\") record Pair { @id(\"depth.update_base.pair.x\") x: i64, } @id(\"depth.update_base.deep\") fn deep(payload: i64) -> i64 { (Pair { x: payload } with { x: 0 }).x } @id(\"app.main\") fn main() -> i64 { 0 }",
            ),
        ];

    for (shape, source) in cases {
        let mut exact = crate::parse(source, Path::new("all-shape-depth.spx")).unwrap();
        let initial_depth = source_depth(&exact);
        assert!(initial_depth < MAX_SEMANTIC_EXPRESSION_DEPTH, "{shape}");
        let payload_depth = payload_depth(&exact);
        let replacement = wrap_source(
            crate::ast::Expr {
                kind: crate::ast::ExprKind::Var("payload".to_owned()),
                span: crate::ast::Span::default(),
            },
            MAX_SEMANTIC_EXPRESSION_DEPTH - payload_depth,
        );
        let function = exact
            .functions
            .iter_mut()
            .find(|function| function.stable_id.ends_with(".deep"))
            .expect("fixture deep function must exist");
        assert!(replace_payload(&mut function.body, &mut Some(replacement)));
        assert_eq!(
            source_depth(&exact),
            MAX_SEMANTIC_EXPRESSION_DEPTH,
            "{shape}"
        );
        validate_native_rust_source_expression_budget(&exact).unwrap();
        let canonical = crate::format::canonical(&exact);
        let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let disposal_capacity = hir_pre_resolve_capacity(&exact, canonical.len(), &mut scan)
            .unwrap()
            .disposal_frames;
        let resolved = hir::resolve(&exact)
            .unwrap_or_else(|diagnostics| panic!("{shape} failed resolution: {diagnostics:?}"));
        validate_native_rust_expression_budget(&resolved).unwrap();
        assert_resolved_owner_disposes_once_without_growth(resolved, disposal_capacity);

        let mut resolved = hir::resolve(&exact)
            .unwrap_or_else(|diagnostics| panic!("{shape} failed resolution: {diagnostics:?}"));

        let mut over_source = exact;
        let function = over_source
            .functions
            .iter_mut()
            .find(|function| function.stable_id.ends_with(".deep"))
            .unwrap();
        function.body = wrap_source(function.body.clone(), 1);
        let error = validate_native_rust_source_expression_budget(&over_source).unwrap_err();
        assert_eq!(error.code, "SPX-B109", "{shape}");
        assert_eq!(
            error.message, "Native Rust Interop max_semantic_expression_depth exceeds 512",
            "{shape}"
        );

        wrap_hir_body_once(&mut resolved);
        let error = validate_native_rust_expression_budget(&resolved).unwrap_err();
        assert_eq!(error.code, "SPX-B109", "{shape}");
        assert_eq!(
            error.message, "Native Rust Interop max_semantic_expression_depth exceeds 512",
            "{shape}"
        );
    }
}

#[test]
fn private_b_builds_exact_static_inventory_without_clobber() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-interop-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("semaprax_native_rust_interop.h"),
        &prepared.generated_header,
    )
    .unwrap();
    std::fs::write(root.join("module.c"), &prepared.generated_c).unwrap();
    let clang = configured_tool("CLANG").unwrap();
    let probe_object = if cfg!(windows) {
        "probe.obj"
    } else {
        "probe.o"
    };
    let mut probe = Command::new(&clang.path);
    probe.env_clear().current_dir(&root).args([
        "-std=c11",
        "-target",
        &prepared.target.triple,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        "-c",
        "module.c",
        "-o",
        probe_object,
    ]);
    bind_test_tool_environment(&mut probe);
    let probe = probe.output().unwrap();
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let output = root.join("bundle");
    let facts = build_native_rust_interop_bundle(&program, spec.as_bytes(), &output).unwrap();
    assert_eq!(facts.output_directory, output);
    assert!(facts.object_path.is_file());
    assert!(facts.descriptor_path.is_file());
    assert!(facts.manifest_path.is_file());
    assert!(facts.manifest_digest.starts_with("sha256:"));
    let manifest = std::fs::read_to_string(&facts.manifest_path).unwrap();
    assert!(manifest.ends_with('\n'));
    assert_eq!(
        domain_digest(BUNDLE_DIGEST_DOMAIN, manifest.as_bytes()),
        facts.manifest_digest
    );
    let value: Value = serde_json::from_str(&manifest).unwrap();
    let row = value.as_object().unwrap();
    assert_eq!(row.len(), 6);
    assert_eq!(
        row.get("schema").and_then(Value::as_str),
        Some(BUNDLE_SCHEMA)
    );
    let descriptor = row.get("descriptor").and_then(Value::as_object).unwrap();
    assert_eq!(descriptor.len(), 3);
    assert_eq!(
        descriptor.get("schema").and_then(Value::as_str),
        Some(DESCRIPTOR_SCHEMA)
    );
    assert_eq!(
        descriptor.get("digest").and_then(Value::as_str),
        Some(prepared.descriptor_digest.as_str())
    );
    assert_eq!(
        descriptor.get("bytes").and_then(Value::as_u64),
        u64::try_from(prepared.descriptor.len()).ok()
    );
    let files = row.get("files").and_then(Value::as_array).unwrap();
    let paths = files
        .iter()
        .map(|file| file.get("path").and_then(Value::as_str).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "descriptor.json",
            "module.c",
            if cfg!(windows) {
                "module.obj"
            } else {
                "module.o"
            },
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
        ]
    );
    for file in files {
        let file = file.as_object().unwrap();
        assert_eq!(file.len(), 3);
        let path = file.get("path").and_then(Value::as_str).unwrap();
        let bytes = std::fs::read(output.join(path)).unwrap();
        let digest = raw_digest(&bytes);
        assert_eq!(
            file.get("bytes").and_then(Value::as_u64),
            u64::try_from(bytes.len()).ok()
        );
        assert_eq!(
            file.get("sha256").and_then(Value::as_str),
            Some(digest.as_str())
        );
    }
    let toolchain = row.get("toolchain").and_then(Value::as_object).unwrap();
    assert_eq!(
        toolchain.get("target").and_then(Value::as_str),
        Some(prepared.target.triple.as_str())
    );
    assert_eq!(
        row.get("nonclaims")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap())
            .collect::<Vec<_>>(),
        NONCLAIMS
    );
    let retry = match build_native_rust_interop_bundle(&program, spec.as_bytes(), &output) {
        Ok(_) => panic!("existing output was overwritten"),
        Err(error) => error,
    };
    assert_eq!(retry[0].code, "SPX-I232");

    let foreign_file = root.join("foreign-file");
    std::fs::write(&foreign_file, b"foreign-file-sentinel").unwrap();
    let error = match build_native_rust_interop_bundle(&program, spec.as_bytes(), &foreign_file) {
        Ok(_) => panic!("foreign file was overwritten"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(
        std::fs::read(&foreign_file).unwrap(),
        b"foreign-file-sentinel"
    );

    let foreign_directory = root.join("foreign-directory");
    std::fs::create_dir(&foreign_directory).unwrap();
    let sentinel = foreign_directory.join("sentinel");
    std::fs::write(&sentinel, b"foreign-directory-sentinel").unwrap();
    let error =
        match build_native_rust_interop_bundle(&program, spec.as_bytes(), &foreign_directory) {
            Ok(_) => panic!("foreign directory was overwritten"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"foreign-directory-sentinel"
    );

    #[cfg(unix)]
    {
        let foreign_target = root.join("foreign-symlink-target");
        std::fs::write(&foreign_target, b"foreign-symlink-sentinel").unwrap();
        let foreign_link = root.join("foreign-symlink");
        std::os::unix::fs::symlink(&foreign_target, &foreign_link).unwrap();
        let error = match build_native_rust_interop_bundle(&program, spec.as_bytes(), &foreign_link)
        {
            Ok(_) => panic!("foreign symlink was followed"),
            Err(error) => error,
        };
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-I232");
        assert_eq!(
            std::fs::read(&foreign_target).unwrap(),
            b"foreign-symlink-sentinel"
        );
        assert!(std::fs::symlink_metadata(&foreign_link)
            .unwrap()
            .file_type()
            .is_symlink());
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn held_file_matching_rejects_symlink_identity_and_permission_drift() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-held-file-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let target = root.join("target");
    let link = root.join("link");
    std::fs::write(&target, b"authenticated-bytes").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let error = match_regular_file(&link, b"authenticated-bytes").unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"authenticated-bytes");

    let permissions = std::fs::metadata(&target).unwrap().permissions();
    let mut denied = permissions.clone();
    denied.set_mode(0o0);
    std::fs::set_permissions(&target, denied).unwrap();
    let result = match_regular_file(&target, b"authenticated-bytes");
    std::fs::set_permissions(&target, permissions).unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn held_stage_rejects_same_path_directory_and_reparse_substitution() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-held-stage-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();

    let parent = hold_stage(root.clone()).unwrap();
    let slot = StageSlot::new(&root, "sha256:held-stage", "identity").unwrap();
    let inventory = platform::prepare_discard_inventory([]).unwrap();
    let stage = create_stage(&parent, slot, &inventory).unwrap();
    stage.recheck().unwrap();
    let displaced = root.join("displaced-stage");
    std::fs::rename(&stage.path, &displaced).unwrap();
    std::fs::create_dir(&stage.path).unwrap();
    let error = stage.recheck().unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    assert!(displaced.is_dir());

    std::fs::remove_dir(&stage.path).unwrap();
    std::os::unix::fs::symlink(&displaced, &stage.path).unwrap();
    let error = stage.recheck().unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    assert!(std::fs::symlink_metadata(&stage.path)
        .unwrap()
        .file_type()
        .is_symlink());

    std::fs::remove_file(&stage.path).unwrap();
    std::fs::remove_dir_all(&displaced).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn linked_bridge_round_trips_rust_to_semaprax_to_rust_and_closes_failures() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let parsed_spec = parse_spec(&program, spec.as_bytes()).unwrap();
    let export = &prepared.exports[0];
    let import = &prepared.imports[0];
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-roundtrip-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    build_native_rust_interop_bundle(&program, spec.as_bytes(), &output).unwrap();

    let harness = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use core::num::NonZeroU32;
use semaprax_native_rust_interop::*;
struct Host{{mode:u8,panicked:bool}}
impl NativeRustImports for Host{{
fn {import_method}(&mut self,arg_0:i64,_arg_1:i64)->NativeRustImportResult<i64>{{
match self.mode{{
0=>NativeRustImportResult::Success(arg_0),
1=>NativeRustImportResult::Status{{code:NonZeroU32::new(7).unwrap(),class:NativeRustStatusClass::Import,retryable:true}},
2=>NativeRustImportResult::HostFailure,
6=>if self.panicked{{NativeRustImportResult::Success(arg_0)}}else{{self.panicked=true;panic!("panic once")}},
4=>NativeRustImportResult::Status{{code:NonZeroU32::new(7).unwrap(),class:NativeRustStatusClass::Semantic,retryable:false}},
_=>panic!("private sentinel must not cross the FFI boundary")
}}
}}
}}
fn bridge(mode:u8)->NativeRustBridge<Host>{{
let caps=NativeRustCapabilities::new(&["host.math"]).unwrap_or_else(|_|std::process::exit(10));
NativeRustBridge::new(Host{{mode,panicked:false}},caps)
}}
fn main(){{
std::panic::set_hook(Box::new(|_|{{}}));
if NativeRustCapabilities::new(&["wrong.capability"]).is_ok(){{std::process::exit(11)}}
let mut success=bridge(0);
match success.{export_method}(20,22){{Ok(42)=>{{}},_=>std::process::exit(12)}}
let mut semantic=bridge(0);
match semantic.{export_method}(i64::MAX,1){{
Err(NativeRustCallError::Semantic{{domain_id:"semaprax.native-rust-semantics.v1",code,class:NativeRustStatusClass::Semantic,retryable:false}}) if code.get()==2=>{{}},
_=>std::process::exit(19)
}}
let mut status=bridge(1);
match status.{export_method}(1,2){{
Err(NativeRustCallError::Semantic{{domain_id:"host.math.v1",code,class:NativeRustStatusClass::Import,retryable:true}}) if code.get()==7=>{{}},
_=>std::process::exit(13)
}}
let mut failed=bridge(2);
match failed.{export_method}(1,2){{Err(NativeRustCallError::HostFailed)=>{{}},_=>std::process::exit(14)}}
let mut panicked=bridge(3);
match panicked.{export_method}(1,2){{Err(NativeRustCallError::HostPanicked)=>{{}},_=>std::process::exit(15)}}
let mut panic_once=bridge(6);match panic_once.{export_method}(1,2){{Err(NativeRustCallError::HostPanicked)=>{{}},_=>std::process::exit(23)}}match panic_once.{export_method}(1,2){{Ok(3)=>{{}},_=>std::process::exit(24)}}
let mut wrong_class=bridge(4);
match wrong_class.{export_method}(1,2){{Err(NativeRustCallError::AdapterRejected)=>{{}},_=>std::process::exit(18)}}
let mut bounded=bridge(0);
for _ in 0..2048{{if !matches!(bounded.{export_method}(1,2),Ok(3)){{std::process::exit(16)}}}}
match bounded.{export_method}(1,2){{Err(NativeRustCallError::AdapterRejected)=>{{}},_=>std::process::exit(17)}}
}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    let active = prepared
            .generated_rust
            .find("||core::mem::replace(&mut self.active,true){return Err(NativeRustCallError::AdapterRejected)}")
            .unwrap();
    let effect = prepared.generated_rust[active..]
        .find("super::ffi::")
        .unwrap();
    assert!(
        effect > 0,
        "reentry must reject before allocating an FFI result slot or performing an import effect"
    );
    assert!(prepared
        .generated_rust
        .contains("impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}"));
    let harness_path = output.join("roundtrip.rs");
    std::fs::write(&harness_path, harness).unwrap();
    let executable = if cfg!(windows) {
        "roundtrip.exe"
    } else {
        "roundtrip"
    };
    let object = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let rustc = configured_tool("RUSTC").unwrap();
    let clang = configured_tool("CLANG").unwrap();
    let sanitizers = sanitizer_mode().unwrap();
    let o0_object = if cfg!(windows) {
        "module_hostile_O0.obj"
    } else {
        "module_hostile_O0.o"
    };
    let mut o0_compile = Command::new(&clang.path);
    o0_compile.env_clear().current_dir(&output).args([
        "-std=c11",
        "-target",
        &prepared.target.triple,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O0",
        "-c",
        "module.c",
        "-o",
        o0_object,
    ]);
    bind_test_tool_environment(&mut o0_compile);
    if sanitizers {
        o0_compile.args(REQUIRED_NATIVE_RUST_SANITIZER_FLAGS);
    }
    assert!(o0_compile.status().unwrap().success());
    for (linked_object, linked_executable) in [(o0_object, "roundtrip_O0"), (object, executable)] {
        let mut roundtrip_compile = Command::new(&rustc.path);
        roundtrip_compile.env_clear().current_dir(&output).args([
            "--edition=2021",
            "-C",
            "panic=unwind",
            "-C",
            &format!("link-arg={linked_object}"),
            "roundtrip.rs",
            "-o",
            linked_executable,
        ]);
        bind_test_tool_environment(&mut roundtrip_compile);
        bind_test_rust_linker(&mut roundtrip_compile, &clang);
        if sanitizers {
            roundtrip_compile.args([
                "-C",
                "link-arg=-fsanitize=address,undefined",
                "-C",
                "link-arg=-fno-sanitize-recover=all",
            ]);
        }
        assert!(roundtrip_compile.status().unwrap().success());
        let mut roundtrip_run = Command::new(output.join(linked_executable));
        roundtrip_run.env_clear().current_dir(&output);
        if sanitizers {
            roundtrip_run
                .env(
                    "ASAN_OPTIONS",
                    "detect_leaks=0:halt_on_error=1:abort_on_error=1",
                )
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        assert!(roundtrip_run.status().unwrap().success());
    }

    let capability_hex = capability_digest(&parsed_spec.capabilities)
        .strip_prefix("sha256:")
        .unwrap()
        .to_owned();
    let capability_bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    let abi_harness = format!(
        r#"#![allow(unsafe_code)]
use core::ffi::c_void;
type Callback=unsafe extern "C" fn(*mut c_void,i64,i64,*mut i64)->u64;
#[repr(C)]struct Imports{{abi_version:u32,size:u32,callback:Option<Callback>}}
#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}
unsafe extern "C"{{fn {export_symbol}(ctx:*const Context,arg_0:i64,arg_1:i64,result_out:*mut i64)->u64;}}
unsafe extern "C" fn callback(userdata:*mut c_void,left:i64,_right:i64,out:*mut i64)->u64{{
let injected=if userdata.is_null(){{0}}else{{unsafe{{*(userdata.cast::<u64>())}}}};
if injected!=0{{return injected}}unsafe{{*out=left}};0}}
fn adapter(code:u64)->u64{{(65535u64<<48)|(4u64<<32)|code}}
fn status(domain:u64,class:u64,retry:u64,code:u64)->u64{{(domain<<48)|(retry<<40)|(class<<32)|code}}
macro_rules! rejected{{($context:expr,$wire:expr)=>{{let mut poisoned=0x5a5a_6b6b_7c7c_8d8di64;assert_eq!({export_symbol}($context,1,2,&mut poisoned),$wire);assert_eq!(poisoned,0x5a5a_6b6b_7c7c_8d8di64);}}}}
fn main(){{unsafe{{
let imports=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,callback:Some(callback)}};
let mut context=Context{{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&imports,capabilities_digest:[{capability_bytes}],call_depth:0,reserved:0}};
let mut out=0i64;
assert_eq!({export_symbol}(&context,20,22,&mut out),0);assert_eq!(out,42);
rejected!(core::ptr::null(),adapter(1));
context.abi_version=2;rejected!(&context,adapter(1));context.abi_version=1;
context.size=0;rejected!(&context,adapter(1));context.size=core::mem::size_of::<Context>() as u32;
context.reserved=1;rejected!(&context,adapter(1));context.reserved=0;
context.imports=core::ptr::null();rejected!(&context,adapter(2));context.imports=&imports;
let bad_imports=Imports{{abi_version:2,size:core::mem::size_of::<Imports>() as u32,callback:Some(callback)}};context.imports=&bad_imports;rejected!(&context,adapter(2));context.imports=&imports;
let missing_callback=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,callback:None}};context.imports=&missing_callback;rejected!(&context,adapter(2));context.imports=&imports;
context.capabilities_digest[0]^=1;rejected!(&context,adapter(3));context.capabilities_digest[0]^=1;
context.call_depth=31;assert_eq!({export_symbol}(&context,1,2,&mut out),0);assert_eq!(out,3);
context.call_depth=32;rejected!(&context,adapter(7));context.call_depth=0;
let mut injected=status(65534,4,0,1);context.userdata=(&mut injected as *mut u64).cast();rejected!(&context,injected);
injected=status(65534,4,0,2);rejected!(&context,injected);
injected=status(65535,4,0,3);rejected!(&context,injected);
for forged in [status(65534,4,0,0),status(65534,4,0,3),status(65534,3,0,1),status(65534,4,1,1),status(65535,4,0,0),status(65535,4,0,9),status(65535,3,0,3),status(65535,4,1,3),status(65535,4,0,3)|(1u64<<41),status(0,4,0,1)]{{injected=forged;context.userdata=core::hint::black_box((&mut injected as *mut u64).cast());rejected!(&context,adapter(8));}}
context.userdata=core::ptr::null_mut();
assert_eq!({export_symbol}(&context,1,2,core::ptr::null_mut()),adapter(5));
let mut result_bytes=[0x5au8;16];let before_result_bytes=result_bytes;let misaligned=result_bytes.as_mut_ptr().add(1).cast::<i64>();assert_eq!({export_symbol}(&context,1,2,misaligned),adapter(5));assert_eq!(result_bytes,before_result_bytes);
let mut context_bytes=[0u8;128];let misaligned_context=context_bytes.as_mut_ptr().add(1).cast::<Context>();rejected!(misaligned_context,adapter(1));
}}}}
"#,
        export_symbol = export.c_symbol,
        capability_bytes = capability_bytes,
    );
    std::fs::write(output.join("abi_hostile.rs"), abi_harness).unwrap();
    let abi_executable = if cfg!(windows) {
        "abi_hostile.exe"
    } else {
        "abi_hostile"
    };
    for (linked_object, linked_executable) in
        [(o0_object, "abi_hostile_O0"), (object, abi_executable)]
    {
        let mut abi_compile = Command::new(&rustc.path);
        abi_compile.env_clear().current_dir(&output).args([
            "--edition=2021",
            "-C",
            "panic=abort",
            "-C",
            &format!("link-arg={linked_object}"),
            "abi_hostile.rs",
            "-o",
            linked_executable,
        ]);
        bind_test_tool_environment(&mut abi_compile);
        bind_test_rust_linker(&mut abi_compile, &clang);
        if sanitizers {
            abi_compile.args([
                "-C",
                "link-arg=-fsanitize=address,undefined",
                "-C",
                "link-arg=-fno-sanitize-recover=all",
            ]);
        }
        assert!(abi_compile.status().unwrap().success());
        let mut abi_run = Command::new(output.join(linked_executable));
        abi_run.env_clear().current_dir(&output);
        if sanitizers {
            abi_run
                .env(
                    "ASAN_OPTIONS",
                    "detect_leaks=0:halt_on_error=1:abort_on_error=1",
                )
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        assert!(abi_run.status().unwrap().success());
    }

    let cross_thread = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use semaprax_native_rust_interop::*;
struct Host;
impl NativeRustImports for Host{{fn {import_method}(&mut self,left:i64,_right:i64)->NativeRustImportResult<i64>{{NativeRustImportResult::Success(left)}}}}
fn main(){{let caps=NativeRustCapabilities::new(&["host.math"]).unwrap_or_else(|_|std::process::exit(1));let mut bridge=NativeRustBridge::new(Host,caps);std::thread::spawn(move||{{let _=bridge.{export_method}(1,2);}}).join().unwrap();}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    std::fs::write(output.join("cross_thread.rs"), cross_thread).unwrap();
    let cross_thread_executable = if cfg!(windows) {
        "cross_thread.exe"
    } else {
        "cross_thread"
    };
    let mut compile = Command::new(&rustc.path);
    compile.env_clear().current_dir(&output).args([
        "--edition=2021",
        "-C",
        "panic=unwind",
        "-C",
        &format!("link-arg={object}"),
        "cross_thread.rs",
        "-o",
        cross_thread_executable,
    ]);
    bind_test_tool_environment(&mut compile);
    bind_test_rust_linker(&mut compile, &clang);
    let compile = compile.output().unwrap();
    assert!(!compile.status.success());
    assert!(!output.join(cross_thread_executable).exists());
    assert!(
        String::from_utf8_lossy(&compile.stderr).contains("cannot be sent between threads safely")
    );

    let nested_borrow = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use semaprax_native_rust_interop::*;
struct Host;
impl NativeRustImports for Host{{fn {import_method}(&mut self,left:i64,_right:i64)->NativeRustImportResult<i64>{{NativeRustImportResult::Success(left)}}}}
fn nested(bridge:&mut NativeRustBridge<Host>){{let borrow=&mut *bridge;let first=bridge.{export_method}(1,2);let second=borrow.{export_method}(1,2);let _=(first,second);}}
fn main(){{}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    std::fs::write(output.join("nested_borrow.rs"), nested_borrow).unwrap();
    let nested = Command::new(&rustc.path)
        .env_clear()
        .current_dir(&output)
        .args([
            "--edition=2021",
            "--crate-type",
            "lib",
            "nested_borrow.rs",
            "-o",
            if cfg!(windows) {
                "nested_borrow.rlib"
            } else {
                "libnested_borrow.rlib"
            },
        ])
        .output()
        .unwrap();
    assert!(!nested.status.success());
    let nested_stderr = String::from_utf8_lossy(&nested.stderr);
    assert!(
        nested_stderr.contains("cannot borrow `*bridge` as mutable more than once at a time"),
        "{nested_stderr}"
    );

    let ffi_sibling = String::from(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
mod sibling{pub fn forge(){let _=super::semaprax_native_rust_interop::ffi::capabilities_digest();}}
fn main(){sibling::forge();}
"#,
    );
    std::fs::write(output.join("ffi_sibling.rs"), ffi_sibling).unwrap();
    let ffi_executable = if cfg!(windows) {
        "ffi_sibling.exe"
    } else {
        "ffi_sibling"
    };
    let mut compile = Command::new(&rustc.path);
    compile.env_clear().current_dir(&output).args([
        "--edition=2021",
        "-C",
        "panic=unwind",
        "-C",
        &format!("link-arg={object}"),
        "ffi_sibling.rs",
        "-o",
        ffi_executable,
    ]);
    bind_test_tool_environment(&mut compile);
    bind_test_rust_linker(&mut compile, &clang);
    let compile = compile.output().unwrap();
    assert!(!compile.status.success());
    assert!(!output.join(ffi_executable).exists());
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(stderr.contains("module `ffi` is private"), "{stderr}");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bool_and_infallible_import_abi_is_exact_at_o0_and_o2() {
    const BOOL_SOURCE: &str = r#"module interop.bool_fixture;

@id("host.bool")
interface HostBool
    permits {  }
{
    @id("host.bool.invert")
    import rust fn invert(value: bool) -> bool
        effects {  }
        failure infallible;
}

@id("interop.bool")
fn call_invert(value: bool) -> bool
{
    invert(value)
}

@id("interop.bool.main")
fn main() -> i64
{
    0
}
"#;
    let program = crate::parse(BOOL_SOURCE, Path::new("native-rust-bool.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: domain_digest(SOURCE_DOMAIN, canonical.as_bytes()),
        target: current_target().unwrap(),
        exports: vec!["interop.bool".to_owned()],
        imports: vec!["host.bool.invert".to_owned()],
        capabilities: Vec::new(),
    };
    let spec = render_spec(&spec);
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let parsed_spec = parse_spec(&program, spec.as_bytes()).unwrap();
    let export = &prepared.exports[0];
    let import = &prepared.imports[0];
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-bool-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    build_native_rust_interop_bundle(&program, spec.as_bytes(), &output).unwrap();
    let rustc = configured_tool("RUSTC").unwrap();
    let clang = configured_tool("CLANG").unwrap();
    let sanitizers = sanitizer_mode().unwrap();
    let object = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let o0_object = if cfg!(windows) {
        "module_bool_O0.obj"
    } else {
        "module_bool_O0.o"
    };
    let mut o0_compile = Command::new(&clang.path);
    o0_compile.env_clear().current_dir(&output).args([
        "-std=c11",
        "-target",
        &prepared.target.triple,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O0",
        "-c",
        "module.c",
        "-o",
        o0_object,
    ]);
    bind_test_tool_environment(&mut o0_compile);
    if sanitizers {
        o0_compile.args(REQUIRED_NATIVE_RUST_SANITIZER_FLAGS);
    }
    assert!(o0_compile.status().unwrap().success());

    let safe_harness = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use core::num::NonZeroU32;
use semaprax_native_rust_interop::*;
struct Host{{mode:u8}}
impl NativeRustImports for Host{{fn {import_method}(&mut self,value:bool)->NativeRustImportResult<bool>{{match self.mode{{0=>NativeRustImportResult::Success(!value),_=>NativeRustImportResult::Status{{code:NonZeroU32::new(9).unwrap(),class:NativeRustStatusClass::Import,retryable:false}}}}}}}}
fn bridge(mode:u8)->NativeRustBridge<Host>{{NativeRustBridge::new(Host{{mode}},NativeRustCapabilities::new(&[]).unwrap_or_else(|_|std::process::exit(10)))}}
fn main(){{let code=NonZeroU32::new(1).unwrap();let _=NativeRustImportResult::<bool>::HostFailure;let probe=NativeRustCallError::Semantic{{domain_id:"semaprax.native-rust-semantics.v1",code,class:NativeRustStatusClass::Semantic,retryable:false}};if let NativeRustCallError::Semantic{{domain_id,code,class,retryable}}=probe{{let _=(domain_id,code,class,retryable);}}let mut success=bridge(0);if !matches!(success.{export_method}(false),Ok(true))||!matches!(success.{export_method}(true),Ok(false)){{std::process::exit(11)}}let mut rejected=bridge(1);if !matches!(rejected.{export_method}(false),Err(NativeRustCallError::AdapterRejected)){{std::process::exit(12)}}}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    std::fs::write(output.join("bool_safe.rs"), safe_harness).unwrap();
    let capability_hex = capability_digest(&parsed_spec.capabilities)
        .strip_prefix("sha256:")
        .unwrap()
        .to_owned();
    let capability_bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    let raw_harness = format!(
        r#"#![allow(unsafe_code)]
use core::ffi::c_void;
type Callback=unsafe extern "C" fn(*mut c_void,u8,*mut u8)->u64;
#[repr(C)]struct Imports{{abi_version:u32,size:u32,callback:Option<Callback>}}
#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}
unsafe extern "C"{{fn {export_symbol}(ctx:*const Context,arg_0:u8,result_out:*mut u8)->u64;}}
fn adapter(code:u64)->u64{{(65535u64<<48)|(4u64<<32)|code}}
unsafe extern "C" fn callback(userdata:*mut c_void,value:u8,out:*mut u8)->u64{{let mode=unsafe{{*(userdata.cast::<u8>())}};match mode{{0=>{{unsafe{{*out=u8::from(value==0)}};0}},1=>{{unsafe{{*out=2}};0}},_=>adapter(3)}}}}
fn main(){{unsafe{{let imports=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,callback:Some(callback)}};let mut mode=0u8;let context=Context{{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:(&mut mode as *mut u8).cast(),imports:&imports,capabilities_digest:[{capability_bytes}],call_depth:0,reserved:0}};let mut out=0u8;assert_eq!({export_symbol}(&context,0,&mut out),0);assert_eq!(out,1);assert_eq!({export_symbol}(&context,1,&mut out),0);assert_eq!(out,0);let mut poison=0x5au8;assert_eq!({export_symbol}(&context,2,&mut poison),adapter(4));assert_eq!(poison,0x5a);mode=1;core::hint::black_box(&mode);assert_eq!({export_symbol}(&context,0,&mut poison),adapter(4));assert_eq!(poison,0x5a);mode=2;core::hint::black_box(&mode);assert_eq!({export_symbol}(&context,0,&mut poison),adapter(3));assert_eq!(poison,0x5a);}}}}
"#,
        export_symbol = export.c_symbol,
        capability_bytes = capability_bytes,
    );
    std::fs::write(output.join("bool_raw.rs"), raw_harness).unwrap();
    for (linked_object, suffix) in [(o0_object, "O0"), (object, "O2")] {
        for source in ["bool_safe.rs", "bool_raw.rs"] {
            let executable = format!(
                "{}_{}{}",
                source.trim_end_matches(".rs"),
                suffix,
                if cfg!(windows) { ".exe" } else { "" }
            );
            let mut compile = Command::new(&rustc.path);
            compile.env_clear().current_dir(&output).args([
                "--edition=2021",
                "-Dwarnings",
                "-C",
                "panic=unwind",
                "-C",
                &format!("link-arg={linked_object}"),
                source,
                "-o",
                &executable,
            ]);
            bind_test_tool_environment(&mut compile);
            bind_test_rust_linker(&mut compile, &clang);
            if sanitizers {
                compile.args([
                    "-C",
                    "link-arg=-fsanitize=address,undefined",
                    "-C",
                    "link-arg=-fno-sanitize-recover=all",
                ]);
            }
            assert!(compile.status().unwrap().success());
            let mut run = Command::new(output.join(&executable));
            run.env_clear().current_dir(&output);
            if sanitizers {
                run.env(
                    "ASAN_OPTIONS",
                    "detect_leaks=0:halt_on_error=1:abort_on_error=1",
                )
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
            }
            assert!(run.status().unwrap().success());
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}
