use super::*;

const SOURCE: &str = r#"
module test.stdout_wasm_authority;
permit { process.stdout.write }
@id("app.main")
fn main() -> i64 uses { process.stdout.write } {
    let data = [65u8];
    let view = array_as_slice(data);
    let written = stdout_write(view);
    if written == 1usize { 0 } else { 1 }
}
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let ast = crate::parse(source, Path::new("stdout-wasm-authority.spx")).unwrap();
    crate::hir::resolve(&ast).unwrap()
}

fn assert_rejected(program: &ResolvedProgram) {
    let diagnostic = emit_resolved_module_with_stdout_transcript(program).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-T269");
    assert!(diagnostic.message.contains("authority mismatch"));
}

fn source_with_unused_inventory(declaration: &str) -> String {
    SOURCE.replace(
        "@id(\"app.main\")",
        &format!("{declaration}\n\n@id(\"app.main\")"),
    )
}

#[test]
fn raw_test_projection_rejects_permit_effect_and_interface_supersets() {
    let ast = crate::parse(SOURCE, Path::new("stdout-wasm-valid.spx")).unwrap();
    assert!(!emit_module_with_stdout_transcript(&ast).unwrap().is_empty());

    let mut widened_permit = resolved(SOURCE);
    widened_permit.permits.push("process.network".to_owned());
    assert_rejected(&widened_permit);

    let mut widened_effect = resolved(SOURCE);
    widened_effect.permits.push("process.network".to_owned());
    widened_effect.functions[0]
        .effects
        .push("process.network".to_owned());
    assert_rejected(&widened_effect);

    let with_interface = resolved(&SOURCE.replace(
        "@id(\"app.main\")",
        r#"@id("host.interface")
interface Host permits {} {
    @id("host.echo")
    import rust fn echo(value: i64) -> i64
        effects {}
        failure status "host.echo.v1";
}
@id("app.main")"#,
    ));
    assert_rejected(&with_interface);

    let generic = resolved(&source_with_unused_inventory(
        r#"@id("unused.identity")
fn identity<T>(value: T) -> T { value }"#,
    ));
    assert_rejected(&generic);

    let authored_type_inventories = [
        r#"@id("unused.record")
record UnusedRecord { @id("unused.record.value") value: i64, }"#,
        r#"@id("unused.resource")
resource UnusedResource { @id("unused.resource.drop") drop trivial; }"#,
        r#"@id("unused.variant")
variant UnusedVariant { @id("unused.variant.none") None, }"#,
        r#"@id("unused.class")
class UnusedClass { @id("unused.class.value") value: i64, }"#,
    ];
    for declaration in authored_type_inventories {
        assert_rejected(&resolved(&source_with_unused_inventory(declaration)));
    }
}
