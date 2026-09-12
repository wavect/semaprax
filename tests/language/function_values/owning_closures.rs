//! SPX-AI-021 bounded owning-capture closure: `own fn() -> R { body }`.
//!
//! This is a reviewed bounded *source-level* profile: it is fully admitted
//! and diagnosed by `source_verify` (including the compile-time one-shot
//! diagnostic this module exists to prove), but HIR resolution refuses it
//! with a stable diagnostic pending independent review -- see
//! `docs/CLOSURES-OWNING-V1.md` for the exact seam and what remains. These
//! tests therefore check `semaprax::check` (source verification) directly,
//! and separately prove the HIR refusal fires exactly there.

use semaprax::diagnostic::Diagnostic;
use semaprax::hir;

/// `checksum` ignores its payload's content (this profile's body grammar
/// admits only a bare transferring call, not byte inspection) but is a
/// genuine one-`own Bytes`-parameter function that legitimately consumes
/// and drops it, which is all this bounded profile requires of a target.
const TARGET: &str = r#"
@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}
"#;

fn source(body: &str) -> String {
    format!("module test.owning_closures;\n{TARGET}\n@id(\"owning.main\") fn main() -> i64 {{\n{body}\n}}\n")
}

fn check_errors(body: &str) -> Vec<Diagnostic> {
    match semaprax::check(&source(body), "owning-closures.spx") {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn error_codes(body: &str) -> Vec<&'static str> {
    check_errors(body)
        .iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn own_fn_literal_round_trips_through_canonical_formatting() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
"#;
    let program = semaprax::check(&source(body), "owning-closures.spx").unwrap();
    let canonical = semaprax::format::canonical(&program);
    assert!(
        canonical.contains("own fn() -> i64 { checksum(payload) }"),
        "canonical output must retain the authored `own fn` syntax verbatim, got:\n{canonical}"
    );
    let reparsed = semaprax::check(&canonical, "owning-closures.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
}

#[test]
fn constructing_the_closure_moves_the_captured_payload_exactly_once() {
    // The capture is used again after `own fn` already moved it: this must
    // fail at the *second* use (the reuse), not merely fail *somewhere*.
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    checksum(payload)
"#;
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-O101"),
        "reusing the captured payload after construction must report SPX-O101 (use after move), got {codes:?}"
    );
}

#[test]
fn calling_an_owning_closure_once_reports_no_diagnostics() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
"#;
    let codes = error_codes(body);
    assert!(
        codes.is_empty(),
        "a single call to a freshly constructed owning closure must be clean, got {codes:?}"
    );
}

#[test]
fn calling_an_owning_closure_twice_fails_at_the_second_call_not_the_first() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    let first = clo();
    let second = clo();
    first + second
"#;
    // Isolate exactly where SPX-O101 fires: replaying with only the first
    // call present must be clean (proving the diagnostic is not a
    // consequence of anything earlier in the fixture), and the two-call
    // fixture must fail with precisely one SPX-O101 (the second call).
    let single_call_body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    let first = clo();
    first
"#;
    assert!(
        error_codes(single_call_body).is_empty(),
        "the one-call baseline must itself be clean"
    );
    let codes = error_codes(body);
    let moved_count = codes.iter().filter(|code| **code == "SPX-O101").count();
    assert_eq!(
        moved_count, 1,
        "a second call must report exactly one SPX-O101 (use after move), got {codes:?}"
    );
}

#[test]
fn dropping_an_uncalled_owning_closure_reports_no_diagnostics() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    0
"#;
    let codes = error_codes(body);
    assert!(
        codes.is_empty(),
        "constructing an owning closure and never calling it must be clean (settles via ordinary scope-exit drop), got {codes:?}"
    );
}

#[test]
fn an_owning_closure_cannot_be_aliased_into_another_binding() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    let alias = clo;
    alias()
"#;
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-T296"),
        "reading an owning closure as a plain value (aliasing it) must be rejected, got {codes:?}"
    );
}

#[test]
fn an_owning_closure_cannot_be_passed_as_an_ordinary_argument() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    identity_len(clo)
"#;
    // `identity_len` need not exist: the escaping read is rejected before
    // argument/name resolution would even matter.
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-T296"),
        "passing an owning closure as an ordinary argument must be rejected as an escaping read, got {codes:?}"
    );
}

#[test]
fn a_target_with_the_wrong_signature_is_rejected() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { byte_len(bytes_as_slice(payload)) };
    clo()
"#;
    // The body is not a single call to a declared one-`own Bytes`-parameter
    // function transferring the capture: it is a byte operation directly
    // (and, incidentally, a `usize` value where `i64` is declared).
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-T292"),
        "a body that is not exactly one call transferring the capture must be rejected, got {codes:?}"
    );
}

#[test]
fn a_declared_target_with_a_mismatched_arity_is_rejected() {
    let source_text = r#"module test.owning_closures_bad_target;
@id("owning.two_params") fn two_params(payload: own Bytes, extra: i64) -> i64 {
    extra
}
@id("owning.main") fn main() -> i64 {
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { two_params(payload) };
    clo()
}
"#;
    let codes = match semaprax::check(source_text, "owning-closures-bad-target.spx") {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
    };
    assert!(
        codes.contains(&"SPX-T293"),
        "a target with the wrong arity must be rejected before an owning closure ever admits it, got {codes:?}"
    );
}

#[test]
fn capturing_a_non_bytes_value_is_rejected() {
    let body = r#"
    let scalar = 4;
    let clo = own fn() -> i64 { checksum(scalar) };
    clo()
"#;
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-T294"),
        "capturing a non-`Bytes` value must be rejected before an owning closure ever admits it, got {codes:?}"
    );
}

#[test]
fn owning_closures_are_not_admitted_inside_a_generic_function() {
    let source_text = r#"module test.owning_closures_generic;
@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}
@id("owning.generic") fn generic<T>(value: T) -> i64 {
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
}
@id("owning.main") fn main() -> i64 {
    generic<i64>(1)
}
"#;
    let codes = match semaprax::check(source_text, "owning-closures-generic.spx") {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
    };
    assert!(
        codes.contains(&"SPX-T291"),
        "an owning closure inside a generic function must be rejected, got {codes:?}"
    );
}

#[test]
fn calling_an_owning_closure_with_an_explicit_argument_is_rejected() {
    // The bounded profile's call site admits zero explicit arguments. Use a
    // literal (not `payload`) so the failure is isolated to the argument
    // count, not conflated with the unrelated use-after-move diagnostic a
    // second reference to the already-moved capture would also report.
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo(1)
"#;
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-T295"),
        "calling an owning closure with an explicit argument must be rejected, got {codes:?}"
    );
    assert!(
        !codes.contains(&"SPX-O101"),
        "the extra-argument rejection must not be conflated with use-after-move, got {codes:?}"
    );
    assert!(
        !codes.contains(&"SPX-T296"),
        "the extra-argument rejection must not be conflated with the escaping-read diagnostic, got {codes:?}"
    );
}

#[test]
fn constructing_an_owning_closure_over_a_maybe_moved_capture_is_rejected() {
    // `check_construction`'s `Availability::MaybeMoved` arm (a capture moved
    // on only one control-flow branch, not every branch) is a distinct
    // diagnostic (`SPX-O107`) from the definite-move case covered above
    // (`SPX-O101`), and no test exercised it: every existing case either
    // moves the capture on every path or not at all. `flag` is an ordinary
    // parameter (not a literal) so the join is genuinely conditional to the
    // checker, matching the pattern used throughout
    // `tests/language/ownership_control_flow.rs`.
    let source_text = r#"module test.owning_closures_maybe_moved;
@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}
@id("owning.branch") fn branch(flag: bool) -> i64 {
    let payload = bytes_zeroed(4usize);
    let chosen = if flag { checksum(payload) } else { 0 };
    let clo = own fn() -> i64 { checksum(payload) };
    clo() + chosen
}
@id("owning.main") fn main() -> i64 { branch(true) }
"#;
    let codes = match semaprax::check(source_text, "owning-closures-maybe-moved.spx") {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
    };
    assert!(
        codes.contains(&"SPX-O107"),
        "constructing an owning closure over a conditionally-moved capture must be rejected, got {codes:?}"
    );
    assert!(
        !codes.contains(&"SPX-O101"),
        "a conditional (not definite) prior move must not be reported as the definite-move code, got {codes:?}"
    );
}

#[test]
fn calling_an_owning_closure_that_was_maybe_moved_on_one_branch_is_rejected() {
    // The mirror case at the *call* site: `check_call`'s own
    // `Availability::MaybeMoved` arm, hit when the closure binding itself
    // (not its capture) was consumed on only one branch of a prior
    // conditional. Distinct code path from the two-call `SPX-O101` case
    // above, and from the construction-time `SPX-O107` case just above.
    let source_text = r#"module test.owning_closures_call_maybe_moved;
@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}
@id("owning.branch") fn branch(flag: bool) -> i64 {
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    let chosen = if flag { clo() } else { 0 };
    let second = clo();
    chosen + second
}
@id("owning.main") fn main() -> i64 { branch(true) }
"#;
    let codes = match semaprax::check(source_text, "owning-closures-call-maybe-moved.spx") {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
    };
    assert!(
        codes.contains(&"SPX-O107"),
        "calling an owning closure conditionally consumed on one branch must be rejected, got {codes:?}"
    );
    assert!(
        !codes.contains(&"SPX-O101"),
        "a conditional (not definite) prior call must not be reported as the definite-move code, got {codes:?}"
    );
}

#[test]
fn constructing_an_owning_closure_over_a_capture_with_a_live_byte_view_is_rejected() {
    // `check_construction` has its own `has_active_overlapping_loan` check
    // (mirroring, but distinct from, the general call-argument check in
    // `arguments.rs`), and no test drove it through the owning-closure
    // path specifically. Pattern mirrors
    // `a_frozen_buffer_has_exactly_one_owner_and_no_stale_view` in
    // `tests/language/owned_byte_buffer_v1.rs`: the view must be used after
    // the transferring point, or the loan would already be released as
    // dead before the check runs.
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let view = bytes_as_slice(payload);
    let clo = own fn() -> i64 { checksum(payload) };
    if byte_len(view) == 4usize { 0 } else { 1 }
"#;
    let codes = error_codes(body);
    assert!(
        codes.contains(&"SPX-T265"),
        "constructing an owning closure that transfers a capture with a live lexical byte view must be rejected, got {codes:?}"
    );
}

#[test]
fn an_owning_closure_with_an_explicit_parameter_is_rejected_by_the_parser() {
    // This bounded profile fixes the parameter list at zero: `own fn(...)`
    // with any explicit parameter must fail to parse with a dedicated code,
    // distinct from every source_verify-level rejection above.
    let body = r#"
    let clo = own fn(payload: own Bytes) -> i64 { checksum(payload) };
    clo()
"#;
    let codes = error_codes(body);
    assert_eq!(
        codes,
        vec!["SPX-P130"],
        "an explicit closure parameter must be rejected by the parser with exactly SPX-P130, got {codes:?}"
    );
}

#[test]
fn owning_closures_are_not_admitted_inside_a_contract_expression() {
    // Contract expressions (`requires`/`ensures`) run with move-tracking
    // disabled (`allow_moves = false`, the same restriction that already
    // rejects consuming a resource in a contract with SPX-O105): a distinct
    // code, SPX-O119, covers constructing an owning closure there. A
    // multi-statement contract clause is written as a doubled block
    // (`ensures {{ ... }}`) elsewhere in this test suite (see
    // `tests/language/while_loops.rs`'s `while_in_contract_expression_is_spx_t253`);
    // this is a fresh top-level program, not `check_errors`'s `main`-wrapped
    // body helper, since a `module`/`fn` declaration cannot nest inside
    // another function.
    let source_text = r#"module test.owning_closures_contract;
@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}
@id("owning.guarded") fn guarded(payload: own Bytes) -> i64
requires {{
    let clo = own fn() -> i64 { checksum(payload) };
    clo() == 42
}}
{ 42 }
@id("owning.main") fn main() -> i64 { guarded(bytes_zeroed(4usize)) }
"#;
    let diagnostics = match semaprax::check(source_text, "owning-closures-contract.spx") {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };
    let codes: Vec<&str> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert!(
        codes.contains(&"SPX-O119"),
        "constructing an owning closure inside a contract expression must be rejected, got {codes:?}"
    );
    assert!(
        !codes.contains(&"SPX-O105"),
        "the contract-construction rejection must use its own code, not the resource-consumption one, got {codes:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-O119"
                && diagnostic.message.contains("contract expression")),
        "the SPX-O119 diagnostic must name the contract-expression restriction, got {diagnostics:?}"
    );
}

#[test]
fn hir_resolution_refuses_an_otherwise_source_clean_owning_closure() {
    // Confirms the exact seam: source verification fully admits and checks
    // this program (proving the compile-time one-shot diagnostics above are
    // real source-level checks, not masked by an earlier failure), but HIR
    // resolution -- which every backend is built from -- refuses it with a
    // stable, distinct message. No backend (interpreter, native, Wasm) can
    // therefore ever observe a partially-lowered owning capture.
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
"#;
    let program = semaprax::check(&source(body), "owning-closures.spx")
        .expect("source verification must admit this program cleanly");
    let errors = hir::resolve(&program).expect_err("HIR resolution must refuse an owning closure");
    assert!(
        errors
            .iter()
            .any(|error| error.code == "SPX-H006"
                && error.message.contains("owning-capture closures")
                && error.message.contains("not yet lowered")),
        "the refusal must use the stable internal-shape diagnostic code and name this exact seam, got {errors:?}"
    );
}
