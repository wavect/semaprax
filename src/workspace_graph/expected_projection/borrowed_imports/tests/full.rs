//! Full overlay acceptance owners; the saved complete producer remains oracle.
use super::super::{construct, eligible, reserve_builder_structure, LayoutRegistry};
use super::{linked, old, parity, sources};
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;

const VEC_PROVIDER: &str = include_str!("../../../../../std/collections/src/collections.spx");
const BOX_PROVIDER: &str = include_str!("../../../../../std/mem/src/mem.spx");
const VEC_CALLER: &str = r#"module overlay.vec;
use function @id("std.collections.vec.with-capacity") from std.collections as with_capacity;
use function @id("std.collections.vec.push") from std.collections as push;
use function @id("std.collections.vec.get") from std.collections as get;
@id("overlay.vec.main") fn main()->i64 { let values=with_capacity<i64>(2usize); let appended=push<i64>(values,42); get<i64>(appended,0usize) }"#;
const BOX_CALLER: &str = r#"module overlay.box;
use function @id("std.mem.box.new") from std.mem as new;
use function @id("std.mem.box.into-inner") from std.mem as into_inner;
@id("overlay.box.main") fn main()->i64 { into_inner<i64>(new<i64>(42)) }"#;
// Complete prior aliases remain independent authenticated refusal corpora.
const VEC_ALIASED_CALLER: &str = r#"module overlay.vec;
use function @id("std.collections.vec.with-capacity") from std.collections as new_vec;
use function @id("std.collections.vec.push") from std.collections as append_vec;
use function @id("std.collections.vec.get") from std.collections as read_vec;
@id("overlay.vec.main") fn main()->i64 { let values=new_vec<i64>(2usize); let appended=append_vec<i64>(values,42); read_vec<i64>(appended,0usize) }"#;
const BOX_ALIASED_CALLER: &str = r#"module overlay.box;
use function @id("std.mem.box.new") from std.mem as new_box;
use function @id("std.mem.box.into-inner") from std.mem as take_box;
@id("overlay.box.main") fn main()->i64 { take_box<i64>(new_box<i64>(42)) }"#;
// Class methods are not module functions; each record/class-only provider also
// declares one inert function so it parses (SPX-P101). Type imports never
// project a provider function, and the original declarations stay verbatim.
const CLASS_PROVIDER: &str = r#"module overlay.class_provider;
@id("overlay.counter") class Counter { @id("overlay.counter.value") value:i64,
@id("overlay.counter.bumped") fn bumped(self:Counter, amount:i64)->Counter { Counter { value:self.value+amount } } }
@id("overlay.class_provider.inert") fn inert()->i64 {0}"#;
const CLASS_CALLER: &str = r#"module overlay.class_app;
use type @id("overlay.counter") from overlay.class_provider as Counter;
@id("overlay.class.main") fn main()->i64 { let counter=Counter {value:41}; let updated=counter.bumped(1); updated.value }"#;
const INVARIANT_PROVIDER: &str = r#"module overlay.invariant_provider;
@id("overlay.row") record Row { @id("overlay.row.x") x:i64, } requires x>=0
@id("overlay.invariant_provider.inert") fn inert()->i64 {0}"#;
const INVARIANT_CALLER: &str = r#"module overlay.invariant_app;
use type @id("overlay.row") from overlay.invariant_provider as Imported;
@id("overlay.invariant.main") fn main()->i64 { let row=Imported{x:41}; let updated=row with {x:42}; updated.x }"#;

// Invariants stay authored in their own module. The imported inert function
// keeps the real provider overlay present without opening forbidden type imports.
const INVARIANT_LOCAL_CALLER: &str = r#"module overlay.invariant_app;
use function @id("overlay.invariant_provider.inert") from overlay.invariant_provider as inert;
@id("overlay.local.row") record Row {@id("overlay.local.row.x") x:i64,} requires x>=0
@id("overlay.invariant.main") fn main()->i64 {let row=Row{x:41};let updated=row with {x:42};inert();updated.x}"#;

fn original_invariant_import_is_still_refused() {
    let programs = sources(INVARIANT_CALLER, INVARIANT_PROVIDER);
    let authored = index_authored(&programs).unwrap();
    let modules = crate::workspace_graph::index_modules(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let errors = crate::workspace_graph::import_validation::validate_uses_with_registry(
        &programs,
        &modules,
        &authored,
        Some(&registry),
    )
    .unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "SPX-G172");
    assert_eq!(errors[0].path.as_deref(), Some("caller.spx"));
    assert_eq!(
        errors[0].help.as_deref(),
        Some("construct and update the record through functions of its own module")
    );
}

#[test]
fn retained_vec_box_class_and_invariant_sources_match_full_owning_projection() {
    original_invariant_import_is_still_refused();
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_LOCAL_CALLER, INVARIANT_PROVIDER),
    ] {
        parity(caller, provider);
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        let owned = old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
        let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        // Retained provider wrappers keep their reserved std identities. Outside
        // the actual resolver's linked scope this foreign caller projection is
        // still refused before any identity request.
        let expected_refusal = match caller {
            VEC_CALLER => Some("SPX-T283"),
            BOX_CALLER => Some("SPX-T286"),
            _ => None,
        };
        if let Some(code) = expected_refusal {
            // Graph serialization owns its own source verification. The
            // flattened projection never authenticates a reserved wrapper
            // outside the ordinary linked boundary.
            let graph_errors = crate::graph::to_json(&owned).unwrap_err();
            assert!(
                graph_errors.iter().any(|error| error.code == code),
                "{graph_errors:?}"
            );
            for unauthenticated in [
                hir::identity_resolution::resolve(
                    &owned,
                    layout.presentation(&programs[0], &owned).unwrap(),
                    |_| -> Result<(), ()> { panic!("identity request before wrapper refusal") },
                )
                .err()
                .unwrap(),
                hir::identity_resolution::resolve_source(
                    &view,
                    layout.presentation(&programs[0], &view).unwrap(),
                    |_| -> Result<(), ()> { panic!("identity request before wrapper refusal") },
                )
                .err()
                .unwrap(),
            ] {
                let hir::identity_resolution::Failure::Source(errors) = unauthenticated else {
                    panic!("unauthenticated wrapper refusal remapped")
                };
                assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
            }
        }
        let mut old_requests = Vec::new();
        let old = linked(|| {
            hir::identity_resolution::resolve(
                &owned,
                layout.presentation(&programs[0], &owned).unwrap(),
                |bytes| {
                    old_requests.push(bytes);
                    Ok::<(), ()>(())
                },
            )
        })
        .unwrap();
        let mut new_requests = Vec::new();
        let new = linked(|| {
            hir::identity_resolution::resolve_source(
                &view,
                layout.presentation(&programs[0], &view).unwrap(),
                |bytes| {
                    new_requests.push(bytes);
                    Ok::<(), ()>(())
                },
            )
        })
        .unwrap();
        assert_eq!(old.0, new.0);
        assert_eq!(old.1.reused(), new.1.reused());
        assert_eq!(old.1.costs(), new.1.costs());
        assert_eq!(old_requests, new_requests);
        // No transformed/generated declaration changes authored identity.
        for (ordinal, function) in programs[0].functions.iter().enumerate() {
            assert!(std::ptr::eq(
                view.functions().get(ordinal).unwrap(),
                function
            ));
        }
    }
}

#[test]
fn invariant_and_wrapper_invalid_cases_keep_the_entire_old_error_order() {
    let cases = [
        (
            VEC_CALLER.replace(
                "with_capacity<i64>(2usize)",
                "with_capacity<string>(2usize)",
            ),
            VEC_PROVIDER.to_owned(),
        ),
        (
            BOX_CALLER.replace(
                "into_inner<i64>(new<i64>(42))",
                "into_inner<i64>(new<i64>(true))",
            ),
            BOX_PROVIDER.to_owned(),
        ),
        (
            CLASS_CALLER.replace("counter.bumped(1)", "counter.missing(1)"),
            CLASS_PROVIDER.to_owned(),
        ),
        (
            INVARIANT_CALLER.to_owned(),
            INVARIANT_PROVIDER.replace("requires x>=0", "requires missing>=0"),
        ),
        (
            INVARIANT_CALLER.to_owned(),
            INVARIANT_PROVIDER.replace("requires x>=0", "requires x"),
        ),
        (
            INVARIANT_CALLER.to_owned(),
            INVARIANT_PROVIDER
                .replace("x:i64", "x:Bytes")
                .replace("requires x>=0", "requires true"),
        ),
    ];
    for (caller, provider) in cases {
        let programs = sources(&caller, &provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        assert!(eligible(&programs[0], &authored, &programs));
        let before = old::construct(&programs[0], &authored, &programs, Some(&registry));
        let after = construct(&programs[0], &authored, &programs, &registry);
        match (before, after) {
            (Err(old), Err(new)) => assert_eq!(old, new),
            (Ok(old), Ok(view)) => {
                let layout = registry.get(&programs[0], &authored, &programs).unwrap();
                // Within the actual linked scope, so each wrapper case reaches
                // its intended type/contract error rather than the reserved-ID
                // refusal shared by both routes outside it.
                let old = linked(|| {
                    hir::resolve_with_function_reuse(
                        &old,
                        None,
                        None,
                        None,
                        layout.presentation(&programs[0], &old).unwrap(),
                    )
                })
                .err()
                .unwrap();
                let new = linked(|| {
                    hir::resolve_source(&view, layout.presentation(&programs[0], &view).unwrap())
                })
                .err()
                .unwrap();
                assert_eq!(old, new);
                assert!(
                    !old.iter()
                        .any(|error| error.code == "SPX-T283" || error.code == "SPX-T286"),
                    "{old:?}"
                );
            }
            _ => panic!("complete old/new constructor verdict differs"),
        };
    }
}

#[test]
fn complete_recursive_owner_receivers_pay_drop_cache_and_short_refusal() {
    original_invariant_import_is_still_refused();
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_LOCAL_CALLER, INVARIANT_PROVIDER),
    ] {
        let programs = sources(caller, provider);
        super::initialization::complete_original_receiver_before_measurement(&programs);
        let authored = index_authored(&programs).unwrap();
        let observe = |limit, retain| {
            let mut phases = [(
                "unreached",
                crate::test_allocation_observer::Metrics::default(),
                None,
            ); 8];
            let measured = with_limit_usage_refusal(limit, || {
                measure(|| {
                    let registry = LayoutRegistry::new(&programs, &authored);
                    phases[0] = (
                        "registry",
                        crate::test_allocation_observer::current_metrics(),
                        crate::bounded_output::active_remaining(),
                    );
                    let result = (|| {
                        let costs =
                            super::super::super::synthetic_builder_bytes_scoped_with_registry(
                                &programs[0],
                                &authored,
                                &programs,
                                None,
                                4,
                                true,
                                Some(&registry),
                            )?;
                        reserve_builder_structure(costs.raw_clone_and_hir)?;
                        let view = construct(&programs[0], &authored, &programs, &registry)?;
                        phases[1] = (
                            "construct",
                            crate::test_allocation_observer::current_metrics(),
                            crate::bounded_output::active_remaining(),
                        );
                        let layout = registry.get(&programs[0], &authored, &programs)?;
                        let presentation = layout.presentation(&programs[0], &view)?;
                        phases[2] = (
                            "presentation",
                            crate::test_allocation_observer::current_metrics(),
                            crate::bounded_output::active_remaining(),
                        );
                        let resolved = linked(|| {
                            hir::identity_resolution::resolve_source(&view, presentation, |bytes| {
                                reserve_builder_structure(bytes)
                            })
                        })
                        .map_err(|error| match error {
                            hir::identity_resolution::Failure::Source(errors) => errors,
                            hir::identity_resolution::Failure::Semantic(error) => vec![error],
                            hir::identity_resolution::Failure::Identity(errors) => errors,
                        })?;
                        phases[3] = (
                            "resolution",
                            crate::test_allocation_observer::current_metrics(),
                            crate::bounded_output::active_remaining(),
                        );
                        hir::validate(&resolved.0).map_err(|error| vec![error])?;
                        phases[4] = (
                            "validation",
                            crate::test_allocation_observer::current_metrics(),
                            crate::bounded_output::active_remaining(),
                        );
                        drop(resolved);
                        phases[5] = (
                            "resolved_drop",
                            crate::test_allocation_observer::current_metrics(),
                            crate::bounded_output::active_remaining(),
                        );
                        if retain {
                            drop(
                                super::super::super::borrowed_leaf::Input::Borrowed(view)
                                    .into_checked_key()?,
                            );
                        } else {
                            drop(view);
                        }
                        phases[6] = (
                            "receiving_drop",
                            crate::test_allocation_observer::current_metrics(),
                            crate::bounded_output::active_remaining(),
                        );
                        Ok::<(), Vec<Diagnostic>>(())
                    })();
                    if crate::bounded_output::active_refused() {
                        assert!(result.is_err());
                        assert!(reserve_builder_structure(0).is_err());
                    } else {
                        assert!(
                            result.is_ok(),
                            "authenticated canonical receiver: {result:?}"
                        );
                    }
                    drop(result);
                    drop(registry);
                    phases[7] = (
                        "registry_drop",
                        crate::test_allocation_observer::current_metrics(),
                        crate::bounded_output::active_remaining(),
                    );
                })
            });
            (measured, phases)
        };
        for retain in [false, true] {
            let ((((), observed), refused, required, _), phases) = observe(usize::MAX, retain);
            assert!(!refused && !observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
            assert!(
                observed.allocated <= required && observed.highwater <= required,
                "caller={caller} retain={retain} observed={observed:?} required={required} phases={phases:?}"
            );
            let ((((), observed), refused, paid, _), _) = observe(required, retain);
            assert!(!refused && !observed.invalid && observed.live == 0 && paid == required);
            let ((((), observed), refused, _, first), _) = observe(required - 1, retain);
            assert!(refused && first.is_some() && !observed.invalid && observed.live == 0);
        }
    }
}

#[test]
fn borrowed_authored_invariant_transform_matches_the_complete_owned_route() {
    let source = r#"module invariant_leaf;
@id("leaf.row") record Row { @id("leaf.row.x") x:i64, } requires x>=0
@id("leaf.main") fn main()->i64 {let mut row=Row{x:41}; row.x=42; row.x}"#;
    let program = crate::parse(source, "invariant-leaf.spx").unwrap();
    let view = super::super::super::borrowed_leaf::construct(&program).unwrap();
    let old = hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
    let new = hir::resolve_source(&view, None).unwrap();
    assert_eq!(old.0, new.0);
    assert_eq!(old.1.costs(), new.1.costs());
    assert!(std::ptr::eq(
        view.types().get(0).unwrap(),
        &program.types[0]
    ));
}

#[test]
fn literal_box_wrapper_overlay_removes_real_caller_copy_and_function_growth() {
    let programs = sources(BOX_CALLER, BOX_PROVIDER);
    let authored = index_authored(&programs).unwrap();
    let observe = |borrow| {
        measure(|| {
            let registry = LayoutRegistry::new(&programs, &authored);
            if borrow {
                drop(construct(&programs[0], &authored, &programs, &registry).unwrap());
            } else {
                drop(old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap());
            }
            drop(registry);
        })
    };
    let ((), before) = observe(false);
    let ((), after) = observe(true);
    // Independently saved producer: one authored function directory (capacity1)
    // followed by two imported wrappers grows to capacity4. Generated-only
    // storage acquires capacity2. The authored main is one Block tail containing
    // two one-argument calls, each with one explicit scalar type argument.
    assert!(std::mem::size_of::<crate::ast::Function>() <= 1024);
    let directories = 3 * std::mem::size_of::<crate::ast::Function>();
    let main_payload = 3 * std::mem::size_of::<crate::ast::Expr>()
        + 2 * std::mem::size_of::<crate::ast::Type>()
        + "overlay.box.main".len()
        + "main".len()
        + "into_inner".len()
        + "new".len();
    let sidecars = 2 * std::mem::size_of::<crate::ast::ModuleUse>()
        + "std.mem.box.new".len()
        + "std.mem.box.into-inner".len()
        + 2 * "std.mem".len()
        + "new".len()
        + "into_inner".len()
        + "caller.spx".len()
        + "overlay.box".len();
    // Both current Program destructors keep these narrow frontiers inline.
    // The original classifier still allocates its literal signature and the
    // old signature clone discards each provider function name before aliasing.
    let (classifier_allocations, classifier_bytes) = removed_classifier_invoice("std.mem");
    let discarded_names = "new".len() + "into_inner".len();
    assert_eq!(
        before.allocated,
        after.allocated
            + directories
            + main_payload
            + sidecars
            + classifier_bytes
            + discarded_names
    );
    assert_eq!(before.reallocations, after.reallocations + 1);
    assert_eq!(
        before.allocations,
        after.allocations + 1 + 9 + 9 + classifier_allocations + 2
    );
    for observed in [before, after] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn conservative_class_default_census_keeps_real_owned_record_forwarding() {
    let caller=super::CALLER.replacen("module function_layout.app;",
        "module function_layout.app; use type @id(\"overlay.unused_class\") from function_layout.provider as Unused;",1);
    let provider=format!("{} @id(\"overlay.unused_class\") class Unused {{ @id(\"overlay.unused_class.x\") x:i64, }}",super::PROVIDER);
    parity(&caller, &provider);
    let programs = sources(&caller, &provider);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
    let forward = view
        .functions()
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    // Retain the complete original Report<String, Vec<Row>> control. Its
    // provider forwards, but the ordinary import classifier intentionally
    // constructs a checked default for this shape; only its narrower
    // Bytes/usize renewal record grants an owner-forwarding prototype.
    let expected = old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
    let expected_forward = expected
        .functions
        .iter()
        .find(|f| f.name == "forward")
        .unwrap();
    assert_eq!(forward.body, expected_forward.body);
    assert!(matches!(
        forward.body.kind,
        crate::ast::ExprKind::ConstructRecord { .. }
    ));
    let provider = programs[1]
        .functions
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    assert_forward_owner(provider);

    let renewal_provider = r#"module renewal.provider;
@id("renewal.packet") record Packet {
 @id("renewal.packet.bytes") bytes:Bytes,
 @id("renewal.packet.len") len:usize,
}
@id("renewal.unused") class Unused {@id("renewal.unused.x") x:i64,}
@id("renewal.forward") fn forward(value:own Packet)->Packet {value}
"#;
    let renewal_caller = r#"module renewal.app;
use type @id("renewal.packet") from renewal.provider as Packet;
use type @id("renewal.unused") from renewal.provider as Unused;
use function @id("renewal.forward") from renewal.provider as forward;
@id("renewal.main") fn main()->i64 {
 let packet=Packet{bytes:bytes_zeroed(0usize),len:0usize};
 let renewed=forward(packet);
 i64_from_usize(renewed.len)
}
"#;
    parity(renewal_caller, renewal_provider);
    let renewal = sources(renewal_caller, renewal_provider);
    let authored = index_authored(&renewal).unwrap();
    let registry = LayoutRegistry::new(&renewal, &authored);
    let view = construct(&renewal[0], &authored, &renewal, &registry).unwrap();
    let mut forwarded = 0;
    for function in view
        .functions()
        .iter()
        .chain(renewal[1].functions.iter())
        .filter(|f| f.name == "forward")
    {
        assert_forward_owner(function);
        forwarded += 1;
    }
    assert_eq!(forwarded, 2, "both current prototype and original provider");
}

fn assert_forward_owner(function: &crate::ast::Function) {
    let mut tail = &function.body;
    while let crate::ast::ExprKind::Block {
        statements,
        tail: next,
    } = &tail.kind
    {
        assert!(statements.is_empty(), "forwarding tail has statements");
        tail = &**next;
    }
    let [owner] = function.params.as_slice() else {
        panic!("forward has exactly one owning parameter")
    };
    assert_eq!(owner.mode, crate::ast::ParamMode::Own);
    assert!(
        matches!(&tail.kind, crate::ast::ExprKind::Var(name) if *name == owner.name),
        "{:?}",
        tail.kind
    );
}

#[path = "full/receiving.rs"]
mod receiving;

// Aliasing a retained reserved wrapper is a source error even under the linked
// guard. Semantic refusal must not be mistaken for sticky physical refusal.
#[test]
fn complete_arbitrary_aliases_keep_original_source_refusal_without_payer_refusal() {
    for (caller, provider, code) in [
        (VEC_ALIASED_CALLER, VEC_PROVIDER, "SPX-T283"),
        (BOX_ALIASED_CALLER, BOX_PROVIDER, "SPX-T286"),
    ] {
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let owned =
                old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
            let costs = super::super::super::synthetic_builder_bytes_scoped_with_registry(
                &programs[0],
                &authored,
                &programs,
                None,
                4,
                true,
                Some(&registry),
            )
            .unwrap();
            reserve_builder_structure(costs.raw_clone_and_hir).unwrap();
            let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
            let receiving =
                super::super::construct_receiving(&programs[0], &authored, &programs, &registry)
                    .unwrap();
            let layout = registry.get(&programs[0], &authored, &programs).unwrap();
            let before = linked(|| {
                hir::resolve_with_function_reuse(
                    &owned,
                    None,
                    None,
                    None,
                    layout.presentation(&programs[0], &owned).unwrap(),
                )
            })
            .err()
            .unwrap();
            for current in [&view, &receiving] {
                let after = linked(|| {
                    hir::resolve_source(
                        current,
                        layout.presentation(&programs[0], current).unwrap(),
                    )
                })
                .err()
                .unwrap();
                assert_eq!(before, after);
                assert!(after.iter().any(|error| error.code == code), "{after:?}");
                assert!(!crate::bounded_output::active_refused());
                assert!(reserve_builder_structure(0).is_ok());
                drop(after);
            }
            drop(layout);
            drop((before, receiving, view, owned, registry));
        });
        assert!(!refused);
    }
}

#[path = "full/transient.rs"]
mod transient;

#[path = "full/publication.rs"]
mod publication;

// Independent literal original-classifier layouts; no measured counters supply this invoice.
fn original_classifier_invoice(module: &str) -> (usize, usize) {
    use crate::ast::{Param, Type};
    use std::mem::size_of;
    match module {
        "std.collections" => {
            // with_capacity/push/get: 5 Param elements in 3 directories,
            // 9 Type elements in 7 directories, 12 T names, 5 argN names
            // (format's 2*literal-prefix estimate), and 4 Vec names.
            (
                3 + 7 + 12 + 5 + 4,
                5 * size_of::<Param>()
                    + 9 * size_of::<Type>()
                    + 12 * "T".len()
                    + 5 * (2 * "arg".len())
                    + 4 * "Vec".len(),
            )
        }
        "std.mem" => {
            // new/into_inner each own one Param directory, one Type
            // directory, four T strings, one Box name and one arg0 name.
            (
                2 * (1 + 1 + 4 + 1 + 1),
                2 * (size_of::<Param>()
                    + size_of::<Type>()
                    + 4 * "T".len()
                    + "Box".len()
                    + "arg0".len()),
            )
        }
        _ => panic!("closed imported std classifier fixtures only"),
    }
}

// Both complete implementations share this fixture. The historical receiver
// retains classifier temporaries on both sides, so its removed invoice is zero.
// The current receiver removes the independently enumerated old signatures.
// No measured value chooses the expected invoice.
fn removed_classifier_invoice(module: &str) -> (usize, usize) {
    if super::super::CLASSIFIER_TEMPORARIES_REMOVED {
        original_classifier_invoice(module)
    } else {
        (0, 0)
    }
}
