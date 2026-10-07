use super::*;
use crate::{graph, hir, parse, verify};

const SOURCE: &str = r#"
module test.stdin_stream;
permit { process.stdin.read }
@id("stream.advance")
fn advance(reader: own StdinReader) -> StdinReader uses { process.stdin.read } {
    let ignored = { let chunk = stdin_stream_chunk(reader); let size = byte_len(chunk); 0 };
    stdin_stream_next(reader)
}
@id("stream.run")
fn run() -> bool uses { process.stdin.read } {
    let mut reader = stdin_stream_open();
    let mut total = 0usize;
    while !stdin_stream_eof(reader) {
        let ignored = {
            let chunk = stdin_stream_chunk(reader);
            total = total + byte_len(chunk);
            0
        };
        reader = advance(reader);
        0
    }
    total >= 0usize
}
@id("app.main") fn main() -> i64 { 0 }
"#;
fn ast(source: &str) -> crate::ast::Program {
    parse(source, "stdin-stream.spx").unwrap()
}
fn checked() -> hir::ResolvedProgram {
    let parsed = ast(SOURCE);
    let diagnostics = verify::verify(&parsed);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&parsed).unwrap()
}
fn refused(source: &str, code: &str) {
    let diagnostics = verify::verify(&ast(source));
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {diagnostics:?}"));
    assert_eq!(diagnostic.path.as_deref(), Some("stdin-stream.spx"));
    assert!(diagnostic.span.is_some());
}
#[test]
fn signatures_preserve_unique_reader_and_fixed_capacity() {
    let open = ResolvedHostCommandOperation::StdinStreamOpen;
    let next = ResolvedHostCommandOperation::StdinStreamNext;
    assert!(host_params(open).is_empty());
    let params = resolved_host_params(next);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].ownership, OwnershipMode::Own);
    assert!(is_reader(&params[0].ty));
    let facts = type_facts(&DeclarationId::new(READER_ID), &[]).unwrap();
    assert!(!facts.copy && !facts.contains_resource && facts.needs_drop);
    assert!(type_facts(&DeclarationId::new(READER_ID), &[ResolvedType::Bytes]).is_none());
    assert_eq!(CHUNK_BYTES, 4096);
}
#[test]
fn source_hir_graph_and_renewal_keep_exact_streaming_facts() {
    let program = checked();
    hir::validate(&program).unwrap();
    let facts = analysis::derive(&program).unwrap();
    assert_eq!(facts[&DeclarationId::new("stream.run")].open_bound, 1);
    assert_eq!(facts[&DeclarationId::new("stream.advance")].open_bound, 0);
    assert!(facts[&DeclarationId::new("stream.advance")]
        .forwarding_parameter
        .is_some());
    let run = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "stream.run")
        .unwrap();
    let views = run
        .loan_plan
        .loans
        .iter()
        .filter(|loan| loan.cause == crate::loan_plan::LoanCause::SliceView)
        .collect::<Vec<_>>();
    assert_eq!(views.len(), 1);
    assert!(views[0].origin.projections.is_empty());
    let reopens = reader_reopens(run);
    assert_eq!(reopens.len(), 1);
    let provenances = program
        .declarations
        .byte_slice_provenances()
        .filter(|(_, p)| p.root_kind == hir::ByteSliceRootKind::StdinStreamReader)
        .collect::<Vec<_>>();
    assert_eq!(provenances.len(), 2);
    for (_, provenance) in provenances {
        assert_eq!(provenance.projected_type, reader());
        assert!(provenance.projections.is_empty());
        assert_eq!(provenance.root_length, hir::ByteSliceExtent::ValueLength);
    }
    let parsed = ast(SOURCE);
    let canonical = crate::format::canonical(&parsed);
    let reparsed = ast(&canonical);
    assert_eq!(canonical, crate::format::canonical(&reparsed));
    assert_eq!(
        crate::prelude::selected_for_program(&parsed).0,
        "semaprax.prelude.v10"
    );
    let document: serde_json::Value =
        serde_json::from_str(&graph::to_json(&parsed).unwrap()).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v65");
    assert_eq!(document["stdin_stream"]["buffer_bytes"], 4096);
    assert_eq!(document["stdin_stream"]["buffer_count"], 1);
    assert_eq!(document["stdin_stream"]["open_prefills"], true);
    assert_eq!(document["stdin_stream"]["failure"]["code"], 3);
}
#[test]
fn direct_reader_refill_retains_same_owner_and_replayed_cleanup() {
    let parsed = ast(&SOURCE.replace(
        "reader = advance(reader);",
        "reader = stdin_stream_next(reader);",
    ));
    let diagnostics = verify::verify(&parsed);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = hir::resolve(&parsed).unwrap();
    hir::validate(&program).unwrap();
    let run = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "stream.run")
        .unwrap();
    assert_eq!(reader_reopens(run).len(), 1);
}
#[test]
fn singleton_open_and_sealed_reader_refusals_have_source_locations() {
    refused(
        &SOURCE.replace(
            "let mut total = 0usize;",
            "let another = stdin_stream_open(); let mut total = 0usize;",
        ),
        "SPX-T270",
    );
    refused(
        &SOURCE.replace(
            "reader = advance(reader);",
            "let another = stdin_stream_open(); reader = advance(reader);",
        ),
        "SPX-T270",
    );
    refused(
        &SOURCE.replace(
            "let mut reader = stdin_stream_open();",
            "let mut reader = StdinReader {};",
        ),
        "SPX-T270",
    );
    refused(
        &SOURCE.replace(
            "fn advance(reader: own StdinReader)",
            "fn advance(reader: shared StdinReader)",
        ),
        "SPX-T270",
    );
    refused(
        &SOURCE.replace("stdin_stream_next(reader)\n}", "advance(reader)\n}"),
        "SPX-T270",
    );
}
#[test]
fn chunk_cannot_be_used_after_reader_refill() {
    let source = r#"module test.expired;
permit { process.stdin.read }
@id("stream.run") fn run() -> bool uses { process.stdin.read } {
    let mut reader = stdin_stream_open();
    let chunk = stdin_stream_chunk(reader);
    reader = stdin_stream_next(reader);
    byte_len(chunk) == 0usize
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    refused(source, "SPX-T265");
}
#[test]
fn stream_profile_is_explicit_and_old_command_profile_stays_closed() {
    let program = checked();
    let command = DeclarationId::new("stream.run");
    crate::command_io_ops::validate_operation_profile(
        &program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )
    .unwrap();
    let refusal = crate::command_io_ops::validate_operation_profile(
        &program,
        &command,
        crate::command_io_ops::CommandOperationProfile::LanguageV1,
    )
    .unwrap_err();
    assert_eq!(refusal.code, "SPX-W114");
    let ordinary = ast("module test.old; @id(\"app.main\") fn main() -> i64 { 0 }");
    assert_ne!(
        crate::prelude::selected_for_program(&ordinary).0,
        "semaprax.prelude.v10"
    );
    let document: serde_json::Value =
        serde_json::from_str(&graph::to_json(&ordinary).unwrap()).unwrap();
    assert!(document.get("stdin_stream").is_none());
}
#[test]
fn forged_hir_reader_construction_is_rejected_before_cleanup_replay() {
    let mut program = checked();
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "stream.advance")
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!()
    };
    tail.kind = hir::ResolvedExprKind::ConstructRecord {
        record: DeclarationId::new(READER_ID),
        fields: Vec::new(),
    };
    let refusal = hir::validate(&program).unwrap_err();
    assert_eq!(refusal.code, "SPX-H006");
    assert!(refusal.message.contains("construction"));
}
#[test]
fn linked_forwarding_helper_retains_sealed_reader_metadata() {
    let program = checked();
    let linked = program
        .functions
        .into_iter()
        .map(|function| hir::LinkedScalarFunction {
            function,
            origin: hir::IdentityOrigin::Explicit,
        })
        .collect();
    let linked = hir::link_stdin_stream_command_workspace(
        "linked.stream".to_owned(),
        DeclarationId::new("app.main"),
        DeclarationId::new("stream.run"),
        linked,
    )
    .unwrap();
    assert!(linked.types.iter().any(|ty| ty.id.as_str() == READER_ID));
    assert!(linked
        .declarations
        .declaration(&DeclarationId::new(READER_ID))
        .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::CompilerOwned));
    hir::validate(&linked).unwrap();
    assert!(
        analysis::derive(&linked).unwrap()[&DeclarationId::new("stream.advance")]
            .forwarding_parameter
            .is_some()
    );
}
#[test]
fn native_open_eof_without_chunk_retains_stream_epoch_carrier() {
    let parsed = ast(r#"module test.stream_empty;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("stream.run") fn run() -> bool uses { process.stdin.read } {
    let reader = stdin_stream_open();
    stdin_stream_eof(reader)
}
@id("app.main") fn main() -> i64 { 0 }
"#);
    let program = hir::resolve(&parsed).unwrap();
    let run = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "stream.run")
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &run.body.kind else {
        panic!()
    };
    assert!(
        matches!(&tail.kind, hir::ResolvedExprKind::Call { callee, .. } if callee.as_str() == EOF_ID)
    );
    // Borrow-only inspection adds no operation failure source; the opened
    // Reader still remains in its lexical slot for canonical finalization.
    assert!(run
        .cleanup_plan
        .status_sources
        .iter()
        .all(|source| source.id.expression != tail.id));
    let reader_slot = run
        .cleanup_plan
        .slots
        .iter()
        .find(|slot| {
            crate::stdin_stream_ops::is_reader(&slot.ty)
                && matches!(&slot.storage, crate::cleanup_plan::StorageId::Value(_))
        })
        .expect("the owned Reader must retain its canonical cleanup slot");
    let crate::cleanup::FieldLivenessShape::Leaf { flag, lifecycle } =
        &reader_slot.field_liveness_shape
    else {
        panic!("the sealed Reader cleanup slot must be a primitive leaf")
    };
    assert_eq!(lifecycle.as_str(), DROP_ID);
    hir::validate(&program).unwrap();
    let emitted = crate::codegen::emit_hir_c_with_stdin_stream(&program, "stream.run").unwrap();
    assert!(emitted.contains("spx_slice_u8_v1"));
    assert!(emitted.contains("captured_epoch"));
    assert!(emitted.contains("spx_stdin_stream"));
    let run_symbol = format!(
        "spx_decl_{}",
        "stream.run"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let run_signature = emitted
        .find(&format!(
            "static __attribute__((unused)) spx_status_token {run_symbol}("
        ))
        .expect("native lowering must emit stream.run");
    let run_body = emitted[run_signature..]
        .find(") {")
        .map(|end| run_signature + end + 3)
        .expect("native lowering must emit a stream.run definition after its prototype");
    let run_end = emitted[run_body..]
        .find("\n}\n")
        .map(|end| run_body + end)
        .expect("stream.run must have a closed C body");
    let run_c = &emitted[run_body..run_end];
    let reader_drop = format!(
        "spx_stdin_stream_drop_v1(spx_ctx, spx_bytes_slot_{});",
        reader_slot.id.0
    );
    let drop_at = run_c
        .find(&reader_drop)
        .expect("native lowering must emit the canonical Reader finalizer");
    assert_eq!(run_c.matches(&reader_drop).count(), 1);
    let publish_at = run_c
        .find("*spx_result_out = spx_result;")
        .expect("native lowering must publish the scalar result");
    assert!(
        drop_at < publish_at,
        "the canonical Reader drop must precede result publication"
    );
    assert!(run_c.contains(&format!("if (spx_bytes_live_{})", flag.0)));
}
#[test]
fn guard_calls_participate_in_effect_authority() {
    let parsed = ast(r#"module test.guard_authority;
permit { process.stdin.read }
@id("stream.run") fn run() -> bool {
    match true {
        true if { let reader = stdin_stream_open(); stdin_stream_eof(reader) } => true,
        _ => false,
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#);
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().any(|d| d.code == "SPX-E102"),
        "{diagnostics:?}"
    );
}

#[test]
fn forged_inspection_temporary_and_mutated_chunk_root_are_refused() {
    let parsed = ast(r#"module test.stream_shape;
permit { process.stdin.read }
@id("stream.run") fn run() -> bool uses { process.stdin.read } {
    let reader = stdin_stream_open(); stdin_stream_eof(reader)
}
@id("app.main") fn main() -> i64 { 0 }
"#);
    let mut program = hir::resolve(&parsed).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "stream.run")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, tail } = &mut function.body.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { value: open, .. } = &statements[0] else {
        panic!()
    };
    let hir::ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        panic!()
    };
    // Preserve the canonical argument identity; only replace its physical shape.
    args[0].kind = open.kind.clone();
    let refusal = hir::validate(&program).unwrap_err();
    assert_eq!(refusal.code, "SPX-H006");
    let mut program = checked();
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "stream.advance")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { value: inner, .. } = &mut statements[0] else {
        panic!()
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut inner.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { value: chunk, .. } = &mut statements[0] else {
        panic!()
    };
    let hir::ResolvedExprKind::BorrowPlace { operation, .. } = &mut chunk.kind else {
        panic!()
    };
    *operation = DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID);
    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn legacy_native_profile_refuses_forwarding_only_reader_signatures() {
    let parsed = ast(r#"module test.reader_profile_boundary;
@id("stream.forward") fn forward(reader: own StdinReader) -> StdinReader { reader }
@id("app.main") fn main() -> i64 { 0 }
"#);
    let program = hir::resolve(&parsed).unwrap();
    assert!(
        analysis::derive(&program).unwrap()[&DeclarationId::new("stream.forward")]
            .forwarding_parameter
            .is_some()
    );
    let refusal = crate::codegen::emit_hir_c(&program).unwrap_err();
    // The early profile boundary uses the native backend's B103 diagnostic,
    // before carrier/runtime emission, even without a stream operation site.
    assert_eq!(refusal.code, "SPX-B103");
    assert_eq!(
        refusal.message,
        "stdin reader requires the explicit native streaming-command profile"
    );
}
