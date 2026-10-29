use super::*;
const PROVIDER: &str = r#"
module reader.provider;
@id("reader.type") record Reader { @id("reader.data") data: Bytes, @id("reader.cursor") cursor: usize, }
@id("reader.new") fn create(data:own Bytes)->Reader { Reader { data, cursor:0usize } }
@id("reader.advance") fn advance(value:own Reader)->Reader {
    match own value { Reader { data, cursor } => Reader { data, cursor:cursor+1usize }, }
}
@id("reader.position") fn position(value:borrow Reader)->usize { value.cursor }
@id("reader.finish") fn finish(value:own Reader)->Bytes {
    match own value { Reader { data, cursor } => data, }
}
"#;
const APP: &str = r#"
module reader.app;
use type @id("reader.type") from reader.provider as Reader;
use function @id("reader.new") from reader.provider as create;
use function @id("reader.advance") from reader.provider as advance;
use function @id("reader.position") from reader.provider as position;
use function @id("reader.finish") from reader.provider as finish;
@id("app.main") fn main()->i64 {
    let input=[65u8];
    let reader=create(bytes_copy(array_as_slice(input)));
    let first=position(reader);
    let next=advance(reader);
    let second=position(next);
    let data=finish(next);
    if first==0usize && second==1usize && byte_len(bytes_as_slice(data))==1usize { 1 } else { 0 }
}
"#;
fn sources(app: &str, provider: &str) -> Vec<WorkspaceSource> {
    [("app.spx", app), ("provider.spx", provider)]
        .into_iter()
        .map(|(path, text)| {
            let program = crate::parse(text, Path::new(path)).expect("fixture parses");
            WorkspaceSource {
                path: path.to_owned(),
                source: crate::format::canonical(&program),
            }
        })
        .collect()
}
#[test]
fn internal_owned_record_imports_preserve_checked_calls_and_scalar_boundary() {
    let built = build_owned(sources(APP, PROVIDER)).expect("record imports authenticate");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .expect("owned Project closure links");
    hir::validate(&linked).expect("independent HIR cleanup replay");
    assert!(linked
        .functions
        .iter()
        .any(|function| function.id.as_str() == "reader.advance"));
    let value = crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
        .expect("linked reader runs");
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    for target in ["reader.advance", "reader.finish"] {
        let mut forged = linked.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == target)
            .expect("retained imported implementation");
        let mut result_at = None;
        crate::hir::function_value::walk(function, |expression| {
            if let crate::hir::ResolvedExprKind::Match { arms, .. } = &expression.kind {
                result_at = Some(arms[0].value.id.clone());
            }
        });
        let result_at = result_at.expect("fixture contains a retained record match");
        let mut removed = false;
        for block in &mut function.cleanup_plan.blocks {
            block.transitions.retain(|transition| {
                let selected = matches!(transition,
                    crate::cleanup_plan::CleanupTransition::Transfer { at, .. } if at == &result_at);
                removed |= selected;
                !selected
            });
        }
        assert!(
            removed,
            "record or Bytes arm result has a canonical transfer"
        );
        assert!(
            hir::validate(&forged).is_err(),
            "missing match result settlement rejects"
        );
    }
    assert!(built.linked_scalar_program("reader.app").is_err());
}
#[test]
fn internal_owned_record_import_requires_direct_types_and_no_generic_widening() {
    // Private Nominal Layout Closure v1 (15149c189): exact imported function
    // signatures root the private `Reader` layout, so a caller that never names
    // `Reader` needs no type import. An authored name still needs the direct
    // import, and generic signatures keep their closed explicit route.
    let missing = APP.replace(
        "use type @id(\"reader.type\") from reader.provider as Reader;",
        "",
    );
    assert_ne!(missing, APP);
    let built = build_owned(sources(&missing, PROVIDER))
        .expect("function-only signature roots authenticate the private record layout");
    assert!(!built.edges.iter().any(|edge| edge.caller_path == "app.spx"
        && matches!(edge.kind, "type_import" | "type_reference")));
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .expect("private-layout closure links the real providers");
    hir::validate(&linked).expect("independent HIR cleanup replay");
    assert!(linked
        .types
        .iter()
        .any(|ty| ty.id.as_str() == "reader.type"));
    let value = crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
        .expect("linked private-layout reader runs");
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    assert!(built.linked_scalar_program("reader.app").is_err());
    let declaration = "@id(\"app.bad\") fn bad(value: borrow Reader) -> usize { value.cursor }";
    build_owned(sources(&format!("{APP}\n{declaration}\n"), PROVIDER))
        .expect("the explicitly imported name may be authored");
    assert!(
        build_owned(sources(&format!("{missing}\n{declaration}\n"), PROVIDER)).is_err(),
        "a private layout grants no source name"
    );
    let generic = PROVIDER.replace(
        "fn advance(value:own Reader)",
        "fn advance<T>(value:own Reader)",
    );
    for app in [APP, missing.as_str()] {
        let errors = build_owned(sources(app, &generic))
            .err()
            .expect("generic import remains closed");
        assert!(errors.iter().any(|error| error.code == "SPX-G172"));
    }
}

#[test]
fn internal_owned_record_import_rejects_same_shape_wrong_identity() {
    let provider = format!("{PROVIDER}\n@id(\"reader.other\") record Other {{ @id(\"other.data\") data: Bytes, @id(\"other.cursor\") cursor:usize, }}\n");
    let app = APP.replace(
        "use type @id(\"reader.type\")",
        "use type @id(\"reader.other\")",
    );
    // Private Nominal Layout Closure v1 (15149c189): the exact function
    // signatures still root the real `reader.type` layout, so an unused alias
    // for a same-shape foreign record substitutes nothing and is inert.
    let built = build_owned(sources(&app, &provider))
        .expect("an unused foreign alias does not replace the signature identity");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    let value =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    // Using the foreign identity where the real record flows refuses.
    let annotate = |app: &str| app.replace("let reader=create(", "let reader:Reader=create(");
    let used = annotate(&app);
    assert_ne!(used, app);
    let errors = build_owned(sources(&used, &provider))
        .err()
        .expect("same shape does not substitute identity");
    assert!(
        errors.iter().any(|error| error.code == "SPX-T232"
            && error
                .message
                .contains("[unimported @id(\"reader.type\") from reader.provider]")),
        "{errors:?}"
    );
    build_owned(sources(&annotate(APP), &provider)).expect("the exact identity is accepted");
}

#[test]
fn internal_owned_record_import_preserves_contract_failure_and_reentry() {
    let provider = PROVIDER.replace(
        "fn advance(value:own Reader)->Reader {",
        "fn advance(value:own Reader)->Reader requires false {",
    );
    let built = build_owned(sources(APP, &provider)).expect("failing import is still checked");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .expect("failing owned closure links");
    for _ in 0..3 {
        let value =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                .expect("failure executes through checked call");
        assert_eq!(
            value.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::LanguageFailure(
                crate::conformance::NormalizedStatus::contract(
                    crate::cleanup_plan::ContractPhase::Requires
                )
            )
        );
    }
}

#[test]
fn borrowed_str_with_owned_byte_record_import_preserves_the_checked_transfer() {
    let provider = r#"
module writer.provider;
@id("writer.type") record Writer { @id("writer.data") data: Bytes, @id("writer.cursor") cursor: usize, }
@id("writer.new") fn create(data: own Bytes) -> Writer { Writer { data: data, cursor: 0usize } }
@id("writer.append") fn append(input: borrow str, output: own Writer) -> Writer {
    if str_is_empty(input) { output } else { output }
}
"#;
    let app = r#"
module writer.app;
use type @id("writer.type") from writer.provider as Writer;
use function @id("writer.new") from writer.provider as create;
use function @id("writer.append") from writer.provider as append;
@id("writer.app.main") fn main() -> i64 {
    let bytes = [0u8, 0u8];
    let output = create(bytes_copy(array_as_slice(bytes)));
    let text = "json";
    let rendered = append(string_as_str(text), output);
    if rendered.cursor == 0usize { 1 } else { 0 }
}
"#;
    let built = build_owned(sources(app, provider))
        .expect("borrowed str plus an owned authenticated byte record imports");
    let linked = built
        .linked_owned_data_api_program_with_roots("writer.app", &[])
        .expect("mixed borrowed/owned internal call links");
    let value =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "writer.app.main", 100_000)
            .expect("mixed borrowed/owned internal call executes");
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
}

#[test]
fn borrowed_str_does_not_admit_a_non_byte_record_import() {
    let provider = r#"
module writer.provider;
@id("writer.type") record Writer { @id("writer.flag") flag: bool, }
@id("writer.new") fn create() -> Writer { Writer { flag: true } }
@id("writer.append") fn append(input: borrow str, output: own Writer) -> Writer {
    if str_is_empty(input) { output } else { output }
}
"#;
    let app = r#"
module writer.app;
use type @id("writer.type") from writer.provider as Writer;
use function @id("writer.new") from writer.provider as create;
use function @id("writer.append") from writer.provider as append;
@id("writer.app.main") fn main() -> i64 {
    let text = "json";
    let rendered = append(string_as_str(text), create());
    if rendered.flag { 1 } else { 0 }
}
"#;
    let errors = build_owned(sources(app, provider))
        .err()
        .expect("borrowed str cannot widen imports to records without Bytes");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));
}

#[test]
fn owned_input_copy_record_result_authenticates_direct_identity_and_cleanup() {
    let provider = format!(
        r#"{PROVIDER}
@id("reader.info") record Info {{ @id("reader.info.cursor") cursor: usize, }}
@id("reader.other-info") record OtherInfo {{ @id("reader.other-info.cursor") cursor: usize, }}
@id("reader.inspect") fn inspect(value:own Reader)->Info {{ Info {{ cursor:value.cursor }} }}
"#
    );
    let app = r#"
module reader.app;
use type @id("reader.type") from reader.provider as Reader;
use type @id("reader.info") from reader.provider as Info;
use function @id("reader.new") from reader.provider as create;
use function @id("reader.inspect") from reader.provider as inspect;
@id("app.main") fn main()->i64 {
    let bytes=[65u8];
    let reader=create(bytes_copy(array_as_slice(bytes)));
    let info=inspect(reader);
    if info.cursor==0usize { 1 } else { 0 }
}
"#;
    let built =
        build_owned(sources(app, &provider)).expect("owned input with authenticated Copy result");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    let run =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert!(matches!(
        run.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    assert!(built.linked_scalar_program("reader.app").is_err());
    let borrowed = provider.replace(
        "fn inspect(value:own Reader)",
        "fn inspect(value:borrow Reader)",
    );
    let borrowed_app = app.replace(
        "if info.cursor==0usize { 1 } else { 0 }",
        "let again=inspect(reader); if info.cursor==0usize && again.cursor==0usize { 1 } else { 0 }",
    );
    let borrowed_built = build_owned(sources(&borrowed_app, &borrowed))
        .expect("borrowed owned-record input may return authenticated Copy data");
    let borrowed_linked = borrowed_built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .unwrap();
    hir::validate(&borrowed_linked).unwrap();
    let observed =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&borrowed_linked, "app.main", 100_000)
            .unwrap();
    assert!(matches!(
        observed.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    // Private Nominal Layout Closure v1 (15149c189): the exact `inspect`
    // signature roots the private Copy `Info` layout. Omitting its import, or
    // aliasing an unused same-shape foreign record, keeps the real identity;
    // authoring the foreign identity for the result refuses.
    let missing = app.replace(
        "use type @id(\"reader.info\") from reader.provider as Info;",
        "",
    );
    let foreign = app.replace(
        "use type @id(\"reader.info\")",
        "use type @id(\"reader.other-info\")",
    );
    let annotate = |app: &str| {
        app.replace(
            "let info=inspect(reader);",
            "let info:Info=inspect(reader);",
        )
    };
    assert_ne!(annotate(app), app);
    for provider in [provider.as_str(), borrowed.as_str()] {
        for admitted in [&missing, &foreign] {
            assert_ne!(admitted, app);
            let built = build_owned(sources(admitted, provider))
                .expect("signature root authenticates the private Copy result");
            let linked = built
                .linked_owned_data_api_program_with_roots("reader.app", &[])
                .unwrap();
            hir::validate(&linked).unwrap();
            assert!(linked
                .types
                .iter()
                .any(|ty| ty.id.as_str() == "reader.info"));
            let observed =
                crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
                    .unwrap();
            assert!(matches!(
                observed.outcome,
                crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
            ));
            assert!(built.linked_scalar_program("reader.app").is_err());
        }
        build_owned(sources(&annotate(app), provider)).expect("exact Info identity is accepted");
        assert!(
            build_owned(sources(&annotate(&missing), provider)).is_err(),
            "a private layout grants no source name"
        );
        let errors = build_owned(sources(&annotate(&foreign), provider))
            .err()
            .expect("Copy result identity is not interchangeable");
        assert!(
            errors.iter().any(|error| error.code == "SPX-T232"
                && error
                    .message
                    .contains("[unimported @id(\"reader.info\") from reader.provider]")),
            "{errors:?}"
        );
    }
    let generic = provider.replace(
        "fn inspect(value:own Reader)",
        "fn inspect<T>(value:own Reader)",
    );
    assert!(
        build_owned(sources(app, &generic)).is_err(),
        "generic import remains closed"
    );
}

#[test]
fn owned_record_import_admits_only_direct_fieldless_variant_result() {
    let provider = format!(
        r#"{PROVIDER}
@id("reader.outcome") variant Outcome {{ @id("reader.outcome.ok") Ok, @id("reader.outcome.no") No, }}
@id("reader.inspect-outcome") fn inspect_outcome(value: own Reader) -> Outcome {{
    let cursor = match own value {{ Reader {{ data, cursor }} => cursor, }};
    if cursor == 0usize {{ Outcome::Ok {{}} }} else {{ Outcome::No {{}} }}
}}
"#
    );
    let app = r#"
module reader.app;
use type @id("reader.type") from reader.provider as Reader;
use type @id("reader.outcome") from reader.provider as Outcome;
use function @id("reader.new") from reader.provider as create;
use function @id("reader.inspect-outcome") from reader.provider as inspect_outcome;
@id("app.main") fn main() -> i64 {
    let input = [65u8];
    match inspect_outcome(create(bytes_copy(array_as_slice(input)))) {
        Outcome::Ok {} => 1,
        Outcome::No {} => 0,
    }
}
"#;
    let built = build_owned(sources(app, &provider)).expect("direct fieldless variant import");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .expect("fieldless variant closure links");
    hir::validate(&linked).expect("variant cleanup replay");
    let value = crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000)
        .expect("fieldless variant executes");
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));

    let missing_type = app.replace(
        "use type @id(\"reader.outcome\") from reader.provider as Outcome;",
        "",
    );
    let errors = build_owned(sources(&missing_type, &provider))
        .err()
        .expect("variant result requires direct type import");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));

    let payload = provider
        .replace(
            "@id(\"reader.outcome.ok\") Ok,",
            "@id(\"reader.outcome.ok\") Ok { @id(\"reader.outcome.ok.value\") value: usize, },",
        )
        .replace("Outcome::Ok {}", "Outcome::Ok { value: 0usize }");
    let payload_app = app.replace("Outcome::Ok {} => 1", "Outcome::Ok { value } => 1");
    let errors = build_owned(sources(&payload_app, &payload))
        .err()
        .expect("payload variant result stays outside owned-record import lane");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));
}

#[test]
fn borrowed_byte_view_and_owned_record_import_compose_without_public_abi_widening() {
    let provider = PROVIDER
        .replace(
            "fn advance(value:own Reader)",
            "fn advance(value:own Reader, view:borrow Slice<u8>)",
        )
        .replace("cursor:cursor+1usize", "cursor:cursor+byte_len(view)");
    let app = APP.replace("advance(reader)", "advance(reader, array_as_slice(input))");
    let built = build_owned(sources(&app, &provider)).expect("byte view plus exact record import");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    let value =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    assert!(built.linked_scalar_program("reader.app").is_err());
    // Private Nominal Layout Closure v1 (15149c189): the exact signatures root
    // the private Reader layout; the composition stays in the internal lane
    // and still never widens the public scalar ABI.
    let missing = app.replace(
        "use type @id(\"reader.type\") from reader.provider as Reader;",
        "",
    );
    assert_ne!(missing, app);
    let built = build_owned(sources(&missing, &provider))
        .expect("function-only signature roots authenticate the record");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    let value =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert!(matches!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    assert!(built.linked_scalar_program("reader.app").is_err());
    let named = format!(
        "{missing}\n@id(\"app.bad\") fn bad(value: borrow Reader) -> usize {{ value.cursor }}\n"
    );
    assert!(
        build_owned(sources(&named, &provider)).is_err(),
        "a private layout grants no source name"
    );
}

#[test]
fn imported_cursor_renewal_uses_allocation_free_prototypes_and_replays_provider_capacity() {
    let app = r#"
module reader.app;
use type @id("reader.type") from reader.provider as Reader;
use function @id("reader.new") from reader.provider as create;
use function @id("reader.advance") from reader.provider as advance;
use function @id("reader.position") from reader.provider as position;
use function @id("reader.finish") from reader.provider as finish;
@id("app.main") fn main()->i64 {
    let mut reader=create(bytes_zeroed(1usize));
    let mut count=0usize;
    while count<3usize {
        reader=advance(reader);
        count=count+1usize;
        0
    }
    let position=position(reader);
    let data=finish(reader);
    if position==3usize && byte_len(bytes_as_slice(data))==1usize {1}else{0}
}
"#;
    let built = build_owned(sources(app, PROVIDER)).expect("renewal prototype does not allocate");
    let linked = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .expect("actual allocation-free provider links");
    hir::validate(&linked).expect("exact retained renewal and cleanup replay");
    let observation =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert!(matches!(
        observation.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));
    let allocating = PROVIDER.replace(
        "Reader { data, cursor:cursor+1usize }",
        "Reader { data:bytes_zeroed(1usize), cursor:cursor+1usize }",
    );
    let built = build_owned(sources(app, &allocating)).expect("prototype keeps exact signature");
    let refused = built
        .linked_owned_data_api_program_with_roots("reader.app", &[])
        .expect_err("real provider allocation remains forbidden under a caller loop");
    assert!(
        refused
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T267"),
        "{refused:?}"
    );
}

const FACTORY_PROVIDER: &str = r#"
module factory.provider;
@id("factory.reader") record Reader { @id("factory.data") data: Bytes, @id("factory.cursor") cursor: usize, }
@id("factory.new") fn new() -> Reader { Reader { data: bytes_zeroed(1usize), cursor: 0usize } }
@id("factory.make") fn make(size: usize) -> Reader { Reader { data: bytes_zeroed(1usize), cursor: size - size } }
@id("factory.advance") fn advance(value: own Reader, view: borrow Slice<u8>, step: usize) -> Reader {
    match own value { Reader { data, cursor } => Reader { data: data, cursor: cursor + byte_len(view) + step }, }
}
@id("factory.position") fn position(value: borrow Reader, index: usize) -> usize { value.cursor + index }
@id("factory.finish") fn finish(value: own Reader) -> Bytes { match own value { Reader { data, cursor } => data, } }
"#;
const FACTORY_APP: &str = r#"
module factory.app;
use type @id("factory.reader") from factory.provider as Reader;
use function @id("factory.new") from factory.provider as new;
use function @id("factory.make") from factory.provider as make;
use function @id("factory.advance") from factory.provider as advance;
use function @id("factory.position") from factory.provider as position;
use function @id("factory.finish") from factory.provider as finish;
@id("factory.app.main") fn main() -> i64 {
    let initial = new();
    let first = position(initial, 0usize);
    let data = finish(initial);
    let input = [65u8];
    let mut reader = make(1usize);
    let view = array_as_slice(input);
    let alias = view;
    let mut count = 0usize;
    while count < 2usize {
        reader = advance(reader, alias, 1usize);
        count = count + 1usize;
        0
    }
    let observed = position(reader, 1usize);
    let final_data = finish(reader);
    if first == 0usize && observed == 5usize && byte_len(bytes_as_slice(data)) == 1usize && byte_len(bytes_as_slice(final_data)) == 1usize { 1 } else { 0 }
}
"#;

#[test]
fn owned_record_factories_and_named_view_renewal_preserve_real_checked_providers() {
    let built = build_owned(sources(FACTORY_APP, FACTORY_PROVIDER))
        .expect("zero/scalar factories, indexed observer and borrowed-view renewal import");
    let linked = built
        .linked_owned_data_api_program_with_roots("factory.app", &[])
        .expect("real allocation-free renewal replaces the non-executable prototype");
    hir::validate(&linked).expect("independent ownership, cleanup and shared-loan replay");
    for _ in 0..3 {
        let observed = crate::interpreter::evaluate_resolved_zero_arg_i64(
            &linked,
            "factory.app.main",
            100_000,
        )
        .unwrap();
        assert!(matches!(
            observed.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
        ));
    }
    assert!(built.linked_scalar_program("factory.app").is_err());

    let str_provider = FACTORY_PROVIDER
        .replace("view: borrow Slice<u8>", "view: borrow str")
        .replace("byte_len(view)", "usize_from_i64(str_len_bytes(view))");
    let str_app = FACTORY_APP
        .replace("let input = [65u8];", "let input = \"A\";")
        .replace("array_as_slice(input)", "string_as_str(input)");
    let str_built = build_owned(sources(&str_app, &str_provider))
        .expect("named independent str input uses the same owner-forwarding prototype");
    let str_linked = str_built
        .linked_owned_data_api_program_with_roots("factory.app", &[])
        .unwrap();
    hir::validate(&str_linked).unwrap();
    let observed = crate::interpreter::evaluate_resolved_zero_arg_i64(
        &str_linked,
        "factory.app.main",
        100_000,
    )
    .unwrap();
    assert!(matches!(
        observed.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
    ));

    let allocating = FACTORY_PROVIDER.replace(
        "data: data, cursor: cursor +",
        "data: bytes_zeroed(1usize), cursor: cursor +",
    );
    let built = build_owned(sources(FACTORY_APP, &allocating))
        .expect("signature-only prototype conveys no provider allocation authority");
    let errors = built
        .linked_owned_data_api_program_with_roots("factory.app", &[])
        .expect_err("linked provider allocation remains forbidden in the caller loop");
    assert!(
        errors.iter().any(|error| error.code == "SPX-T267"),
        "{errors:?}"
    );

    for setup in [
        "let view = bytes_as_slice(reader.data); let alias = view;",
        "let view = bytes_as_slice(reader.data); let intermediate = view; let alias = intermediate;",
    ] {
        let hostile = FACTORY_APP.replace(
            "let view = array_as_slice(input);\n    let alias = view;", setup,
        );
        let errors = build_owned(sources(&hostile, FACTORY_PROVIDER))
            .err().expect("a forwarded prototype cannot hide an overlapping input loan");
        assert!(errors.iter().any(|error| error.code == "SPX-T265"), "{errors:?}");
    }
    let moved_input = FACTORY_APP
        .replace(
            "@id(\"factory.app.main\") fn main() -> i64 {",
            "@id(\"factory.app.hold_bytes\") fn hold_bytes(value: own Bytes) -> Bytes { value }\n@id(\"factory.app.main\") fn main() -> i64 {",
        )
        .replace(
            "let view = array_as_slice(input);\n    let alias = view;",
            "let view = bytes_as_slice(data); let alias = view; let moved = hold_bytes(data);",
        );
    let errors = build_owned(sources(&moved_input, FACTORY_PROVIDER))
        .err()
        .expect("independent input storage cannot move while its named view is live");
    assert!(
        errors.iter().any(|error| error.code == "SPX-T265"),
        "{errors:?}"
    );
}

#[test]
fn owned_record_factory_missing_nominal_import_reports_exact_repair() {
    let missing = FACTORY_APP.replace(
        "use type @id(\"factory.reader\") from factory.provider as Reader;",
        "",
    );
    let parsed = crate::parse(&missing, Path::new("app.spx")).unwrap();
    let canonical = crate::format::canonical(&parsed);
    let parsed = crate::parse(&canonical, Path::new("app.spx")).unwrap();
    let expected_span = parsed.module_uses[0].span;
    // Private Nominal Layout Closure v1 (15149c189): the exact factory, renewal
    // and observer signatures root the private Reader layout, so this caller,
    // which never names `Reader`, needs no type import.
    let built = build_owned(sources(&missing, FACTORY_PROVIDER))
        .expect("function-only signature roots authenticate the factory result");
    let linked = built
        .linked_owned_data_api_program_with_roots("factory.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    for _ in 0..3 {
        let observed = crate::interpreter::evaluate_resolved_zero_arg_i64(
            &linked,
            "factory.app.main",
            100_000,
        )
        .unwrap();
        assert!(matches!(
            observed.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(1)
        ));
    }
    assert!(built.linked_scalar_program("factory.app").is_err());
    // A generic factory is not a signature root; its refused import still
    // carries the exact repair at its own span.
    let generic = FACTORY_PROVIDER.replace("fn new()", "fn new<T>()");
    let errors = build_owned(sources(&missing, &generic))
        .err()
        .expect("zero-arg factory does not widen generic admission");
    let error = errors
        .iter()
        .find(|error| error.code == "SPX-G172" && error.span == Some(expected_span))
        .unwrap_or_else(|| panic!("{errors:?}"));
    assert_eq!(error.path.as_deref(), Some("app.spx"));
    assert_eq!(error.help.as_deref(), Some(
        "missing direct nominal type import: use type @id(\"factory.reader\") from factory.provider as Reader;"
    ));
    let errors = build_owned(sources(FACTORY_APP, &generic))
        .err()
        .expect("zero-arg factory does not widen generic admission");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));

    let nested_provider = r#"
module factory.provider;
@id("factory.inner") record Inner { @id("factory.inner.data") data: Bytes, }
@id("factory.outer") record Outer { @id("factory.outer.inner") inner: Inner, }
@id("factory.nested") fn nested() -> Outer { Outer { inner: Inner { data: bytes_zeroed(1usize) } } }
"#;
    let nested_app = r#"
module factory.app;
use type @id("factory.outer") from factory.provider as Outer;
use function @id("factory.nested") from factory.provider as nested;
@id("factory.app.main") fn main() -> i64 { let value = nested(); 0 }
"#;
    // The explicit Outer root replays its private Inner record descendant.
    let built = build_owned(sources(nested_app, nested_provider))
        .expect("explicit root authenticates its private nested record");
    let linked = built
        .linked_owned_data_api_program_with_roots("factory.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    assert!(linked
        .types
        .iter()
        .any(|ty| ty.id.as_str() == "factory.inner"));
    let observed =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "factory.app.main", 100_000)
            .unwrap();
    assert!(matches!(
        observed.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(0)
    ));
    let repaired = nested_app.replace(
        "module factory.app;",
        "module factory.app; use type @id(\"factory.inner\") from factory.provider as Inner;",
    );
    let built = build_owned(sources(&repaired, nested_provider))
        .expect("complete direct nested type imports authenticate the existing factory profile");
    let linked = built
        .linked_owned_data_api_program_with_roots("factory.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    // A nested variant descendant stays outside the private layout profile.
    let variant_provider = nested_provider
        .replace(
            "record Inner { @id(\"factory.inner.data\") data: Bytes, }",
            "variant Inner { @id(\"factory.inner.data\") Data { @id(\"factory.inner.data.bytes\") bytes: Bytes, }, @id(\"factory.inner.none\") None, }",
        )
        .replace(
            "Inner { data: bytes_zeroed(1usize) }",
            "Inner::Data { bytes: bytes_zeroed(1usize) }",
        );
    assert!(variant_provider.contains("variant Inner") && variant_provider.contains("Inner::Data"));
    let errors = build_owned(sources(nested_app, &variant_provider))
        .err()
        .expect("a nested variant descendant remains refused");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G172"),
        "{errors:?}"
    );
}

#[test]
fn concrete_copy_record_vector_return_import_retains_the_real_provider_body() {
    let provider = r#"module rows.provider;
@id("row") record Row { @id("row.n") n:i64, }
@id("rows.make") fn make()->Vec<Row> {
let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:9});
vec_push<Row>(first,Row{n:3})
}
@id("rows.order") fn order(values:own Vec<Row>)->Vec<Row>{vec_sort<Row>(values)}
@id("rows.first") fn first(values:borrow Vec<Row>)->Row{vec_get<Row>(values,0usize)}
"#;
    let app = r#"module rows.app;
use type @id("row") from rows.provider as ImportedRow;
use function @id("rows.make") from rows.provider as make;
use function @id("rows.order") from rows.provider as order;
use function @id("rows.first") from rows.provider as first;
@id("app.main") fn main()->i64 {let values=order(make());let row=first(values);row.n}
"#;
    let built = build_owned(sources(app, provider))
        .expect("exact record Vec return has a checked signature prototype");
    let linked = built
        .linked_owned_data_api_program_with_roots("rows.app", &[])
        .unwrap();
    hir::validate(&linked).expect("real body and cleanup independently replay");
    let value =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert_eq!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(3)
    );
    // Private Nominal Layout Closure v1 (15149c189): the exact `Vec<Row>` and
    // `Row` signatures root the private flat element layout, so a caller that
    // never names `Row` needs no type import.
    let missing = app.replace(
        "use type @id(\"row\") from rows.provider as ImportedRow;",
        "",
    );
    assert_ne!(missing, app);
    let built = build_owned(sources(&missing, provider))
        .expect("signature roots authenticate the private Vec element");
    let linked = built
        .linked_owned_data_api_program_with_roots("rows.app", &[])
        .unwrap();
    hir::validate(&linked).expect("real body and cleanup independently replay");
    assert!(linked.types.iter().any(|ty| ty.id.as_str() == "row"));
    let value =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert_eq!(
        value.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(3)
    );
    let named = format!("{missing}\n@id(\"app.bad\") fn bad()->i64 {{let row=Row{{n:1}};row.n}}\n");
    assert!(
        build_owned(sources(&named, provider)).is_err(),
        "a private layout grants no constructor name"
    );
    let nested = provider.replace("n:i64", "n:Vec<i64>");
    for app in [app, missing.as_str()] {
        assert!(
            build_owned(sources(app, &nested)).is_err(),
            "nested Vec elements remain outside the flat classifier"
        );
    }
}

const BYTE_WRITER_PROVIDER: &str = r#"module writer.provider;
@id("writer.write") fn write(text:borrow str,position:usize,output:own Bytes)->Bytes {
 let first=match str_byte_at(text,0usize){Option::Some{value}=>value,Option::None{}=>0u8,};
 let mut next=output;
 next=bytes_set(next,position,first);
 next
}
"#;
const BYTE_WRITER_APP: &str = r#"module writer.app;
use function @id("writer.write") from writer.provider as write;
@id("writer.main") fn main()->i64 {
 let text="A";let output=write(string_as_str(text),0usize,bytes_zeroed(1usize));
 match byte_get(bytes_as_slice(output),0usize){Option::Some{value}=>i64_from_u8(value),Option::None{}=>0,}
}
"#;
#[test]
fn internal_byte_writer_import_authenticates_actual_body_and_preserves_public_refusal() {
    let direct_tail = BYTE_WRITER_PROVIDER.replace(
        " let mut next=output;\n next=bytes_set(next,position,first);\n next",
        " bytes_set(output,position,first)",
    );
    assert!(crate::check(&direct_tail, "provider.spx")
        .unwrap_err()
        .iter()
        .any(|error| error.code == "SPX-T271"));
    let built = build_owned(sources(BYTE_WRITER_APP, BYTE_WRITER_PROVIDER)).unwrap();
    let linked = built
        .linked_owned_data_api_program_with_roots("writer.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    assert!(linked
        .functions
        .iter()
        .any(|function| function.id.as_str() == "writer.write"));
    let result =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "writer.main", 100_000)
            .unwrap();
    assert!(matches!(
        result.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(65)
    ));
    crate::codegen::emit_hir_c(&linked).unwrap();
    let wasm = crate::wasm::emit_resolved_module(&linked).unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();
    assert!(built.linked_scalar_program("writer.app").is_err());
    let target = linked
        .functions
        .iter()
        .find(|f| f.id.as_str() == "writer.write")
        .unwrap();
    assert!(crate::loop_calls::byte_writer::resolved_nonallocating(
        &linked, target
    ));
    for mutation in 0..4 {
        let mut forged = target.clone();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut forged.body.kind else {
            panic!("writer body");
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("byte binding");
        };
        let hir::ResolvedExprKind::Match { scrutinee, .. } = &mut value.kind else {
            panic!("byte option");
        };
        let hir::ResolvedExprKind::Call {
            args,
            type_arguments,
            ..
        } = &mut scrutinee.kind
        else {
            panic!("byte read");
        };
        match mutation {
            0 => type_arguments.push(hir::ResolvedType::I64),
            1 => args[0].ownership = hir::OwnershipMode::Own,
            2 => args[1].ty = hir::ResolvedType::I64,
            _ => scrutinee.ty = hir::ResolvedType::U8,
        }
        assert!(
            !crate::loop_calls::byte_writer::resolved_nonallocating(&linked, &forged),
            "forged borrowed-Str signature {mutation}"
        );
    }
}
#[test]
fn internal_byte_writer_import_refuses_allocating_generic_and_mismatched_owner_providers() {
    for provider in [
        BYTE_WRITER_PROVIDER.replace("bytes_set(next,position,first)", "bytes_zeroed(1usize)"),
        BYTE_WRITER_PROVIDER.replace("fn write(", "fn write<T>("),
        BYTE_WRITER_PROVIDER.replace("output:own Bytes", "output:borrow Bytes"),
        BYTE_WRITER_PROVIDER.replace("@id(\"writer.write\") ", ""),
    ] {
        let errors = build_owned(sources(BYTE_WRITER_APP, &provider))
            .err()
            .expect("unproved writer source must refuse");
        assert!(errors.iter().any(|error| error.code == "SPX-G172"));
    }
}

#[test]
fn outcome_payload_and_vector_nominals_replay_private_descendants_and_explicit_root() {
    let provider = r#"
module outcome.provider;
@id("outcome.config") record Config { @id("outcome.config.label") label:string, }
@id("outcome.row") record Row { @id("outcome.row.sku") sku:string, }
@id("outcome.report") record Report {
    @id("outcome.report.config") config:Config,
    @id("outcome.report.rows") rows:Vec<Row>,
}
@id("outcome.decoded") variant Decoded {
    @id("outcome.decoded.ready") Ready { @id("outcome.decoded.ready.value") value:Report, },
    @id("outcome.decoded.error") Error {
        @id("outcome.decoded.error.code") code:i64,
        @id("outcome.decoded.error.offset") offset:usize,
        @id("outcome.decoded.error.field") field:i64,
    },
}
@id("outcome.decode") fn decode(input:borrow Slice<u8>)->Decoded { Decoded::Error { code:0, offset:0usize, field:0 } }
"#;
    let app = r#"
module outcome.app;
use type @id("outcome.decoded") from outcome.provider as Decoded;
use type @id("outcome.report") from outcome.provider as Report;
use type @id("outcome.config") from outcome.provider as Config;
use type @id("outcome.row") from outcome.provider as Row;
use function @id("outcome.decode") from outcome.provider as decode;
@id("outcome.main") fn main()->i64 {
    let input=[0u8]; let decoded=decode(array_as_slice(input));
    match own decoded { Decoded::Error { code, offset, field } => code, Decoded::Ready { value } => 0, }
}
"#;
    for (id, name) in [
        ("outcome.report", "Report"),
        ("outcome.config", "Config"),
        ("outcome.row", "Row"),
    ] {
        let import = format!("use type @id(\"{id}\") from outcome.provider as {name};");
        assert_eq!(app.matches(&import).count(), 1);
        let missing = app.replace(&import, "");
        let built = build_owned(sources(&missing, provider))
            .expect("root import authenticates private descendant layout");
        let linked = built
            .linked_owned_data_api_program_with_roots("outcome.app", &[])
            .unwrap();
        hir::validate(&linked).unwrap();
        assert!(linked.types.iter().any(|t| t.id.as_str() == id));
    }
    let built = build_owned(sources(app, provider))
        .expect("complete imports retain the existing closed outcome profile");
    let linked = built
        .linked_owned_data_api_program_with_roots("outcome.app", &[])
        .unwrap();
    hir::validate(&linked).unwrap();
    let vector = hir::DeclarationId::new(crate::prelude::VEC_ID);
    assert_eq!(
        linked.declarations.type_parameters(&vector).unwrap().len(),
        1
    );
    assert!(linked
        .types
        .iter()
        .any(|declaration| declaration.id == vector));
    let mut forged = linked.clone();
    assert!(forged.declarations.remove_type_parameters_for_test(&vector));
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
}
