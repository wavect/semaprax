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
            let mut validator = HirValidator::new(&program).unwrap();
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
            validator.validate_while_admission(&baseline).unwrap();
            validator.validate_while_admission(&nested).unwrap();
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
    let program = crate::hir::resolve(&parsed).unwrap();
    let diagnostic = crate::hir::validate(&program).unwrap_err();
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
            "bind the byte view before the loop, for example `let bytes = str_as_bytes(text);`, then pass `bytes` to `byte_len` or `byte_get`"
        )
    );
}
