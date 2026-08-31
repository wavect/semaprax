//! Default-stack compilation regressions, not target-execution evidence.
use super::{emit_module, program, InternalStringOptions};
use crate::hir::ResolvedExprKind;

#[derive(Clone, Copy)]
enum Form {
    Block,
    Unary,
    Binary,
    Call,
    If,
    Match,
    Mixed,
}

const FORMS: [Form; 6] = [
    Form::Block,
    Form::Unary,
    Form::Binary,
    Form::Call,
    Form::If,
    Form::Match,
];

fn source(form: Form) -> String {
    let mut expression = "42".to_owned();
    for index in 0..32 {
        let form = if matches!(form, Form::Mixed) {
            FORMS[index % FORMS.len()]
        } else {
            form
        };
        expression = match form {
            Form::Block => format!("{{ {expression} }}"),
            Form::Unary => format!("-({expression})"),
            Form::Binary => format!("0 + ({expression})"),
            Form::Call => format!("identity({expression})"),
            // Only one branch contains the previous expression. This is not
            // a binary tree duplicating all earlier source and semantic paths.
            Form::If => format!("if true {{ {expression} }} else {{ 0 }}"),
            Form::Match => format!("match ({expression}) {{ value => value, }}"),
            Form::Mixed => unreachable!("mixed form is selected before wrapping"),
        };
    }
    format!(
        "module test.string_nesting;\n\
         @id(\"nest.identity\") fn identity(value: i64) -> i64 {{ value }}\n\
         @id(\"nest.main\") fn main() -> i64 {{ {expression} }}\n"
    )
}

fn assert_compiles(form: Form, wrappers: [usize; 6]) {
    let ast = program(&source(form));
    let canonical = crate::format::canonical(&ast);
    let reparsed = program(&canonical);
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    assert_eq!(
        crate::graph::to_json(&ast).unwrap(),
        crate::graph::to_json(&reparsed).unwrap()
    );

    let resolved = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nest.main")
        .unwrap();
    let mut observed = [0usize; 6];
    let mut maximum_depth = 0usize;
    let mut pending = vec![(&main.body, 1usize)];
    while let Some((expression, depth)) = pending.pop() {
        maximum_depth = maximum_depth.max(depth);
        let kind = match expression.kind {
            ResolvedExprKind::Block { .. } => Some(0),
            ResolvedExprKind::Unary { .. } => Some(1),
            ResolvedExprKind::Binary { .. } => Some(2),
            ResolvedExprKind::Call { .. } => Some(3),
            ResolvedExprKind::If { .. } => Some(4),
            ResolvedExprKind::Match { .. } => Some(5),
            _ => None,
        };
        if let Some(kind) = kind {
            observed[kind] += 1;
        }
        for child in crate::interpreter::trace_child_expressions(expression) {
            pending.push((child, depth + 1));
        }
    }
    let mut expected = wrappers;
    // The function body and both branches of each authored `if` retain their
    // own blocks in addition to explicitly requested Block wrappers.
    expected[0] += 1 + 2 * wrappers[4];
    assert_eq!(observed, expected);
    assert!((34..=66).contains(&maximum_depth), "depth {maximum_depth}");

    let selected = ["nest.main".to_owned()];
    let first = emit_module(&ast, &selected, InternalStringOptions::default()).unwrap();
    wasmparser::Validator::new()
        .validate_all(first.wasm_bytes())
        .unwrap();
    let second = emit_module(&reparsed, &selected, InternalStringOptions::default()).unwrap();
    // Same-revision canonical determinism only. Frozen prior-emitter artifact
    // comparisons are a separate gate, not inferred from these equalities.
    assert_eq!(first.wasm_bytes(), second.wasm_bytes());
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.runtime_source(), second.runtime_source());
}

macro_rules! nesting_case {
    ($name:ident, $form:ident, $counts:expr) => {
        #[test]
        fn $name() {
            assert_compiles(Form::$form, $counts);
        }
    };
}

nesting_case!(
    nested_blocks_compile_on_default_stack,
    Block,
    [32, 0, 0, 0, 0, 0]
);
nesting_case!(
    nested_unary_compile_on_default_stack,
    Unary,
    [0, 32, 0, 0, 0, 0]
);
nesting_case!(
    nested_binary_compile_on_default_stack,
    Binary,
    [0, 0, 32, 0, 0, 0]
);
nesting_case!(
    nested_calls_compile_on_default_stack,
    Call,
    [0, 0, 0, 32, 0, 0]
);
nesting_case!(nested_if_compile_on_default_stack, If, [0, 0, 0, 0, 32, 0]);
nesting_case!(
    nested_scalar_match_compile_on_default_stack,
    Match,
    [0, 0, 0, 0, 0, 32]
);
nesting_case!(
    mixed_nesting_compiles_on_default_stack,
    Mixed,
    [6, 6, 5, 5, 5, 5]
);
