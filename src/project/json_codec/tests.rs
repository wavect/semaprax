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
    assert!(source.contains("let _ = if negative { error = 6; offset = start; false } else {"));
    assert!(source.contains("value_1 = if error == 0 { number } else { value_1 };\nerror == 0\n}"));
    assert!(source.contains("number != other || number < 0 || number > 255"));
    assert!(source.contains("required > output_limit"));
    assert!(!source.contains("serde"));
    assert!(source.len() <= MAX_GENERATED_BYTES);
    // Constant field selectors must be installed once before member iteration;
    // fixed-array construction is deliberately closed inside source loops.
    assert!(
        source
            .find("let key_view_0 = array_as_slice(key_0)")
            .unwrap()
            < source.find("while error == 0 && key < length").unwrap()
    );
    let fresh = super::template::discard_bindings(&source, "", "codec.spx").unwrap();
    assert!(!fresh.contains("let _ ="));
    assert_eq!(
        fresh,
        super::template::discard_bindings(&source, "", "codec.spx").unwrap()
    );
    // Every admitted field position can select the unsigned conversion branch;
    // it must end with a value rather than the preceding assignment statement.
    let unsigned = parse_schema("module schema; @id(\"unsigned.row\") record Row { @id(\"u.a\") a:usize, @id(\"u.b\") b:usize, @id(\"u.c\") c:usize, @id(\"u.d\") d:usize, @id(\"u.e\") e:usize, @id(\"u.f\") f:usize, @id(\"u.g\") g:usize, @id(\"u.h\") h:usize, }");
    let generated = emit::source(&unsigned, &unsigned.types[0]);
    let canonical = crate::format::canonical(&crate::parse(&generated, "unsigned.spx").unwrap());
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "unsigned-canonical.spx").unwrap())
    );
}

#[test]
fn candidate_source_handoff_reuses_buffer_and_preserves_full_project_refusals() {
    let manifest = crate::project::ProjectManifest::parse(include_str!(
        "../../../examples/calculator-project/semaprax.toml"
    ))
    .unwrap();
    let sources = [
        (
            "src/app.spx",
            include_str!("../../../examples/calculator-project/src/app.spx"),
        ),
        (
            "src/core.spx",
            include_str!("../../../examples/calculator-project/src/core.spx"),
        ),
        (
            "src/tests.spx",
            include_str!("../../../examples/calculator-project/src/tests.spx"),
        ),
    ]
    .into_iter()
    .map(|(path, source)| SemanticWorkspaceSource {
        path: path.into(),
        source: source.into(),
    })
    .collect();
    let built = crate::project::build::build_owned(&manifest, sources).unwrap();
    let revision = ProjectRevision::from_built(manifest, built);
    let original = revision
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap()
        .source()
        .to_owned();
    let source = format!("{original}\n@id(\"replacement.probe\") fn probe()->i64 {{42}}\n");
    let canonical = crate::format::canonical(&crate::parse(&source, "src/core.spx").unwrap());
    let expected = canonical.clone();
    let pointer = canonical.as_ptr();
    let capacity = canonical.capacity();
    let checked = validate_candidate_source(&revision, "src/core.spx", canonical).unwrap();
    assert_eq!(checked, expected);
    assert_eq!(checked.as_ptr(), pointer);
    assert_eq!(checked.capacity(), capacity);
    let absent = validate_candidate_source(&revision, "src/absent.spx", checked.clone())
        .expect_err("an absent replacement path is a controlled refusal");
    assert_eq!(absent[0].code, "SPX-J180");
    assert_eq!(
        absent[0].message,
        "JSON codec replacement is absent from its checked candidate"
    );

    let invalid = checked.replace("fn probe() -> i64", "fn probe() -> bool");
    assert_ne!(invalid, checked);
    // The prior complete-Project recipe is an independent refusal oracle:
    // the handoff must not bypass typing of an unselected generated helper.
    let previous_sources = revision
        .sources()
        .iter()
        .map(|source| SemanticWorkspaceSource {
            path: source.path().to_owned(),
            source: if source.path() == "src/core.spx" {
                invalid.clone()
            } else {
                source.source().to_owned()
            },
        })
        .collect();
    let previous = crate::project::build::build_owned(revision.manifest(), previous_sources)
        .err()
        .expect("invalid helper is refused by ordinary Project construction");
    let current = validate_candidate_source(&revision, "src/core.spx", invalid).unwrap_err();
    let signatures = |errors: &[Diagnostic]| {
        errors
            .iter()
            .map(|error| (error.code, error.message.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(signatures(&current), signatures(&previous));
    assert_eq!(
        revision
            .sources()
            .iter()
            .find(|s| s.path() == "src/core.spx")
            .unwrap()
            .source(),
        original
    );
}
