//! Selects the exact shared prelude declaration inventory retained by a workspace.

use super::*;

pub(super) fn uses_vec(programs: &[Program]) -> bool {
    programs.iter().any(prelude::program_uses_vec)
}
pub(super) fn uses_box(programs: &[Program]) -> bool {
    programs.iter().any(prelude::program_uses_box)
}
pub(super) fn uses_iterator(programs: &[Program]) -> bool {
    programs
        .iter()
        .any(crate::iterator_ops::program_uses_iterator)
}
pub(super) fn uses_list(programs: &[Program]) -> bool {
    programs.iter().any(crate::list_ops::program_uses_list)
}
pub(super) fn uses_record_iterator(programs: &[Program]) -> bool {
    programs
        .iter()
        .any(crate::source_verify::program_uses_record_iterator)
}

pub(super) fn uses_stream(programs: &[Program]) -> bool {
    programs.iter().any(crate::stdin_stream_ops::program_uses)
}

/// Synthetic direct-import stubs select the provider's Reader signature too.
/// Derive that selection from authored source, never from retained HIR facts.
pub(super) fn module_uses_stream(source: &Program, programs: &[Program]) -> bool {
    crate::stdin_stream_ops::program_uses(source)
        || source.module_uses.iter().any(|import| {
            import.kind == ModuleUseKind::Function
                && programs
                    .iter()
                    .filter(|program| program.module == import.target_module)
                    .flat_map(|program| &program.functions)
                    .any(|function| {
                        function.stable_id == import.persistent_id
                            && (crate::stdin_stream_ops::ast_type_uses(&function.return_type)
                                || function
                                    .params
                                    .iter()
                                    .any(|param| crate::stdin_stream_ops::ast_type_uses(&param.ty)))
                    })
        })
}

pub(super) fn ids(programs: &[Program], resolved_record_iterator: bool) -> BTreeSet<&'static str> {
    if resolved_record_iterator || uses_record_iterator(programs) || uses_stream(programs) {
        let mut ids = prelude::all_type_ids_v9()
            .into_iter()
            .collect::<BTreeSet<_>>();
        ids.insert(crate::stdin_stream_ops::READER_ID);
        return ids;
    }
    if uses_list(programs) {
        prelude::all_type_ids_v9().into_iter().collect()
    } else if uses_iterator(programs) {
        prelude::all_type_ids_v7().into_iter().collect()
    } else if uses_box(programs) {
        prelude::all_type_ids_v4().into_iter().collect()
    } else if uses_vec(programs) {
        prelude::all_type_ids_v2().into_iter().collect()
    } else {
        prelude::all_ids_v1().into_iter().collect()
    }
}

pub(super) fn ids_from_facts(
    programs: &[Program],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> BTreeSet<&'static str> {
    ids(
        programs,
        facts.contains_key(crate::stdin_stream_ops::READER_ID),
    )
}

pub(super) fn expected_declaration_facts_for_programs(
    programs: &[Program],
    resolved_record_iterator: bool,
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    expected_declaration_facts_for(
        uses_vec(programs),
        uses_box(programs),
        uses_iterator(programs),
        uses_list(programs),
        uses_stream(programs),
        resolved_record_iterator,
    )
}

pub(super) fn expected_declaration_facts(
    include_vec: bool,
    include_iterator: bool,
    include_list: bool,
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let mut facts = BTreeMap::new();
    for declaration in prelude::declarations().iter().filter(|declaration| {
        declaration.stable_id != crate::stdin_stream_ops::READER_ID
            && declaration.stable_id != prelude::BOX_ID
            && (include_vec || declaration.stable_id != prelude::VEC_ID)
            && (include_iterator
                || !matches!(
                    declaration.stable_id.as_str(),
                    crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID
                ))
            && (include_list
                || !matches!(
                    declaration.stable_id.as_str(),
                    crate::list_ops::LIST_ID | crate::list_ops::STEP_ID
                ))
    }) {
        let kind = match &declaration.kind {
            TypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
            TypeDeclarationKind::Class { .. } => hir::DeclarationKind::Class,
            TypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
            TypeDeclarationKind::Resource { .. } => {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "compiler prelude unexpectedly declares a resource authority",
                )]);
            }
        };
        insert_expected_compiler_declaration(&mut facts, &declaration.stable_id, kind, None)?;
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    insert_expected_compiler_declaration(
                        &mut facts,
                        &field.stable_id,
                        hir::DeclarationKind::Field,
                        Some(&declaration.stable_id),
                    )?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    insert_expected_compiler_declaration(
                        &mut facts,
                        &case.stable_id,
                        hir::DeclarationKind::VariantCase,
                        Some(&declaration.stable_id),
                    )?;
                    for field in &case.fields {
                        insert_expected_compiler_declaration(
                            &mut facts,
                            &field.stable_id,
                            hir::DeclarationKind::CaseField,
                            Some(&case.stable_id),
                        )?;
                    }
                }
            }
            TypeDeclarationKind::Resource { .. } => unreachable!("resource rejected above"),
        }
    }
    Ok(facts)
}

pub(super) fn expected_declaration_facts_for(
    include_vec: bool,
    include_box: bool,
    include_iterator: bool,
    include_list: bool,
    include_stream: bool,
    include_record_iterator: bool,
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    // Stream v10 retains v9; record-iterator v11 retains the complete v10
    // inventory, including the independently versioned Reader declaration.
    let complete_v10 = include_stream || include_record_iterator;
    let include_vec = include_vec || complete_v10;
    let include_box = include_box || complete_v10;
    let include_iterator = include_iterator || complete_v10;
    let include_list = include_list || complete_v10;
    // Prelude v4 is additive over the Vec-bearing v2/v3 predecessors, so a
    // Box-selected module retains the exact Vec declaration as well.
    let mut facts = expected_declaration_facts(
        include_vec || include_box || include_iterator || include_list,
        include_iterator || include_list,
        include_list,
    )?;
    if include_box || include_iterator || include_list {
        let declaration = prelude::declarations()
            .iter()
            .find(|d| d.stable_id == prelude::BOX_ID)
            .expect("Box prelude declaration");
        insert_expected_compiler_declaration(
            &mut facts,
            &declaration.stable_id,
            hir::DeclarationKind::Record,
            None,
        )?;
    }
    if complete_v10 {
        insert_expected_compiler_declaration(
            &mut facts,
            crate::stdin_stream_ops::READER_ID,
            hir::DeclarationKind::Record,
            None,
        )?;
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, text: &str) -> WorkspaceSource {
        let parsed = crate::parse(text, path).unwrap();
        WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&parsed),
        }
    }

    #[test]
    fn independent_stream_prelude_keeps_exact_v10_and_legacy_facts() {
        let legacy =
            expected_declaration_facts_for(false, false, false, false, false, false).unwrap();
        assert_eq!(
            legacy.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            prelude::all_ids_v1().into_iter().collect()
        );
        assert!(!legacy.contains_key(crate::stdin_stream_ops::READER_ID));
        let expected =
            expected_declaration_facts_for(false, false, false, false, true, false).unwrap();
        let mut ids = prelude::all_type_ids_v9()
            .into_iter()
            .collect::<BTreeSet<_>>();
        ids.insert(crate::stdin_stream_ops::READER_ID);
        assert_eq!(
            expected.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            ids
        );
        let fact = &expected[crate::stdin_stream_ops::READER_ID];
        assert_eq!(fact.kind, hir::DeclarationKind::Record);
        assert_eq!(fact.origin, hir::IdentityOrigin::CompilerOwned);
        assert_eq!(fact.owner, None);
        let mut hostile = expected.clone();
        hostile
            .get_mut(crate::stdin_stream_ops::READER_ID)
            .unwrap()
            .kind = hir::DeclarationKind::Resource;
        assert_ne!(hostile, expected);
        let mut hostile = expected.clone();
        hostile
            .get_mut(crate::stdin_stream_ops::READER_ID)
            .unwrap()
            .owner = Some(prelude::BOX_ID.to_owned());
        assert_ne!(hostile, expected);
    }

    #[test]
    fn record_iterator_v11_retains_stream_v10_and_iterator_facts() {
        let expected =
            expected_declaration_facts_for(false, false, false, false, false, true).unwrap();
        let mut ids = prelude::all_type_ids_v9()
            .into_iter()
            .collect::<BTreeSet<_>>();
        ids.insert(crate::stdin_stream_ops::READER_ID);
        assert_eq!(
            expected.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            ids
        );
        assert!(expected.contains_key(crate::iterator_ops::ITER_ID));
        assert!(expected.contains_key(crate::stdin_stream_ops::READER_ID));
    }

    #[test]
    fn imported_record_iterator_selects_complete_v11_shared_facts() {
        let build = build_owned(vec![
            source(
                "app.spx",
                r#"module record.app;
use type @id("record.line") from record.types as Line;
@id("record.consume") fn consume(values:own Vec<Line>)->i64 {
 let mut total=0;
 for own line in vec_into_iter<Line>(values){match own line {Line{left,right,marker}=>{total=total+marker;0},}}
 total
}
@id("app.main") fn main()->i64 {consume(vec_with_capacity<Line>(0usize))}
"#,
            ),
            source(
                "types.spx",
                r#"module record.types;
@id("record.line") record Line {
 @id("record.line.left") left:Bytes,
 @id("record.line.right") right:Bytes,
 @id("record.line.marker") marker:i64,
}
@id("types.main") fn main()->i64 {0}
"#,
            ),
        ])
        .unwrap();
        for id in [
            crate::iterator_ops::ITER_ID,
            crate::list_ops::LIST_ID,
            crate::stdin_stream_ops::READER_ID,
        ] {
            assert!(build.hir.shared_prelude_ids.contains(id), "missing {id}");
        }
    }

    #[test]
    fn mixed_stream_workspace_authenticates_each_modules_selected_prelude() {
        let sources = vec![
            source(
                "app.spx",
                r#"module stream.app;
use function @id("stream.inspect") from stream.input as inspect;
permit { process.stdin.read }
@id("stream.command") fn command() -> bool uses { process.stdin.read } { inspect() }
@id("app.main") fn main() -> i64 { 0 }
"#,
            ),
            source(
                "input.spx",
                r#"module stream.input;
permit { process.stdin.read }
@id("stream.inspect") fn inspect() -> bool uses { process.stdin.read } {
    let reader = stdin_stream_open(); stdin_stream_eof(reader)
}
@id("input.main") fn main() -> i64 { 0 }
"#,
            ),
        ];
        let build = build_owned(sources).unwrap();
        let reader = &build.hir.declarations[crate::stdin_stream_ops::READER_ID];
        assert_eq!(reader.kind, hir::DeclarationKind::Record);
        assert_eq!(reader.origin, hir::IdentityOrigin::CompilerOwned);
        assert_eq!(reader.owner, None);
        assert!(build
            .hir
            .shared_prelude_ids
            .contains(crate::stdin_stream_ops::READER_ID));
    }

    #[test]
    fn direct_import_reader_signature_selects_stream_even_when_unused() {
        let provider = crate::parse(
            r#"module stream.input;
@id("stream.inspect") fn inspect(reader: borrow StdinReader) -> bool { stdin_stream_eof(reader) }
@id("input.main") fn main() -> i64 { 0 }
"#,
            "input.spx",
        )
        .unwrap();
        let caller = crate::parse(
            r#"module stream.app;
use function @id("stream.inspect") from stream.input as inspect;
@id("app.main") fn main() -> i64 { 0 }
"#,
            "app.spx",
        )
        .unwrap();
        assert!(!crate::stdin_stream_ops::program_uses(&caller));
        assert!(module_uses_stream(
            &caller,
            &[caller.clone(), provider.clone()]
        ));
        let build = build_owned(vec![
            source("app.spx", &format::canonical(&caller)),
            source("input.spx", &format::canonical(&provider)),
        ])
        .unwrap();
        assert!(build
            .hir
            .shared_prelude_ids
            .contains(crate::stdin_stream_ops::READER_ID));
    }
    #[test]
    fn imported_owned_reader_helper_uses_an_identity_stub_without_a_constructor() {
        let build = build_owned(vec![
            source(
                "app.spx",
                r#"module stream.app;
use function @id("stream.forward") from stream.input as forward;
permit { process.stdin.read }
@id("stream.command") fn command() -> bool uses { process.stdin.read } {
    let reader = stdin_stream_open(); let successor = forward(reader); stdin_stream_eof(successor)
}
@id("app.main") fn main() -> i64 { 0 }
"#,
            ),
            source(
                "input.spx",
                r#"module stream.input;
@id("stream.forward") fn forward(reader: own StdinReader) -> StdinReader { reader }
@id("input.main") fn main() -> i64 { 0 }
"#,
            ),
        ])
        .unwrap();
        assert!(build
            .hir
            .shared_prelude_ids
            .contains(crate::stdin_stream_ops::READER_ID));
    }
    #[test]
    fn imported_reader_helpers_refuse_consuming_scalar_and_view_results() {
        for signature in [
            "fn inspect(reader: own StdinReader) -> bool { false }",
            "fn inspect(reader: borrow StdinReader) -> Slice<u8> { stdin_stream_chunk(reader) }",
        ] {
            let error = build_owned(vec![
                source("app.spx", r#"module stream.app;
use function @id("stream.inspect") from stream.input as inspect;
@id("app.main") fn main() -> i64 { 0 }
"#),
                source("input.spx", &format!("module stream.input; @id(\"stream.inspect\") {signature} @id(\"input.main\") fn main() -> i64 {{ 0 }}")),
            ]).err().expect("unsupported Reader helper must be refused");
            assert_eq!(error[0].code, "SPX-G172");
            assert_eq!(error[0].path.as_deref(), Some("app.spx"));
            assert!(error[0].span.is_some());
        }
    }
}
