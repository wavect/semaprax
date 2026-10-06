//! Low-stack admission of deeply nested indexed matches.

use super::*;

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
