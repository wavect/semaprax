//! Whole original verifier/type-table receivers and constructor lifetime controls.
use super::*;
use crate::ast::TypeDeclaration;
use crate::source_verify::private_names::{PrivateNames, Providers};
use crate::test_allocation_observer::measure;
#[rustfmt::skip]
mod corpus;
fn json(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics.iter().map(Diagnostic::json).collect()
}
fn verify(program: &dyn ProgramSource) -> Vec<Diagnostic> {
    let current = super::verify(program);
    let old = declaration::old::verify(program);
    assert_eq!(json(&current), json(&old));
    assert_eq!(
        current.first().map(Diagnostic::json),
        old.first().map(Diagnostic::json)
    );
    current
}
fn compare_public_source(source: &str) {
    let program = crate::parse(source, "constructor-full-public.spx").unwrap();
    let before = program.clone();
    let _ = verify(&program);
    assert_eq!(program, before);
}
/// The one pinned intentional-recovery fixture: current public verification
/// differs from the saved old declaration pass by exactly the dependent read.
fn compare_public_intentional(source: &str) {
    let program = crate::parse(source, "constructor-full-public.spx").unwrap();
    let before = program.clone();
    let current = super::verify(&program);
    let old = declaration::old::verify(&program);
    super::recovery_tests::intentional_wrong_field::assert_public(source, &current, &old);
    assert_eq!(program, before);
}
const GENERIC: &str = r#"module constructor.generic;
@id("packet") record Packet { @id("packet.n") n:i64, }
@id("identity") fn identity<T>(value:T)->T {value}
@id("relay") fn relay<A,B>(left:A,right:B)->A {identity(left)}
@id("probe") fn probe(n:i64)->i64 requires n>=0 ensures result>=0 {identity(n)}
@id("main") fn main()->i64 {relay(1,true)}
"#;
const FIELDS: &str = r#"module constructor.fields;
@id("packet.a") record Packet { @id("packet.x.a") x:i64, @id("packet.x.b") x:bool, }
@id("packet.b") record Packet { @id("packet.x.c") x:bool, @id("packet.y") y:i64, }
@id("cycle.a") class A:B { @id("a.x") x:i64, }
@id("cycle.b") class B:A { @id("b.x") x:bool, }
@id("main") fn main()->i64 {0}
"#;
#[test]
fn complete_old_public_pass_preserves_generic_duplicate_namespace_cycle_and_first_json() {
    for source in [
        GENERIC,
        FIELDS,
        "module t; fn same(x:i64)->i64 {x} fn same<T>(x:T)->T {x} fn main()->i64 {same(1)}",
        "module t; fn recursive<T>(x:T)->T {recursive(x)} fn main()->i64 {0}",
        "module t; fn probe(n:i64,n:bool)->i64 requires n ensures n {0} fn main()->i64 {0}",
        "module t; fn main()->i64 {let x=1; let x=missing; x}",
        "module t; fn main()->i64 {let values=vec_with_capacity<i64>(1usize);vec_len<i64>(values)}",
    ] {
        compare_public_source(source);
    }
    let program = crate::parse(GENERIC, "constructor-partition.spx").unwrap();
    assert_eq!(
        json(&super::verify_partition(&program)),
        json(&declaration::old::verify_partition(&program))
    );
}
#[test]
fn borrowed_field_lookup_keeps_last_field_and_winning_declaration_including_prelude() {
    let mut program = crate::parse(FIELDS, "constructor-fields.spx").unwrap();
    let current = TypeTable::new(&program);
    let old = TypeTable::new_owned_reference(&program);
    for declaration in program
        .types
        .iter()
        .chain(crate::prelude::declarations_for_program(&program))
    {
        let winner = current.declaration(&declaration.name);
        assert_eq!(
            winner.map(|v| v as *const _),
            old.declaration(&declaration.name).map(|v| v as *const _)
        );
        for name in ["x", "y", "missing"] {
            assert_eq!(
                current
                    .declared_field(&declaration.name, name)
                    .map(|v| v as *const _),
                old.declared_field(&declaration.name, name)
                    .map(|v| v as *const _)
            );
        }
    }
    let winner = &program.types[1];
    let TypeDeclarationKind::Record { fields } = &winner.kind else {
        panic!("record");
    };
    assert!(std::ptr::eq(
        current.declared_field("Packet", "x").unwrap(),
        &fields[0]
    ));
    assert_eq!(current.merged_class_fields, old.merged_class_fields);
    assert_eq!(current.class_parents, old.class_parents);
    drop(current);
    drop(old);
    program.types.truncate(1);
    let current = TypeTable::new(&program);
    let TypeDeclarationKind::Record { fields } = &program.types[0].kind else {
        panic!("record");
    };
    assert!(std::ptr::eq(
        current.declared_field("Packet", "x").unwrap(),
        &fields[1]
    ));
    drop(current);
    let mut collision = crate::parse(
        "module t; fn main()->i64 {let values=vec_with_capacity<i64>(1usize);0}",
        "constructor-prelude.spx",
    )
    .unwrap();
    let mut authored = program.types[0].clone();
    authored.name = "Vec".to_owned();
    collision.types.push(authored);
    let current = TypeTable::new(&collision);
    let old = TypeTable::new_owned_reference(&collision);
    assert_eq!(
        current.declaration("Vec").map(|v| v as *const _),
        old.declaration("Vec").map(|v| v as *const _)
    );
    assert!(!std::ptr::eq(
        current.declaration("Vec").unwrap(),
        &collision.types[0]
    ));
}
/// A held closure whose single copy is the last declaration of the program.
struct Held<'a>(&'a TypeDeclaration);
impl Providers for Held<'_> {
    fn provider(&self, index: usize) -> Option<(&str, &TypeDeclaration)> {
        (index == 0).then_some(("held.provider", self.0))
    }
}

const PROVIDER: &str = "module held.provider;

@id(\"copy\")
record Decoded {
    @id(\"copy.n\")
    n: i64,
}

@id(\"zero\")
fn zero() -> i64
{
    0
}
";

// `_spx_layout_0` is an authored declaration below the bound copies; it keeps
// its authored spelling even though it shares the private prefix.
const SYNTHETIC: &str = "module held.caller;

@id(\"authored\")
record _spx_layout_0 {
    @id(\"authored.n\")
    n: i64,
}

@id(\"copy\")
record _spx_layout_1 {
    @id(\"copy.n\")
    n: i64,
}

@id(\"pick\")
fn pick(value: _spx_layout_1, other: _spx_layout_0) -> i64
{
    if value.n == 0 { value } else { other }
}
";

const PRESENTED: &str =
    "`if` branches return different types: Decoded [unimported @id(\"copy\") from held.provider] \
     and _spx_layout_0";
const ORDINARY: &str = "`if` branches return different types: _spx_layout_1 and _spx_layout_0";

#[test]
fn complete_old_private_presentation_keeps_exact_subject_provider_and_authored_spelling() {
    let provider = crate::parse(PROVIDER, "provider.spx").unwrap();
    let program = crate::parse(SYNTHETIC, "caller.spx").unwrap();
    let held = Held(&provider.types[0]);
    let view = PrivateNames::new(&program, &program, program.types.len() - 1, &held);
    let current = super::verify_presenting(&program, Some(view));
    let old = declaration::old::verify_presenting(&program, Some(view));
    assert_eq!(json(&current), json(&old));
    assert_eq!(
        current
            .iter()
            .find(|d| d.code == "SPX-T211")
            .unwrap()
            .message,
        PRESENTED
    );
    let mut generic_private = program.clone();
    let mut generic_program = crate::parse(GENERIC, "generic-private.spx").unwrap();
    generic_private
        .functions
        .append(&mut generic_program.functions);
    let generic_view = PrivateNames::new(
        &generic_private,
        &generic_private,
        generic_private.types.len() - 1,
        &held,
    );
    assert_eq!(
        json(&super::verify_presenting(
            &generic_private,
            Some(generic_view)
        )),
        json(&declaration::old::verify_presenting(
            &generic_private,
            Some(generic_view)
        )),
    );
    let drifted = program.clone();
    let current = super::verify_presenting(&drifted, Some(view));
    let old = declaration::old::verify_presenting(&drifted, Some(view));
    assert_eq!(json(&current), json(&old));
    assert_eq!(
        current
            .iter()
            .find(|d| d.code == "SPX-T211")
            .unwrap()
            .message,
        ORDINARY
    );
    assert_eq!(
        json(&super::verify_presenting(&program, None)),
        json(&declaration::old::verify_presenting(&program, None))
    );
    assert_eq!(
        json(&super::verify(&program)),
        json(&declaration::old::verify(&program))
    );
}
fn assert_observed_drop(
    label: &str,
    current: crate::test_allocation_observer::Metrics,
    old: crate::test_allocation_observer::Metrics,
) {
    assert!(
        !current.invalid && !old.invalid && current.live == 0 && old.live == 0,
        "{label}: current={current:?} old={old:?}"
    );
    assert_eq!(
        current.allocated, current.freed,
        "{label}: current={current:?}"
    );
    assert_eq!(old.allocated, old.freed, "{label}: old={old:?}");
    assert!(
        current.allocations < old.allocations && current.allocated < old.allocated,
        "{label}: current={current:?} old={old:?}"
    );
}
#[test]
fn full_public_receiver_and_secondary_directories_observe_actual_requests_through_drop() {
    let program = crate::parse(GENERIC, "constructor-physical.spx").unwrap();
    // `prelude::declarations()` is a process-lived OnceLock that the first
    // verifier or TypeTable would otherwise initialize inside the measured
    // scope, leaving its owner live after Drop. Hold exactly that immutable
    // table before any measured invocation; nothing else is warmed.
    let held = crate::prelude::declarations();
    let identity = (held.as_ptr(), held.len());
    let ((), current) = measure(|| {
        drop(super::verify(&program));
    });
    let ((), old) = measure(|| {
        drop(declaration::old::verify(&program));
    });
    assert_observed_drop("full verify", current, old);
    let ((), current) = measure(|| {
        drop(TypeTable::new(&program));
    });
    let ((), old) = measure(|| {
        drop(TypeTable::new_owned_reference(&program));
    });
    assert_observed_drop("type table", current, old);
    let after = crate::prelude::declarations();
    assert_eq!(identity, (after.as_ptr(), after.len()));
    assert!(std::ptr::eq(held, after));
    // Complete current/old Diagnostic parity, deliberately outside observation.
    assert_eq!(
        json(&super::verify(&program)),
        json(&declaration::old::verify(&program))
    );
    // Ordinary verifier setup remains public; these are actual requested layouts,
    // not ledger attribution, budget discounts, RSS or application-fit evidence.
}
#[test]
fn cold_process_prelude_acquisition_is_a_separate_process_lifetime_scope() {
    // The OnceLock is shared by every test in this process, so this scope is
    // cold only when no other test acquired it first. Both outcomes are
    // asserted; a cold acquisition is retained for the process lifetime and is
    // never part of a measured verifier invocation.
    let (identity, acquisition) = measure(|| {
        let held = crate::prelude::declarations();
        (held.as_ptr(), held.len())
    });
    assert!(!acquisition.invalid, "{acquisition:?}");
    if acquisition.allocations == 0 {
        assert_eq!(acquisition.live, 0, "warm: {acquisition:?}");
    } else {
        assert!(acquisition.live > 0, "cold: {acquisition:?}");
        assert!(
            acquisition.allocated > acquisition.freed,
            "cold: {acquisition:?}"
        );
    }
    let again = crate::prelude::declarations();
    assert_eq!(identity, (again.as_ptr(), again.len()));
    assert_eq!(again.len(), 11);
}
#[test]
fn complete_prelude_owner_construction_drops_through_actual_measurement() {
    let held = crate::prelude::declarations();
    let owned = crate::prelude::owned_declarations_for_tests();
    let reference = crate::prelude::original_initializer_reference();
    assert_eq!(owned, reference);
    assert_eq!(owned.as_slice(), held);
    assert!(!std::ptr::eq(owned.as_ptr(), held.as_ptr()));
    drop(owned);
    drop(reference);
    let ((), construction) = measure(|| {
        drop(crate::prelude::owned_declarations_for_tests());
    });
    assert!(!construction.invalid, "{construction:?}");
    assert!(construction.allocations > 0, "{construction:?}");
    assert_eq!(
        construction.allocated, construction.freed,
        "{construction:?}"
    );
    assert_eq!(construction.live, 0, "{construction:?}");
    let ((), original) = measure(|| {
        drop(crate::prelude::original_initializer_reference());
    });
    assert!(!original.invalid, "{original:?}");
    assert!(original.allocations > 0, "{original:?}");
    assert_eq!(original.allocated, original.freed, "{original:?}");
    assert_eq!(original.live, 0, "{original:?}");
    assert_eq!(construction, original);
    assert!(std::ptr::eq(held, crate::prelude::declarations()));
}
#[test]
fn requires_borrows_same_entry_state_and_body_ensures_keep_order_and_input_unchanged() {
    for source in [
        "module t; fn probe(flag:bool,n:i64)->i64 requires {let n=true;n} requires flag ensures result==n {n} fn main()->i64 {0}",
        "module t; fn inspect(x:own Bytes)->i64 requires {let local=bytes_zeroed(2usize);byte_len(bytes_as_slice(local))==2usize} ensures result==0 {byte_len(bytes_as_slice(x));0} fn main()->i64 {0}",
        "module t; fn probe(n:i64)->i64 requires missing ensures also_missing {body_missing} fn main()->i64 {0}",
    ] {
        compare_public_source(source);
    }
}
#[test]
fn complete_saved_pass_sources_and_all_children_remain_joined_and_within_caps() {
    let source = concat!(
        include_str!("../source_verify.rs"),
        include_str!("recovery.rs"),
        include_str!("recovery/callee_components.rs"),
        include_str!("recovery_tests/callee_components.rs"),
        include_str!("recovery_tests/extended_components.rs"),
        include_str!("iterative/enter/enter_before_extended.rs"),
        include_str!("iterative/calls/calls_before_extended.rs"),
        include_str!("iterative/records/records_before_extended.rs"),
        include_str!("scope/layout_original.rs"),
        include_str!("iterative/enter/enter_before_components.rs"),
        include_str!("iterative/calls/calls_before_components.rs"),
        include_str!("recovery_legacy.rs"),
        include_str!("recovery_legacy/enter.rs"),
        include_str!("recovery_legacy/blocks.rs"),
        include_str!("recovery_legacy/operators.rs"),
        include_str!("recovery_legacy/records.rs"),
        include_str!("recovery_legacy/match_arms.rs"),
        include_str!("recovery_tests/refusal_dependency.rs"),
        include_str!("recovery_tests.rs"),
        include_str!("recovery_tests/operators_799.rs"),
        include_str!("recovery_tests/flow_799.rs"),
        include_str!("iterative/matching/matching_before_799.rs"),
        include_str!("iterative/match_arms/arms_before_799.rs"),
        include_str!("iterative/match_arms/refused_patterns.rs"),
        include_str!("iterative/operators/before_799.rs"),
        include_str!("recovery_tests/boundaries.rs"),
        include_str!("recovery_tests/fixture_controls.rs"),
        include_str!("recovery_tests/intentional_wrong_field.rs"),
        include_str!("../prelude.rs"),
        include_str!("declaration.rs"),
        include_str!("declaration/functions.rs"),
        include_str!("declaration/old.rs"),
        include_str!("declaration/functions/old.rs"),
        include_str!("type_table.rs"),
        include_str!("type_table/native_containers.rs"),
        include_str!("type_table/old.rs"),
        include_str!("constructor_tests.rs"),
        include_str!("constructor_tests/corpus.rs")
    );
    assert!(source.contains("entry_variables = variables.clone()"));
    assert!(source.contains("complete old field index"));
    assert!(source.contains("DECLARATIONS.get_or_init(owned_declarations)"));
    assert!(source.contains("fn original_initializer_reference"));
    for part in [
        include_str!("declaration/functions/old.rs"),
        include_str!("declaration/old.rs"),
        include_str!("type_table/old.rs"),
        include_str!("constructor_tests/corpus.rs"),
        include_str!("recovery_tests/fixture_controls.rs"),
        include_str!("recovery_tests/refusal_dependency.rs"),
        include_str!("recovery_tests/boundaries.rs"),
    ] {
        assert!(part.lines().count() <= 1500);
    }
}
