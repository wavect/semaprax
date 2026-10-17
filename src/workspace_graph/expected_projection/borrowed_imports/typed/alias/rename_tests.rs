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
            let expected = linked(|| {
                crate::hir::identity_resolution::before_function_alias::resolve_source_nominal(
                    &old,
                    layout.presentation(&programs[0], &old).unwrap(),
                    crate::workspace_graph::reserve_builder_structure,
                )
            })
            .unwrap()
            .0;
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
