//! The same invariant transformations at their genuine borrowed-source
//! boundary. Complete ordinary owning producers remain independent oracles.
use super::{check_function, invariant_function, rewrite, Sites};
use crate::ast::program_source::ProgramSource;
use crate::ast::{Program, TypeDeclarationKind};
use crate::hir::nodes::{ResolvedProgram, ResolvedType};
use std::collections::BTreeMap;

// Transformation requires owned writable AST. Copy exactly the verified
// presentation once here; no owning synthesis is needed merely for lookup.
fn clone_source(program: &dyn ProgramSource) -> Program {
    Program {
        path: program.path().to_owned(),
        module: program.module().to_owned(),
        module_uses: program.module_uses().to_vec(),
        permits: program.permits().to_vec(),
        types: program.types().iter().cloned().collect(),
        interfaces: program.interfaces().to_vec(),
        protocols: program.protocols().to_vec(),
        implementations: program.implementations().to_vec(),
        session_protocols: program.session_protocols().to_vec(),
        agents: program.agents().to_vec(),
        functions: program.functions().iter().cloned().collect(),
    }
}

pub(in crate::hir) fn desugar(
    program: &dyn ProgramSource,
    resolved: &ResolvedProgram,
) -> Option<Program> {
    let declared = program
        .types()
        .iter()
        .filter(|declaration| !declaration.invariants().is_empty())
        .filter_map(|declaration| match &declaration.kind {
            TypeDeclarationKind::Record { fields } => Some((declaration, fields.as_slice())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if declared.is_empty() {
        return None;
    }
    let checked = declared
        .iter()
        .filter(|(declaration, _)| {
            let ty = ResolvedType::Nominal {
                declaration: crate::hir::DeclarationId::new(declaration.stable_id.clone()),
                arguments: Vec::new(),
            };
            resolved
                .declarations
                .type_facts(&ty)
                .is_some_and(|facts| facts.copy)
        })
        .map(|(declaration, fields)| {
            (
                declaration.stable_id.as_str(),
                (declaration.name.clone(), *fields),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sites = Sites::collect(resolved, &checked);
    let mut rewritten = clone_source(program);
    for (declaration, fields) in &declared {
        rewritten
            .functions
            .push(invariant_function(declaration, fields));
    }
    let mut roots = Vec::new();
    for function in &mut rewritten.functions {
        roots.push(&mut function.body);
        roots.extend(function.requires.iter_mut());
        roots.extend(function.ensures.iter_mut());
    }
    for declaration in &mut rewritten.types {
        if let TypeDeclarationKind::Class { methods, .. } = &mut declaration.kind {
            for method in methods {
                roots.push(&mut method.body);
                roots.extend(method.requires.iter_mut());
                roots.extend(method.ensures.iter_mut());
            }
        }
    }
    rewrite(roots, &sites);
    for (declaration, fields) in declared {
        if checked.contains_key(declaration.stable_id.as_str()) {
            rewritten
                .functions
                .push(check_function(declaration, fields));
        }
    }
    Some(rewritten)
}

/// Resolve authored clauses as ordinary synthesized preconditions so their
/// update sites have the same typed inventory as function contracts.
pub(in crate::hir) fn clause_program(program: &dyn ProgramSource) -> Option<Program> {
    if !program
        .types()
        .iter()
        .any(|declaration| !declaration.invariants().is_empty())
    {
        return None;
    }
    let mut augmented = clone_source(program);
    for declaration in program.types().iter() {
        if !declaration.invariants().is_empty() {
            if let TypeDeclarationKind::Record { fields } = &declaration.kind {
                augmented
                    .functions
                    .push(invariant_function(declaration, fields));
            }
        }
    }
    (augmented.functions.len() != program.functions().len()).then_some(augmented)
}
