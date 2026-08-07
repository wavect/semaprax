use std::collections::{HashMap, HashSet};

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Program, Type, UnaryOp};
use crate::diagnostic::Diagnostic;

pub fn verify(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut names = HashMap::new();
    let mut ids = HashSet::new();
    for function in &program.functions {
        if !plain_identifier(&function.name) {
            diagnostics.push(error(
                program,
                "SPX-S104",
                format!("`{}` is not a valid function identifier", function.name),
                function.name_span,
            ));
        }
        if names.insert(function.name.as_str(), function).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S101",
                format!("duplicate function `{}`", function.name),
                function.name_span,
            ));
        }
        if !ids.insert(function.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", function.stable_id),
                function.span,
            ));
        }
        if !function.explicit_id {
            diagnostics.push(
                Diagnostic::warning(
                    "SPX-S103",
                    format!(
                        "function `{}` has an automatic identity that changes when renamed",
                        function.name
                    ),
                    function.name_span,
                )
                .at_path(&program.path)
                .with_help("add @id(\"your.namespace.symbol\") before the declaration"),
            );
        }
    }

    for function in &program.functions {
        let mut variables = HashMap::new();
        for param in &function.params {
            if !plain_identifier(&param.name) {
                diagnostics.push(error(
                    program,
                    "SPX-S105",
                    format!("`{}` is not a valid parameter identifier", param.name),
                    param.span,
                ));
            }
            if variables
                .insert(param.name.as_str(), param.ty.clone())
                .is_some()
            {
                diagnostics.push(error(
                    program,
                    "SPX-T102",
                    format!("duplicate parameter `{}`", param.name),
                    param.span,
                ));
            }
        }
        if let Some(actual) = check_expr(
            program,
            function,
            &function.body,
            &variables,
            &names,
            None,
            &mut diagnostics,
        ) {
            if actual != function.return_type {
                diagnostics.push(error(
                    program,
                    "SPX-T103",
                    format!(
                        "function `{}` returns {actual}, but its signature declares {}",
                        function.name, function.return_type
                    ),
                    function.body.span,
                ));
            }
        }

        for contract in &function.requires {
            require_bool(
                program,
                function,
                contract,
                &variables,
                &names,
                None,
                &mut diagnostics,
                "precondition",
            );
        }
        for contract in &function.ensures {
            require_bool(
                program,
                function,
                contract,
                &variables,
                &names,
                Some(&function.return_type),
                &mut diagnostics,
                "postcondition",
            );
        }

        let declared: HashSet<_> = function.effects.iter().map(String::as_str).collect();
        for effect in &function.effects {
            if !program.permits.iter().any(|permit| permit == effect) {
                diagnostics.push(error(
                    program,
                    "SPX-E101",
                    format!(
                        "function `{}` uses `{effect}` but module `{}` does not permit it",
                        function.name, program.module
                    ),
                    function.span,
                ));
            }
        }
        function.body.visit_calls(&mut |callee, span| {
            if let Some(target) = names.get(callee) {
                for effect in &target.effects {
                    if !declared.contains(effect.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-E102",
                            format!(
                                "call to `{callee}` requires effect `{effect}`; add it to `{}`",
                                function.name
                            ),
                            span,
                        ));
                    }
                }
            }
        });
    }

    if let Some(main) = names.get("main") {
        if !main.params.is_empty() || main.return_type != Type::I64 {
            diagnostics.push(error(
                program,
                "SPX-T104",
                "entry function must have signature `fn main() -> i64`",
                main.span,
            ));
        }
    } else {
        diagnostics.push(
            Diagnostic::error(
                "SPX-T105",
                "executable module must define `fn main() -> i64`",
                program.functions[0].span,
            )
            .at_path(&program.path),
        );
    }
    diagnostics
}

#[allow(clippy::too_many_arguments)]
fn check_expr(
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &HashMap<&str, Type>,
    functions: &HashMap<&str, &Function>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Type> {
    match &expr.kind {
        ExprKind::Int(_) => Some(Type::I64),
        ExprKind::Bool(_) => Some(Type::Bool),
        ExprKind::Var(name) if name == "result" => result_type.cloned().or_else(|| {
            diagnostics.push(error(
                program,
                "SPX-T201",
                "`result` is only available in postconditions",
                expr.span,
            ));
            None
        }),
        ExprKind::Var(name) => variables.get(name.as_str()).cloned().or_else(|| {
            diagnostics.push(error(
                program,
                "SPX-T202",
                format!("unknown value `{name}` in `{}`", current.name),
                expr.span,
            ));
            None
        }),
        ExprKind::Call { name, args } => {
            let Some(target) = functions.get(name.as_str()) else {
                diagnostics.push(error(
                    program,
                    "SPX-T203",
                    format!("unknown function `{name}`"),
                    expr.span,
                ));
                return None;
            };
            if args.len() != target.params.len() {
                diagnostics.push(error(
                    program,
                    "SPX-T204",
                    format!(
                        "`{name}` expects {} arguments, received {}",
                        target.params.len(),
                        args.len()
                    ),
                    expr.span,
                ));
            }
            for (arg, param) in args.iter().zip(&target.params) {
                if let Some(actual) = check_expr(
                    program,
                    current,
                    arg,
                    variables,
                    functions,
                    result_type,
                    diagnostics,
                ) {
                    if actual != param.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T205",
                            format!(
                                "argument `{}` to `{name}` expects {}, received {actual}",
                                param.name, param.ty
                            ),
                            arg.span,
                        ));
                    }
                }
            }
            Some(target.return_type.clone())
        }
        ExprKind::Unary { op, value } => {
            let actual = check_expr(
                program,
                current,
                value,
                variables,
                functions,
                result_type,
                diagnostics,
            )?;
            let expected = match op {
                UnaryOp::Neg => Type::I64,
                UnaryOp::Not => Type::Bool,
            };
            if actual != expected {
                diagnostics.push(error(
                    program,
                    "SPX-T206",
                    format!("unary operator expects {expected}, received {actual}"),
                    expr.span,
                ));
            }
            Some(expected)
        }
        ExprKind::Binary { op, left, right } => {
            let left_ty = check_expr(
                program,
                current,
                left,
                variables,
                functions,
                result_type,
                diagnostics,
            );
            let right_ty = check_expr(
                program,
                current,
                right,
                variables,
                functions,
                result_type,
                diagnostics,
            );
            let (expected, output) = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    (Type::I64, Type::I64)
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    (Type::I64, Type::Bool)
                }
                BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
                BinaryOp::Eq | BinaryOp::Ne => {
                    if left_ty.is_some() && right_ty.is_some() && left_ty != right_ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T207",
                            "equality operands must have the same type",
                            expr.span,
                        ));
                    }
                    return Some(Type::Bool);
                }
            };
            if left_ty.as_ref().is_some_and(|ty| ty != &expected)
                || right_ty.as_ref().is_some_and(|ty| ty != &expected)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T208",
                    format!("operator `{}` expects {expected} operands", op.text()),
                    expr.span,
                ));
            }
            Some(output)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn require_bool(
    program: &Program,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<&str, Type>,
    functions: &HashMap<&str, &Function>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    if check_expr(
        program,
        function,
        contract,
        variables,
        functions,
        result_type,
        diagnostics,
    )
    .is_some_and(|ty| ty != Type::Bool)
    {
        diagnostics.push(error(
            program,
            "SPX-C101",
            format!("{kind} on `{}` must be bool", function.name),
            contract.span,
        ));
    }
}

fn error(
    program: &Program,
    code: &'static str,
    message: impl Into<String>,
    span: crate::ast::Span,
) -> Diagnostic {
    Diagnostic::error(code, message, span).at_path(&program.path)
}

fn plain_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
