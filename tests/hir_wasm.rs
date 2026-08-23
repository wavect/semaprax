use std::path::Path;

use semaprax::hir::{self, DeclarationId, ResolvedExprKind, ResolvedStatement, ResolvedType};
use semaprax::{parse, wasm};

const SOURCE: &str = r#"
module test.hir_wasm;
@id("math.choose")
fn choose(flag: bool, value: i64) -> i64 {
    let adjusted = if flag { value + 1 } else { value - 1 };
    adjusted
}
@id("app.main")
fn main() -> i64
    ensures result == 42
{
    choose(true, 41)
}
"#;

#[test]
fn resolved_hir_is_the_wasm_lowering_contract() {
    let program = parse(SOURCE, Path::new("hir-wasm.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();

    let from_source = wasm::emit_module(&program).unwrap();
    let from_hir = wasm::emit_resolved_module(&resolved).unwrap();
    assert_eq!(from_hir, from_source);

    let mut renamed_metadata = resolved.clone();
    let helper = &mut renamed_metadata.functions[0];
    helper.params[0].name = "display_parameter".to_owned();
    if let ResolvedExprKind::Block { statements, .. } = &mut helper.body.kind {
        let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
            panic!("expected a resolved let statement")
        };
        binding.name = "display_local".to_owned();
    } else {
        panic!("expected a resolved block body");
    }
    assert_eq!(
        wasm::emit_resolved_module(&renamed_metadata).unwrap(),
        from_hir,
        "Wasm lowering must use declaration and value identities, not display names"
    );
}

#[test]
fn missing_hir_entrypoint_identity_is_rejected() {
    let program = parse(SOURCE, Path::new("hir-wasm.spx")).unwrap();
    let mut resolved = hir::resolve(&program).unwrap();
    resolved.entrypoint = DeclarationId::new("missing.entrypoint");

    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn unresolved_generic_hir_is_rejected_instead_of_guessed() {
    let program = parse(SOURCE, Path::new("hir-wasm.spx")).unwrap();
    let mut resolved = hir::resolve(&program).unwrap();
    resolved.functions[0].return_type = ResolvedType::TypeParameter {
        owner: DeclarationId::new("math.choose"),
        index: 0,
    };

    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}
