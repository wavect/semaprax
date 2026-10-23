//! Ordinary native receiving controls; no compiler subprocess or C execution.
use super::*;
use std::path::Path;

fn resolve(body: &str) -> ResolvedProgram {
    let source = format!("module test.native_validation_reuse;\n{body}");
    let parsed = crate::parse(&source, Path::new("native-validation-reuse.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn emit(
    program: &ResolvedProgram,
    old: bool,
    profile: NativeOutputProfile,
) -> Result<String, Diagnostic> {
    let labels = HashMap::new();
    if old {
        before::emit_hir_c_with_receiving(program, &labels, profile, None, None, None, None, None)
    } else {
        emit_hir_c_with_receiving(program, &labels, profile, None, None, None, None, None)
    }
}

#[test]
fn ordinary_route_matches_complete_previous_emission_and_removes_one_validation() {
    for body in [
        "@id(\"app.main\") fn main() -> i64 { 42 }",
        "@id(\"app.main\") fn main() -> i64 { if true { 4 + 2 } else { 9 } }",
        "@id(\"app.helper\") fn helper(x: i64) -> i64 { x + 1 }\n@id(\"app.main\") fn main() -> i64 { helper(41) }",
    ] {
        let program = resolve(body);
        native_resource::take_ordinary_validation_count();
        let expected = emit(&program, true, NativeOutputProfile::Legacy).unwrap();
        assert_eq!(native_resource::take_ordinary_validation_count(), 2);
        let actual = emit(&program, false, NativeOutputProfile::Legacy).unwrap();
        assert_eq!(native_resource::take_ordinary_validation_count(), 1);
        assert_eq!(actual, expected);
    }
    // A single shared reference, retained only from the original validation
    // point to ABI construction. Option uses the reference's null niche.
    assert_eq!(
        std::mem::size_of::<native_resource::OrdinaryValidatedProgram<'_>>(),
        std::mem::size_of::<&ResolvedProgram>()
    );
    assert_eq!(
        std::mem::size_of::<Option<native_resource::OrdinaryValidatedProgram<'_>>>(),
        std::mem::size_of::<&ResolvedProgram>()
    );
    assert!(!std::mem::needs_drop::<
        native_resource::OrdinaryValidatedProgram<'_>,
    >());
}

#[test]
fn hostile_hir_and_profile_diagnostics_keep_original_precedence() {
    let valid = resolve("@id(\"app.main\") fn main() -> i64 { 42 }");
    let mut hostile = valid.clone();
    hostile.entrypoint = DeclarationId::new("missing.entry");
    for profile in [
        NativeOutputProfile::Legacy,
        NativeOutputProfile::SelectedFinalUtf8,
    ] {
        let expected = emit(&hostile, true, profile).unwrap_err();
        assert_eq!(emit(&hostile, false, profile).unwrap_err(), expected);
        // The raw ABI builder still independently refuses the hostile subject.
        assert_eq!(
            native_resource::build_resource_abi(&hostile).unwrap_err(),
            expected
        );
    }
    for profile in [
        NativeOutputProfile::SelectedFinalUtf8,
        NativeOutputProfile::SelectedPartition,
        NativeOutputProfile::CheckedAllocation,
    ] {
        assert_eq!(
            emit(&valid, false, profile).unwrap_err(),
            emit(&valid, true, profile).unwrap_err()
        );
    }
}

#[test]
fn initial_h006_refusal_is_unchanged_and_never_reaches_abi() {
    let program = resolve("@id(\"app.main\") fn main() -> i64 { 42 }");
    native_resource::take_ordinary_validation_count();
    let (old, _) =
        crate::bounded_output::with_limit(0, || emit(&program, true, NativeOutputProfile::Legacy));
    let old = old.unwrap_err();
    assert_eq!(old.code, "SPX-H006");
    assert_eq!(native_resource::take_ordinary_validation_count(), 1);
    let (current, _) =
        crate::bounded_output::with_limit(0, || emit(&program, false, NativeOutputProfile::Legacy));
    assert_eq!(current.unwrap_err(), old);
    assert_eq!(native_resource::take_ordinary_validation_count(), 1);
}

#[test]
fn active_budget_removes_exactly_the_duplicate_validator_debits() {
    let program = resolve("@id(\"app.main\") fn main() -> i64 { 42 }");
    let limit = 64 * 1024 * 1024;
    let (validation, refused, validation_bytes) =
        crate::bounded_output::with_limit_usage(limit, || hir::validate(&program));
    validation.unwrap();
    assert!(!refused);
    let (old, refused, old_bytes) = crate::bounded_output::with_limit_usage(limit, || {
        emit(&program, true, NativeOutputProfile::Legacy)
    });
    let old = old.unwrap();
    assert!(!refused);
    let (current, refused, current_bytes) = crate::bounded_output::with_limit_usage(limit, || {
        emit(&program, false, NativeOutputProfile::Legacy)
    });
    assert_eq!(current.unwrap(), old);
    assert!(!refused);
    assert!(validation_bytes > 0);
    assert_eq!(old_bytes.checked_sub(current_bytes), Some(validation_bytes));
}

#[test]
fn ordinary_receivers_replay_loan_inventory_cleanup_status_and_identity() {
    // Genuine projected-Bytes fixture from hir/projected_byte_field_provenance_tests.rs,
    // plus an ordinary propagated scalar call as in hir/record_tests.rs controls.
    let original = resolve(
        r#"
@id("packet") record Packet {
    @id("packet.payload") payload: Bytes,
    @id("packet.sibling") sibling: Bytes,
}
@id("packet.view") fn view(packet: own Packet) -> usize {
    let direct = bytes_as_slice(packet.payload);
    let alias = direct;
    let range = byte_range(alias, 0usize, byte_len(alias));
    byte_len(range)
}
@id("app.helper") fn helper(x: i64) -> i64 { x + 1 }
@id("app.main") fn main() -> i64 { helper(41) }
"#,
    );
    hir::validate(&original).unwrap();
    let view = original
        .functions
        .iter()
        .position(|f| f.id.as_str() == "packet.view")
        .unwrap();
    let main = original
        .functions
        .iter()
        .position(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert!(!original.functions[view].loan_plan.loans.is_empty());
    assert!(!original.functions[view].cleanup.slots.is_empty());
    assert!(!original.functions[view].cleanup_plan.slots.is_empty());
    assert!(!original.functions[main]
        .cleanup_plan
        .status_sources
        .is_empty());
    let mut hostile = original.clone();
    hostile.functions[view].loan_plan.loans.clear();
    assert_hostile_receivers(&hostile);
    let mut hostile = original.clone();
    hostile.functions[view].cleanup.slots.clear();
    assert_hostile_receivers(&hostile);
    let mut hostile = original.clone();
    hostile.functions[view].cleanup_plan.slots.clear();
    assert_hostile_receivers(&hostile);
    let mut hostile = original.clone();
    hostile.functions[main].cleanup_plan.status_sources.clear();
    assert_hostile_receivers(&hostile);
    let mut hostile = original;
    hostile.functions[view].id = DeclarationId::new("packet.view\0forged");
    assert_hostile_receivers(&hostile);
}

fn assert_hostile_receivers(program: &ResolvedProgram) {
    let expected = hir::validate(program).unwrap_err();
    assert_eq!(
        native_resource::build_resource_abi(program).unwrap_err(),
        expected
    );
    let receipt_error = match native_resource::OrdinaryValidatedProgram::validate(program) {
        Ok(_) => panic!("hostile proof attachment received an ordinary receipt"),
        Err(error) => error,
    };
    assert_eq!(receipt_error, expected);
    for profile in [
        NativeOutputProfile::Legacy,
        NativeOutputProfile::SelectedFinalUtf8,
    ] {
        assert_eq!(emit(program, true, profile).unwrap_err(), expected);
        assert_eq!(emit(program, false, profile).unwrap_err(), expected);
    }
    assert_eq!(crate::codegen::emit_hir_c(program).unwrap_err(), expected);
}

#[test]
fn inherited_ordinary_provider_receipt_refuses_foreign_programs_and_selected_profiles() {
    let program = resolve("@id(\"app.main\") fn main() -> i64 { 42 }");
    let foreign = resolve("@id(\"app.main\") fn main() -> i64 { 43 }");
    for (subject, profile) in [
        (&foreign, NativeOutputProfile::OwnedDataProvider),
        (&program, NativeOutputProfile::Legacy),
        (&program, NativeOutputProfile::SelectedFinalUtf8),
        (&program, NativeOutputProfile::SelectedPartition),
        (&program, NativeOutputProfile::CheckedAllocation),
    ] {
        let receipt = native_resource::OrdinaryValidatedProgram::validate(&program)
            .unwrap_or_else(|error| panic!("{error:?}"));
        let error = emit_hir_c_with_validation(
            subject,
            &HashMap::new(),
            profile,
            None,
            None,
            None,
            None,
            None,
            Some(receipt),
        )
        .unwrap_err();
        assert_eq!(
            error,
            backend_error("ordinary provider validation subject or profile is not current")
        );
    }
}
