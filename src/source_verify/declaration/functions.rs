//! Function-level checks: declaration admission, generic call cycles, and the
//! per-function body, contract, and effect checks.

use crate::ast::{
    Expr, ExprKind, Function, ImportDeclaration, InterfaceDeclaration, MatchPattern, ParamMode,
    Program, RecordMatchFieldPattern, Statement, Type,
};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::declared_type::generic_result;
use crate::source_verify::declared_type::{
    check_declared_type, check_ownership_mode, function_reaches, function_reaches_any,
    generic_function_arguments_are_forwarded, generic_function_contains_nested_owned_record_slot,
    generic_function_expression_is_direct_scalar,
    generic_function_expression_is_owned_record_composition,
    generic_function_has_owned_record_composition, generic_function_owned_record_slot,
    generic_function_signature_slot, owned_record_function_substitutions,
    scalar_function_substitutions, validation_specialize_function,
};
use crate::source_verify::diagnostics::{
    error, invalid_stable_id, reject_native_unit_value, reject_reserved_host_id, require_bool,
    source_identifier,
};
use crate::source_verify::iterative::check_expr_iterative;
use crate::source_verify::type_table::TypeTable;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) fn check_function_declarations<'p>(
    program: &'p Program,
    functions: &mut HashMap<&'p str, &'p Function>,
    ids: &mut HashSet<&'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for function in &program.functions {
        if !source_identifier(&function.name) {
            diagnostics.push(error(
                program,
                "SPX-S104",
                format!("`{}` is not a valid function identifier", function.name),
                function.name_span,
            ));
        }
        if crate::string_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned string operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        if crate::str_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned borrowed string operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        if crate::byte_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned byte operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        if crate::vec_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned vector operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        if crate::box_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned box operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        if crate::host_io_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned host I/O operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        if crate::command_io_ops::by_name(&function.name).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned command I/O operations",
                    function.name
                ),
                function.name_span,
            ));
        }
        reject_reserved_host_id(
            program,
            &function.stable_id,
            "function",
            function.span,
            diagnostics,
        );
        if functions.insert(function.name.as_str(), function).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S101",
                format!("duplicate function `{}`", function.name),
                function.name_span,
            ));
        }
        if function.stable_id.is_empty() {
            diagnostics.push(
                error(
                    program,
                    "SPX-S102",
                    format!("function `{}` has an empty stable id", function.name),
                    function.name_span,
                )
                .with_help("give the declaration a dotted stable identity with @id(\"your.namespace.symbol\")"),
            );
        } else if function.stable_id.contains('\0') {
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-S102",
                format!("function `{}`", function.name),
                function.span,
            ));
        } else if function
            .stable_id
            .starts_with("semaprax.function-execution.v1:")
        {
            diagnostics.push(error(
                program,
                "SPX-T225",
                format!(
                    "function `{}` uses the reserved generic execution identity domain",
                    function.name
                ),
                function.span,
            ));
        } else if !ids.insert(function.stable_id.as_str()) {
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
        if crate::vec_ops::wrapper_by_id(&function.stable_id).is_some()
            && !crate::vec_ops::source_module_is_authenticated(program)
        {
            diagnostics.push(error(
                program,
                "SPX-T283",
                format!(
                    "vector wrapper identity `{}` is reserved for module `{}`",
                    function.stable_id,
                    crate::vec_ops::MODULE
                ),
                function.span,
            ));
            continue;
        }
        if crate::box_ops::wrapper_by_id(&function.stable_id).is_some()
            && !crate::box_ops::source_module_is_authenticated(program)
        {
            diagnostics.push(error(
                program,
                "SPX-T286",
                format!(
                    "box wrapper identity `{}` is reserved for module `{}`",
                    function.stable_id,
                    crate::box_ops::MODULE
                ),
                function.span,
            ));
            continue;
        }
        if crate::box_ops::wrapper_by_id(&function.stable_id).is_some()
            && crate::box_ops::source_wrapper(program, function).is_none()
        {
            diagnostics.push(error(
                program,
                "SPX-T286",
                format!(
                    "`{}.{}` must be one exact transparent compiler-owned box wrapper",
                    program.module, function.name
                ),
                function.span,
            ));
            continue;
        }
        if crate::vec_ops::wrapper_by_id(&function.stable_id).is_some()
            && crate::vec_ops::source_wrapper(program, function).is_none()
        {
            diagnostics.push(error(
                program,
                "SPX-T283",
                format!(
                    "`{}.{}` must be one exact transparent compiler-owned vector wrapper",
                    program.module, function.name
                ),
                function.span,
            ));
            continue;
        }
        if !function.type_parameters.is_empty() {
            let transparent_vec_wrapper = crate::vec_ops::source_wrapper(program, function);
            let transparent_box_wrapper = crate::box_ops::source_wrapper(program, function);
            if crate::vec_ops::is_source_candidate(program, function)
                && transparent_vec_wrapper.is_none()
            {
                diagnostics.push(error(
                    program,
                    "SPX-T283",
                    format!(
                        "`{}.{}` must be one exact transparent compiler-owned vector wrapper",
                        program.module, function.name
                    ),
                    function.span,
                ));
                continue;
            }
            if transparent_vec_wrapper.is_some() {
                continue;
            }
            if crate::box_ops::is_source_candidate(program, function)
                && transparent_box_wrapper.is_none()
            {
                diagnostics.push(error(
                    program,
                    "SPX-T286",
                    format!(
                        "`{}.{}` must be one exact transparent compiler-owned box wrapper",
                        program.module, function.name
                    ),
                    function.span,
                ));
                continue;
            }
            if transparent_box_wrapper.is_some() {
                continue;
            }
            if !(1..=2).contains(&function.type_parameters.len()) {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` requires one or two type parameters",
                        function.name
                    ),
                    function.span,
                ));
            }
            let mut parameter_names = HashSet::new();
            for parameter in &function.type_parameters {
                if !source_identifier(&parameter.name)
                    || !parameter_names.insert(parameter.name.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "invalid or duplicate type parameter `{}` on function `{}`",
                            parameter.name, function.name
                        ),
                        parameter.span,
                    ));
                }
            }
            if !function.effects.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T226",
                    format!(
                        "generic function `{}` must be effect-free in this slice",
                        function.name
                    ),
                    function.span,
                ));
            }
            let variant = crate::source_verify::declared_type::generic_variant::profile(
                function,
                &TypeTable::new(program),
            );
            let owned_result = generic_result::profile(function);
            let collection =
                crate::source_verify::declared_type::generic_collection::profile(function);
            for param in &function.params {
                let owned_record = generic_function_owned_record_slot(
                    function,
                    &param.ty,
                    &TypeTable::new(program),
                );
                if !((param.mode == ParamMode::Value
                    && (generic_function_signature_slot(&param.ty, &parameter_names)
                        || ((collection || variant)
                            && crate::vec_ops::ast_element_is_admitted(&param.ty))))
                    || (param.mode == ParamMode::Own
                        && (owned_record
                            || (variant
                                && crate::source_verify::declared_type::generic_variant::slot(
                                    function,
                                    &param.ty,
                                    &TypeTable::new(program),
                                ))
                            || (collection
                                && crate::source_verify::declared_type::generic_collection::slot(
                                    function, &param.ty,
                                ))
                            || (owned_result && generic_result::slot(function, &param.ty)))))
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "generic function `{}.{}` must use the direct-scalar profile by value or one admitted flat owned-record template by ownership",
                            function.name, param.name
                        ),
                        param.span,
                    ));
                }
            }
            if !owned_result
                && !variant
                && !collection
                && !generic_function_signature_slot(&function.return_type, &parameter_names)
                && !generic_function_owned_record_slot(
                    function,
                    &function.return_type,
                    &TypeTable::new(program),
                )
            {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` must return the direct-scalar profile or one admitted flat owned-record template",
                        function.name
                    ),
                    function.span,
                ));
            }
            if generic_function_contains_nested_owned_record_slot(
                function,
                &TypeTable::new(program),
            ) && !generic_function_has_owned_record_composition(
                function,
                &TypeTable::new(program),
            ) {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` must transfer exactly one bounded owned-record parameter into an identical result type",
                        function.name
                    ),
                    function.span,
                ));
            }
            let types = TypeTable::new(program);
            let invalid_contract = function
                .requires
                .iter()
                .chain(&function.ensures)
                .any(|expression| !generic_function_expression_is_direct_scalar(expression));
            let invalid_body = !crate::source_verify::declared_type::generic_variant::body(
                function,
                &types,
                &function.body,
            ) && !generic_result::body(function, &function.body)
                && !generic_function_expression_is_direct_scalar(&function.body)
                && !generic_function_expression_is_owned_record_composition(
                    function,
                    &types,
                    &function.body,
                );
            if invalid_contract || invalid_body {
                diagnostics.push(error(
                    program,
                    "SPX-T226",
                    format!(
                        "generic function `{}` uses an expression outside the direct-scalar slice",
                        function.name
                    ),
                    function.span,
                ));
            }
        }
    }
}

pub(super) fn check_generic_function_cycles<'p>(
    program: &'p Program,
    call_graph: &HashMap<String, Vec<String>>,
    generic_functions: &HashSet<&'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let functions = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let types = TypeTable::new(program);
    for function in program
        .functions
        .iter()
        .filter(|function| !function.type_parameters.is_empty())
    {
        let mut bindings = function
            .params
            .iter()
            .map(|parameter| (parameter.name.clone(), static_binding(parameter.ty.clone())))
            .collect::<HashMap<_, _>>();
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            check_omitted_generic_mappings(
                program,
                function,
                expression,
                &mut bindings,
                &functions,
                &types,
                diagnostics,
                0,
            );
            expression.visit_call_instances(&mut |callee, arguments, span| {
                if let Some(target) = program.functions.iter().find(|target| {
                    target.name == callee && !target.type_parameters.is_empty()
                }) {
                    if arguments.is_empty() {
                        return;
                    }
                    if !generic_function_arguments_are_forwarded(function, target, arguments) {
                        diagnostics.push(error(
                            program,
                            "SPX-T225",
                            format!(
                                "generic function `{}` must supply admitted explicit type-argument mappings when calling generic function `{callee}`",
                                function.name
                            ),
                            span,
                        ));
                    }
                }
            });
        }
        let participates_in_cycle = call_graph.get(&function.name).is_some_and(|callees| {
            callees.iter().any(|callee| {
                function_reaches(call_graph, callee, &function.name, &mut HashSet::new())
            })
        });
        if participates_in_cycle {
            diagnostics.push(error(
                program,
                "SPX-T226",
                format!(
                    "generic function `{}` participates in a recursive call cycle",
                    function.name
                ),
                function.span,
            ));
        }
        let direct_generic_call = call_graph.get(&function.name).is_some_and(|callees| {
            callees
                .iter()
                .any(|callee| generic_functions.contains(callee.as_str()))
        });
        let reaches_other_generic = call_graph.get(&function.name).is_some_and(|callees| {
            callees.iter().any(|callee| {
                function_reaches_any(call_graph, callee, generic_functions, &mut HashSet::new())
            })
        });
        if reaches_other_generic && !direct_generic_call && !participates_in_cycle {
            diagnostics.push(error(
                program,
                "SPX-T226",
                format!(
                    "generic function `{}` transitively reaches another generic function",
                    function.name
                ),
                function.span,
            ));
        }
    }
}

const MAX_STATIC_MAPPING_DEPTH: usize = 128;

fn static_binding(ty: Type) -> Binding {
    Binding {
        ty,
        mode: ParamMode::Value,
        availability: Availability::Available,
        moved_places: HashMap::new(),
        definitely_partial: HashSet::new(),
        native_unit_discard: false,
        mutable: false,
        active_loans: BTreeSet::new(),
        borrow_origin: None,
    }
}

/// Carries the lexical scope, declaration table, and bounded traversal state
/// required to authenticate an omitted symbolic mapping before body checking.
#[allow(clippy::too_many_arguments)]
fn check_omitted_generic_mappings(
    program: &Program,
    current: &Function,
    expression: &Expr,
    bindings: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    depth: usize,
) {
    if depth >= MAX_STATIC_MAPPING_DEPTH {
        expression.visit_call_instances(&mut |callee, arguments, span| {
            if arguments.is_empty()
                && functions
                    .get(callee)
                    .is_some_and(|target| !target.type_parameters.is_empty())
            {
                diagnostics.push(error(
                    program,
                    "SPX-T225",
                    format!(
                        "generic function `{}` has an omitted type-argument mapping beyond the bounded static traversal",
                        current.name
                    ),
                    span,
                ));
            }
        });
        return;
    }
    let next = depth + 1;
    match &expression.kind {
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            for argument in args {
                check_omitted_generic_mappings(
                    program,
                    current,
                    argument,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
            let Some(target) = functions.get(name.as_str()) else {
                return;
            };
            if target.type_parameters.is_empty() || !type_arguments.is_empty() {
                return;
            }
            let inferred = crate::source_verify::generic_inference::arguments(
                program, current, target, args, bindings, functions, types,
            );
            if !inferred.as_ref().is_some_and(|arguments| {
                generic_function_arguments_are_forwarded(current, target, arguments)
            }) {
                diagnostics.push(error(
                    program,
                    "SPX-T225",
                    format!(
                        "generic function `{}` must supply admitted explicit type-argument mappings when calling generic function `{name}`",
                        current.name
                    ),
                    expression.span,
                ));
            }
        }
        ExprKind::Block { statements, tail } => {
            let mut scope = bindings.clone();
            for statement in statements {
                match statement {
                    Statement::Let {
                        name,
                        declared,
                        value,
                        ..
                    } => {
                        check_omitted_generic_mappings(
                            program,
                            current,
                            value,
                            &mut scope,
                            functions,
                            types,
                            diagnostics,
                            next,
                        );
                        let ty = declared.clone().or_else(|| {
                            crate::source_verify::generic_inference::expression_type(
                                program, current, value, &scope, functions, types,
                            )
                        });
                        scope.remove(name);
                        if let Some(ty) = ty {
                            scope.insert(name.clone(), static_binding(ty));
                        }
                    }
                    Statement::Assign { value, .. } => check_omitted_generic_mappings(
                        program,
                        current,
                        value,
                        &mut scope,
                        functions,
                        types,
                        diagnostics,
                        next,
                    ),
                    Statement::Unsafe { body, .. } => check_omitted_generic_mappings(
                        program,
                        current,
                        body,
                        &mut scope,
                        functions,
                        types,
                        diagnostics,
                        next,
                    ),
                    Statement::While {
                        condition, body, ..
                    } => {
                        check_omitted_generic_mappings(
                            program,
                            current,
                            condition,
                            &mut scope,
                            functions,
                            types,
                            diagnostics,
                            next,
                        );
                        check_omitted_generic_mappings(
                            program,
                            current,
                            body,
                            &mut scope,
                            functions,
                            types,
                            diagnostics,
                            next,
                        );
                    }
                    Statement::For {
                        item, values, body, ..
                    } => {
                        check_omitted_generic_mappings(
                            program,
                            current,
                            values,
                            &mut scope,
                            functions,
                            types,
                            diagnostics,
                            next,
                        );
                        let mut body_scope = scope.clone();
                        body_scope.remove(item);
                        check_omitted_generic_mappings(
                            program,
                            current,
                            body,
                            &mut body_scope,
                            functions,
                            types,
                            diagnostics,
                            next,
                        );
                    }
                }
            }
            check_omitted_generic_mappings(
                program,
                current,
                tail,
                &mut scope,
                functions,
                types,
                diagnostics,
                next,
            );
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Project { base: value, .. } => check_omitted_generic_mappings(
            program,
            current,
            value,
            bindings,
            functions,
            types,
            diagnostics,
            next,
        ),
        ExprKind::Binary { left, right, .. } => {
            for child in [left.as_ref(), right.as_ref()] {
                check_omitted_generic_mappings(
                    program,
                    current,
                    child,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for child in [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ] {
                check_omitted_generic_mappings(
                    program,
                    current,
                    child,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                check_omitted_generic_mappings(
                    program,
                    current,
                    &field.value,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::UpdateRecord { base, fields } => {
            check_omitted_generic_mappings(
                program,
                current,
                base,
                bindings,
                functions,
                types,
                diagnostics,
                next,
            );
            for field in fields {
                check_omitted_generic_mappings(
                    program,
                    current,
                    &field.value,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            check_omitted_generic_mappings(
                program,
                current,
                scrutinee,
                bindings,
                functions,
                types,
                diagnostics,
                next,
            );
            for arm in arms {
                let mut arm_scope = bindings.clone();
                clear_pattern_bindings(&arm.pattern, &mut arm_scope);
                if let Some(guard) = &arm.guard {
                    check_omitted_generic_mappings(
                        program,
                        current,
                        guard,
                        &mut arm_scope,
                        functions,
                        types,
                        diagnostics,
                        next,
                    );
                }
                check_omitted_generic_mappings(
                    program,
                    current,
                    &arm.value,
                    &mut arm_scope,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            check_omitted_generic_mappings(
                program,
                current,
                receiver,
                bindings,
                functions,
                types,
                diagnostics,
                next,
            );
            for argument in args {
                check_omitted_generic_mappings(
                    program,
                    current,
                    argument,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::SuperMethod { args, .. } => {
            for argument in args {
                check_omitted_generic_mappings(
                    program,
                    current,
                    argument,
                    bindings,
                    functions,
                    types,
                    diagnostics,
                    next,
                );
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Char(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Var(_) => {}
    }
}

fn clear_pattern_bindings(pattern: &MatchPattern, bindings: &mut HashMap<String, Binding>) {
    match pattern {
        MatchPattern::Variant { fields, .. } => {
            for field in fields {
                bindings.remove(&field.binding);
            }
        }
        MatchPattern::Record { fields, .. } => {
            for field in fields {
                clear_record_pattern_bindings(&field.pattern, bindings);
            }
        }
        MatchPattern::Binding { name, .. } => {
            bindings.remove(name);
        }
        MatchPattern::Wildcard { .. } | MatchPattern::Literal { .. } | MatchPattern::Or { .. } => {}
    }
}

fn clear_record_pattern_bindings(
    pattern: &RecordMatchFieldPattern,
    bindings: &mut HashMap<String, Binding>,
) {
    match pattern {
        RecordMatchFieldPattern::Binding { name, .. } => {
            bindings.remove(name);
        }
        RecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                clear_record_pattern_bindings(&field.pattern, bindings);
            }
        }
        RecordMatchFieldPattern::Wildcard { .. } => {}
    }
}

pub(super) fn check_function_bodies<'p>(
    program: &'p Program,
    functions: &HashMap<&'p str, &'p Function>,
    import_keys: &HashMap<&'p str, (&'p InterfaceDeclaration, &'p ImportDeclaration)>,
    types: &TypeTable<'p>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function_value_targets =
        crate::source_verify::function_value_inventory::function_value_targets(program, functions);
    for template in &program.functions {
        let type_parameters = template
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        check_declared_type(
            program,
            &template.return_type,
            template.span,
            types,
            &type_parameters,
            diagnostics,
        );
        for param in &template.params {
            check_declared_type(
                program,
                &param.ty,
                param.span,
                types,
                &type_parameters,
                diagnostics,
            );
        }
        if !template.type_parameters.is_empty()
            && (template
                .params
                .iter()
                .any(|parameter| matches!(parameter.ty, Type::Function { .. }))
                || matches!(template.return_type, Type::Function { .. }))
        {
            diagnostics.push(error(
                program,
                "SPX-T287",
                "generic function signatures cannot contain function values",
                template.span,
            ));
        }
        let generic_parameter_list_is_valid = (1..=2).contains(&template.type_parameters.len())
            && template
                .type_parameters
                .iter()
                .all(|parameter| source_identifier(&parameter.name))
            && template
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>()
                .len()
                == template.type_parameters.len();
        let transparent_vec_wrapper = crate::vec_ops::source_wrapper(program, template);
        let transparent_box_wrapper = crate::box_ops::source_wrapper(program, template);
        let specializations = if template.type_parameters.is_empty()
            || transparent_vec_wrapper.is_some()
            || transparent_box_wrapper.is_some()
        {
            vec![template.clone()]
        } else if generic_parameter_list_is_valid {
            // These clones exist only to validate every admitted direct-scalar
            // substitution. Executable HIR instances are discovered separately
            // from reachable explicit calls and never originate here.
            let owned_record = template
                .params
                .iter()
                .any(|param| generic_function_owned_record_slot(template, &param.ty, types))
                || generic_function_owned_record_slot(template, &template.return_type, types);
            let substitutions = if generic_result::profile(template) {
                generic_result::substitutions(template)
            } else if owned_record
                || crate::source_verify::declared_type::generic_variant::profile(template, types)
                || crate::source_verify::declared_type::generic_collection::profile(template)
            {
                owned_record_function_substitutions(template.type_parameters.len())
            } else {
                scalar_function_substitutions(template.type_parameters.len())
            };
            substitutions
                .iter()
                .filter_map(|arguments| validation_specialize_function(template, arguments))
                .collect()
        } else {
            Vec::new()
        };
        let mut specialized_diagnostics = HashSet::new();
        for function in &specializations {
            let specialized_diagnostic_start = diagnostics.len();
            let mut variables = HashMap::new();
            if function.return_type == Type::Str {
                diagnostics.push(error(
                    program,
                    "SPX-O116",
                    format!(
                        "function `{}` cannot return borrowed `str`; borrowed text is confined to the invocation",
                        function.name
                    ),
                    function.span,
                ));
            }
            if function.return_type == Type::SliceU8 {
                diagnostics.push(error(
                    program,
                    "SPX-T264",
                    format!(
                        "function `{}` cannot return borrowed `Slice<u8>`; byte views cannot escape their invocation",
                        function.name
                    ),
                    function.span,
                ));
            }
            for param in &function.params {
                if !source_identifier(&param.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S105",
                        format!("`{}` is not a valid parameter identifier", param.name),
                        param.span,
                    ));
                }
                if transparent_vec_wrapper.is_none() && transparent_box_wrapper.is_none() {
                    check_ownership_mode(program, function, param, types, diagnostics);
                }
                // By-value `string` parameters carry unique ownership. Bytes
                // use explicit `own Bytes`, but the shared predicate keeps
                // this source-side binding rule identical to resolved HIR.
                let binding_mode = if param.mode == ParamMode::Value && param.ty.is_uniquely_owned()
                {
                    ParamMode::Own
                } else {
                    param.mode
                };
                if variables
                    .insert(
                        param.name.clone(),
                        Binding {
                            ty: param.ty.clone(),
                            mode: binding_mode,
                            availability: Availability::Available,
                            moved_places: HashMap::new(),
                            definitely_partial: HashSet::new(),
                            native_unit_discard: false,
                            mutable: false,
                            active_loans: BTreeSet::new(),
                            borrow_origin: None,
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
                contract.visit_calls(&mut |callee, span| {
                    if crate::host_io_ops::by_name(callee).is_some() {
                        diagnostics.push(error(
                            program,
                            "SPX-T269",
                            "stdout_write is not admitted in contracts",
                            span,
                        ));
                    }
                    if crate::command_io_ops::by_name(callee).is_some() {
                        diagnostics.push(error(
                            program,
                            "SPX-T270",
                            "command I/O operations are not admitted in contracts",
                            span,
                        ));
                    }
                });
                require_bool(
                    program,
                    function,
                    contract,
                    &entry_variables,
                    functions,
                    types,
                    None,
                    diagnostics,
                    "precondition",
                );
            }

            if let Some(actual) = check_expr_iterative(
                program,
                function,
                &function.body,
                &mut variables,
                functions,
                types,
                None,
                true,
                diagnostics,
            ) {
                if actual.native_unit {
                    reject_native_unit_value(program, &function.body, &actual, diagnostics);
                }
                if !actual.native_unit && actual.ty != function.return_type {
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
                if types.needs_drop(&function.return_type) && actual.mode != ParamMode::Own {
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
                        .with_help(
                            "return an owned resource or declare a future lifetime-bound view",
                        ),
                    );
                }
            }

            for contract in &function.ensures {
                contract.visit_calls(&mut |callee, span| {
                    if crate::host_io_ops::by_name(callee).is_some() {
                        diagnostics.push(error(
                            program,
                            "SPX-T269",
                            "stdout_write is not admitted in contracts",
                            span,
                        ));
                    }
                    if crate::command_io_ops::by_name(callee).is_some() {
                        diagnostics.push(error(
                            program,
                            "SPX-T270",
                            "command I/O operations are not admitted in contracts",
                            span,
                        ));
                    }
                });
                require_bool(
                    program,
                    function,
                    contract,
                    &variables,
                    functions,
                    types,
                    Some(&function.return_type),
                    diagnostics,
                    "postcondition",
                );
            }

            let declared: HashSet<_> = function.effects.iter().map(String::as_str).collect();
            let mut required_lifecycle_effects = BTreeSet::new();
            for param in &function.params {
                if param.mode == ParamMode::Own {
                    required_lifecycle_effects
                        .extend(types.lifecycle_effects(&param.ty, import_keys));
                }
            }
            required_lifecycle_effects
                .extend(types.lifecycle_effects(&function.return_type, import_keys));
            for callee in crate::source_verify::function_value_inventory::calls(
                function,
                functions,
                &function_value_targets,
            ) {
                if let Some(target) = functions.get(callee.as_str()) {
                    required_lifecycle_effects
                        .extend(types.lifecycle_effects(&target.return_type, import_keys));
                }
            }
            for effect in required_lifecycle_effects {
                if !declared.contains(effect.as_str()) {
                    diagnostics.push(
                    error(
                        program,
                        "SPX-E103",
                        format!(
                            "function `{}` can own a resource; automatic finalization requires effect `{effect}`",
                            function.name
                        ),
                        function.span,
                    )
                    .with_help(format!(
                        "add `{effect}` to the function's `uses` set and module permits"
                    )),
                );
                }
            }
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
            for callee in crate::source_verify::function_value_inventory::calls(
                function,
                functions,
                &function_value_targets,
            ) {
                let span = function.body.span;
                if let Some(op) = crate::host_io_ops::by_name(&callee) {
                    if !declared.contains(op.effect()) {
                        diagnostics.push(error(
                            program,
                            "SPX-E102",
                            format!(
                                "call to `{callee}` requires effect `{}`; add it to `{}`",
                                op.effect(),
                                function.name
                            ),
                            span,
                        ));
                    }
                    continue;
                }
                if let Some(op) = crate::command_io_ops::by_name(&callee) {
                    for effect in crate::command_io_ops::required_effects(op) {
                        if !declared.contains(effect) {
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
                    continue;
                }
                if let Some(target) = functions.get(callee.as_str()) {
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
            }
            if !template.type_parameters.is_empty() {
                let added = diagnostics
                    .drain(specialized_diagnostic_start..)
                    .collect::<Vec<_>>();
                for diagnostic in added {
                    if specialized_diagnostics.insert(diagnostic.json()) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod generic_inference_tests {
    use super::*;
    use std::path::Path;

    fn parsed(source: &str) -> Program {
        crate::parse(source, Path::new("generic-forwarding-precheck.spx")).unwrap()
    }

    fn mapping_diagnostics(program: &Program) -> Vec<Diagnostic> {
        let functions = program
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function))
            .collect::<HashMap<_, _>>();
        let function_value_targets =
            crate::source_verify::function_value_inventory::function_value_targets(
                program, &functions,
            );
        let call_graph = program
            .functions
            .iter()
            .map(|function| {
                (
                    function.name.clone(),
                    crate::source_verify::function_value_inventory::calls(
                        function,
                        &functions,
                        &function_value_targets,
                    ),
                )
            })
            .collect();
        let generic_functions = program
            .functions
            .iter()
            .filter(|function| !function.type_parameters.is_empty())
            .map(|function| function.name.as_str())
            .collect();
        let mut diagnostics = Vec::new();
        check_generic_function_cycles(program, &call_graph, &generic_functions, &mut diagnostics);
        diagnostics
    }

    fn nested_calls(count: usize, explicit: bool) -> Program {
        let mut program = parsed(
            r#"
module test.generic_precheck;
@id("infer.id") fn id<T>(value:T)->T{value}
@id("infer.outer") fn outer<T>(value:T)->T{value}
@id("app.main") fn main()->i64{0}
"#,
        );
        let mut expression = Expr {
            kind: ExprKind::Var("value".to_owned()),
            span: crate::ast::Span::default(),
        };
        for _ in 0..count {
            expression = Expr {
                kind: ExprKind::Call {
                    name: "id".to_owned(),
                    type_arguments: if explicit {
                        vec![Type::Named {
                            name: "T".to_owned(),
                            arguments: Vec::new(),
                        }]
                    } else {
                        Vec::new()
                    },
                    args: vec![expression],
                },
                span: crate::ast::Span::default(),
            };
        }
        program
            .functions
            .iter_mut()
            .find(|function| function.name == "outer")
            .unwrap()
            .body = expression;
        program
    }

    #[test]
    fn omitted_mapping_depth_fails_closed_without_rejecting_explicit_mapping() {
        assert!(mapping_diagnostics(&nested_calls(128, false)).is_empty());
        let omitted_program = nested_calls(129, false);
        let omitted = mapping_diagnostics(&omitted_program);
        assert!(omitted.iter().any(|diagnostic| {
            diagnostic.code == "SPX-T225"
                && diagnostic
                    .message
                    .contains("beyond the bounded static traversal")
        }));
        let explicit_program = nested_calls(129, true);
        assert!(mapping_diagnostics(&explicit_program).is_empty());
    }

    #[test]
    fn unknown_let_shadow_cannot_reuse_the_outer_parameter_fact() {
        let program = parsed(
            r#"
module test.generic_shadow;
@id("infer.id") fn id<T>(value:T)->T{value}
@id("infer.outer") fn outer<T>(value:T)->T{{let value=missing;id(value)}}
@id("app.main") fn main()->i64{0}
"#,
        );
        let diagnostics = mapping_diagnostics(&program);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T225"));
    }

    #[test]
    fn match_binding_shadow_cannot_reuse_the_outer_parameter_fact() {
        let program = parsed(
            r#"
module test.generic_match_shadow;
@id("infer.id") fn id<T>(value:T)->T{value}
@id("infer.outer") fn outer<T>(value:T)->T{match value{value=>id(value),}}
@id("app.main") fn main()->i64{0}
"#,
        );
        let diagnostics = mapping_diagnostics(&program);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T225"));
    }
}
