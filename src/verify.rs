use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, ParamMode, Program, Span, Statement, Type, UnaryOp,
};
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug)]
struct Binding {
    ty: Type,
    mode: ParamMode,
    availability: Availability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Available,
    Moved,
    MaybeMoved,
}

impl Availability {
    fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Availability::MaybeMoved
        }
    }
}

#[derive(Clone, Debug)]
struct CheckedValue {
    ty: Type,
    mode: ParamMode,
}

impl CheckedValue {
    fn value(ty: Type) -> Self {
        Self {
            ty,
            mode: ParamMode::Value,
        }
    }

    fn returned(ty: Type) -> Self {
        let mode = if ty.is_resource() {
            ParamMode::Own
        } else {
            ParamMode::Value
        };
        Self { ty, mode }
    }
}

pub fn verify(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut functions = HashMap::new();
    let mut ids = HashSet::new();
    let mut resources = HashSet::new();

    for resource in &program.resources {
        if !source_identifier(&resource.name) {
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
        if !source_identifier(&function.name) {
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
            if !source_identifier(&param.name) {
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
                    param.name.clone(),
                    Binding {
                        ty: param.ty.clone(),
                        mode: param.mode,
                        availability: Availability::Available,
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

        let entry_variables = variables.clone();
        for contract in &function.requires {
            require_bool(
                program,
                function,
                contract,
                &entry_variables,
                &functions,
                None,
                &mut diagnostics,
                "precondition",
            );
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
            if actual.ty != function.return_type {
                diagnostics.push(error(
                    program,
                    "SPX-T103",
                    format!(
                        "function `{}` returns {}, but its signature declares {}",
                        function.name, actual.ty, function.return_type
                    ),
                    function.body.span,
                ));
            }
            if function.return_type.is_resource() && actual.mode != ParamMode::Own {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O104",
                        format!(
                            "function `{}` cannot return a {} resource as owned",
                            function.name,
                            actual.mode.text()
                        ),
                        function.body.span,
                    )
                    .with_help("return an owned resource or declare a future lifetime-bound view"),
                );
            }
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
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    match &expr.kind {
        ExprKind::Int(_) => Some(CheckedValue::value(Type::I64)),
        ExprKind::Bool(_) => Some(CheckedValue::value(Type::Bool)),
        ExprKind::Var(name) if name == "result" => result_type
            .cloned()
            .map(CheckedValue::returned)
            .or_else(|| {
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
                match binding.availability {
                    Availability::Moved => diagnostics.push(
                        error(
                            program,
                            "SPX-O101",
                            format!("use of resource `{name}` after ownership was moved"),
                            expr.span,
                        )
                        .with_help("borrow the resource if the callee does not need ownership"),
                    ),
                    Availability::MaybeMoved => diagnostics.push(
                        error(
                            program,
                            "SPX-O107",
                            format!(
                                "resource `{name}` may have been moved on another control-flow path"
                            ),
                            expr.span,
                        )
                        .with_help("move the resource on every path or keep it borrowed"),
                    ),
                    Availability::Available => {}
                }
                CheckedValue {
                    ty: binding.ty.clone(),
                    mode: binding.mode,
                }
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
            let target = functions.get(name.as_str()).copied();
            if target.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T203",
                    format!("unknown function `{name}`"),
                    expr.span,
                ));
            }
            if target.is_some_and(|target| args.len() != target.params.len()) {
                let target = target.expect("checked above");
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
            for (index, arg) in args.iter().enumerate() {
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
                let Some(param) = target.and_then(|target| target.params.get(index)) else {
                    continue;
                };
                if actual.as_ref().is_some_and(|actual| actual.ty != param.ty) {
                    diagnostics.push(error(
                        program,
                        "SPX-T205",
                        format!(
                            "argument `{}` to `{name}` expects {}, received {}",
                            param.name,
                            param.ty,
                            actual.as_ref().expect("type checked above").ty
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
                    actual.as_ref(),
                    variables,
                    allow_moves,
                    diagnostics,
                );
            }
            target.map(|target| CheckedValue::returned(target.return_type.clone()))
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
            if actual.ty != expected {
                diagnostics.push(error(
                    program,
                    "SPX-T206",
                    format!("unary operator expects {expected}, received {}", actual.ty),
                    expr.span,
                ));
            }
            Some(CheckedValue::value(expected))
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
            let right_ty = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                let names = variables.keys().cloned().collect::<Vec<_>>();
                let mut right_variables = variables.clone();
                let value = check_expr(
                    program,
                    current,
                    right,
                    &mut right_variables,
                    functions,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                join_conditional(variables, &right_variables, &names);
                value
            } else {
                check_expr(
                    program,
                    current,
                    right,
                    variables,
                    functions,
                    result_type,
                    allow_moves,
                    diagnostics,
                )
            };
            let (expected, output) = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    (Type::I64, Type::I64)
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    (Type::I64, Type::Bool)
                }
                BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
                BinaryOp::Eq | BinaryOp::Ne => {
                    if left_ty.is_some()
                        && right_ty.is_some()
                        && left_ty.as_ref().map(|value| &value.ty)
                            != right_ty.as_ref().map(|value| &value.ty)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T207",
                            "equality operands must have the same type",
                            expr.span,
                        ));
                    }
                    return Some(CheckedValue::value(Type::Bool));
                }
            };
            if left_ty.as_ref().is_some_and(|value| value.ty != expected)
                || right_ty.as_ref().is_some_and(|value| value.ty != expected)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T208",
                    format!("operator `{}` expects {expected} operands", op.text()),
                    expr.span,
                ));
            }
            Some(CheckedValue::value(output))
        }
        ExprKind::Block { statements, tail } => {
            let outer_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut scope = variables.clone();
            for statement in statements {
                match statement {
                    Statement::Let {
                        name,
                        name_span,
                        value,
                        ..
                    } => {
                        if !source_identifier(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-S109",
                                format!("`{name}` is reserved and cannot name a local binding"),
                                *name_span,
                            ));
                        }
                        let actual = check_expr(
                            program,
                            current,
                            value,
                            &mut scope,
                            functions,
                            result_type,
                            allow_moves,
                            diagnostics,
                        );
                        if scope.contains_key(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-T209",
                                format!("local binding `{name}` shadows an existing value"),
                                *name_span,
                            ));
                            continue;
                        }
                        if let Some(actual) = actual {
                            if actual.ty.is_resource() && actual.mode == ParamMode::Own {
                                if allow_moves {
                                    mark_value_sources_moved(value, &mut scope);
                                } else {
                                    diagnostics.push(error(
                                        program,
                                        "SPX-O105",
                                        "contract expression cannot transfer an owned resource into a local binding",
                                        value.span,
                                    ));
                                }
                            }
                            scope.insert(
                                name.clone(),
                                Binding {
                                    ty: actual.ty,
                                    mode: actual.mode,
                                    availability: Availability::Available,
                                },
                            );
                        }
                    }
                }
            }
            let actual = check_expr(
                program,
                current,
                tail,
                &mut scope,
                functions,
                result_type,
                allow_moves,
                diagnostics,
            );
            merge_moved(variables, &scope, &outer_names);
            actual
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if check_expr(
                program,
                current,
                condition,
                variables,
                functions,
                result_type,
                allow_moves,
                diagnostics,
            )
            .is_some_and(|value| value.ty != Type::Bool)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T210",
                    "`if` condition must be bool",
                    condition.span,
                ));
            }
            let original_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut then_variables = variables.clone();
            let mut else_variables = variables.clone();
            let then_value = check_expr(
                program,
                current,
                then_branch,
                &mut then_variables,
                functions,
                result_type,
                allow_moves,
                diagnostics,
            );
            let else_value = check_expr(
                program,
                current,
                else_branch,
                &mut else_variables,
                functions,
                result_type,
                allow_moves,
                diagnostics,
            );
            for name in &original_names {
                if let Some(binding) = variables.get_mut(name) {
                    let then_state = then_variables
                        .get(name)
                        .map_or(Availability::Available, |value| value.availability);
                    let else_state = else_variables
                        .get(name)
                        .map_or(Availability::Available, |value| value.availability);
                    binding.availability = then_state.join(else_state);
                }
            }
            match (then_value, else_value) {
                (Some(then_value), Some(else_value)) => {
                    if then_value.ty != else_value.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T211",
                            format!(
                                "`if` branches return different types: {} and {}",
                                then_value.ty, else_value.ty
                            ),
                            expr.span,
                        ));
                    }
                    if then_value.ty.is_resource() && then_value.mode != else_value.mode {
                        diagnostics.push(error(
                            program,
                            "SPX-O106",
                            "`if` branches must produce the same resource ownership mode",
                            expr.span,
                        ));
                    }
                    Some(then_value)
                }
                _ => None,
            }
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
    actual: Option<&CheckedValue>,
    variables: &mut HashMap<String, Binding>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(actual) = actual else {
        return;
    };
    if !actual.ty.is_resource() {
        return;
    }
    match param.mode {
        ParamMode::Own => {
            if actual.mode != ParamMode::Own {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O102",
                        format!(
                            "argument to `{}.{}` is {}, so `{current_name}` cannot transfer it to `{callee}`",
                            current.name,
                            param.name,
                            actual.mode.text(),
                            current_name = current.name
                        ),
                        arg.span,
                    )
                    .with_help(format!(
                        "provide an owned `{}` value at this ownership boundary",
                        actual.ty
                    )),
                );
            } else if allow_moves {
                mark_value_sources_moved(arg, variables);
            } else {
                diagnostics.push(error(
                    program,
                    "SPX-O105",
                    format!("contract expression cannot transfer a resource into `{callee}`"),
                    arg.span,
                ));
            }
        }
        ParamMode::Shared if actual.mode != ParamMode::Shared => diagnostics.push(
            error(
                program,
                "SPX-O103",
                format!("`{callee}` requires shared resource ownership"),
                arg.span,
            )
            .with_help("create or receive an explicitly shared resource before this call"),
        ),
        ParamMode::Borrow | ParamMode::Shared | ParamMode::Value => {}
    }
}

fn mark_value_sources_moved(expr: &Expr, variables: &mut HashMap<String, Binding>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            if let Some(binding) = variables.get_mut(name) {
                if binding.ty.is_resource()
                    && binding.mode == ParamMode::Own
                    && binding.availability == Availability::Available
                {
                    binding.availability = Availability::Moved;
                }
            }
        }
        ExprKind::Block { tail, .. } => mark_value_sources_moved(tail, variables),
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            let names = variables.keys().cloned().collect::<Vec<_>>();
            let mut then_variables = variables.clone();
            let mut else_variables = variables.clone();
            mark_value_sources_moved(then_branch, &mut then_variables);
            mark_value_sources_moved(else_branch, &mut else_variables);
            for name in names {
                if let Some(binding) = variables.get_mut(&name) {
                    let then_state = then_variables
                        .get(&name)
                        .map_or(Availability::Available, |value| value.availability);
                    let else_state = else_variables
                        .get(&name)
                        .map_or(Availability::Available, |value| value.availability);
                    binding.availability = then_state.join(else_state);
                }
            }
        }
        _ => {}
    }
}

fn merge_moved(
    target: &mut HashMap<String, Binding>,
    source: &HashMap<String, Binding>,
    names: &[String],
) {
    for name in names {
        if let (Some(target), Some(source)) = (target.get_mut(name), source.get(name)) {
            target.availability = source.availability;
        }
    }
}

fn join_conditional(
    baseline: &mut HashMap<String, Binding>,
    conditional: &HashMap<String, Binding>,
    names: &[String],
) {
    for name in names {
        if let (Some(baseline), Some(conditional)) = (baseline.get_mut(name), conditional.get(name))
        {
            baseline.availability = baseline.availability.join(conditional.availability);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn require_bool(
    program: &Program,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    contract.visit_calls(&mut |callee, span| {
        if let Some(target) = functions.get(callee) {
            if !target.effects.is_empty() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-C102",
                        format!(
                            "{kind} on `{}` calls effectful function `{callee}` with effects {{{}}}",
                            function.name,
                            target.effects.join(", ")
                        ),
                        span,
                    )
                    .with_help("contracts must be deterministic and effect-free"),
                );
            }
        }
    });
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
    .is_some_and(|value| value.ty != Type::Bool)
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

fn source_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let plain = matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    plain
        && !matches!(
            value,
            "module"
                | "permit"
                | "resource"
                | "fn"
                | "own"
                | "borrow"
                | "shared"
                | "uses"
                | "requires"
                | "ensures"
                | "let"
                | "if"
                | "else"
                | "true"
                | "false"
                | "result"
        )
}
