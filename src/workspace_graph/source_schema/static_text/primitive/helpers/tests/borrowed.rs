//! Real standard-library byte-reader bodies; Option receiving admission remains closed.
use super::*;
fn provider(source: &str, module: &str, id: &str) -> String {
    let marker = format!("@id(\"{id}\")");
    let start = source.find(&marker).unwrap();
    let rest = &source[start..];
    let end = rest[marker.len()..]
        .find("\n@id(")
        .map_or(rest.len(), |n| n + marker.len());
    format!("module {module};\n{}", &rest[..end])
}
fn pure(source: &str) -> hir::ResolvedProgram {
    // Single-file checking requires an entry (`SPX-T105`); the appended entry
    // follows the provider in source order and is excluded from the shape scan.
    let source = format!("{source}\n@id(\"pure.fixture.main\") fn main()->i64 {{0}}\n");
    let program = hir::resolve(&crate::check(&source, "borrowed-reader.spx").unwrap()).unwrap();
    for function in program
        .functions
        .iter()
        .filter(|function| function.id.as_str() != "pure.fixture.main")
    {
        let mut remaining = 1_048_576;
        let mut debits = 0;
        assert!(
            scalar_helpers::pure_shape_metered(function, &mut remaining, &mut |units| {
                debits += units;
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(debits, 1_048_576 - remaining);
    }
    program
}
#[test]
fn actual_lines_reader_body_preserves_ordinary_while_and_exact_option_elimination() {
    let source = include_str!("../../../../../../../std/io-lines/src/lines.spx");
    let selected = provider(source, "std.io.lines", "std.io.lines.line-end");
    let program = pure(&selected);
    let function = &program.functions[0];
    assert!(!hir::lexical_exit::requires(function).unwrap());
    let local = format!(
        "{selected}\n@id(\"app.main\") fn main()->i64{{let text=str\"a\\n\\u{{0}}é\";let end=line_end(str_as_bytes(text),0usize);if end==1usize{{42}}else{{0}}}}"
    );
    let control = hir::resolve(&crate::check(&local, "lines-control.spx").unwrap()).unwrap();
    assert_eq!(
        crate::interpreter::evaluate_resolved_zero_arg_i64(&control, "app.main", 100_000)
            .unwrap()
            .outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    );
    // The one-iteration loop keeps the caller on the selected source route; a
    // loop-free return would normalize into an ordinary tail import.
    let caller_source = r#"module scalar.app;use function @id("std.io.lines.line-end") from std.io.lines as line_end;
@id("app.main") fn main()->i64 {let text=str"a\né";let end=line_end(str_as_bytes(text),0usize);let mut tick=0usize;while tick<1usize {tick=tick+1usize;return if end==1usize {42}else{0};} 0}"#;
    // Independent shape support grants no production Option receiving lease:
    // the reader retains no provider fact, so the selected entry refuses.
    let build = build_sources(&[("caller.spx", caller_source), ("lines.spx", &selected)]).unwrap();
    assert_selected_borrowed_inventory(&build);
    assert!(build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "std.io.lines")
        .unwrap()
        .scalar_helpers
        .is_none());
    assert!(schema(&build)
        .err()
        .unwrap_or_else(|| panic!("Option receiving route remains closed"))
        .iter()
        .any(|d| d.code == "SPX-G410"));
}
#[test]
fn actual_json_byte_reader_and_u8_conversion_have_exact_nonowning_shapes() {
    supported_length_provider_does_not_retain_unrelated_option_or_scalar_match_facts();
    let source = include_str!("../../../../../../../std/data-json-utf8/src/utf8.spx");
    for id in ["std.data.json.utf8.at_is", "std.data.json.utf8.at_in"] {
        let shape = pure(&provider(source, "std.data.json.utf8", id));
        let function = &shape.functions[0];
        let mut remaining = 1_048_576;
        let mut debits = 0;
        assert!(
            !scalar_helpers::pure_metered(function, &mut remaining, &mut |units| {
                debits += units;
                Ok(())
            })
            .unwrap()
        );
        // The original profile stops at Block then Match, before inspecting
        // alternative bodies or hashing/cloning a retained provider fact.
        assert_eq!(debits, function.params.len() + 1 + 2);
        assert_eq!(remaining, 1_048_576 - debits);
    }
    pure(
        r#"module scalar.leaf; @id("leaf.read") fn read(view:borrow Slice<u8>,at:usize)->i64 {
        match byte_get(view,at) {Option::Some {value}=>i64_from_u8(value),Option::None {}=>0,}}
"#,
    );
}
#[test]
fn byte_reader_borrow_modes_owned_roots_and_non_u8_payloads_remain_closed() {
    let source = r#"module scalar.leaf; @id("leaf.read") fn read(view:borrow Slice<u8>,at:usize)->i64 {match byte_get(view,at) {Option::Some {value}=>i64_from_u8(value),Option::None {}=>0,}}"#;
    let program = pure(source);
    for (mutation, label) in [
        (0, "owned parameter"),
        (1, "foreign root identity"),
        (2, "owned result"),
    ] {
        let mut function = program.functions[0].clone();
        match mutation {
            0 => function.params[0].ownership = hir::OwnershipMode::Own,
            1 => {
                function.params[0].id =
                    hir::ValueId::intrinsic_parameter("forged.borrow.generation", 0)
            }
            _ => function.return_type = hir::ResolvedType::Bytes,
        }
        let mut remaining = 1_048_576;
        assert!(
            !scalar_helpers::pure_shape_metered(&function, &mut remaining, &mut |_| Ok(()))
                .unwrap(),
            "mutation {mutation} ({label})"
        );
    }
}

#[test]
fn imported_exact_u8_conversion_keeps_saved_local_status_and_result_control() {
    let caller_source = r#"module scalar.app;use function @id("leaf.read") from scalar.leaf as read;
@id("app.main") fn main()->i64 {let label=str"é\u{0}";return read(41u8);}"#;
    let leaf_source =
        r#"module scalar.leaf;@id("leaf.read") fn read(value:u8)->i64 {i64_from_u8(value)+1}"#;
    let local_source = r#"module scalar.local;@id("leaf.read") fn read(value:u8)->i64 {i64_from_u8(value)+1}
@id("app.main") fn main()->i64 {let label=str"é\u{0}";return read(41u8);}"#;
    let build = build_sources(&[("caller.spx", caller_source), ("leaf.spx", leaf_source)]).unwrap();
    let linked = build
        .linked_owned_data_api_program_with_roots("scalar.app", &[])
        .unwrap();
    let control =
        hir::resolve(&crate::check(local_source, "conversion-control.spx").unwrap()).unwrap();
    runtime::run(&linked, &control);
}

fn supported_length_provider_does_not_retain_unrelated_option_or_scalar_match_facts() {
    let caller_source = r#"module scalar.app;use function @id("leaf.length") from scalar.leaf as length;
@id("app.main") fn main()->i64 {let text=str"abc";return if length(str_as_bytes(text))==3usize {42}else{0};}"#;
    let provider_source = r#"module scalar.leaf;
@id("leaf.length") fn length(view:borrow Slice<u8>)->usize {byte_len(view)}
@id("leaf.unused") fn unused(view:borrow Slice<u8>)->bool {match byte_get(view,0usize) {Option::Some {value}=>value==0u8,Option::None {}=>false,}}
@id("leaf.unused.scalar") fn scalar(value:bool)->i64 {match value {true=>1,_=>0,}}
"#;
    let build =
        build_sources(&[("caller.spx", caller_source), ("leaf.spx", provider_source)]).unwrap();
    let module = build
        .hir
        .modules
        .iter()
        .find(|m| m.module == "scalar.leaf")
        .unwrap();
    let inventory = module.scalar_helpers.as_ref().unwrap();
    assert_eq!(
        inventory
            .functions
            .iter()
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        ["leaf.length"]
    );
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
}
