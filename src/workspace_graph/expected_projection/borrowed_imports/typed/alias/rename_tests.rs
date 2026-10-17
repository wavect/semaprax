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
                other => panic!(
                    "historical owning intrinsic-name limitation changed: {:?}",
                    other.as_ref().map(|_| ())
                ),
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
            let diagnostic = if current {
                match construct_transient(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                    None,
                    &mut owner,
                ) {
                    Err(Failure::One(diagnostic)) => diagnostic,
                    _ => panic!("original generic malformed provider did not retain its early refusal"),
                }
            } else {
                match super::super::before_alias::construct_using(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                    None,
                    &mut owner,
                ) {
                    Err(Failure::One(diagnostic)) => diagnostic,
                    _ => panic!("original generic malformed provider did not retain its early refusal"),
                }
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
                                // Corrected common semantic baseline, applied
                                // identically after the complete real producer.
                                let mut directory = match common_provider_binding(&source, &programs, &mut receiver) {
                                    Ok(Some(directory)) => directory,
                                    _ => return false,
                                };
                                // Move, do not clone/discard, ALL original roots.
                                // Retain the old current entry backing as well.
                                directory.roots = source.functions.roots;
                                let original_entries = source.functions.entries;
                                let adapted = crate::ast::program_source::function_alias::TransientFunctions::new(&programs[0], directory);
                                let accepted = resolve(&adapted);
                                drop(adapted);
                                drop(original_entries);
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
                                // Corrected common semantic baseline, applied
                                // identically after the complete real producer.
                                let mut directory = match common_provider_binding(&source, &programs, &mut receiver) {
                                    Ok(Some(directory)) => directory,
                                    _ => return false,
                                };
                                let (_, original_roots) = source.into_overlay_owner();
                                directory.roots = original_roots;
                                let adapted = crate::ast::program_source::function_alias::TransientFunctions::new(&programs[0], directory);
                                let accepted = resolve(&adapted);
                                drop(adapted);
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
        eprintln!("847 corrected common semantic baseline; complete owning producer paid={} physical={:?}; current producer paid={} physical={:?}; renamed={}",owning.2,owning.0.1,current.2,current.0.1,caller == RENAMED);
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
        assert!(!view.function_refs().contains_presentation(&specialized_cell));
        assert!(crate::vec_ops::source_wrapper_ref(&view, &cell).is_none());
        assert!(view.function_refs().contains_presentation(&cell));
        assert!(!view.function_refs().contains_presentation(&alien));
        assert_eq!(cell.declaration_name(), original.name);
        linked(|| {
            assert_eq!(crate::vec_ops::source_wrapper_ref(&view, &cell), Some(crate::vec_ops::VecOp::WithCapacity));
            // Directory membership is checked above, independently of the
            // original pure classifier and its authenticated-module scope.
            assert_eq!(crate::vec_ops::source_wrapper_ref(&view, &alien), Some(crate::vec_ops::VecOp::WithCapacity));
        });
        let mut changed = original.clone();
        changed.name = "make_vec".to_owned();
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        linked(|| assert!(crate::vec_ops::source_wrapper_ref(&view, &FunctionRef::ordinary(&changed)).is_none()));
        assert!(FunctionView::checked(&programs[1], &changed, "make_vec").is_none());
        changed = original.clone();
        changed.effects.push("hostile.effect".to_owned());
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        linked(|| assert!(crate::vec_ops::source_wrapper_ref(&view, &FunctionRef::ordinary(&changed)).is_none()));
        changed = original.clone();
        changed.params[0].mode = crate::ast::ParamMode::Own;
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        linked(|| assert!(crate::vec_ops::source_wrapper_ref(&view, &FunctionRef::ordinary(&changed)).is_none()));
        changed = original.clone();
        changed.body = programs[0].functions[0].body.clone();
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        linked(|| assert!(crate::vec_ops::source_wrapper_ref(&view, &FunctionRef::ordinary(&changed)).is_none()));
        changed = original.clone();
        changed.stable_id = "forged.wrapper".to_owned();
        assert!(crate::vec_ops::source_wrapper(&programs[1], &changed).is_none());
        linked(|| assert!(crate::vec_ops::source_wrapper_ref(&view, &FunctionRef::ordinary(&changed)).is_none()));
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
    // Project source custody requires canonical bytes. Keep the original raw
    // fixture inputs and use the actual compiler formatter for every file.
    let sources = [("src/app.spx", RENAMED), ("src/provider.spx", PROVIDER), ("src/tests.spx", tests)]
        .map(|(path, source)| (path, crate::format::canonical(
            &crate::parse(source, std::path::Path::new(path)).unwrap())));
    for (path, source) in &sources {
        std::fs::write(root.join(path), source).unwrap();
    }
    crate::project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        crate::hir::validate(snapshot.entry_program()).unwrap();
        for (path, source) in &sources {
            assert_eq!(snapshot.sources().iter().find(|input| input.path() == *path).unwrap().source(), source.as_str());
        }
        let evaluated = crate::interpreter::evaluate_resolved_zero_arg_i64(snapshot.entry_program(), "alias.current.main", 100_000)?;
        assert!(matches!(evaluated.outcome, crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)));

        Ok(())
    }).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}


// Type-only receiving ledgers: no tuple/array below is instantiated.
// Each comparison frame pays both operand borrows and four call/return/chain
// verdict slots. Each sequence names Vec/slice/iterator/item receivers too.
type CommonCompareFrame<'a, T: ?Sized> = (&'a T, &'a T, [bool; 4]);
type CommonSequenceFrame<'a, T> = ([&'a Vec<T>; 4], [&'a [T]; 4], [std::slice::Iter<'a, T>; 4], [Option<&'a T>; 8], [usize; 8], [bool; 8]);
type CommonFacadeSlots<'a> = (
    [crate::ast::program_source::FunctionSlices<'a>; 8], [Option<crate::ast::program_source::FunctionSlices<'a>>; 8],
    [crate::ast::program_source::FunctionRef<'a>; 8], [Option<crate::ast::program_source::FunctionRef<'a>>; 8],
    [Option<&'a crate::ast::program_source::function_alias::FunctionEntry<'a>>; 2], [Option<&'a Function>; 4],
    [Option<&'a &'a str>; 2], [Option<&'a [usize]>; 2], [Option<usize>; 2], [usize; 12], [bool; 12],
);
type CommonEqSequences<'a> = (
    [CommonSequenceFrame<'a, crate::ast::Type>; 12], [CommonSequenceFrame<'a, crate::ast::Expr>; 8],
    [CommonSequenceFrame<'a, crate::ast::Statement>; 4], [CommonSequenceFrame<'a, crate::ast::Param>; 4],
    [CommonSequenceFrame<'a, crate::ast::TypeParameterDeclaration>; 4], [CommonSequenceFrame<'a, String>; 4],
    CommonCompareFrame<'a, Option<crate::ast::YieldsClause>>,
    CommonCompareFrame<'a, Option<Box<crate::ast::SessionProtocolFollowsClause>>>,
);
type CommonCallerEqSlots<'a> = (
    CommonFacadeSlots<'a>, CommonEqSequences<'a>, [CommonCompareFrame<'a, Function>; 4],
    [CommonCompareFrame<'a, crate::ast::Expr>; 36], [CommonCompareFrame<'a, crate::ast::ExprKind>; 36],
    [CommonCompareFrame<'a, crate::ast::Statement>; 8], [CommonCompareFrame<'a, crate::ast::Type>; 16],
    [CommonCompareFrame<'a, Option<crate::ast::Type>>; 8], [CommonCompareFrame<'a, crate::ast::LetSyntax>; 8],
    [CommonCompareFrame<'a, crate::ast::Span>; 64], [CommonCompareFrame<'a, usize>; 256],
    [CommonCompareFrame<'a, str>; 36], [CommonCompareFrame<'a, String>; 36], [bool; 64],
);
type CommonWrapperEqSlots<'a> = (
    CommonFacadeSlots<'a>, CommonEqSequences<'a>, [CommonCompareFrame<'a, Function>; 4],
    [CommonCompareFrame<'a, crate::ast::Expr>; 16], [CommonCompareFrame<'a, crate::ast::ExprKind>; 16],
    [CommonCompareFrame<'a, crate::ast::Type>; 32], [CommonCompareFrame<'a, crate::ast::Param>; 8], [CommonCompareFrame<'a, crate::ast::ParamMode>; 8],
    [CommonCompareFrame<'a, crate::ast::TypeParameterDeclaration>; 4],
    [CommonCompareFrame<'a, crate::ast::Span>; 64], [CommonCompareFrame<'a, usize>; 256],
    [CommonCompareFrame<'a, str>; 96], [CommonCompareFrame<'a, String>; 96], [crate::ast::Type; 2], [crate::ast::ParamMode; 4], [crate::hir::OwnershipMode; 4],
    [&'a [crate::ast::Type]; 4], [&'a [crate::ast::Expr]; 4], [&'a [crate::ast::Param]; 4], [&'a [&'a str]; 2],
    std::slice::Iter<'a, crate::ast::Param>, std::slice::Iter<'a, &'a str>,
    std::iter::Zip<std::slice::Iter<'a, crate::ast::Param>, std::slice::Iter<'a, &'a str>>,
    std::iter::Enumerate<std::iter::Zip<std::slice::Iter<'a, crate::ast::Param>, std::slice::Iter<'a, &'a str>>>,
    [Option<(usize, (&'a crate::ast::Param, &'a &'a str))>; 4],
    std::iter::Zip<std::slice::Iter<'a, crate::ast::Expr>, std::slice::Iter<'a, crate::ast::Param>>,
    [Option<(&'a crate::ast::Expr, &'a crate::ast::Param)>; 4],
    [crate::vec_ops::VecOp; 17], std::array::IntoIter<crate::vec_ops::VecOp, 17>,
    [Option<crate::vec_ops::VecOp>; 24], [crate::vec_ops::VecOp; 32], [Option<&'a str>; 68], [&'a crate::vec_ops::VecOp; 34],
    [bool; 68], // four verdict slots per actual17-case wrapper_by_id walk
    [bool; 32], // complete FunctionRef field chain and raw-name guard
    [bool; 64], // source_wrapper signature/body/generic/ownership predicates
    [bool; 32], // directory/Option/op-kind/reserve branches
    [Option<crate::ast::program_source::function_alias::FunctionView<'a>>; 2],
    [crate::ast::program_source::function_alias::FunctionView<'a>; 2],
    [crate::ast::program_source::function_alias::FunctionEntry<'a>; 2], [&'a crate::ast::ModuleUse; 3],
);

/// Both arms use this correction only AFTER their complete original producer.
/// It grants no verdict: the ordinary source verifier and full HIR resolver run
/// again on a fresh presentation. All original source owners remain borrowed.
fn common_provider_binding<'p>(
    source: &dyn ProgramSource,
    programs: &'p [Program],
    receiver: &mut Receiver,
) -> Result<Option<crate::ast::program_source::function_alias::FunctionDirectory<'p>>, Failure> {
    use crate::ast::program_source::function_alias::{FunctionDirectory, FunctionEntry, FunctionView, TransientFunctions};
    use crate::ast::program_source::{generated_roots::GeneratedRoots, FunctionSlices};
    use std::mem::size_of;
    receiver.entry::<Option<FunctionDirectory<'p>>>()?;
    // Closed three-wrapper fixture: no fallback body, types or owned functions
    // are manufactured. This common bound covers all top-level guard/iterator,
    // borrowing, result and directory-header receivers before they are formed.
    receiver.reserve(size_of::<([&dyn ProgramSource; 2], [&[Program]; 2], [&Program; 4], CommonFacadeSlots<'_>, [FunctionSlices<'_>; 3], [crate::ast::program_source::DeclSlices<'_, crate::ast::TypeDeclaration>; 4], [Option<crate::ast::program_source::DeclSlices<'_, crate::ast::TypeDeclaration>>; 4], [Option<FunctionDirectory<'p>>; 2], [Result<Option<FunctionDirectory<'p>>, Failure>; 2], [Option<&crate::ast::TypeDeclaration>; 2], [&[crate::ast::ModuleUse]; 2], [&[String]; 2], [&[crate::ast::InterfaceDeclaration]; 2], [&[crate::ast::ProtocolDeclaration]; 2], [&[crate::ast::ProtocolImplementation]; 2], [&[crate::ast::SessionProtocolDeclaration]; 2], [&[crate::ast::AgentDeclaration]; 2], [std::ops::Range<usize>; 2], [bool; 32], [usize; 16], [Option<&Program>; 2], [GeneratedRoots; 3], (&Program, GeneratedRoots), FunctionDirectory<'p>, [TransientFunctions<'p>; 2], [FunctionDirectory<'p>; 2], [Vec<FunctionEntry<'p>>; 2], [&crate::ast::TypeDeclaration; 8])>())?;
    if programs.len() != 2 || programs[0].functions.len() != 1 || source.function_refs().len() != 4 {
        return Ok(None);
    }
    let authored = &programs[0];
    if authored.module_uses.len() != 3 || source.path() != authored.path || source.module() != authored.module
        || !source.module_uses().is_empty() || source.permits() != authored.permits
        || source.interfaces() != authored.interfaces || source.protocols() != authored.protocols
        || source.implementations() != authored.implementations || source.session_protocols() != authored.session_protocols
        || source.agents() != authored.agents || source.types().len() < authored.types.len() {
        return Ok(None);
    }
    for index in 0..authored.types.len() {
        if source.types().get(index) != Some(&authored.types[index]) { return Ok(None); }
    }
    for index in 0..authored.functions.len() {
        receiver.reserve(size_of::<CommonCallerEqSlots<'_>>())?;
        if !source.function_refs().get(index).unwrap().same_ordinary_declaration(&authored.functions[index]) { return Ok(None); }
    }
    let mut directory = FunctionDirectory {
        roots: GeneratedRoots::plain(Vec::new(), Vec::new()),
        entries: super::directory::<FunctionEntry<'p>>(3, receiver)?,
    };
    for index in 0..3 {
        // A conservative complete receiving-frame bound for each admitted
        // with_capacity/push/get wrapper: <=2 parameters, <=4 body nodes and
        // <=8 type nodes. It pays authentication, not fictitious payload copies.
        receiver.reserve(size_of::<CommonWrapperEqSlots<'p>>())?;
        let import = &authored.module_uses[index];
        let provider = &programs[1];
        if import.kind != crate::ast::ModuleUseKind::Function || import.target_module != provider.module { return Ok(None); }
        // Full linear membership acquisition is paid identically in both arms.
        let scan_bytes = provider.functions.len().checked_mul(size_of::<([Option<&Function>; 4], [&Function; 8], [&&Function; 4], [*const Function; 4], [&str; 4], [&String; 4], [bool; 8])>()).ok_or_else(|| receiver.limit())?;
        let scan_bytes = scan_bytes.checked_add(size_of::<[std::slice::Iter<'_, Function>; 2]>()).ok_or_else(|| receiver.limit())?;
        receiver.reserve(scan_bytes)?;
        let Some(original) = provider.functions.iter().find(|function| function.stable_id == import.persistent_id) else { return Ok(None); };
        let Some(op) = crate::vec_ops::source_wrapper(provider, original) else { return Ok(None); };
        if !matches!(op, crate::vec_ops::VecOp::WithCapacity | crate::vec_ops::VecOp::Push | crate::vec_ops::VecOp::Get) || original.yields.is_some() || original.follows.is_some() { return Ok(None); }
        let Some(view) = FunctionView::checked(provider, original, &import.alias) else { return Ok(None); };
        directory.entries.push(FunctionEntry::Borrowed(view));
        let expected = FunctionSlices::projected(&[], &directory).unwrap().get(index).unwrap();
        let actual = source.function_refs().get(authored.functions.len() + index).unwrap();
        // same_declaration compares EVERY field, ordered payload/body included,
        // under the effective binding name. Also authenticate the raw spelling:
        // exactly the old materialized alias OR the actual provider declaration.
        if !actual.same_declaration(&expected) || !(actual.declaration_name() == import.alias || actual.declaration_name() == original.name) { return Ok(None); }
    }
    // The caller moves the complete original GeneratedRoots into this header.
    // Empty plain roots here own no allocation and destroy no original data.
    Ok(Some(directory))
}


#[test]
fn corrected_common_binding_rejects_forged_payload_and_preserves_raw_old_refusal() {
    use crate::ast::program_source::SyntheticProgramView;
    for caller in [RENAMED, super::tests::CALLER] {
        let programs = super::tests::sources(caller);
        let authored = index_authored(&programs).unwrap();
        let (raw, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut receiver = Receiver::acquire().unwrap();
            super::super::before_alias::construct_using(&programs[0], &authored, &programs, &registry, None, &mut receiver).unwrap()
        });
        assert!(!refused && first.is_none());
        if caller == RENAMED {
            let diagnostics = linked(|| crate::source_verify::verify(&raw));
            super::owned_growth_tests::assert_old_alias_refusal(&raw, &diagnostics);
        }
        let (current, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut receiver = Receiver::acquire().unwrap();
            construct_transient(&programs[0], &authored, &programs, &registry, None, &mut receiver).unwrap()
        });
        assert!(!refused && first.is_none());
        // This ADDITIONAL observation isolates only the common adapter fee;
        // the original strict owner above still measures both whole producers.
        let adapter_only = |source: &dyn ProgramSource| with_limit_usage_refusal(LIMIT, || {
            crate::test_allocation_observer::measure_with_capacity(1 << 16, || {
                let mut receiver = Receiver::acquire().unwrap();
                let result = common_provider_binding(source, &programs, &mut receiver).unwrap();
                let accepted = result.is_some();
                drop(result);
                drop(receiver);
                accepted
            })
        });
        let old_adapter = adapter_only(&raw);
        let current_adapter = adapter_only(&current);
        assert!(old_adapter.0 .0 && current_adapter.0 .0);
        assert!(!old_adapter.1 && !current_adapter.1 && old_adapter.3.is_none() && current_adapter.3.is_none());
        assert_eq!(old_adapter.2, current_adapter.2, "same complete common adapter bill");
        assert_eq!(old_adapter.0 .1, current_adapter.0 .1, "same complete actual adapter allocation metrics");
        assert!(!old_adapter.0 .1.invalid && old_adapter.0 .1.live == 0 && old_adapter.0 .1.allocated == old_adapter.0 .1.freed);
        drop(current);
        let (authored_source, roots) = raw.into_overlay_owner();
        for changed in 0..8 {
            // Negative input construction is outside the observed paid adapter,
            // using a complete real old materialization, never a fake verdict.
            let mut forged = crate::ast::program_source::generated_roots::GeneratedRoots::iterative(roots.types.clone(), roots.functions.clone());
            let function = &mut forged.functions[0];
            match changed {
                0 => function.name = "forged_binding".to_owned(),
                1 => function.stable_id = "forged.identity".to_owned(),
                2 => function.effects.push("forged.effect".to_owned()),
                3 => function.params[0].mode = crate::ast::ParamMode::Own,
                4 => function.body = programs[0].functions[0].body.clone(),
                5 => function.explicit_id = false,
                6 => forged.functions.swap(0, 1),
                _ => forged.functions.push(roots.functions[0].clone()),
            }
            let forged = SyntheticProgramView::with_generated_owner(authored_source, forged).unwrap();
            let (accepted, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                let mut receiver = Receiver::acquire().unwrap();
                common_provider_binding(&forged, &programs, &mut receiver).unwrap().is_some()
            });
            assert!(!accepted && !refused && first.is_none(), "forged field {changed} admitted");
        }
    }
}
