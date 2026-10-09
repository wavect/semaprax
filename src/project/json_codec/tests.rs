use super::*;

fn record(source: &str) -> TypeDeclaration {
    parse_schema(source).types.remove(0)
}

fn parse_schema(source: &str) -> crate::ast::Program {
    let source = format!("{source}\n@id(\"app.schema.anchor\") fn schema_anchor()->i64 {{0}}\n");
    crate::parse(&source, "schema.spx").unwrap()
}

#[test]
fn codec_shape_requires_authored_ids_and_refuses_unenforced_invariants() {
    let good = "module schema; @id(\"app.row\") record Row { @id(\"app.row.n\") n: i64, }";
    validate_record(&record(good)).unwrap();
    for bad in [
        "module schema; record Row { @id(\"x\") n: i64, }",
        "module schema; @id(\"x\") record Row { n: i64, }",
        "module schema; @id(\"x\") record Row { @id(\"y\") n: string, }",
        "module schema; @id(\"x\") record Row<T> { @id(\"y\") n: T, }",
        "module schema; @id(\"x\") record Row {}",
    ] {
        assert_eq!(
            validate_record(&record(bad)).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    let mut forged = record(good);
    forged.invariants = Some(Box::new(vec![
        crate::parse("module t; fn main()->bool { false }", "t.spx")
            .unwrap()
            .functions
            .remove(0)
            .body,
    ]));
    assert_eq!(validate_record(&forged).unwrap_err()[0].code, "SPX-J180");
    forged.invariants = None;
    let TypeDeclarationKind::Record { fields } = &mut forged.kind else {
        unreachable!()
    };
    let field = fields[0].clone();
    fields.extend(std::iter::repeat_n(field, 8));
    assert_eq!(validate_record(&forged).unwrap_err()[0].code, "SPX-J180");
}

#[test]
fn codec_source_is_deterministic_ordinary_ast_and_has_no_authority_escape() {
    let program = parse_schema("module schema; @id(\"app.row\") record Row { @id(\"a\") n: i64, @id(\"b\") count: usize, @id(\"c\") byte: u8, @id(\"d\") ok: bool, }");
    let source = emit::source(&program, &program.types[0]);
    assert_eq!(source, emit::source(&program, &program.types[0]));
    let parsed = crate::parse(&source, "codec.spx").unwrap();
    let encode = parsed
        .types
        .iter()
        .find(|declaration| declaration.name == "RowJsonEncode")
        .unwrap();
    let TypeDeclarationKind::Variant { cases } = &encode.kind else {
        panic!("encode variant")
    };
    assert_eq!(cases[0].fields[0].ty, Type::String);
    assert!(parsed.permits.is_empty());
    assert!(parsed
        .functions
        .iter()
        .all(|function| function.effects.is_empty()));
    assert_eq!(parsed.functions.len(), 3);
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "codec.spx").unwrap())
    );
    assert!(source.contains("jc_strict_end(input, 32usize, 0)"));
    assert!(source.contains("1844674407370955161usize"));
    assert!(source.contains("number != other || number < 0 || number > 255"));
    assert!(source.contains("required > output_limit"));
    assert!(!source.contains("serde"));
    assert!(source.len() <= MAX_GENERATED_BYTES);
}
