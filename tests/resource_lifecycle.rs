use std::path::Path;

use semaprax::ast::{ImportFailure, ResourceLifecycleKind, TypeDeclarationKind};
use semaprax::hir::{
    self, DeclarationKind, ResolvedImportFailure, ResolvedResourceDropKind,
    ResolvedTypeDeclarationKind,
};
use semaprax::{check, codegen, format, graph, parse, verify, wasm};

const IMPORTED: &str = r#"module test.lifecycle;

@id("io.file")
resource File {
    @id("io.file.drop")
    drop import "io.file.finalize";
}

@id("io.file.host")
interface FileHost
    permits { filesystem.handle.release }
{
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes file always;
}

@id("app.main")
fn main() -> i64
{
    42
}
"#;

fn codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("resource-lifecycle.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn lifecycle_and_interface_contract_round_trip_canonically() {
    let program = parse(IMPORTED, Path::new("resource-lifecycle.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), IMPORTED);
    let reparsed = parse(&format::canonical(&program), Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), IMPORTED);

    let TypeDeclarationKind::Resource { lifecycles } = &program.types[0].kind else {
        panic!("File must be a resource")
    };
    assert_eq!(lifecycles[0].stable_id.as_deref(), Some("io.file.drop"));
    assert!(matches!(
        &lifecycles[0].kind,
        ResourceLifecycleKind::Imported { import_key } if import_key == "io.file.finalize"
    ));
    assert_eq!(
        program.interfaces[0].imports[0].stable_id,
        "io.file.finalize"
    );
    assert_eq!(
        program.interfaces[0].imports[0].failure,
        ImportFailure::Infallible
    );
}

#[test]
fn trivial_lifecycle_is_explicit_noncopy_hir() {
    let source = r#"module test.trivial;

@id("token.type")
resource Token {
    @id("token.type.drop")
    drop trivial;
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let ast = parse(source, Path::new("trivial.spx")).unwrap();
    assert_eq!(format::canonical(&ast), source);
    let program = hir::resolve(&ast).unwrap();
    let ResolvedTypeDeclarationKind::Resource { drop } = &program.types[0].kind else {
        panic!("Token must be a resource")
    };
    assert!(matches!(drop.kind, ResolvedResourceDropKind::Trivial));
    let ty = hir::ResolvedType::Nominal {
        declaration: program.types[0].id.clone(),
        arguments: Vec::new(),
    };
    let facts = program.declarations.type_facts(&ty).unwrap();
    assert!(!facts.copy);
    assert!(facts.needs_drop);
}

#[test]
fn legacy_and_unidentified_lifecycles_have_stable_ownership_diagnostics() {
    let legacy = "module test.legacy; @id(\"token\") resource Token; @id(\"app.main\") fn main() -> i64 { 0 }";
    assert_eq!(codes(legacy), ["SPX-O112"]);

    let missing_id = "module test.missing; @id(\"token\") resource Token { drop trivial; } @id(\"app.main\") fn main() -> i64 { 0 }";
    assert_eq!(codes(missing_id), ["SPX-O113"]);

    let duplicate = "module test.duplicate; @id(\"token\") resource Token { @id(\"token.drop.one\") drop trivial; @id(\"token.drop.two\") drop trivial; } @id(\"app.main\") fn main() -> i64 { 0 }";
    assert_eq!(codes(duplicate), ["SPX-O113"]);
}

#[test]
fn interface_and_import_contracts_fail_closed() {
    let source = r#"
module test.invalid_interface;
@id("token")
resource Token { @id("token.drop") drop import "token.finalize"; }
interface Host permits { token.release } {
    @id("")
    import fn finalize(token: borrow Token) -> unit
        effects { outside.effect }
        failure status ""
        consumes missing always;
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let found = codes(source);
    assert!(found.contains(&"SPX-I403"));
    assert!(found.contains(&"SPX-I404"));
    assert!(found.contains(&"SPX-O113"));
}

#[test]
fn malformed_lifecycle_and_interface_grammar_has_stable_parser_codes() {
    let lifecycle = "module bad; @id(\"token\") resource Token { @id(\"token.drop\") drop maybe; } fn main() -> i64 { 0 }";
    assert_eq!(
        parse(lifecycle, Path::new("bad-lifecycle.spx"))
            .unwrap_err()
            .code,
        "SPX-P106"
    );
    let interface = "module bad; @id(\"host\") interface Host permits {} { @id(\"drop\") import fn drop_it(value: own Missing) -> unit effects {} failure infallible; } fn main() -> i64 { 0 }";
    assert_eq!(
        parse(interface, Path::new("bad-interface.spx"))
            .unwrap_err()
            .code,
        "SPX-P104"
    );
}

#[test]
fn declaration_only_lifecycle_module_fails_without_panicking() {
    let source = r#"module test.declarations_only;
@id("token")
resource Token { @id("token.drop") drop trivial; }
@id("token.host")
interface TokenHost permits {} {
    @id("token.close")
    import fn close(token: own Token) -> unit
        effects {}
        failure infallible
        consumes token always;
}
"#;
    assert_eq!(
        parse(source, Path::new("declarations-only.spx"))
            .unwrap_err()
            .code,
        "SPX-P101"
    );
    assert_eq!(
        check(source, "declarations-only.spx").unwrap_err()[0].code,
        "SPX-P101"
    );
    let mut program = parse(
        &format!("{source}@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"),
        Path::new("direct-ast-without-functions.spx"),
    )
    .unwrap();
    program.functions.clear();
    assert_eq!(verify::verify(&program)[0].code, "SPX-T105");
    assert_eq!(hir::resolve(&program).unwrap_err()[0].code, "SPX-T105");
    assert_eq!(graph::to_json(&program).unwrap_err()[0].code, "SPX-T105");
    assert_eq!(codegen::emit_c(&program).unwrap_err().code, "SPX-T105");
    assert_eq!(wasm::emit_module(&program).unwrap_err().code, "SPX-T105");
}

#[test]
fn wrong_resource_arity_mode_and_failure_cannot_be_automatic_drop() {
    let cases = [
        (
            "file: borrow File",
            "failure infallible",
            "consumes file always",
        ),
        (
            "file: own File, other: own File",
            "failure infallible",
            "consumes file always",
        ),
        (
            "file: own File",
            "failure status \"io.error.v1\"",
            "consumes file always",
        ),
        (
            "file: own Socket",
            "failure infallible",
            "consumes file always",
        ),
    ];
    for (params, failure, consumes) in cases {
        let source = format!(
            r#"
module test.bad_drop;
@id("io.file") resource File {{ @id("io.file.drop") drop import "io.file.finalize"; }}
@id("io.socket") resource Socket {{ @id("io.socket.drop") drop trivial; }}
@id("io.host") interface Host permits {{ io.release }} {{
    @id("io.file.finalize")
    import fn finalize({params}) -> unit
        effects {{ io.release }}
        {failure}
        {consumes};
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        assert!(codes(&source).contains(&"SPX-O113"));
    }
}

#[test]
fn imported_drop_effects_recurse_through_owned_records_only() {
    let source = r#"
module test.lifecycle_effects;
permit { filesystem.handle.release }
@id("io.file")
resource File { @id("io.file.drop") drop import "io.file.finalize"; }
@id("io.box")
record Box { @id("io.box.file") file: File, }
@id("io.file.host")
interface FileHost permits { filesystem.handle.release } {
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes file always;
}
@id("box.needs_drop")
fn needs_drop(value: own Box) -> i64 { 1 }
@id("box.borrowed")
fn borrowed(value: borrow Box) -> i64 { 1 }
@id("box.shared")
fn shared_view(value: shared Box) -> i64 { 1 }
@id("file.identity")
fn identity(value: own File) -> File uses { filesystem.handle.release } { value }
@id("file.call_return")
fn call_return(value: own File) -> i64 uses { filesystem.handle.release } {
    let returned = identity(value);
    1
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(
        codes(source)
            .iter()
            .filter(|code| **code == "SPX-E103")
            .count(),
        1
    );
    let fixed = source.replace(
        "fn needs_drop(value: own Box) -> i64 { 1 }",
        "fn needs_drop(value: own Box) -> i64 uses { filesystem.handle.release } { 1 }",
    );
    let fixed_codes = codes(&fixed);
    assert!(fixed_codes.is_empty(), "{fixed_codes:?}");
    let ast = parse(&fixed, Path::new("lifecycle-effects-fixed.spx")).unwrap();
    let mut resolved = hir::resolve(&ast).unwrap();
    resolved
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "box.needs_drop")
        .unwrap()
        .effects
        .clear();
    assert_eq!(hir::validate(&resolved).unwrap_err().code, "SPX-H006");
    assert_eq!(codegen::emit_hir_c(&resolved).unwrap_err().code, "SPX-H006");
    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn lifecycle_contract_resolves_and_hostile_hir_is_rejected() {
    let ast = parse(IMPORTED, Path::new("resource-lifecycle.spx")).unwrap();
    let program = hir::resolve(&ast).unwrap();
    let file = &program.types[0];
    let ResolvedTypeDeclarationKind::Resource { drop } = &file.kind else {
        panic!("File must resolve as a resource")
    };
    assert_eq!(drop.id.as_str(), "io.file.drop");
    assert!(matches!(
        &drop.kind,
        ResolvedResourceDropKind::Imported { import, import_key }
            if import.as_str() == "io.file.finalize" && import_key == "io.file.finalize"
    ));
    let import = &program.interfaces[0].imports[0];
    assert_eq!(import.import_key, "io.file.finalize");
    assert_eq!(import.required_authority, ["filesystem.handle.release"]);
    assert_eq!(import.result.out_slot_initialization, "success_only");
    assert!(matches!(import.failure, ResolvedImportFailure::Infallible));
    assert_eq!(
        program.declarations.declaration(&drop.id).unwrap().kind,
        DeclarationKind::ResourceDrop
    );

    let mut forged = program.clone();
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut forged.types[0].kind else {
        unreachable!()
    };
    let ResolvedResourceDropKind::Imported { import_key, .. } = &mut drop.kind else {
        unreachable!()
    };
    *import_key = "forged.key".to_owned();
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    assert_eq!(codegen::emit_hir_c(&forged).unwrap_err().code, "SPX-H006");
    assert_eq!(
        wasm::emit_resolved_module(&forged).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_parameter = program.clone();
    wrong_parameter.interfaces[0].imports[0].parameters[0].ty = hir::ResolvedType::I64;
    assert_eq!(
        hir::validate(&wrong_parameter).unwrap_err().code,
        "SPX-H006"
    );

    let mut duplicate_effect = program;
    duplicate_effect.interfaces[0].imports[0]
        .effects
        .push("filesystem.handle.release".to_owned());
    duplicate_effect.interfaces[0].imports[0]
        .required_authority
        .push("filesystem.handle.release".to_owned());
    assert_eq!(
        hir::validate(&duplicate_effect).unwrap_err().code,
        "SPX-H006"
    );
}
