use super::*;
use crate::cleanup_plan::build::{capacity_observation, helper_index};
use crate::hir::{DeclarationId, ExpressionId, FunctionExecutionId};
use crate::test_allocation_observer::{measure, measure_with_capacity};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
const SMALL: &str = "module capacity.small; @id(\"main\") fn main()->i64 {0}";
const HOSTILE: &str = include_str!("../../../../../tests/fixtures/native_rust_hir_capacity.spx");
const VARIANT: &str = include_str!("../../../../../tests/owned_byte_variant_v1_fixture.spx");

fn input(source: &str) -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(source, std::path::Path::new("actual-attribution.spx")).unwrap(),
    )
    .unwrap()
}
fn plans(
    program: &crate::hir::ResolvedProgram,
    old: bool,
) -> Vec<crate::cleanup_plan::CleanupPlan> {
    if old {
        helper_index::original_plans::build_program_plans(program)
    } else {
        helper_index::build_program_plans(program)
    }
    .unwrap()
}
#[derive(Clone, Copy, Debug)]
struct Lifecycle {
    input_retained: Metrics,
    replacement_retained: Metrics,
    replacement_dropped: Metrics,
    complete_drop: Metrics,
    trace: Trace,
    model: capacity_observation::Components,
}
fn observed(source: &str, old: bool, publish: bool) -> (Lifecycle, Metrics) {
    // Independent complete old output is a semantic comparator, not a current
    // input owner. Its allocation is outside both matched physical windows.
    let expected_input = input(source);
    let expected = plans(&expected_input, true);
    capacity_observation::reset_lower_capacity_high_water();
    measure_with_capacity(crate::cache_codec::MAX_NODES, || {
        // Input AST, actual HIR and its originally attached plans are all born
        // inside this ONE continuous observer, not prewarmed excluded owners.
        let parsed = crate::parse(source, std::path::Path::new("actual-attribution.spx")).unwrap();
        let mut program = crate::hir::resolve(&parsed).unwrap();
        let count = program.functions.len() + program.function_instances.len();
        assert_eq!(count, expected.len());
        let input_retained = current_metrics();
        let scope = Scope::begin();
        let replacement_retained;
        let replacement_dropped;
        if publish {
            if old {
                helper_index::original_plans::attach_program_plans(&mut program)
            } else {
                helper_index::attach_program_plans(&mut program)
            }
            .unwrap();
            replacement_retained = current_metrics();
            for (function, reference) in program
                .functions
                .iter()
                .chain(program.function_instances.iter().map(|i| &i.function))
                .zip(&expected)
            {
                assert_eq!(&function.cleanup_plan, reference);
            }
            // Published replacements are retained by the original input owner;
            // there is no artificial early Drop boundary for their payloads.
            replacement_dropped = replacement_retained;
        } else {
            let replacement = plans(&program, old);
            replacement_retained = current_metrics();
            assert_eq!(replacement, expected);
            drop(replacement);
            replacement_dropped = current_metrics();
        }
        let trace = scope.snapshot();
        assert!(trace.valid);
        if !old {
            assert_eq!(trace.counts, [1, 1, count, count, 1, usize::from(publish)]);
            for point in [
                Point::IndexStart,
                Point::IndexEnd,
                Point::BeforePush,
                Point::AfterPush,
                Point::Return,
            ] {
                if count != 0 || !matches!(point, Point::BeforePush | Point::AfterPush) {
                    assert!(trace.first[point as usize].is_some());
                    assert!(trace.last[point as usize].is_some());
                }
            }
        } else {
            assert_eq!(trace.counts, [0; POINTS]);
        }
        drop(scope);
        let model = capacity_observation::LAST.with(Cell::get);
        drop(program);
        drop(parsed);
        let complete_drop = current_metrics();
        Lifecycle {
            input_retained,
            replacement_retained,
            replacement_dropped,
            complete_drop,
            trace,
            model,
        }
    })
}
fn valid(receipt: &Lifecycle, physical: Metrics) {
    assert!(!physical.invalid && !receipt.complete_drop.invalid);
    assert_eq!(physical, receipt.complete_drop);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
    assert!(receipt.input_retained.live > 0);
    assert!(receipt.replacement_retained.allocated >= receipt.input_retained.allocated);
    assert!(receipt.replacement_dropped.freed >= receipt.replacement_retained.freed);
    assert!(receipt.model.valid);
    assert!(receipt
        .trace
        .first
        .iter()
        .flatten()
        .chain(receipt.trace.last.iter().flatten())
        .chain(receipt.trace.largest_sampled_live.iter().flatten())
        .all(|sample| !sample.invalid
            && sample.live <= sample.highwater
            && sample.highwater <= physical.highwater));
    // The continuous raw allocator peak, not these samples or the historical
    // model, includes every unsampled constructor and returned-plan window.
}
#[test]
fn complete_hostile_input_attached_replacement_and_publication_lifetimes_keep_original_receiver() {
    let parsed = crate::parse(
        HOSTILE,
        std::path::Path::new("native-rust-hir-capacity.spx"),
    )
    .unwrap();
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(canonical.as_bytes()))
        ),
        "sha256:2a012464bb1bdb624a79972d558fe837f6d55a9cd9f40d2ead16bfbba615f316"
    );
    for source in [HOSTILE, VARIANT, SMALL] {
        for publish in [false, true] {
            let (old, before) = observed(source, true, publish);
            let (current, after) = observed(source, false, publish);
            valid(&old, before);
            valid(&current, after);
            assert_eq!(
                before, after,
                "allocation-free hooks must not change actual full receiving totals"
            );
            assert_eq!(old.input_retained, current.input_retained);
            assert_eq!(old.replacement_retained, current.replacement_retained);
            assert_eq!(old.replacement_dropped, current.replacement_dropped);
            // All raw receipts print outside either observer. They have no
            // source authority and are not substituted for the original pin.
            eprintln!(
                "OPT811 actual whole lifecycle publish={publish}: {current:?}; allocator={after:?}"
            );
        }
    }
}

#[test]
fn shared_borrowed_identity_aliases_and_actual_tree_drop_are_distinct_from_legacy_lengths() {
    use crate::cleanup_plan::build::{cleanup_place_owned_capacity, storage_id_owned_capacity};
    use crate::cleanup_plan::{CleanupPlace, StorageId};
    let text = "owner.λ\0whole";
    let (receipts, physical) = measure(|| {
        let declaration = DeclarationId::new(text);
        let execution = FunctionExecutionId::Monomorphic(declaration.clone());
        let expression = ExpressionId::new(&execution, "borrowed\0shared");
        let value = crate::hir::ValueId::local(&execution, "borrowed\0value");
        let birth = current_metrics();
        let borrowed = &expression;
        let aliases = [expression.clone(), expression.clone(), (*borrowed).clone()];
        let value_aliases = [value.clone(), value.clone()];
        let static_identity = DeclarationId::compiler(crate::hir::CompilerDeclaration::Vec);
        assert_eq!(static_identity.owned_allocation_bytes(), 0);
        assert_eq!(current_metrics(), birth);
        let historical_value_terms = value_aliases
            .iter()
            .map(|id| storage_id_owned_capacity(&StorageId::Value(id.clone())))
            .sum::<usize>();
        assert_eq!(historical_value_terms, 2 * value.as_str().len());
        assert_eq!(current_metrics(), birth);
        let historical_alias_terms = aliases
            .iter()
            .map(|id| storage_id_owned_capacity(&StorageId::Temporary(id.clone())))
            .sum::<usize>();
        assert_eq!(current_metrics(), birth);
        assert_eq!(historical_alias_terms, 3 * expression.as_str().len());
        assert!(historical_alias_terms > 0);
        // Exact byte equality does not collapse independent physical backing.
        let independent = DeclarationId::new(text);
        assert_eq!(independent, declaration);
        assert_ne!(
            independent.shared_allocation_key(),
            declaration.shared_allocation_key()
        );
        drop(independent);
        let identity_only = current_metrics();
        let mut directory = BTreeMap::new();
        directory.insert(declaration.clone(), aliases[0].clone());
        let directory_retained = current_metrics();
        assert!(directory_retained.live > identity_only.live);
        let lookup = directory.get(text).unwrap();
        assert_eq!(lookup, &expression);
        assert_eq!(current_metrics(), directory_retained);
        drop(directory);
        let directory_dropped = current_metrics();
        assert_eq!(directory_dropped.live, identity_only.live);
        assert!(directory_dropped.freed > directory_retained.freed);
        // No len×item/header formula purports to price this BTree node.
        let place = CleanupPlace {
            storage: StorageId::Temporary(expression.clone()),
            projections: vec![declaration.clone(), declaration.clone()],
        };
        assert_eq!(
            cleanup_place_owned_capacity(&place),
            expression.as_str().len()
                + place.projections.capacity() * std::mem::size_of::<DeclarationId>()
                + 2 * text.len()
        );
        let place_retained = current_metrics();
        drop(place);
        let place_dropped = current_metrics();
        assert_eq!(place_dropped.live, identity_only.live);
        drop(aliases);
        drop(value_aliases);
        drop(value);
        drop(static_identity);
        drop(expression);
        drop(execution);
        drop(declaration);
        (
            birth,
            identity_only,
            directory_retained,
            directory_dropped,
            place_retained,
            place_dropped,
            historical_alias_terms,
        )
    });
    assert!(!physical.invalid);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
    eprintln!("OPT811 raw identity/tree owners={receipts:?}; completeDrop={physical:?}");
}

#[test]
fn attribution_scope_restores_on_unwind_and_disabled_hooks_allocate_nothing() {
    index_start();
    staging_before_push();
    published();
    let ((), physical) = measure(|| {
        let before = current_metrics();
        let outer = Scope::begin();
        let _ = std::panic::catch_unwind(|| {
            let _inner = Scope::begin();
            index_start();
            std::panic::resume_unwind(Box::new("attribution-unwind"));
        });
        assert_eq!(outer.snapshot().counts, [0; POINTS]);
        drop(outer);
        index_start();
        assert!(TRACE.with(Cell::get).is_none());
        // Catch-unwind's own panic payload is deliberately observed, not
        // subtracted or called constructor backing.
        assert!(current_metrics().allocated >= before.allocated);
    });
    assert!(!physical.invalid);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
}

#[test]
fn whole_original_sources_and_named_observer_projection_preserve_every_old_control() {
    let old_helper = include_str!("../../../../../docs/evidence/capacity-attribution-811/src__cleanup_plan__build__helper_index.rs.before.txt");
    assert_eq!(
        project_original_helper(include_str!("../../helper_index.rs")),
        old_helper
    );
    let old_observation = include_str!("../../../../../docs/evidence/capacity-attribution-811/src__cleanup_plan__build__capacity_observation.rs.before.txt");
    let registration = "\npub(super) mod actual_attribution;\n";
    assert_eq!(
        include_str!("../../capacity_observation.rs")
            .matches(registration)
            .count(),
        1
    );
    assert_eq!(
        include_str!("../../capacity_observation.rs").replace(registration, ""),
        old_observation
    );
    let old_controls = include_str!("../../../../../docs/evidence/capacity-attribution-811/src__cleanup_plan__build__capacity_observation__tests.rs.before.txt");
    let current_controls = include_str!("../tests.rs");
    let old_binding = "    let current = include_str!(\"../helper_index.rs\");";
    let current_binding = "    let current =\n        super::actual_attribution::project_original_helper(include_str!(\"../helper_index.rs\"));";
    assert_eq!(current_controls.matches(current_binding).count(), 1);
    assert_eq!(
        current_controls.replace(current_binding, old_binding),
        old_controls
    );
    let old_build = include_str!("../../../../../docs/evidence/capacity-attribution-811/src__cleanup_plan__build.rs.before.txt");
    assert_eq!(include_str!("../../../build.rs"), old_build);

    for (text, expected) in [
        (
            old_helper,
            "1f2fa2d7674259652ce5e45d9fd53e4d18dc915b8c3e381dc3f4b52a44cac210",
        ),
        (
            old_observation,
            "96df701b7c571b3b7976dc9e334869f67083824d5a00f67f2c1f215569fd34ce",
        ),
        (
            old_controls,
            "e79da3035a46178cb00279319e5fd17aaaf9ab47a99ad911b1893ac3b3ee20de",
        ),
        (
            old_build,
            "d2b0c09384cf4aa8c03112e5f96779bd36b938ba51f5ccde0db212df04a694e7",
        ),
    ] {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            ),
            expected
        );
    }
    let original = old_helper
        .split("pub(crate) fn build_program_plans(")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]\nthread_local!")
        .next()
        .unwrap();
    let saved = include_str!("../../helper_index/original_plans.rs")
        .split("pub(crate) fn build_program_plans(")
        .nth(1)
        .unwrap();
    assert_eq!(saved.trim(), original.trim());
}
