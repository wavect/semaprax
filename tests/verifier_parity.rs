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
{"code":"SPX-T105","severity":"error","message":"executable module must define `fn main() -> i64`","path":"fixtures/verifier-mixed.spx","location":{"line":8,"column":1,"start":134,"end":232},"help":null}"#
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
