//! Issue #191: hostile compile-time proof that `std.auth.identity.Identity`
//! and `std.auth.authorization.Authorization` (declared in
//! `std/auth/src/auth.spx`) are distinct nominal types the compiler enforces,
//! not merely a convention two `bool`-returning functions happen to follow.
//! These are checked directly against `crate::check`, the same
//! parse/resolve/type-check/verify pipeline `semaprax check` drives, on small
//! self-contained fixtures that mirror `auth.spx`'s own declarations — not
//! against the real package, since a hostile case that must fail to compile
//! cannot live inside a package whose own test suite must build cleanly (see
//! `auth.spx`'s `Identity`/`Authorization` doc comment, point 5, and
//! `secret_source_tests.rs`'s identical rationale for `Secret<T>`).
//!
//! The property that matters most is proven here, not merely documented:
//! issue #191's acceptance mapping asks for "authentication and
//! authorization" as "separate typed concepts", graded "not met as a
//! distinct nominal type the compiler enforces one cannot smuggle past" —
//! i.e. a value proving *who you are* must not be usable where a value
//! proving *what you may do* is required, and vice versa. Every hostile case
//! below is paired with a positive control proving the same fixture compiles
//! when the argument's declared type actually matches the parameter, so a
//! regression that collapsed the two types back into interchangeable shapes
//! (or back into a bare `bool`) would flip the negative case from `Err` to
//! `Ok`, not merely fail to reject.
//!
//! **Correction, caught by this file's own suite**: an earlier version of
//! this module additionally claimed that calling a function typed over
//! `Identity` fails on the interpreter with `SPX-F102`, generalizing
//! `auth.spx`'s `Secret<T>` doc comment (point 4) from "a user *generic*
//! record" to "any record or class with no `Bytes` field". That broader claim
//! was wrong: `calling_a_function_typed_over_identity_executes_on_the_
//! retained_call_interpreter` below (the corrected, inverted form of that
//! same test) shows `crate::interpreter::retained_call::evaluate_retained_call`
//! returns `Ok` for exactly that call. The mechanism, read from
//! `src/interpreter/retained_call/copy_records.rs`: the retained-call seam
//! (used by this test, and distinct from the plain `admitted_resolved_
//! functions` path `semaprax run`/`semaprax test` use) has a `flat_copy_
//! record` extension admitting a **non-generic** record with only closed
//! scalar leaves across a function boundary. `Secret<T>` is declared
//! generic, so it is excluded by that same extension's `record.type_
//! parameters.is_empty()` check —
//! `calling_a_function_typed_over_generic_secret_is_rejected_by_the_
//! retained_call_interpreter` below confirms `SPX-F102` still fires for
//! `Secret<i64>` on this identical seam. So "generic" was the right
//! discriminator for this seam all along; the error was generalizing a
//! true, narrow, `Secret<T>`-specific claim into a broader one about "any
//! record" without checking a non-generic record against this specific
//! execution path first.

fn identity_authorization_fixture() -> &'static str {
    r#"module test.identity_authorization_source;

@id("test.identity")
record Identity {
    @id("test.identity.subject_id")
    subject_id: usize,
}

@id("test.authorization")
record Authorization {
    @id("test.authorization.permitted")
    permitted: bool,
}

@id("test.requires_identity")
fn requires_identity(who: Identity) -> usize
{
    who.subject_id
}

@id("test.requires_authorization")
fn requires_authorization(decision: Authorization) -> bool
{
    decision.permitted
}

"#
}

fn fixture_with_main(body: &str) -> String {
    format!(
        "{}@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n",
        identity_authorization_fixture()
    )
}

fn has_code(diagnostics: &[crate::diagnostic::Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

/// Non-vacuity control: an `Identity` value really is accepted where an
/// `Identity` is required, so the negative case below is a real refusal of a
/// specific *wrong* type, not an unrelated failure that would reject any
/// argument at all.
#[test]
fn requires_identity_accepts_an_identity_value() {
    let source = fixture_with_main(
        r#"    let who = Identity { subject_id: 1usize };
    if requires_identity(who) == 1usize { 0 } else { 1 }"#,
    );
    assert!(
        crate::check(&source, "identity-accepts-identity.spx").is_ok(),
        "an Identity value must be accepted where Identity is required"
    );
}

/// The property that matters most, direction one: an `Authorization` value —
/// proof of *what you may do* — must not be usable where an `Identity` —
/// proof of *who you are* — is required. If this ever started compiling, the
/// nominal distinction would have collapsed back to interchangeable shapes.
#[test]
fn authorization_cannot_be_used_where_identity_is_required() {
    let source = fixture_with_main(
        r#"    let decision = Authorization { permitted: true };
    if requires_identity(decision) == 1usize { 0 } else { 1 }"#,
    );
    let diagnostics = crate::check(&source, "authorization-as-identity.spx")
        .err()
        .expect("passing an Authorization value where Identity is required must fail to compile");
    assert!(
        has_code(&diagnostics, "SPX-T205"),
        "expected SPX-T205 (argument type mismatch), got {diagnostics:?}"
    );
}

/// Non-vacuity control for the other direction.
#[test]
fn requires_authorization_accepts_an_authorization_value() {
    let source = fixture_with_main(
        r#"    let decision = Authorization { permitted: true };
    if requires_authorization(decision) { 0 } else { 1 }"#,
    );
    assert!(
        crate::check(&source, "authorization-accepts-authorization.spx").is_ok(),
        "an Authorization value must be accepted where Authorization is required"
    );
}

/// The property that matters most, direction two: an `Identity` value must
/// not be usable where an `Authorization` value is required — the exact
/// mirror image of the case above, since a one-directional check alone would
/// leave the types distinguishable in name only from one side.
#[test]
fn identity_cannot_be_used_where_authorization_is_required() {
    let source = fixture_with_main(
        r#"    let who = Identity { subject_id: 1usize };
    if requires_authorization(who) { 0 } else { 1 }"#,
    );
    let diagnostics = crate::check(&source, "identity-as-authorization.spx")
        .err()
        .expect("passing an Identity value where Authorization is required must fail to compile");
    assert!(
        has_code(&diagnostics, "SPX-T205"),
        "expected SPX-T205 (argument type mismatch), got {diagnostics:?}"
    );
}

/// `Identity`/`Authorization` inherit the same whole-value-equality refusal
/// `Secret<T>` proves in `secret_source_tests.rs` — a general `Type::Named`
/// invariant (`SPX-T207`), not a mechanism either type declares for itself.
/// Paired with the field-comparison positive control, exactly like the
/// `Secret<T>` proof, so a regression that started admitting aggregate
/// equality again would flip this from `Err` to `Ok`, not merely stop being
/// asserted.
#[test]
fn identity_field_comparison_compiles_but_whole_value_equality_is_rejected() {
    let field_source = fixture_with_main(
        r#"    let a = Identity { subject_id: 1usize };
    let b = Identity { subject_id: 1usize };
    if a.subject_id == b.subject_id { 0 } else { 1 }"#,
    );
    assert!(
        crate::check(&field_source, "identity-field-comparison.spx").is_ok(),
        "unwrapped scalar-field comparison must compile"
    );

    let whole_value_source = fixture_with_main(
        r#"    let a = Identity { subject_id: 1usize };
    let b = Identity { subject_id: 1usize };
    if a == b { 0 } else { 1 }"#,
    );
    let diagnostics = crate::check(&whole_value_source, "identity-whole-value-eq.spx")
        .err()
        .expect("comparing two Identity values with `==` must fail to compile");
    assert!(
        has_code(&diagnostics, "SPX-T207"),
        "expected SPX-T207 (aggregate equality outside the executable comparison \
         profile), got {diagnostics:?}"
    );
}

/// The interpreter-backend ceiling `auth.spx`'s `Identity`/`Authorization`
/// doc comment (point 4) claimed this fails with `SPX-F102` for *any* user
/// record, generic or not — that claim was wrong, caught by this very test
/// (see the correction note above the module doc and in `auth.spx`).
/// `src/interpreter/retained_call.rs`'s `copy_records::admitted_functions`
/// (`src/interpreter/retained_call/copy_records.rs`'s `flat_copy_record`)
/// is a retained-call-only admission extension for exactly this shape: a
/// **non-generic** record (`record.type_parameters.is_empty()` at the
/// declaration, `arguments.is_empty()` at the use site) whose fields are all
/// closed scalar leaves. `Identity`/`Authorization` both qualify, so a call
/// across a function boundary typed over either executes cleanly through
/// `crate::interpreter::retained_call`. `semaprax check` already proved the
/// call is statically well-typed; this proves the interpreter (this
/// specific execution seam) actually admits and executes it, not merely
/// that it type-checks.
#[test]
fn calling_a_function_typed_over_identity_executes_on_the_retained_call_interpreter() {
    let module = fixture_with_main(
        r#"    let who = Identity { subject_id: 1usize };
    if requires_identity(who) == 1usize { 0 } else { 1 }"#,
    );
    let program = crate::hir::resolve(
        &crate::parse(
            &module,
            std::path::Path::new("identity-interpreter-admits.spx"),
        )
        .expect("the fixture parses"),
    )
    .expect("the fixture resolves");
    let prepared = crate::interpreter::retained_call::prepare_retained_call(&program, "app.main")
        .expect("the entry point prepares");
    let evaluation =
        crate::interpreter::retained_call::evaluate_retained_call(&program, &prepared, &[], 10_000)
            .expect(
                "a call across a function boundary typed over a plain, non-generic record \
                 (Identity) must execute on the retained-call interpreter seam — if this starts \
                 failing, `copy_records::flat_copy_record`'s admission has narrowed and this \
                 test, auth.spx, and AUTHENTICATION-SESSIONS-V1.md all need re-checking together",
            );
    // Non-vacuity: the call must have actually run `requires_identity` and
    // taken the `== 1usize` branch, not merely returned *some* `Ok`.
    assert_eq!(
        evaluation.outcome,
        crate::interpreter::retained_call::RetainedCallOutcome::Returned(
            crate::interpreter::retained_call::RetainedValue::I64(0)
        ),
        "expected main to return 0 (requires_identity(who) == 1usize), got {:?}",
        evaluation.outcome
    );
}

/// Concrete Copy substitution crosses the retained-call boundary. Generic
/// record admission does not widen the frozen direct-String call signature.
#[test]
fn calling_a_function_typed_over_generic_secret_executes_and_retains_string_refusal() {
    let module = r#"module test.generic_secret_interpreter_ceiling;

@id("test.secret")
record Secret<T> {
    @id("test.secret.value")
    value: T,
}

@id("test.requires_secret")
fn requires_secret(held: Secret<i64>) -> i64
{
    held.value
}

@id("app.main")
fn main() -> i64
{
    let held = Secret<i64> { value: 1 };
    if requires_secret(held) == 1 { 0 } else { 1 }
}
"#;
    let program = crate::hir::resolve(
        &crate::parse(
            module,
            std::path::Path::new("generic-secret-interpreter-ceiling.spx"),
        )
        .expect("the fixture parses"),
    )
    .expect("the fixture resolves");
    crate::hir::validate(&program).expect("the generic Copy fixture validates");
    let prepared = crate::interpreter::retained_call::prepare_retained_call(&program, "app.main")
        .expect("the concrete generic Copy call prepares");
    let evaluation =
        crate::interpreter::retained_call::evaluate_retained_call(&program, &prepared, &[], 10_000)
            .expect("the original generic Secret<i64> call executes");
    assert_eq!(
        evaluation.outcome,
        crate::interpreter::retained_call::RetainedCallOutcome::Returned(
            crate::interpreter::retained_call::RetainedValue::I64(0)
        )
    );
    // Keep the same generic carrier and call boundary, but a direct String
    // result remains outside the ordinary retained-call interpreter profile.
    let refused = module
        .replace(
            "-> i64\n{\n    held.value",
            "-> String\n{\n    \"withheld\"",
        )
        .replace(
            "if requires_secret(held) == 1 { 0 } else { 1 }",
            "let text = requires_secret(held); string_len(text)",
        );
    assert_ne!(refused, module);
    let refused = crate::hir::resolve(
        &crate::parse(
            &refused,
            std::path::Path::new("generic-secret-frozen-string-boundary.spx"),
        )
        .unwrap(),
    )
    .unwrap();
    crate::hir::validate(&refused).unwrap();
    let diagnostics =
        crate::interpreter::retained_call::prepare_retained_call(&refused, "app.main")
            .err()
            .expect("direct String signatures retain ordinary F102 refusal");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-F102");
    assert_eq!(
        diagnostics[0].message,
        "interpreter admission failed (unsupported_callee)"
    );
}
