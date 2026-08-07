use std::collections::{HashMap, HashSet};

use crate::ast::{BinaryOp, Expr, ExprKind, Function, ParamMode, Program, Span, Type, UnaryOp};
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug)]
struct Binding {
    ty: Type,
    mode: ParamMode,
    moved: bool,
}

pub fn verify(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut functions = HashMap::new();
    let mut ids = HashSet::new();
    let mut resources = HashSet::new();

    for resource in &program.resources {
        if !plain_identifier(&resource.name) {
            diagnostics.push(error(
                program,
                "SPX-S106",
                format!("`{}` is not a valid resource identifier", resource.name),
                resource.name_span,
            ));
        }
        if !resources.insert(resource.name.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S107",
                format!("duplicate resource `{}`", resource.name),
                resource.span,
            ));
        }
        if !ids.insert(resource.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", resource.stable_id),
                resource.span,
            ));
        }
        if !resource.explicit_id {
            diagnostics.push(
                Diagnostic::warning(
                    "SPX-S108",
                    format!(
                        "resource `{}` has an automatic identity that changes when renamed",
                        resource.name
                    ),
                    resource.name_span,
                )
                .at_path(&program.path)
                .with_help("add @id(\"your.namespace.resource\") before the declaration"),
            );
        }
    }

    for function in &program.functions {
        if !plain_identifier(&function.name) {
            diagnostics.push(error(
                program,
                "SPX-S104",
                format!("`{}` is not a valid function identifier", function.name),
                function.name_span,
            ));
        }
        if functions.insert(function.name.as_str(), function).is_some() {
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
        check_declared_type(
            program,
            &function.return_type,
            function.span,
            &resources,
            &mut diagnostics,
        );
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
            check_declared_type(program, &param.ty, param.span, &resources, &mut diagnostics);
            check_ownership_mode(program, function, param, &mut diagnostics);
            if variables
                .insert(
                    param.name.as_str(),
                    Binding {
                        ty: param.ty.clone(),
                        mode: param.mode,
                        moved: false,
                    },
                )
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
            &mut variables,
            &functions,
            None,
            true,
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
            check_resource_return(
                program,
                function,
                &function.body,
                &variables,
                &mut diagnostics,
            );
        }

        for contract in &function.requires {
            require_bool(
                program,
                function,
                contract,
                &variables,
                &functions,
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
                &functions,
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
            if let Some(target) = functions.get(callee) {
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

    if let Some(main) = functions.get("main") {
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

fn check_declared_type(
    program: &Program,
    ty: &Type,
    span: Span,
    resources: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Type::Resource(name) = ty {
        if !resources.contains(name.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-T001",
                format!("unknown type `{name}`; declare it with `resource {name};`"),
                span,
            ));
        }
    }
}

fn check_ownership_mode(
    program: &Program,
    function: &Function,
    param: &crate::ast::Param,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match (param.ty.is_resource(), param.mode) {
        (true, ParamMode::Value) => diagnostics.push(
            error(
                program,
                "SPX-O001",
                format!(
                    "resource parameter `{}.{}` needs `own`, `borrow`, or `shared`",
                    function.name, param.name
                ),
                param.span,
            )
            .with_help(format!(
                "use `{}: own {}` to transfer ownership",
                param.name, param.ty
            )),
        ),
        (false, mode) if mode != ParamMode::Value => diagnostics.push(error(
            program,
            "SPX-O002",
            format!(
                "ownership mode `{}` is only valid for resource types; `{}` is a value type",
                mode.text(),
                param.ty
            ),
            param.span,
        )),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn check_expr(
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<&str, Binding>,
    functions: &HashMap<&str, &Function>,
    result_type: Option<&Type>,
    allow_moves: bool,
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
        ExprKind::Var(name) => variables
            .get(name.as_str())
            .map(|binding| {
                if binding.moved {
                    diagnostics.push(
                        error(
                            program,
                            "SPX-O101",
                            format!("use of resource `{name}` after ownership was moved"),
                            expr.span,
                        )
                        .with_help("borrow the resource if the callee does not need ownership"),
                    );
                }
                binding.ty.clone()
            })
            .or_else(|| {
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
                let actual = check_expr(
                    program,
                    current,
                    arg,
                    variables,
                    functions,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if actual.as_ref().is_some_and(|actual| actual != &param.ty) {
                    diagnostics.push(error(
                        program,
                        "SPX-T205",
                        format!(
                            "argument `{}` to `{name}` expects {}, received {}",
                            param.name,
                            param.ty,
                            actual.expect("type checked above")
                        ),
                        arg.span,
                    ));
                }
                check_argument_ownership(
                    program,
                    current,
                    name,
                    arg,
                    param,
                    variables,
                    allow_moves,
                    diagnostics,
                );
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
                allow_moves,
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
                allow_moves,
                diagnostics,
            );
            let right_ty = check_expr(
                program,
                current,
                right,
                variables,
                functions,
                result_type,
                allow_moves,
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
fn check_argument_ownership(
    program: &Program,
    current: &Function,
    callee: &str,
    arg: &Expr,
    param: &crate::ast::Param,
    variables: &mut HashMap<&str, Binding>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExprKind::Var(name) = &arg.kind else {
        return;
    };
    let Some(binding) = variables.get_mut(name.as_str()) else {
        return;
    };
    if !binding.ty.is_resource() || binding.moved {
        return;
    }
    match param.mode {
        ParamMode::Own => {
            if binding.mode != ParamMode::Own {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O102",
                        format!(
                            "`{}.{name}` is {}, so `{current_name}` cannot transfer it to `{callee}`",
                            current.name,
                            binding.mode.text(),
                            current_name = current.name
                        ),
                        arg.span,
                    )
                    .with_help(format!("change `{name}` to `own {}` at its ownership boundary", binding.ty)),
                );
            } else if allow_moves {
                binding.moved = true;
            } else {
                diagnostics.push(error(
                    program,
                    "SPX-O105",
                    format!("contract expression cannot move resource `{name}` into `{callee}`"),
                    arg.span,
                ));
            }
        }
        ParamMode::Shared if binding.mode != ParamMode::Shared => diagnostics.push(
            error(
                program,
                "SPX-O103",
                format!("`{callee}` requires shared ownership of `{name}`"),
                arg.span,
            )
            .with_help("create or receive an explicitly shared resource before this call"),
        ),
        ParamMode::Borrow | ParamMode::Shared | ParamMode::Value => {}
    }
}

fn check_resource_return(
    program: &Program,
    function: &Function,
    body: &Expr,
    variables: &HashMap<&str, Binding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !function.return_type.is_resource() {
        return;
    }
    if let ExprKind::Var(name) = &body.kind {
        if let Some(binding) = variables.get(name.as_str()) {
            if binding.mode != ParamMode::Own {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O104",
                        format!(
                            "function `{}` cannot return {} resource `{name}` as owned",
                            function.name,
                            binding.mode.text()
                        ),
                        body.span,
                    )
                    .with_help("return an owned resource or declare a future lifetime-bound view"),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn require_bool(
    program: &Program,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<&str, Binding>,
    functions: &HashMap<&str, &Function>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    let mut contract_variables = variables.clone();
    if check_expr(
        program,
        function,
        contract,
        &mut contract_variables,
        functions,
        result_type,
        false,
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
    span: Span,
) -> Diagnostic {
    Diagnostic::error(code, message, span).at_path(&program.path)
}

fn plain_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
