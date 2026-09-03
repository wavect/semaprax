//! Program-level verification: `verify` builds the shared declaration state
//! and runs the ordered passes that own each concern.

use self::classes::{
    check_class_cycles, check_class_methods, check_class_overrides, check_class_parents,
};
use self::declarations::{
    check_byte_data_declarations, check_declared_fields, check_native_rust_imports,
    check_record_layouts, check_resource_lifecycles,
};
use self::functions::{
    check_function_bodies, check_function_declarations, check_generic_function_cycles,
};
use self::identity::{check_interface_identities, check_type_identities};
use super::capacity::{source_capacity_functions, verify_byte_data_capacity};
use super::diagnostics::error;
use super::type_table::TypeTable;
use crate::ast::{Program, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{HashMap, HashSet};

mod classes;
mod declarations;
mod functions;
mod identity;

pub(crate) fn verify(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Err(diagnostic) = crate::static_protocol::validate(program) {
        diagnostics.push(diagnostic.at_path(&program.path));
        return diagnostics;
    }
    if !program.module_uses.is_empty() {
        diagnostics.push(
            Diagnostic::io(
                "SPX-G172",
                "source module imports require Workspace Semantic Graph resolution",
            )
            .at_path(&program.path),
        );
        return diagnostics;
    }
    let mut functions = HashMap::new();
    let mut ids = crate::prelude::all_ids()
        .into_iter()
        .collect::<HashSet<_>>();
    let mut type_names = HashSet::new();

    check_type_identities(program, &mut ids, &mut type_names, &mut diagnostics);

    let mut interface_names = HashSet::new();
    let mut import_keys = HashMap::new();
    check_interface_identities(
        program,
        &mut ids,
        &mut interface_names,
        &mut import_keys,
        &mut diagnostics,
    );

    let types = TypeTable::new(program);
    check_byte_data_declarations(program, &types, &mut diagnostics);
    let mut native_rust_names = HashSet::new();
    check_native_rust_imports(program, &types, &mut native_rust_names, &mut diagnostics);
    check_resource_lifecycles(program, &import_keys, &mut diagnostics);
    check_declared_fields(program, &types, &mut diagnostics);
    let mut checked_layouts = HashSet::new();
    check_record_layouts(program, &types, &mut checked_layouts, &mut diagnostics);

    check_function_declarations(program, &mut functions, &mut ids, &mut diagnostics);

    check_class_methods(program, &types, &mut ids, &mut diagnostics);

    // Class Inheritance v1: static structural checks over `extends` links —
    // unknown or non-class parents, cycles, member collisions with ancestors,
    // and exact-signature override validation.
    let mut class_parents: HashMap<&str, &str> = HashMap::new();
    check_class_parents(program, &types, &mut class_parents, &mut diagnostics);
    check_class_cycles(program, &class_parents, &mut diagnostics);
    check_class_overrides(program, &types, &class_parents, &mut diagnostics);

    let call_graph = program
        .functions
        .iter()
        .map(|function| {
            let mut callees = Vec::new();
            for contract in &function.requires {
                contract.visit_calls(&mut |callee, _| callees.push(callee.to_owned()));
            }
            function
                .body
                .visit_calls(&mut |callee, _| callees.push(callee.to_owned()));
            for contract in &function.ensures {
                contract.visit_calls(&mut |callee, _| callees.push(callee.to_owned()));
            }
            (function.name.clone(), callees)
        })
        .collect::<HashMap<_, _>>();
    let generic_functions = program
        .functions
        .iter()
        .filter(|function| !function.type_parameters.is_empty())
        .map(|function| function.name.as_str())
        .collect::<HashSet<_>>();
    check_generic_function_cycles(program, &call_graph, &generic_functions, &mut diagnostics);

    check_function_bodies(program, &functions, &import_keys, &types, &mut diagnostics);

    if let Some(main) = functions.get("main") {
        if !main.type_parameters.is_empty()
            || !main.params.is_empty()
            || main.return_type != Type::I64
        {
            diagnostics.push(error(
                program,
                "SPX-T104",
                "entry function must be monomorphic with signature `fn main() -> i64`",
                main.span,
            ));
        }
    } else {
        diagnostics.push(
            Diagnostic::error(
                "SPX-T105",
                "executable module must define `fn main() -> i64`",
                program
                    .functions
                    .first()
                    .map_or(Span::default(), |function| function.span),
            )
            .at_path(&program.path),
        );
    }
    if let Err(capacity) = verify_byte_data_capacity(program, &types) {
        if capacity.diagnostic != crate::byte_data_capacity::CapacityDiagnostic::Invariant
            || !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity.is_error())
        {
            let span = capacity
                .function
                .as_deref()
                .and_then(|identity| {
                    source_capacity_functions(program)
                        .into_iter()
                        .find(|(_, function)| function.stable_id == identity)
                })
                .map_or(Span::default(), |(_, function)| function.span);
            diagnostics.push(error(
                program,
                capacity.diagnostic.code(),
                capacity.detail,
                span,
            ));
        }
    }
    let mut native_interop_failures = HashSet::new();
    diagnostics.retain(|diagnostic| {
        diagnostic.code != "SPX-B107" || native_interop_failures.insert(diagnostic.message.clone())
    });
    if let Some(native_failure) = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-B107")
        .cloned()
    {
        return vec![native_failure];
    }
    diagnostics
}
