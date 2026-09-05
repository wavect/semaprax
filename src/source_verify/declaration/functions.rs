//! Function-level checks: declaration admission, generic call cycles, and the
//! per-function body, contract, and effect checks.

use crate::ast::{Function, ImportDeclaration, InterfaceDeclaration, ParamMode, Program, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::declared_type::{
    check_declared_type, check_ownership_mode, function_reaches, function_reaches_any,
    generic_function_expression_is_direct_scalar, generic_function_signature_slot,
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
        if !function.type_parameters.is_empty() {
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
            for param in &function.params {
                if param.mode != ParamMode::Value
                    || !generic_function_signature_slot(&param.ty, &parameter_names)
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "generic function `{}.{}` must use a direct `i64`, `bool`, or an in-scope function type parameter by value",
                            function.name, param.name
                        ),
                        param.span,
                    ));
                }
            }
            if !generic_function_signature_slot(&function.return_type, &parameter_names) {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` must return direct `i64`, `bool`, or an in-scope function type parameter",
                        function.name
                    ),
                    function.span,
                ));
            }
            if function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| !generic_function_expression_is_direct_scalar(expression))
            {
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
    for function in program
        .functions
        .iter()
        .filter(|function| !function.type_parameters.is_empty())
    {
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

pub(super) fn check_function_bodies<'p>(
    program: &'p Program,
    functions: &HashMap<&'p str, &'p Function>,
    import_keys: &HashMap<&'p str, (&'p InterfaceDeclaration, &'p ImportDeclaration)>,
    types: &TypeTable<'p>,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
        let specializations = if template.type_parameters.is_empty() {
            vec![template.clone()]
        } else if generic_parameter_list_is_valid {
            // These clones exist only to validate every admitted direct-scalar
            // substitution. Executable HIR instances are discovered separately
            // from reachable explicit calls and never originate here.
            scalar_function_substitutions(template.type_parameters.len())
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
                check_ownership_mode(program, function, param, types, diagnostics);
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
            function.body.visit_calls(&mut |callee, _| {
                if let Some(target) = functions.get(callee) {
                    required_lifecycle_effects
                        .extend(types.lifecycle_effects(&target.return_type, import_keys));
                }
            });
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
            function.body.visit_calls(&mut |callee, span| {
                if let Some(op) = crate::host_io_ops::by_name(callee) {
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
                    return;
                }
                if let Some(op) = crate::command_io_ops::by_name(callee) {
                    let effect = crate::command_io_ops::effect(op);
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
                    return;
                }
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
