use super::*;

const COLLECTION_SOURCE: &str = r#"
module test.borrowed_collection_call_hir;
@id("collection.carrier") record Carrier {
  @id("collection.words") words: Map<i64,string>,
  @id("collection.keys") keys: Set<i64>,
  @id("collection.title") title: string,
  @id("collection.old.words") old_words: Map<string,i64>,
}
@id("collection.read") fn read(words: borrow Map<i64,string>) -> i64 {
  string_len(map_get_or<i64,string>(words,3,"missing"))
}
@id("collection.legacy") fn legacy(words: borrow Map<string,i64>) -> usize { map_len(words) }
@id("collection.keys.read") fn keys_read(keys: borrow Set<i64>) -> usize { set_len<i64>(keys) }
@id("collection.inspect") fn inspect(carrier: borrow Carrier) -> i64 {
  read(carrier.words) + read(carrier.words) + string_len(carrier.title)
}
@id("collection.take") fn take(carrier: own Carrier) -> Map<i64,string> {
  match own carrier { Carrier{words,keys,title,old_words} => words, }
}
@id("collection.take.title") fn take_title(carrier: own Carrier) -> string {
  match own carrier { Carrier{words,keys,title,old_words} => title, }
}
@id("collection.take.legacy") fn take_legacy(carrier: own Carrier) -> Map<string,i64> {
  match own carrier { Carrier{words,keys,title,old_words} => old_words, }
}
@id("collection.main") fn main() -> i64 {
  let words0=map_new<i64,string>(1usize);
  let words=map_set<i64,string>(words0,3,"abc");
  let before=read(words);
  let after=read(words);
  let keys=set_new<i64>(1usize);
  let count=keys_read(keys);
  let again=keys_read(keys);
  let old=map_new(1usize);
  let old_count=legacy(old);
  let old_again=legacy(old);
  let carrier=Carrier{words:words,keys:keys,title:"tag",old_words:old};
  let inspected=inspect(carrier)+inspect(carrier);
  let extracted=take(carrier);
  before + after + inspected + read(extracted)
}
"#;

#[test]
fn collection_borrow_parameters_and_projected_reads_preserve_owner_availability() {
    let parsed = crate::check(COLLECTION_SOURCE, "borrowed-collection-call-hir.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    for id in [
        "collection.read",
        "collection.legacy",
        "collection.keys.read",
        "collection.inspect",
    ] {
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        assert_eq!(function.params[0].ownership, OwnershipMode::Borrow, "{id}");
    }
    crate::hir::validate(&program).unwrap();

    let mut moved = program.clone();
    moved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "collection.read")
        .unwrap()
        .params[0]
        .ownership = OwnershipMode::Own;
    assert_eq!(crate::hir::validate(&moved).unwrap_err().code, "SPX-H006");
}

const SOURCE: &str = r#"
module test.borrowed_bytes_call_hir;
@id("packet.type") record Packet {
  @id("packet.payload") payload: Bytes,
  @id("packet.sibling") sibling: Bytes,
}
@id("packet.inspect") fn inspect(value: borrow Bytes) -> usize {
  byte_len(bytes_as_slice(value))
}
@id("packet.caller") fn caller(packet: own Packet) -> usize { inspect(packet.payload) }
@id("app.main") fn main() -> i64 { 0 }
"#;

fn fixture() -> ResolvedProgram {
    let parsed = crate::parse(
        SOURCE,
        std::path::Path::new("borrowed-bytes-call-hir-v1.spx"),
    )
    .expect("fixture parses");
    let program = crate::hir::resolve(&parsed).expect("fixture resolves");
    crate::hir::validate(&program).expect("fixture validates");
    program
}

fn caller(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.caller")
        .expect("caller function")
}

fn caller_mut(program: &mut ResolvedProgram) -> &mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "packet.caller")
        .expect("caller function")
}

fn call_argument_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("caller body remains a block")
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        panic!("caller tail remains a call")
    };
    &mut args[0]
}

#[test]
fn resolver_preserves_borrowed_bytes_and_attaches_projected_call_loan_identity() {
    let program = fixture();
    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.inspect")
        .expect("inspect function");
    assert_eq!(inspect.params[0].ownership, OwnershipMode::Borrow);

    let loan = caller(&program)
        .loan_plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::BorrowedCall { argument: 0 })
        .expect("borrowed call loan");
    assert_eq!(
        loan.origin.projections,
        [PlaceProjection::Field(DeclarationId::new("packet.payload"))]
    );
    assert_eq!(loan.start.phase, LoanPointPhase::Before);
    assert!(!loan.end_edges.is_empty());
}

#[test]
fn hostile_call_place_and_attached_loan_field_identity_fail_closed() {
    let mut forged_hir = fixture();
    let argument = call_argument_mut(caller_mut(&mut forged_hir));
    let ResolvedExprKind::Place(place) = &mut argument.kind else {
        panic!("borrowed argument remains a place")
    };
    place.projections = vec![PlaceProjection::Field(DeclarationId::new("packet.sibling"))];
    let diagnostic = crate::hir::validate(&forged_hir).expect_err("HIR drift must fail");
    assert_eq!(diagnostic.code, "SPX-H006");

    let mut forged_plan = fixture();
    let loan = caller_mut(&mut forged_plan)
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| loan.cause == LoanCause::BorrowedCall { argument: 0 })
        .expect("borrowed call loan");
    loan.origin.projections = vec![PlaceProjection::Field(DeclarationId::new("packet.sibling"))];
    let diagnostic = crate::hir::validate(&forged_plan).expect_err("loan identity drift must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
}

#[test]
fn hostile_non_place_borrowed_bytes_argument_is_h006() {
    let mut program = fixture();
    let argument = call_argument_mut(caller_mut(&mut program));
    argument.kind = ResolvedExprKind::Int(0);
    let diagnostic = crate::hir::validate(&program).expect_err("non-place must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
}

#[test]
fn ownership_mismatch_uses_argument_span_and_keeps_missing_span_locationless() {
    let mut program = fixture();
    let argument_span;
    {
        let argument = call_argument_mut(caller_mut(&mut program));
        argument.kind = ResolvedExprKind::Int(0);
        argument_span = argument.span;
    }
    let caller = caller(&program);
    let ResolvedExprKind::Block { tail, .. } = &caller.body.kind else {
        panic!("caller body remains a block")
    };
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        panic!("caller body tail remains a call")
    };
    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.inspect")
        .expect("inspect function");
    let validator = HirValidator::new(&program).expect("fixture identities remain indexed");
    let diagnostic = validator
        .validate_argument_ownership(&args[0], &inspect.params[0])
        .expect_err("forged argument ownership must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        format!(
            "argument ownership is incompatible with parameter `{}`",
            inspect.params[0].id
        )
    );
    assert_eq!(diagnostic.span, Some(argument_span));

    let mut missing_span = args[0].clone();
    missing_span.span = crate::ast::Span::default();
    let diagnostic = validator
        .validate_argument_ownership(&missing_span, &inspect.params[0])
        .expect_err("forged argument ownership must still fail");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(diagnostic.span, None);
}

#[test]
fn borrowed_bytes_call_shape_uses_call_span_and_keeps_missing_span_locationless() {
    let program = fixture();
    let caller = caller(&program);
    let ResolvedExprKind::Block { tail, .. } = &caller.body.kind else {
        panic!("caller body remains a block")
    };
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        panic!("caller body tail remains a call")
    };
    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.inspect")
        .expect("inspect function");
    let validator = HirValidator::new(&program).expect("fixture identities remain indexed");

    let mut malformed_call = tail.clone();
    malformed_call.kind = ResolvedExprKind::Int(0);
    let diagnostic = validator
        .validate_borrowed_bytes_call_argument(
            &malformed_call,
            &args[0],
            &inspect.params[0],
            0,
            &std::collections::BTreeMap::new(),
        )
        .expect_err("forged call shape must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "borrowed Bytes argument is not attached to an exact call"
    );
    assert_eq!(diagnostic.span, Some(tail.span));

    malformed_call.span = crate::ast::Span::default();
    let diagnostic = validator
        .validate_borrowed_bytes_call_argument(
            &malformed_call,
            &args[0],
            &inspect.params[0],
            0,
            &std::collections::BTreeMap::new(),
        )
        .expect_err("forged call shape without a source span must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(diagnostic.span, None);
}

const FUSED_STRING_VIEW: &str = r#"
module test.fused_string_byte_view;
@id("bytes.measure") fn measure(text: string) -> usize {
  let bytes = str_as_bytes(string_as_str(text));
  byte_len(bytes)
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn fused_string_byte_view_has_rooted_graph_fact_and_full_owner_loan() {
    let parsed = crate::parse(FUSED_STRING_VIEW, "fused-string-byte-view.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "bytes.measure")
        .unwrap();
    let owner = function.params[0].id.clone();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("function body is a block")
    };
    let ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("first statement binds the fused view")
    };
    let (slice, provenance) = program
        .declarations
        .byte_slice_provenances()
        .find(|(_, provenance)| provenance.root_kind == crate::hir::ByteSliceRootKind::OwnedString)
        .expect("fused view retains an owned-String provenance root");
    assert_eq!(provenance.root, owner);
    assert_eq!(provenance.projected_type, ResolvedType::String);
    assert!(provenance.projections.is_empty());
    assert_eq!(
        provenance.root_length,
        crate::hir::ByteSliceExtent::ValueLength
    );
    let producer = provenance
        .producer
        .as_ref()
        .expect("view producer is recorded");
    let loan = function
        .loan_plan
        .loans
        .iter()
        .find(|loan| loan.site == *producer && loan.cause == LoanCause::SliceView)
        .expect("SliceView loan protects the owner");
    assert_eq!(loan.origin.root, owner);
    assert!(!loan.end_edges.is_empty());
    assert_eq!(slice, &binding.id);

    let graph = crate::graph::to_json(&parsed).unwrap();
    let graph_value: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(graph_value["schema"], "semaprax.graph.v71");
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v71"
    );
    assert_eq!(
        crate::graph::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances,
        )
        .unwrap(),
        "semaprax.graph.v71"
    );
    assert!(graph.contains("\"root_kind\":\"owned_string\""));
    let stale_schema = graph.replacen("semaprax.graph.v71", "semaprax.graph.v17", 1);
    assert!(crate::graph::verify_json(&parsed, &stale_schema).is_err());
    let forged_root = graph.replacen(
        "\"root_kind\":\"owned_string\"",
        "\"root_kind\":\"borrowed_str\"",
        1,
    );
    assert!(crate::graph::verify_json(&parsed, &forged_root).is_err());
    assert!(crate::graph::to_legacy_json(&parsed).is_err());
}

#[test]
fn existing_borrowed_str_byte_view_keeps_graph_v17() {
    let source = r#"
module test.existing_borrowed_str_byte_view;
@id("bytes.measure") fn measure(text: borrow str) -> usize {
  byte_len(str_as_bytes(text))
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, "existing-borrowed-str-byte-view.spx").unwrap();
    let graph = crate::graph::to_json(&parsed).unwrap();
    let value: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v17");
}

#[test]
fn fused_string_view_from_owned_record_pattern_selects_graph_v71_from_parts() {
    let source = r#"
module test.fused_string_pattern_byte_view;
@id("carrier.type") record Carrier { @id("carrier.text") text: string, }
@id("bytes.measure") fn measure(carrier: own Carrier) -> usize {
  match own carrier {
    Carrier{text} => byte_len(str_as_bytes(string_as_str(text))),
  }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, "fused-string-pattern-byte-view.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&program).unwrap();
    assert_eq!(
        crate::graph::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances,
        )
        .unwrap(),
        "semaprax.graph.v71"
    );
}

#[test]
fn fused_string_byte_view_rejects_a_forged_operation_on_the_string_root() {
    let parsed = crate::parse(FUSED_STRING_VIEW, "fused-string-byte-view-hostile.spx").unwrap();
    let mut program = crate::hir::resolve(&parsed).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "bytes.measure")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("fused view is a local initializer")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("first statement binds the fused view")
    };
    let ResolvedExprKind::BorrowPlace { operation, .. } = &mut value.kind else {
        panic!("fused view resolves to authenticated BorrowPlace HIR")
    };
    *operation = DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID);
    let diagnostic = crate::hir::validate(&program)
        .expect_err("a different operation cannot borrow from a String root");
    assert_eq!(diagnostic.code, "SPX-H006");
}

#[test]
fn fused_string_byte_view_keeps_the_string_owner_borrowed_until_last_use() {
    let source = r#"
module test.fused_string_byte_view_mutation;
@id("bytes.measure") fn measure() -> usize {
  let mut text = "before";
  let bytes = str_as_bytes(string_as_str(text));
  text = "after";
  byte_len(bytes)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, "fused-string-byte-view-mutation.spx").unwrap();
    let diagnostics = crate::verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T265"),
        "mutation of the borrowed String must remain rejected: {diagnostics:?}"
    );
}

#[test]
fn borrowed_view_place_uses_expression_span_and_keeps_missing_span_locationless() {
    let program = fixture();
    let caller = caller(&program);
    let ResolvedExprKind::Block { tail, .. } = &caller.body.kind else {
        panic!("caller body remains a block")
    };
    let validator = HirValidator::new(&program).expect("fixture identities remain indexed");
    let missing = Place {
        root: ValueId::new("missing".to_owned()),
        projections: Vec::new(),
    };
    let scope = std::collections::BTreeMap::new();

    let diagnostic = validator
        .validate_byte_view_place(
            crate::byte_ops::ByteOp::StringAsStr,
            &missing,
            tail.span,
            &scope,
        )
        .expect_err("out-of-scope borrowed view must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(diagnostic.message, "borrowed view root is out of scope");
    assert_eq!(diagnostic.span, Some(tail.span));

    let diagnostic = validator
        .validate_byte_view_place(
            crate::byte_ops::ByteOp::StringAsStr,
            &missing,
            crate::ast::Span::default(),
            &scope,
        )
        .expect_err("out-of-scope borrowed view without span must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(diagnostic.span, None);
}
