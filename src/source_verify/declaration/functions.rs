//! Function-level checks: declaration admission, generic call cycles, and the
//! per-function body, contract, and effect checks.
use crate::ast::program_source::ProgramSource;
use crate::ast::{
    Expr, ExprKind, Function, ImportDeclaration, InterfaceDeclaration, MatchPattern, ParamMode,
    RecordMatchFieldPattern, Statement, Type,
};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::declared_type::{
    check_declared_type, check_ownership_mode_ref, function_reaches, function_reaches_any,
    generic_function_arguments_are_forwarded, generic_function_contains_nested_owned_record_slot,
    generic_function_expression_is_direct_scalar,
    generic_function_expression_is_owned_record_composition,
    generic_function_has_owned_record_composition, generic_function_owned_record_slot,
    generic_function_signature_slot, owned_record_function_substitutions,
    scalar_function_substitutions,
};
use crate::source_verify::declared_type::{declared_collection::reject_function, generic_result};
use crate::source_verify::diagnostics::{
    error, invalid_stable_id, reject_native_unit_value, reject_reserved_host_id,
    require_bool_with_signature_ref, source_identifier,
};
use crate::source_verify::iterative::check_expr_with_signature_cell;
mod signature_refusal;
use crate::source_verify::type_table::TypeTable;
pub(in crate::source_verify) use signature_refusal::SignatureOrigin;
use std::collections::{BTreeSet, HashMap, HashSet};
pub(super) fn check_function_declarations<'p>(
    program: &'p dyn ProgramSource,
    cells: &'p [crate::ast::program_source::FunctionCell<'p>],
    types: &TypeTable<'p>,
    functions: &mut crate::source_verify::function_map::FunctionMap<'p>,
    ids: &mut HashSet<&'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, function) in cells.iter().take(program.function_refs().len()) {
        if !source_identifier(function.name()) {
            diagnostics.push(error(
                program,
                "SPX-S104",
                format!("`{}` is not a valid function identifier", function.name()),
                function.payload().name_span,
            ));
        }
        if function.name() == crate::literal_format::NAME
            || crate::string_ops::by_name(function.name()).is_some()
            || crate::map_ops::by_name(function.name()).is_some()
        {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned string operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        if crate::str_ops::by_name(function.name()).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned borrowed string operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        if crate::byte_ops::by_name(function.name()).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned byte operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        if super::super::vec_field::reserved_function_ref(function) {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned vector operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        if crate::box_ops::by_name(function.name()).is_some()
            || crate::iterator_ops::by_name(function.name()).is_some()
            || crate::list_ops::by_name(function.name()).is_some()
        {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned box operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        if crate::host_io_ops::by_name(function.name()).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned host I/O operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        if crate::command_io_ops::by_name(function.name()).is_some()
            || crate::stdin_stream_ops::pure_by_name(function.name()).is_some()
        {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "function name `{}` is reserved by the compiler-owned command I/O operations",
                    function.name()
                ),
                function.payload().name_span,
            ));
        }
        reject_reserved_host_id(
            program,
            &function.payload().stable_id,
            "function",
            function.payload().span,
            diagnostics,
        );
        if functions.insert(function.name(), function).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S101",
                format!("duplicate function `{}`", function.name()),
                function.payload().name_span,
            ));
        }
        if function.payload().stable_id.is_empty() {
            diagnostics.push(
                error(
                    program,
                    "SPX-S102",
                    format!("function `{}` has an empty stable id", function.name()),
                    function.payload().name_span,
                )
                .with_help("give the declaration a dotted stable identity with @id(\"your.namespace.symbol\")"),
            );
        } else if function.payload().stable_id.contains('\0') {
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-S102",
                format!("function `{}`", function.name()),
                function.payload().span,
            ));
        } else if function
            .payload()
            .stable_id
            .starts_with("semaprax.function-execution.v1:")
        {
            diagnostics.push(error(
                program,
                "SPX-T225",
                format!(
                    "function `{}` uses the reserved generic execution identity domain",
                    function.name()
                ),
                function.payload().span,
            ));
        } else if !ids.insert(function.payload().stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", function.payload().stable_id),
                function.payload().span,
            ));
        }
        if !function.payload().explicit_id {
            diagnostics.push(
                Diagnostic::warning(
                    "SPX-S103",
                    format!(
                        "function `{}` has an automatic identity that changes when renamed",
                        function.name()
                    ),
                    function.payload().name_span,
                )
                .at_path(program.path())
                .with_help("add @id(\"your.namespace.symbol\") before the declaration"),
            );
        }
        if crate::vec_ops::wrapper_by_id(&function.payload().stable_id).is_some()
            && !crate::vec_ops::source_module_is_authenticated(program)
        {
            diagnostics.push(error(
                program,
                "SPX-T283",
                format!(
                    "vector wrapper identity `{}` is reserved for module `{}`",
                    function.payload().stable_id,
                    crate::vec_ops::MODULE
                ),
                function.payload().span,
            ));
            continue;
        }
        if crate::box_ops::wrapper_by_id(&function.payload().stable_id).is_some()
            && !crate::box_ops::source_module_is_authenticated(program)
        {
            diagnostics.push(error(
                program,
                "SPX-T286",
                format!(
                    "box wrapper identity `{}` is reserved for module `{}`",
                    function.payload().stable_id,
                    crate::box_ops::MODULE
                ),
                function.payload().span,
            ));
            continue;
        }
        if crate::box_ops::wrapper_by_id(&function.payload().stable_id).is_some()
            && crate::box_ops::source_wrapper_ref(program, &*function).is_none()
        {
            diagnostics.push(error(
                program,
                "SPX-T286",
                format!(
                    "`{}.{}` must be one exact transparent compiler-owned box wrapper",
                    program.module(),
                    function.name()
                ),
                function.payload().span,
            ));
            continue;
        }
        if crate::vec_ops::wrapper_by_id(&function.payload().stable_id).is_some()
            && crate::vec_ops::source_wrapper_ref(program, &*function).is_none()
        {
            diagnostics.push(error(
                program,
                "SPX-T283",
                format!(
                    "`{}.{}` must be one exact transparent compiler-owned vector wrapper",
                    program.module(),
                    function.name()
                ),
                function.payload().span,
            ));
            continue;
        }
        if !function.payload().type_parameters.is_empty() {
            let transparent_vec_wrapper = crate::vec_ops::source_wrapper_ref(program, &*function);
            let transparent_box_wrapper = crate::box_ops::source_wrapper_ref(program, &*function);
            if crate::vec_ops::is_source_candidate_ref(program, &*function)
                && transparent_vec_wrapper.is_none()
            {
                diagnostics.push(error(
                    program,
                    "SPX-T283",
                    format!(
                        "`{}.{}` must be one exact transparent compiler-owned vector wrapper",
                        program.module(),
                        function.name()
                    ),
                    function.payload().span,
                ));
                continue;
            }
            if transparent_vec_wrapper.is_some() {
                continue;
            }
            if crate::box_ops::is_source_candidate_ref(program, &*function)
                && transparent_box_wrapper.is_none()
            {
                diagnostics.push(error(
                    program,
                    "SPX-T286",
                    format!(
                        "`{}.{}` must be one exact transparent compiler-owned box wrapper",
                        program.module(),
                        function.name()
                    ),
                    function.payload().span,
                ));
                continue;
            }
            if transparent_box_wrapper.is_some() {
                continue;
            }
            if !(1..=2).contains(&function.payload().type_parameters.len()) {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` requires one or two type parameters",
                        function.name()
                    ),
                    function.payload().span,
                ));
            }
            let mut parameter_names = HashSet::new();
            for parameter in &function.payload().type_parameters {
                if !source_identifier(&parameter.name)
                    || !parameter_names.insert(parameter.name.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "invalid or duplicate type parameter `{}` on function `{}`",
                            parameter.name,
                            function.name()
                        ),
                        parameter.span,
                    ));
                }
            }
            if !function.payload().effects.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T226",
                    format!(
                        "generic function `{}` must be effect-free in this slice",
                        function.name()
                    ),
                    function.payload().span,
                ));
            }
            let variant = crate::source_verify::declared_type::generic_variant::profile(
                function.payload(),
                types,
            );
            let owned_result = generic_result::profile(function.payload());
            let collection = crate::source_verify::declared_type::generic_collection::profile(
                function.payload(),
            );
            for param in &function.payload().params {
                let owned_record =
                    generic_function_owned_record_slot(function.payload(), &param.ty, types);
                if !((param.mode == ParamMode::Value
                    && (generic_function_signature_slot(&param.ty, &parameter_names)
                        || (collection
                            && crate::source_verify::declared_type::generic_collection::callback(
                                function.payload(),
                                &param.ty,
                            ))
                        || ((collection || variant)
                            && crate::vec_ops::ast_element_is_admitted(&param.ty))))
                    || (param.mode == ParamMode::Own
                        && (owned_record
                            || (variant
                                && crate::source_verify::declared_type::generic_variant::slot(
                                    function.payload(),
                                    &param.ty,
                                    types,
                                ))
                            || (collection
                                && crate::source_verify::declared_type::generic_collection::slot(
                                    function.payload(),
                                    &param.ty,
                                ))
                            || (owned_result
                                && generic_result::slot(function.payload(), &param.ty)))))
                    && !signature_refusal::unknown_component(
                        program,
                        &param.ty,
                        param.span,
                        types,
                        &parameter_names,
                    )
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "generic function `{}.{}` must use the direct-scalar profile by value or one admitted flat owned-record template by ownership",
                            function.name(), param.name
                        ),
                        param.span,
                    ));
                }
            }
            if !owned_result
                && !variant
                && !collection
                && !generic_function_signature_slot(
                    &function.payload().return_type,
                    &parameter_names,
                )
                && !generic_function_owned_record_slot(
                    function.payload(),
                    &function.payload().return_type,
                    types,
                )
                && !signature_refusal::unknown_component(
                    program,
                    &function.payload().return_type,
                    function.payload().span,
                    types,
                    &parameter_names,
                )
            {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` must return the direct-scalar profile or one admitted flat owned-record template",
                        function.name()
                    ),
                    function.payload().span,
                ));
            }
            if generic_function_contains_nested_owned_record_slot(function.payload(), types)
                && !generic_function_has_owned_record_composition(function.payload(), types)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` must transfer exactly one bounded owned-record parameter into an identical result type",
                        function.name()
                    ),
                    function.payload().span,
                ));
            }
            if let Err(diagnostic) =
                crate::source_verify::closure::validate_generic_syntax(program, function.payload())
            {
                diagnostics.push(diagnostic);
            }
            let invalid_contract = function
                .payload()
                .requires
                .iter()
                .chain(&function.payload().ensures)
                .any(|expression| !generic_function_expression_is_direct_scalar(expression));
            let invalid_body =
                !crate::source_verify::declared_type::generic_variant::body(
                    function.payload(),
                    types,
                    &function.payload().body,
                ) && !generic_result::body(function.payload(), &function.payload().body)
                    && !generic_function_expression_is_direct_scalar(&function.payload().body)
                    && !generic_function_expression_is_owned_record_composition(
                        function.payload(),
                        types,
                        &function.payload().body,
                    );
            let invalid_body = invalid_body && !(crate::source_verify::generic_collection_profile(function.payload())
                && crate::source_verify::declared_type::generic_collection_expression_is_admitted(&function.payload().body));
            if invalid_contract || invalid_body {
                diagnostics.push(error(
                    program,
                    "SPX-T226",
                    format!(
                        "generic function `{}` uses an expression outside the direct-scalar slice",
                        function.name()
                    ),
                    function.payload().span,
                ));
            }
        }
    }
}
pub(super) fn check_generic_function_cycles_with_types<'p>(
    program: &'p dyn ProgramSource,
    cells: &'p [crate::ast::program_source::FunctionCell<'p>],
    call_graph: &HashMap<String, Vec<String>>,
    generic_functions: &HashSet<&'p str>,
    types: &TypeTable<'p>,
    functions: &crate::source_verify::function_map::FunctionMap<'p>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, function) in cells
        .iter()
        .take(program.functions().len())
        .filter(|(_, function)| !function.payload().type_parameters.is_empty())
    {
        let mut bindings = function
            .payload()
            .params
            .iter()
            .map(|parameter| (parameter.name.clone(), static_binding(parameter.ty.clone())))
            .collect::<HashMap<_, _>>();
        for expression in function
            .payload()
            .requires
            .iter()
            .chain(std::iter::once(&function.payload().body))
            .chain(&function.payload().ensures)
        {
            check_omitted_generic_mappings(
                program,
                &function,
                expression,
                &mut bindings,
                &functions,
                types,
                diagnostics,
                0,
            );
            expression.visit_call_instances(&mut |callee, arguments, span| {
                if let Some(target) = program.function_refs().find_named_payload(callee, |target| !target.type_parameters.is_empty()) {
                    if arguments.is_empty() {
                        return;
                    }
                    if !generic_function_arguments_are_forwarded(function.payload(), target.payload(), arguments) {
                        diagnostics.push(error(
                            program,
                            "SPX-T225",
                            format!(
                                "generic function `{}` must supply admitted explicit type-argument mappings when calling generic function `{callee}`",
                                function.name()
                            ),
                            span,
                        ));
                    }
                }
            });
        }
        let participates_in_cycle = call_graph.get(function.name()).is_some_and(|callees| {
            callees.iter().any(|callee| {
                function_reaches(call_graph, callee, function.name(), &mut HashSet::new())
            })
        });
        if participates_in_cycle {
            diagnostics.push(error(
                program,
                "SPX-T226",
                format!(
                    "generic function `{}` participates in a recursive call cycle",
                    function.name()
                ),
                function.payload().span,
            ));
        }
        let direct_generic_call = call_graph.get(function.name()).is_some_and(|callees| {
            callees
                .iter()
                .any(|callee| generic_functions.contains(callee.as_str()))
        });
        let reaches_other_generic = call_graph.get(function.name()).is_some_and(|callees| {
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
                    function.name()
                ),
                function.payload().span,
            ));
        }
    }
}
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod traversal_scope;
use traversal_scope::static_binding;
const MAX_STATIC_MAPPING_DEPTH: usize = 128;
/// Carries the lexical scope, declaration table, and bounded traversal state
/// required to authenticate an omitted symbolic mapping before body checking.
#[allow(clippy::too_many_arguments)]
fn check_omitted_generic_mappings(
    program: &dyn ProgramSource,
    current: &crate::ast::program_source::FunctionRef<'_>,
    expression: &Expr,
    bindings: &mut HashMap<String, Binding>,
    functions: &crate::source_verify::function_map::FunctionMap<'_>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    depth: usize,
) {
    if depth >= MAX_STATIC_MAPPING_DEPTH {
        expression.visit_call_instances(&mut |callee, arguments, span| {
            if arguments.is_empty()
                && functions
                    .get(callee)
                    .is_some_and(|target| !target.payload().type_parameters.is_empty())
            {
                diagnostics.push(error(
                    program,
                    "SPX-T225",
                    format!(
                        "generic function `{}` has an omitted type-argument mapping beyond the bounded static traversal",
                        current.name()
                    ),
                    span,
                ));
            }
        });
        return;
    }
    let next = depth + 1;
    match &expression.kind {
        ExprKind::LexicalExit { value: None, .. } => {}
        ExprKind::Closure { .. } => {}
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
            let Some(target) = functions.get(name.as_str()).copied() else {
                return;
            };
            if target.payload().type_parameters.is_empty() || !type_arguments.is_empty() {
                return;
            }
            let inferred = crate::source_verify::generic_inference::arguments_ref(
                program, &current, &target, args, bindings, functions, types,
            );
            if !inferred.as_ref().is_some_and(|arguments| {
                generic_function_arguments_are_forwarded(
                    current.payload(),
                    target.payload(),
                    arguments,
                )
            }) {
                diagnostics.push(error(
                    program,
                    "SPX-T225",
                    format!(
                        "generic function `{}` must supply admitted explicit type-argument mappings when calling generic function `{name}`",
                        current.name()
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
                            crate::source_verify::generic_inference::expression_type_ref(
                                program, &current, value, &scope, functions, types,
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
                    }
                    | Statement::ForBorrow {
                        item, values, body, ..
                    }
                    | Statement::ForOwn {
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
                        if let Statement::ForBorrow {
                            index: Some(index), ..
                        } = statement
                        {
                            body_scope.remove(&index.name);
                        }
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
        ExprKind::LexicalExit {
            value: Some(value), ..
        }
        | ExprKind::VecConstructor { value, .. }
        | ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Yield { request: value }
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
        | ExprKind::BorrowedUtf8Literal(_)
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
    program: &'p dyn ProgramSource,
    cells: &'p [crate::ast::program_source::FunctionCell<'p>],
    functions: &crate::source_verify::function_map::FunctionMap<'p>,
    import_keys: &HashMap<&'p str, (&'p InterfaceDeclaration, &'p ImportDeclaration)>,
    types: &TypeTable<'p>,
    diagnostics: &mut Vec<Diagnostic>,
    fused_producer: Option<&'p Expr>,
    funding: &mut crate::source_verify::funding::SourceFunding<'_>,
) -> Result<(), crate::source_verify::funding::SourceStop> {
    let byte_helpers = crate::loop_calls::byte_writer::SourceIndex::new(program);
    let function_value_targets =
        crate::source_verify::function_value_inventory::function_value_targets(program, functions);
    for (_, template) in cells.iter().take(program.functions().len()) {
        reject_function(program, template.payload(), types, diagnostics);
        let type_parameters = template
            .payload()
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        let return_frontier = diagnostics.len();
        check_declared_type(
            program,
            &template.payload().return_type,
            template.payload().span,
            types,
            &type_parameters,
            diagnostics,
        );
        let return_unknown = diagnostics[return_frontier..]
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T001");
        for param in &template.payload().params {
            check_declared_type(
                program,
                &param.ty,
                param.span,
                types,
                &type_parameters,
                diagnostics,
            );
        }
        if !template.payload().type_parameters.is_empty()
            && !crate::source_verify::declared_type::generic_collection::profile(template.payload())
            && (template
                .payload()
                .params
                .iter()
                .any(|parameter| matches!(parameter.ty, Type::Function { .. }))
                || matches!(template.payload().return_type, Type::Function { .. }))
        {
            diagnostics.push(error(
                program,
                "SPX-T287",
                "generic function signatures cannot contain function values",
                template.payload().span,
            ));
        }
        let generic_parameter_list_is_valid = (1..=2)
            .contains(&template.payload().type_parameters.len())
            && template
                .payload()
                .type_parameters
                .iter()
                .all(|parameter| source_identifier(&parameter.name))
            && template
                .payload()
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>()
                .len()
                == template.payload().type_parameters.len();
        let transparent_vec_wrapper = crate::vec_ops::source_wrapper_ref(program, &template);
        let transparent_box_wrapper = crate::box_ops::source_wrapper_ref(program, &template);
        let specializations: std::borrow::Cow<'_, [Function]> =
            if template.payload().type_parameters.is_empty()
                || transparent_vec_wrapper.is_some()
                || transparent_box_wrapper.is_some()
            {
                std::borrow::Cow::Borrowed(std::slice::from_ref(template.payload()))
            } else if generic_parameter_list_is_valid {
                // These clones exist only to validate every admitted direct-scalar
                // substitution. Executable HIR instances are discovered separately
                // from reachable explicit calls and never originate here.
                let owned_record = template.payload().params.iter().any(|param| {
                    generic_function_owned_record_slot(template.payload(), &param.ty, types)
                }) || generic_function_owned_record_slot(
                    template.payload(),
                    &template.payload().return_type,
                    types,
                );
                let substitutions = if generic_result::profile(template.payload()) {
                    generic_result::substitutions(template.payload())
                } else if owned_record
                    || crate::source_verify::declared_type::generic_variant::profile(
                        template.payload(),
                        types,
                    )
                    || crate::source_verify::declared_type::generic_collection::profile(
                        template.payload(),
                    )
                {
                    owned_record_function_substitutions(template.payload().type_parameters.len())
                } else {
                    scalar_function_substitutions(template.payload().type_parameters.len())
                };
                std::borrow::Cow::Owned(
                    substitutions
                        .iter()
                        .filter_map(|arguments| {
                            crate::source_verify::closure::specialize_checked(
                                program,
                                template.payload(),
                                arguments,
                                diagnostics,
                            )
                        })
                        .collect(),
                )
            } else {
                std::borrow::Cow::Owned(Vec::new())
            };
        let mut specialized_diagnostics = HashSet::new();
        for function in specializations.iter() {
            let specialized_view;
            let function = if std::ptr::eq(function, template.payload()) {
                template
            } else {
                funding.specialized_presentation()?;
                specialized_view = template.specialized_payload(function);
                &specialized_view
            };
            let return_unknown = return_unknown
                || signature_refusal::origin_ref(program, template, function)
                    .and_then(|origin| {
                        origin.return_refusal_ref(program, function, types, diagnostics)
                    })
                    .is_some();
            let specialized_diagnostic_start = diagnostics.len();
            let mut variables = HashMap::new();
            if function.payload().return_type == Type::Str {
                diagnostics.push(error(
                    program,
                    "SPX-O116",
                    format!(
                        "function `{}` cannot return borrowed `str`; borrowed text is confined to the invocation",
                        function.name()
                    ),
                    function.payload().span,
                ).with_help("return an owned `string` instead: a literal, `string_concat`, or `string_from_i64` result is owned"));
            }
            super::super::declared_type::string_record::reject(
                program,
                &function.payload().return_type,
                "returned",
                function.payload().span,
                types,
                diagnostics,
            );
            if function.payload().return_type == Type::SliceU8 {
                diagnostics.push(error(
                    program,
                    "SPX-T264",
                    format!(
                        "function `{}` cannot return borrowed `Slice<u8>`; byte views cannot escape their invocation",
                        function.name()
                    ),
                    function.payload().span,
                ));
            }
            for param in &function.payload().params {
                if !source_identifier(&param.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S105",
                        format!("`{}` is not a valid parameter identifier", param.name),
                        param.span,
                    ));
                }
                if transparent_vec_wrapper.is_none() && transparent_box_wrapper.is_none() {
                    check_ownership_mode_ref(program, function, param, types, diagnostics);
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
                            contract_local: false,
                            traversal_row: None,
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

            for contract in &function.payload().requires {
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
                require_bool_with_signature_ref(
                    program,
                    function,
                    contract,
                    &variables,
                    functions,
                    types,
                    None,
                    diagnostics,
                    "precondition",
                    signature_refusal::origin_ref(program, template, function),
                );
            }

            let (body_value, refusal_view) = check_expr_with_signature_cell(
                program,
                function,
                &function.payload().body,
                &mut variables,
                functions,
                types,
                None,
                true,
                diagnostics,
                Some(&byte_helpers),
                fused_producer,
                signature_refusal::origin_ref(program, template, function),
            );
            if let Some(actual) = body_value {
                if actual.native_unit {
                    reject_native_unit_value(
                        program,
                        &function.payload().body,
                        &actual,
                        diagnostics,
                    );
                }
                if !actual.native_unit
                    && actual.ty != Type::Never
                    && actual.ty != function.payload().return_type
                    && !return_unknown
                    && !refusal_view
                        .is_some_and(|view| view.declared == &function.payload().return_type)
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T103",
                        format!(
                            "function `{}` returns {}, but its signature declares {}",
                            function.name(),
                            crate::source_verify::private_names::type_name(
                                program, types, &actual.ty
                            ),
                            crate::source_verify::private_names::type_name(
                                program,
                                types,
                                &function.payload().return_type
                            )
                        ),
                        function.payload().body.span,
                    ));
                }
                if actual.ty != Type::Never
                    && (actual.native_unit || actual.ty == function.payload().return_type)
                    && types.needs_drop(&function.payload().return_type)
                    && actual.mode != ParamMode::Own
                {
                    diagnostics.push(
                        error(
                            program,
                            "SPX-O104",
                            format!(
                                "function `{}` cannot return a {} resource as owned",
                                function.name(),
                                actual.mode.text()
                            ),
                            function.payload().body.span,
                        )
                        .with_help(
                            "return an owned resource or declare a future lifetime-bound view",
                        ),
                    );
                }
            }

            for contract in &function.payload().ensures {
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
                require_bool_with_signature_ref(
                    program,
                    function,
                    contract,
                    &variables,
                    functions,
                    types,
                    Some(&function.payload().return_type),
                    diagnostics,
                    "postcondition",
                    signature_refusal::origin_ref(program, template, function),
                );
            }

            let declared: HashSet<_> = function
                .payload()
                .effects
                .iter()
                .map(String::as_str)
                .collect();
            let mut required_lifecycle_effects = BTreeSet::new();
            for param in &function.payload().params {
                if param.mode == ParamMode::Own {
                    required_lifecycle_effects
                        .extend(types.lifecycle_effects(&param.ty, import_keys));
                }
            }
            required_lifecycle_effects
                .extend(types.lifecycle_effects(&function.payload().return_type, import_keys));
            for callee in crate::source_verify::function_value_inventory::calls(
                function.payload(),
                functions,
                &function_value_targets,
            ) {
                if let Some(target) = functions.get(callee.as_str()) {
                    required_lifecycle_effects.extend(
                        types.lifecycle_effects(&target.payload().return_type, import_keys),
                    );
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
                            function.name()
                        ),
                        function.payload().span,
                    )
                    .with_help(format!(
                        "add `{effect}` to the function's `uses` set and module permits"
                    )),
                );
                }
            }
            for effect in &function.payload().effects {
                if !program.permits().iter().any(|permit| permit == effect) {
                    diagnostics.push(crate::source_verify::hints::unpermitted_effect_ref(
                        program, function, effect,
                    ));
                }
            }
            for callee in crate::source_verify::function_value_inventory::calls(
                function.payload(),
                functions,
                &function_value_targets,
            ) {
                let span = function.payload().body.span;
                if let Some(effect) = crate::string_ops::by_name(&callee)
                    .and_then(crate::string_ops::StringOp::effect)
                {
                    if !declared.contains(effect) {
                        diagnostics.push(crate::source_verify::hints::missing_effect_ref(
                            program, &callee, effect, function, span,
                        ));
                    }
                    continue;
                }
                if let Some(op) = crate::host_io_ops::by_name(&callee) {
                    if !declared.contains(op.effect()) {
                        diagnostics.push(crate::source_verify::hints::missing_effect_ref(
                            program,
                            &callee,
                            op.effect(),
                            function,
                            span,
                        ));
                    }
                    continue;
                }
                if let Some(op) = crate::command_io_ops::by_name(&callee) {
                    for effect in crate::command_io_ops::required_effects(op) {
                        if !declared.contains(effect) {
                            diagnostics.push(crate::source_verify::hints::missing_effect_ref(
                                program, &callee, effect, function, span,
                            ));
                        }
                    }
                    continue;
                }
                if let Some(target) = functions.get(callee.as_str()) {
                    for effect in &target.payload().effects {
                        if !declared.contains(effect.as_str()) {
                            diagnostics.push(crate::source_verify::hints::missing_effect_ref(
                                program, &callee, effect, function, span,
                            ));
                        }
                    }
                }
            }
            if !template.payload().type_parameters.is_empty() {
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
    if let Some(error) = byte_helpers.failure() {
        diagnostics.push(error.at_path(program.path()));
    }
    Ok(())
}

#[cfg(test)]
mod generic_inference_tests;

#[cfg(test)]
#[rustfmt::skip]
pub(super) mod old;

#[cfg(test)]
#[path = "functions/generic_cycle_compat.rs"]
mod generic_cycle_compat;
#[cfg(test)]
pub(super) use generic_cycle_compat::check_generic_function_cycles;

#[cfg(test)]
#[rustfmt::skip]
pub(in crate::source_verify) mod public_signature_before;

#[cfg(test)]
#[path = "functions/funding_tests.rs"]
mod funding_tests;
