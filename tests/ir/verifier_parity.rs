use std::path::Path;

use semaprax::{hir, parse, verify};

fn diagnostics_json(diagnostics: &[semaprax::diagnostic::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(semaprax::diagnostic::Diagnostic::json)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn mixed_source_diagnostics_are_an_exact_ordered_public_contract() {
    let source = r#"
module test.verifier_parity;
permit { clock.read }
resource Buffer {
    @id("test.verifier_parity.buffer.drop")
    drop trivial;
}
fn helper(value: Buffer, number: own i64) -> bool
    uses { network.send }
{
    missing(value)
}
"#;
    let program = parse(source, Path::new("fixtures/verifier-mixed.spx")).unwrap();
    let verified = verify::verify(&program);
    let analysis = hir::analyze(&program);
    let actual = diagnostics_json(&verified);

    assert_eq!(diagnostics_json(&analysis.diagnostics), actual);
    assert!(analysis.resolved.is_none());
    assert_eq!(
        diagnostics_json(&hir::resolve(&program).unwrap_err()),
        actual
    );
    assert!(!verified
        .iter()
        .any(|diagnostic| diagnostic.code.starts_with("SPX-H")));
    assert_eq!(
        actual,
        r#"{"code":"SPX-S108","severity":"warning","message":"resource `Buffer` has an automatic identity that changes when renamed","path":"fixtures/verifier-mixed.spx","location":{"line":4,"column":10,"start":61,"end":67},"help":"add @id(\"your.namespace.resource\") before the declaration"}
{"code":"SPX-S103","severity":"warning","message":"function `helper` has an automatic identity that changes when renamed","path":"fixtures/verifier-mixed.spx","location":{"line":8,"column":4,"start":137,"end":143},"help":"add @id(\"your.namespace.symbol\") before the declaration"}
{"code":"SPX-O001","severity":"error","message":"resource parameter `helper.value` needs `own`, `borrow`, or `shared`","path":"fixtures/verifier-mixed.spx","location":{"line":8,"column":11,"start":144,"end":149},"help":"use `value: own Buffer` to transfer ownership"}
{"code":"SPX-O002","severity":"error","message":"ownership mode `own` is only valid for resource types; `i64` is a value type","path":"fixtures/verifier-mixed.spx","location":{"line":8,"column":26,"start":159,"end":165},"help":null}
{"code":"SPX-T203","severity":"error","message":"unknown function `missing`","path":"fixtures/verifier-mixed.spx","location":{"line":11,"column":5,"start":216,"end":230},"help":null}
{"code":"SPX-E101","severity":"error","message":"function `helper` uses `network.send` but module `test.verifier_parity` does not permit it","path":"fixtures/verifier-mixed.spx","location":{"line":8,"column":1,"start":134,"end":232},"help":null}
{"code":"SPX-T105","severity":"error","message":"executable module must define `fn main() -> i64`","path":"fixtures/verifier-mixed.spx","location":{"line":8,"column":1,"start":134,"end":232},"help":"a module without `fn main() -> i64` is a library module: check it through the project that owns it with `semaprax check <project-dir>`, or add `main` to run this file alone"}"#
    );
}

#[test]
fn warnings_only_analysis_retains_diagnostics_and_resolves_hir() {
    let source = r#"
module test.verifier_warnings;
fn main() -> i64 { 42 }
"#;
    let program = parse(source, Path::new("fixtures/verifier-warnings.spx")).unwrap();
    let verified = verify::verify(&program);
    let analysis = hir::analyze(&program);
    let actual = diagnostics_json(&verified);

    assert_eq!(diagnostics_json(&analysis.diagnostics), actual);
    assert!(analysis.resolved.is_some());
    assert!(hir::resolve(&program).is_ok());
    assert_eq!(
        actual,
        r#"{"code":"SPX-S103","severity":"warning","message":"function `main` has an automatic identity that changes when renamed","path":"fixtures/verifier-warnings.spx","location":{"line":3,"column":4,"start":35,"end":39},"help":"add @id(\"your.namespace.symbol\") before the declaration"}"#
    );
}

fn ordered_errors(program: &semaprax::ast::Program) -> Vec<String> {
    verify::verify(program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect()
}

#[test]
fn iterative_recovery_preserves_child_parent_and_fallback_order() {
    let source = r#"
module test.verifier_recovery;

@id("test.zero")
fn zero() -> i64 { 0 }

@id("test.recover")
fn recover() -> i64 {
    let call = zero(missing_a, missing_b);
    let unary = -missing_unary;
    let projected = missing_base.field;
    missing_tail
}

@id("test.fallback")
fn fallback(value: Result<i64, bool>) -> i64 {
    let unwrapped = value?;
    missing_after_fallback
}

@id("test.branch")
fn branch() -> i64 {
    if 1 { missing_then } else { missing_else }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("fixtures/verifier-recovery.spx")).unwrap();
    let expected = vec![
        "SPX-T204: `zero` expects 0 arguments, received 2",
        "SPX-T202: unknown value `missing_a` in `recover`",
        "SPX-T202: unknown value `missing_b` in `recover`",
        "SPX-T202: unknown value `missing_unary` in `recover`",
        "SPX-T202: unknown value `missing_base` in `recover`",
        "SPX-T202: unknown value `missing_tail` in `recover`",
        "SPX-T218: function `fallback` must return the ordinary compiler-owned Result to propagate a Result with `?`",
        "SPX-T202: unknown value `missing_after_fallback` in `fallback`",
        "SPX-T210: `if` condition must be bool",
        "SPX-T202: unknown value `missing_then` in `branch`",
        "SPX-T202: unknown value `missing_else` in `branch`",
    ];

    assert_eq!(ordered_errors(&program), expected);
    let analysis = hir::analyze(&program);
    assert!(analysis.resolved.is_none());
    assert_eq!(
        analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn iterative_ownership_mutations_commit_only_at_the_frozen_scope_boundaries() {
    let source = r#"
module test.verifier_ownership_recovery;

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64 { 1 }

@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64 { 1 }

@id("test.zero")
fn zero() -> i64 { 0 }

@id("test.commit_after_none")
fn commit_after_none(buffer: own Buffer) -> i64 {
    let consumed = consume(buffer) + missing_rhs;
    inspect(buffer)
}

@id("test.unmatched_argument")
fn unmatched_argument(buffer: own Buffer) -> i64 {
    let ignored = zero(buffer);
    inspect(buffer)
}

@id("test.rejected_shadow")
fn rejected_shadow(buffer: own Buffer) -> i64 {
    let buffer = buffer;
    inspect(buffer)
}

@id("test.branch_join")
fn branch_join(flag: bool, buffer: own Buffer) -> i64 {
    let maybe = if flag { consume(buffer) } else { missing_else };
    inspect(buffer)
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(
        source,
        Path::new("fixtures/verifier-ownership-recovery.spx"),
    )
    .unwrap();
    let expected = vec![
        "SPX-T202: unknown value `missing_rhs` in `commit_after_none`",
        "SPX-O101: use of resource `buffer` after ownership was moved",
        "SPX-T204: `zero` expects 0 arguments, received 1",
        "SPX-T209: local binding `buffer` shadows an existing value",
        "SPX-T202: unknown value `missing_else` in `branch_join`",
        "SPX-O107: resource `buffer` may have been moved on another control-flow path",
    ];

    assert_eq!(ordered_errors(&program), expected);
    let analysis = hir::analyze(&program);
    assert!(analysis.resolved.is_none());
    assert_eq!(
        analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>(),
        expected
    );
}
