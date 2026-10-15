//! Low-stack admission of deeply nested indexed matches.

use super::*;

#[test]
fn resolved_while_admits_closed_map_and_set_operations() {
    let source = r#"
module test.validation_collection_loop;
@id("collection.loop") fn main() -> i64 {
    let mut values=map_new<i64,i64>(2usize);
    let mut keys=set_new<i64>(2usize);
    let mut index=0;
    while index < 2 {
        values=map_set<i64,i64>(values,index,index);
        keys=set_insert<i64>(keys,index);
        index=index+1;
        0
    }
    if map_len<i64,i64>(values)==2usize && set_len<i64>(keys)==2usize {7}else{-1}
}
"#;
    let parsed = crate::check(source, "validation-collection-loop.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&program).unwrap();
}

const INDEXED_LOOP: &str = r#"
module test.validation_indexed_depth;
@id("indexed.deep")
fn deep(bytes: borrow Slice<u8>) -> usize {
    let length = byte_len(bytes);
    let mut index = 0usize;
    let mut total = 0usize;
    while index <= length {
        total = total + match byte_get(bytes, index) {
            Option::Some { value: byte } => if byte == 255u8 { 1usize } else { 0usize },
            Option::None {} => 0usize,
        };
        index = index + 1usize;
        index <= length
    }
    total
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn exact_512_nested_indexed_matches_validate_on_a_low_stack() {
    let parsed = crate::parse(
        INDEXED_LOOP,
        std::path::Path::new("validation-indexed-depth.spx"),
    )
    .unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "indexed.deep")
        .unwrap();
    let slice_parameter = function.params[0].id.clone();
    let mut pending = vec![&function.body];
    let template = loop {
        let expression = pending.pop().expect("indexed match retained");
        if matches!(expression.kind, ResolvedExprKind::Match { .. }) {
            break expression.clone();
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    };
    let mut nested = template.clone();
    for _ in 1..512 {
        let mut outer = template.clone();
        let ResolvedExprKind::Match { arms, .. } = &mut outer.kind else {
            unreachable!();
        };
        arms[0].value = nested;
        nested = outer;
    }
    let mut cursor = &nested;
    let mut depth = 0;
    while let ResolvedExprKind::Match { arms, .. } = &cursor.kind {
        depth += 1;
        cursor = &arms[0].value;
    }
    assert_eq!(depth, 512);
    let baseline = template;

    let (program, nested) = std::thread::Builder::new()
        .name("indexed-match-admission-depth".to_owned())
        .stack_size(128 * 1024)
        .spawn(move || {
            // Direct stderr survives libtest's captured-output loss on abort.
            use std::io::Write;
            std::io::stderr()
                .write_all(b"indexed-depth: constructor\n")
                .unwrap();
            let mut validator = HirValidator::new(&program).unwrap();
            std::io::stderr()
                .write_all(b"indexed-depth: constructor complete\n")
                .unwrap();
            // `validate_function` authenticates borrowed slice parameters
            // before invoking this private admission pass. Recreate that
            // exact prerequisite because this focused test calls the pass
            // directly instead of revalidating the whole function.
            validator.byte_slice_aliases.insert(
                slice_parameter.clone(),
                Place {
                    root: slice_parameter,
                    projections: Vec::new(),
                },
            );
            std::io::stderr()
                .write_all(b"indexed-depth: baseline\n")
                .unwrap();
            validator.validate_while_admission(&baseline).unwrap();
            std::io::stderr()
                .write_all(b"indexed-depth: nested\n")
                .unwrap();
            validator.validate_while_admission(&nested).unwrap();
            std::io::stderr()
                .write_all(b"indexed-depth: nested complete\n")
                .unwrap();
            (program, nested)
        })
        .unwrap()
        .join()
        .unwrap();
    drop(nested);
    drop(program);
}

#[test]
fn indexed_read_from_inline_byte_view_points_to_the_view_and_names_the_alias_fix() {
    let source = r#"
module test.validation_indexed_alias_diagnostic;
@id("indexed.alias")
fn indexed(text: string) -> usize {
    let view = string_as_str(text);
    let mut index = 0usize;
    let mut total = 0usize;
    while index < 1usize {
        total = total + byte_len(str_as_bytes(view));
        index = index + 1usize;
        index < 1usize
    }
    total
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(
        source,
        std::path::Path::new("validation-indexed-alias-diagnostic.spx"),
    )
    .unwrap();
    let mut diagnostics = crate::hir::resolve(&parsed).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics.remove(0);
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "while loop indexed byte reads require an existing byte-slice alias"
    );
    let start = source.find("str_as_bytes(view)").unwrap();
    let span = diagnostic.span.expect("inline byte view has a source span");
    assert_eq!(
        (span.start, span.end),
        (start, start + "str_as_bytes(view)".len())
    );
    let prefix = &source[..start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rfind('\n')
        .map_or(start + 1, |newline| start - newline);
    assert_eq!((span.line, span.column), (line, column));
    assert_eq!(
        diagnostic.help.as_deref(),
        Some(
            "bind the byte view of a named `str` before the loop: `let bytes = str_as_bytes(view);`, then pass `bytes` to `byte_len` or `byte_get`"
        )
    );
}

#[path = "return_operand_tests.rs"]
mod return_operand;

const INLINE_STRING_COPY: &str = r#"module test.inline_string_copy;
@id("main") fn main()->i64 {
    let row="é\u{0}";
    let mut output="";
    let mut at=0;
    while at<2 {
        output=string_concat(output,string_from_str(string_as_str(row)));
        at=at+1;
        0
    }
    string_len(output)
}
"#;

#[test]
fn inline_named_string_view_copy_retains_ordinary_loop_and_loan_validation() {
    let ast = crate::check(INLINE_STRING_COPY, "inline-string-copy.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let ast = crate::check(&canonical, "inline-string-copy.spx").unwrap();
    assert_eq!(crate::format::canonical(&ast), canonical);
    let resolved = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
}

#[test]
fn inline_string_copy_refuses_forged_view_operation_type_and_root() {
    fn input(expression: &mut ResolvedExpr) -> Option<&mut ResolvedExpr> {
        match &mut expression.kind {
            ResolvedExprKind::Call { callee, args, .. } => {
                if callee.as_str() == crate::string_ops::FROM_STR_ID {
                    args.first_mut()
                } else {
                    args.iter_mut().find_map(input)
                }
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let found = match statement {
                        ResolvedStatement::Assign { value, .. }
                        | ResolvedStatement::CopyDiscard { value, .. }
                        | ResolvedStatement::Let { value, .. } => input(value),
                        ResolvedStatement::While { body, .. }
                        | ResolvedStatement::LexicalWhile { body, .. } => input(body),
                        ResolvedStatement::Unsafe { body, .. } => input(body),
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                input(tail)
            }
            _ => None,
        }
    }
    let ast = crate::check(INLINE_STRING_COPY, "inline-string-forged.spx").unwrap();
    let original = crate::hir::resolve(&ast).unwrap();
    for attack in 0..3 {
        let mut candidate = original.clone();
        let function = candidate
            .functions
            .iter_mut()
            .find(|function| function.id == candidate.entrypoint)
            .unwrap();
        let view = input(&mut function.body).expect("actual inline view");
        let ResolvedExprKind::BorrowPlace { operation, place } = &mut view.kind else {
            panic!("view")
        };
        match attack {
            0 => *operation = DeclarationId::new(crate::byte_ops::STR_AS_BYTES_ID),
            1 => view.ty = ResolvedType::SliceU8,
            _ => place.root = ValueId::new("missing.owner".to_owned()),
        }
        assert_eq!(
            crate::hir::validate(&candidate).unwrap_err().code,
            "SPX-H006"
        );
    }
}

#[test]
fn resolved_string_slot_fused_loop_keeps_named_monomorphic_admission() {
    let source = include_str!("../../../tests/fixtures/string-slot-views.spx");
    let parsed = crate::check(source, "resolved-slots-loop.spx").unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&resolved).unwrap();
}

#[test]
fn resolved_string_slot_loop_refuses_typed_computed_and_missing_operands() {
    let source = include_str!("../../../tests/fixtures/string-slot-views.spx");
    for replacement in [
        "str_as_bytes(vec_str_at<string>(values,0usize))",
        "str_as_bytes(vec_str_at({values},0usize))",
        "str_as_bytes(vec_str_at(values))",
    ] {
        let changed = source.replace("str_as_bytes(vec_str_at(values,0usize))", replacement);
        assert_ne!(changed, source);
        // Bypass source verification to exercise the independent HIR guard.
        let parsed =
            crate::parse(&changed, std::path::Path::new("resolved-slots-loop.spx")).unwrap();
        let error = crate::hir::resolve(&parsed).unwrap_err();
        // Resolution accumulates operand diagnostics after the primary loop refusal.
        assert_eq!(error[0].code, "SPX-T283", "{replacement}: {error:?}");
        assert_eq!(
            error[0].message,
            "only exact typed scalar Vec update/read operations are admitted in while bodies",
            "{replacement}: {error:?}"
        );
    }
}
