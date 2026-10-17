//! Current renamed producer controls. The old-policy suites remain unchanged
//! against name_equal; independent old ABI comparison uses frozen engine context.
use super::tests::{linked, LIMIT, PROVIDER};
use super::*;
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::workspace_graph::index_authored;

const RENAMED: &str = r#"module alias.current;
use function @id("std.collections.vec.with-capacity") from std.collections as make_vec;
use function @id("std.collections.vec.push") from std.collections as append;
use function @id("std.collections.vec.get") from std.collections as read_value;
@id("alias.current.main") fn main()->i64 {let values=make_vec<i64>(2usize);let appended=append<i64>(values,42);read_value<i64>(appended,0usize)}"#;
const BOX_CALLER: &str = r#"module alias.boxes;
use function @id("std.mem.box.new") from std.mem as make_box;
use function @id("std.mem.box.get") from std.mem as read_box;
use function @id("std.mem.box.into-inner") from std.mem as take_box;
@id("alias.boxes.main") fn main()->i64 {let value=make_box<i64>(42);let result=read_box<i64>(value);take_box<i64>(value)}"#;

#[test]
fn renamed_vec_and_box_payloads_lend_provider_fields_through_actual_owner_and_hir() {
    for (caller, provider) in [
        (RENAMED, PROVIDER),
        (
            BOX_CALLER,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/std/mem/src/mem.spx")),
        ),
    ] {
        let programs = [
            crate::parse(caller, "caller.spx").unwrap(),
            crate::parse(provider, "provider.spx").unwrap(),
        ];
        let authored = index_authored(&programs).unwrap();
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut receiver = Receiver::acquire().unwrap();
            let old = super::super::before_alias::construct_using(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut receiver,
            )
            .unwrap();
            let view = construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut receiver,
            )
            .unwrap();
            assert!(view.functions.roots.functions.is_empty());
            assert_eq!(view.functions().len(), old.functions().len());
            for (ordinal, import) in programs[0].module_uses.iter().enumerate() {
                let cell = view
                    .function_refs()
                    .get(programs[0].functions.len() + ordinal)
                    .unwrap();
                let original = authored[import.persistent_id.as_str()].function.unwrap();
                let owning = old
                    .functions()
                    .get(programs[0].functions.len() + ordinal)
                    .unwrap();
                assert!(cell.has_stored_alias());
                assert_eq!(cell.name(), import.alias);
                assert!(std::ptr::eq(cell.payload(), original));
                assert!(std::ptr::eq(&cell.payload().body, &original.body));
                assert!(cell.same_ordinary_declaration(owning));
                // Current projected module and effective spelling cannot borrow
                // canonical provider wrapper authority from the raw payload.
                assert!(crate::vec_ops::source_wrapper_ref(&view, &cell).is_none());
                assert!(crate::box_ops::source_wrapper_ref(&view, &cell).is_none());
            }
            let layout = registry.get(&programs[0], &authored, &programs).unwrap();
            let old_result = linked(|| {
                crate::hir::identity_resolution::before_function_alias::resolve_source_nominal(
                    &old,
                    layout.presentation(&programs[0], &old).unwrap(),
                    crate::workspace_graph::reserve_builder_structure,
                )
            });
            let diagnostics = match old_result {
                Err(crate::hir::identity_resolution::Failure::Source(diagnostics)) => diagnostics,
                other => panic!("historical owning intrinsic-name limitation changed: {other:?}"),
            };
            super::owned_growth_tests::assert_old_alias_refusal(&old, &diagnostics);
            // Additional whole-payload-authenticated provider/binding oracle;
            // the old receiver above is explicitly not claimed to succeed.
            let oracle = super::owned_growth_tests::provider_binding_oracle(&old, &programs);
            let expected = linked(|| {
                crate::hir::identity_resolution::resolve_source_nominal(
                    &oracle,
                    layout.presentation(&programs[0], &oracle).unwrap(),
                    crate::workspace_graph::reserve_builder_structure,
                )
            }).unwrap().0;
            let mut owner =
                crate::bounded_output::identity_reservation::IdentityReservation::acquire()
                    .unwrap();
            let mut invocation = crate::hir::prelude_bundle::Invocation::new();
            let actual = linked(|| {
                crate::hir::identity_resolution::resolve_source_checked_previous_with_owner(
                    &view,
                    None,
                    None,
                    None,
                    layout.presentation(&programs[0], &view).unwrap(),
                    &mut invocation,
                    &mut owner,
                )
            })
            .unwrap()
            .0;
            assert_eq!(actual, expected);
            crate::hir::validate(&actual).unwrap();
            drop((
                actual, expected, invocation, owner, layout, view, old, receiver,
            ));
            drop(registry);
        });
        assert!(!refused && first.is_none());
    }
}

#[test]
fn provider_invariants_preselect_complete_owning_roots_before_any_borrowed_entry() {
    let mut programs = super::tests::sources(RENAMED);
    const MARKER: &str =
        "module marker; @id(\"marker.type\") record Marker { @id(\"marker.value\") value:i64, }";
    let diagnostic = crate::parse(MARKER, "marker.spx").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-P101");
    assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
    assert_eq!(
        diagnostic.message,
        "a module must declare at least one function"
    );
    assert_eq!(diagnostic.path.as_deref(), Some("marker.spx"));
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start, span.end, span.line, span.column)),
        Some((82, 82, 1, 83))
    );
    assert_eq!(diagnostic.help, None);
    // Only the complete original Marker declaration is extracted below. The
    // persistent anchor makes parsing legal and never reaches either receiver.
    let marker = format!("{MARKER} @id(\"marker.fixture-anchor\") fn fixture_anchor()->i64 {{0}}");
    let mut declaration = crate::parse(&marker, "marker.spx").unwrap().types.remove(0);
    declaration.invariants = Some(Box::new(vec![programs[0].functions[0].body.clone()]));
    programs[1].types.push(declaration);
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::super::before_alias::construct_using(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let view = construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        assert_eq!(
            view.functions().iter().collect::<Vec<_>>(),
            old.functions().iter().collect::<Vec<_>>()
        );
        assert!(view.functions.entries.iter().all(|entry| matches!(
            entry,
            crate::ast::program_source::function_alias::FunctionEntry::Owned(_)
        )));
        drop((view, old, owner));
        drop(registry);
    });
    assert!(!refused && first.is_none());
}

#[test]
fn malformed_provider_cannot_gain_borrowing_through_a_renamed_import() {
    let mut programs = super::tests::sources(RENAMED);
    programs[1]
        .functions
        .iter_mut()
        .find(|f| f.stable_id == "std.collections.vec.get")
        .unwrap()
        .effects
        .push("hostile.effect".to_owned());
    // Preserve the complete original malformed generic provider and its actual
    // earlier refusal. It cannot reach a borrowed-entry control through this
    // signature-only importer: generic cross-file types are not admitted.
    let ((), negative_refused, _, negative_first) = with_limit_usage_refusal(LIMIT, || {
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        for current in [false, true] {
            let result = if current {
                construct_transient(&programs[0], &authored, &programs, &registry, None, &mut owner)
            } else {
                super::super::before_alias::construct_using(&programs[0], &authored, &programs, &registry, None, &mut owner)
            };
            let diagnostic = match result {
                Err(Failure::One(diagnostic)) => diagnostic,
                _ => panic!("original generic malformed provider did not retain its early refusal"),
            };
            assert_eq!(diagnostic.code, "SPX-G172");
            assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
            assert_eq!(diagnostic.message, "generic cross-file types are not admitted");
            assert_eq!(diagnostic.path, None);
            assert_eq!(diagnostic.span, None);
            assert_eq!(diagnostic.help, None);
        }
    });
    assert!(!negative_refused && negative_first.is_none());
    // A parsed, nongeneric scalar signature reaches the intended owning
    // fallback. Its reserved identity/name and hostile effect remain intact,
    // so the unchanged full intrinsic guard must still reject the declaration.
    let mut scalar = crate::parse("module hostile; @id(\"hostile.scalar\") fn scalar(value:i64,index:usize)->i64 {value}", "hostile.spx").unwrap().functions.remove(0);
    let malformed = programs[1].functions.iter_mut().find(|function| function.stable_id == "std.collections.vec.get").unwrap();
    malformed.type_parameters = Vec::new();
    malformed.params = std::mem::take(&mut scalar.params);
    malformed.return_type = scalar.return_type.clone();
    malformed.body = scalar.body.clone();
    assert!(crate::vec_ops::source_wrapper(&programs[1], programs[1].functions.iter().find(|function| function.stable_id == "std.collections.vec.get").unwrap()).is_none());
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, _) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::super::before_alias::construct_using(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let view = construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let cell = view
            .function_refs()
            .get(programs[0].functions.len() + 2)
            .unwrap();
        assert!(!cell.has_stored_alias());
        assert!(cell.same_ordinary_declaration(
            old.functions()
                .get(programs[0].functions.len() + 2)
                .unwrap()
        ));
        assert_eq!(
            crate::source_verify::verify(&view),
            crate::source_verify::verify(&old)
        );
    });
    assert!(!refused);
}

#[test]
fn renamed_and_ordinary_whole_producer_receiver_drop_and_refusal_costs_are_observed() {
    use crate::test_allocation_observer::measure_with_capacity;
    for caller in [RENAMED, super::tests::CALLER] {
        let programs = super::tests::sources(caller);
        let authored = index_authored(&programs).unwrap();
        let observe = |borrowed, limit| {
            with_limit_usage_refusal(limit, || {
                measure_with_capacity(1 << 16, || {
                    let registry = LayoutRegistry::new(&programs, &authored);
                    let mut receiver = match Receiver::acquire() {
                        Ok(r) => r,
                        Err(_) => return false,
                    };
                    let resolve = |source: &dyn ProgramSource| {
                        let layout = match registry.get(&programs[0], &authored, &programs) {
                            Ok(l) => l,
                            Err(_) => return false,
                        };
                        let mut owner = match crate::bounded_output::identity_reservation::IdentityReservation::acquire() { Ok(o) => o, Err(_) => return false };
                        let mut invocation = crate::hir::prelude_bundle::Invocation::new();
                        let presentation = match layout.presentation(&programs[0], source) {
                            Ok(p) => p,
                            Err(_) => return false,
                        };
                        let result = linked(|| {
                            crate::hir::identity_resolution::resolve_source_checked_previous_with_owner(source, None, None, None, presentation, &mut invocation, &mut owner)
                        });
                        let accepted = result.is_ok();
                        drop(result);
                        drop(invocation);
                        drop(owner);
                        drop(layout);
                        accepted
                    };
                    let accepted = if borrowed {
                        match construct_transient(
                            &programs[0],
                            &authored,
                            &programs,
                            &registry,
                            None,
                            &mut receiver,
                        ) {
                            Ok(source) => {
                                let accepted = resolve(&source);
                                drop(source);
                                accepted
                            }
                            Err(failure) => {
                                drop(failure);
                                false
                            }
                        }
                    } else {
                        match super::super::before_alias::construct_using(
                            &programs[0],
                            &authored,
                            &programs,
                            &registry,
                            None,
                            &mut receiver,
                        ) {
                            Ok(source) => {
                                let accepted = resolve(&source);
                                drop(source);
                                accepted
                            }
                            Err(failure) => {
                                drop(failure);
                                false
                            }
                        }
                    };
                    drop(receiver);
                    drop(registry);
                    accepted
                })
            })
        };
        assert!(observe(false, LIMIT).0 .0);
        let owning = observe(false, LIMIT);
        let current = observe(true, LIMIT);
        assert!(owning.0 .0 && current.0 .0 && !owning.1 && !current.1);
        for result in [&owning, &current] {
            let physical = result.0 .1;
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
        eprintln!("847 whole real owner current-ABI owning counterfactual paid={} physical={:?}; current paid={} physical={:?}; renamed={}",owning.2,owning.0.1,current.2,current.0.1,caller == RENAMED);
        for limit in [0, current.2 - 1, current.2] {
            let observed = observe(true, limit);
            assert_eq!(observed.0 .0, limit == current.2);
            let physical = observed.0 .1;
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
    }
}


#[test]
fn provider_bound_intrinsic_alias_requires_existing_authority_exact_membership_and_original_guard() {
    use crate::ast::program_source::function_alias::FunctionView;
    use crate::ast::program_source::FunctionRef;
    struct BeforeFunctionRef<'a> {
        payload: &'a crate::ast::Function,
        alias_cell: Option<&'a &'a str>,
    }
    // Compile the entire original facade layout beside the current one. These
    // exact-profile assertions are executable prerequisites, not measured claims.
    assert_eq!(std::mem::size_of::<FunctionRef<'_>>(), std::mem::size_of::<BeforeFunctionRef<'_>>());
    assert_eq!(std::mem::align_of::<FunctionRef<'_>>(), std::mem::align_of::<BeforeFunctionRef<'_>>());
    let programs = super::tests::sources(RENAMED);
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let view = construct_transient(&programs[0], &authored, &programs, &registry, None, &mut owner).unwrap();
        let foreign = construct_transient(&programs[0], &authored, &programs, &registry, None, &mut owner).unwrap();
        let index = programs[0].functions.len();
        let cell = view.function_refs().get(index).unwrap();
        let alien = foreign.function_refs().get(index).unwrap();
        let original = authored["std.collections.vec.with-capacity"].function.unwrap();
        let alias = cell.name();
        let before = BeforeFunctionRef { payload: original, alias_cell: Some(&alias) };
        assert!(std::ptr::eq(before.payload, original));
        assert_eq!(before.alias_cell.copied(), Some(alias));
        let specialized = original.clone();
        let specialized_cell = cell.specialized_payload(&specialized);
        assert!(specialized_cell.provider_declaration().is_none());
        linked(|| assert!(crate::vec_ops::source_wrapper_ref(&view, &specialized_cell).is_none()));
        assert!(crate::vec_ops::source_wrapper_ref(&view, &cell).is_none());
        assert!(cell.provider_declaration().is_some());
        linked(|| {
            assert_eq!(crate::vec_ops::source_wrapper_ref(&view, &cell), Some(crate::vec_ops::VecOp::WithCapacity));
            assert!(crate::vec_ops::source_wrapper_ref(&view, &alien).is_none());
        });
        let mut changed = original.clone();
        changed.name = "make_vec".to_owned();
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        assert!(FunctionView::checked(&programs[1], &changed, "make_vec").is_none());
        changed = original.clone();
        changed.effects.push("hostile.effect".to_owned());
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        changed = original.clone();
        changed.params[0].mode = crate::ast::ParamMode::Own;
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        changed = original.clone();
        changed.body = programs[0].functions[0].body.clone();
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        changed = original.clone();
        changed.stable_id = "forged.wrapper".to_owned();
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        drop((view, foreign, owner));
        drop(registry);
    });
    assert!(!refused && first.is_none());
}

#[test]
fn ordinary_project_import_as_vec_alias_preserves_real_provider_declarations_and_complete_sources() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!("intrinsic-import-as-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let manifest = "schema = \"semaprax.project.v1\"\nname = \"intrinsic-alias-witness\"\nentry = \"alias.current\"\nsources = [\"src/app.spx\", \"src/provider.spx\", \"src/tests.spx\"]\nweb_exports = [\"alias.current.main\"]\ntests = [\"alias.witness_tests\"]\n";
    let tests = "module alias.witness_tests; @id(\"alias.witness.smoke\") fn smoke()->bool {true}";
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    for (path, source) in [("src/app.spx", RENAMED), ("src/provider.spx", PROVIDER), ("src/tests.spx", tests)] {
        std::fs::write(root.join(path), source).unwrap();
    }
    crate::project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        crate::hir::validate(snapshot.entry_program()).unwrap();
        for (path, source) in [("src/app.spx", RENAMED), ("src/provider.spx", PROVIDER), ("src/tests.spx", tests)] {
            assert_eq!(snapshot.sources().iter().find(|input| input.path() == path).unwrap().source(), source);
        }
        let evaluated = crate::interpreter::evaluate_resolved_zero_arg_i64(snapshot.entry_program(), "alias.current.main", 100_000)?;
        assert!(matches!(evaluated.outcome, crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)));

        Ok(())
    }).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
