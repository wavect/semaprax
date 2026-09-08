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
    let missing = APP.replace(
        "use type @id(\"reader.type\") from reader.provider as Reader;",
        "",
    );
    let errors = build_owned(sources(&missing, PROVIDER))
        .err()
        .expect("direct type import required");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));
    let generic = PROVIDER.replace(
        "fn advance(value:own Reader)",
        "fn advance<T>(value:own Reader)",
    );
    let errors = build_owned(sources(APP, &generic))
        .err()
        .expect("generic import remains closed");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));
}

#[test]
fn internal_owned_record_import_rejects_same_shape_wrong_identity() {
    let provider = format!("{PROVIDER}\n@id(\"reader.other\") record Other {{ @id(\"other.data\") data: Bytes, @id(\"other.cursor\") cursor:usize, }}\n");
    let app = APP.replace(
        "use type @id(\"reader.type\")",
        "use type @id(\"reader.other\")",
    );
    let errors = build_owned(sources(&app, &provider))
        .err()
        .expect("same shape does not substitute identity");
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));
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
    for hostile in [
        app.replace(
            "use type @id(\"reader.info\") from reader.provider as Info;",
            "",
        ),
        app.replace(
            "use type @id(\"reader.info\")",
            "use type @id(\"reader.other-info\")",
        ),
    ] {
        let errors = build_owned(sources(&hostile, &provider))
            .err()
            .expect("Copy result identity is not interchangeable");
        assert!(errors.iter().any(|error| error.code == "SPX-G172"));
    }
    let borrowed = provider.replace(
        "fn inspect(value:own Reader)",
        "fn inspect(value:borrow Reader)",
    );
    assert!(
        build_owned(sources(app, &borrowed)).is_err(),
        "new mixed-result lane requires owned input"
    );
    let generic = provider.replace(
        "fn inspect(value:own Reader)",
        "fn inspect<T>(value:own Reader)",
    );
    assert!(
        build_owned(sources(app, &generic)).is_err(),
        "generic import remains closed"
    );
}
