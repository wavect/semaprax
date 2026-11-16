//! Bounded raw identity discount for the mode-six workspace estimates.
//! The unchanged raw-identity producer remains the reference for other modes.

use super::call_identity::{primitive, roots, walk};
use super::{active_builder_limit, limit_error, AuthoredDeclaration};
use crate::ast::{
    ExprKind, MatchPattern, ModuleUseKind, Program, RecordMatchFieldPattern, Statement,
};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;

mod locals;
mod names;
use names::{reserve_exact, Names, Scratch};

const MAX_NAMES: usize = 4096;

pub(super) fn discount(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    include_imports: bool,
) -> Result<usize, Vec<Diagnostic>> {
    if crate::bounded_output::active_refused() {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    let scratch = Scratch::new(active_builder_limit());
    let declared = Declared {
        program,
        authored,
        include_imports,
        scratch: &scratch,
        backing: std::cell::OnceCell::new(),
    };
    discount_functions(program, &declared)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

type Declaration<'a> = (&'a str, Option<usize>);

struct Declared<'a, 'b, 's> {
    program: &'a Program,
    authored: &'b BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    include_imports: bool,
    scratch: &'s Scratch,
    backing: std::cell::OnceCell<Option<Vec<Declaration<'a>>>>,
}

impl<'a> Declared<'a, '_, '_> {
    fn directory(&self) -> Option<&[Declaration<'a>]> {
        self.backing
            .get_or_init(|| {
                declaration_directory(
                    self.program,
                    self.authored,
                    self.include_imports,
                    self.scratch,
                )
            })
            .as_deref()
    }
}

fn declaration_directory<'a>(
    program: &'a Program,
    authored: &BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    include_imports: bool,
    scratch: &Scratch,
) -> Option<Vec<Declaration<'a>>> {
    let imports = program.module_uses.iter().filter_map(|item| {
        if !include_imports || item.kind != ModuleUseKind::Function {
            return None;
        }
        let target = authored.get(item.persistent_id.as_str())?;
        (target.module == item.target_module)
            .then_some(target.function)
            .flatten()
            .map(|function| (item.alias.as_str(), function))
    });
    let rows = program
        .functions
        .len()
        .checked_add(imports.clone().count())?;
    let mut declared = Vec::new();
    reserve_exact(&mut declared, rows, scratch)?;
    declared.extend(
        program
            .functions
            .iter()
            .map(|f| (f.name.as_str(), f))
            .chain(imports)
            .map(|(name, function)| {
                let signature = (function.type_parameters.is_empty()
                    && primitive(&function.return_type))
                .then_some(function.params.len());
                (name, signature)
            }),
    );
    declared.sort_unstable_by(|left, right| left.0.cmp(right.0));
    Some(declared)
}

fn discount_functions(program: &Program, declared: &Declared<'_, '_, '_>) -> Option<usize> {
    program
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            let scratch = declared.scratch;
            let mut shadowed = Names::<()>::default();
            let mut refused = false;
            let mut complete = true;
            for root in roots(function) {
                complete &= walk(root, &mut |expression| {
                    match &expression.kind {
                        ExprKind::Block { statements, .. } => {
                            for statement in statements {
                                let name = match statement {
                                    Statement::Let { name, .. }
                                    | Statement::Assign { name, .. } => Some(name),
                                    Statement::For { item, .. }
                                    | Statement::ForBorrow { item, .. }
                                    | Statement::ForOwn { item, .. } => Some(item),
                                    _ => None,
                                };
                                if let Statement::ForBorrow {
                                    index: Some(index), ..
                                } = statement
                                {
                                    if shadowed.entry(&index.name, scratch).is_none() {
                                        refused = true;
                                        return false;
                                    }
                                }
                                if let Some(name) = name {
                                    if shadowed.entry(name, scratch).is_none() {
                                        refused = true;
                                        return false;
                                    }
                                }
                            }
                        }
                        ExprKind::Closure { params, .. } => {
                            for param in params {
                                if shadowed.entry(&param.name, scratch).is_none() {
                                    refused = true;
                                    return false;
                                }
                            }
                        }
                        ExprKind::Match { arms, .. } => {
                            for arm in arms {
                                match complete_pattern(&arm.pattern, &mut shadowed, 0, scratch) {
                                    Some(true) => {}
                                    Some(false) => return false,
                                    None => {
                                        refused = true;
                                        return false;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    shadowed.len() <= MAX_NAMES
                });
                if refused {
                    return None;
                }
            }
            if !complete || shadowed.len() > MAX_NAMES {
                return Some(total);
            }
            // Parameters are in scope in contracts and the body. No declaration
            // anywhere may reuse a candidate name (including closure/pattern
            // scopes), so binding-first resolution is sufficient: no inference.
            let mut scalar_parameters = Names::<()>::default();
            for parameter in function.params.iter().filter(|parameter| {
                parameter.mode == crate::ast::ParamMode::Value
                    && primitive(&parameter.ty)
                    && !shadowed.contains(parameter.name.as_str())
                    && function
                        .params
                        .iter()
                        .filter(|other| other.name == parameter.name)
                        .count()
                        == 1
            }) {
                scalar_parameters.entry(&parameter.name, scratch)?;
            }
            for parameter in &function.params {
                shadowed.entry(&parameter.name, scratch)?;
            }
            let mut count =
                locals::discount(function, program, &scalar_parameters, declared, &shadowed)?;
            for root in roots(function) {
                if !walk(root, &mut |expression| {
                    if let ExprKind::Var(name) = &expression.kind {
                        if scalar_parameters.contains(name.as_str()) {
                            let Some(next) = count.checked_add(1) else {
                                refused = true;
                                return false;
                            };
                            count = next;
                        }
                    }
                    if let ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } = &expression.kind
                    {
                        if type_arguments.is_empty()
                            && !shadowed.contains(name.as_str())
                            && !program
                                .interfaces
                                .iter()
                                .any(|i| i.imports.iter().any(|f| f.name == *name))
                        {
                            match scalar_callee(name, args.len(), declared) {
                                Some(true) => {
                                    let Some(next) = count.checked_add(2) else {
                                        refused = true;
                                        return false;
                                    };
                                    count = next;
                                }
                                Some(false) => {}
                                None => {
                                    refused = true;
                                    return false;
                                }
                            }
                        }
                    }
                    true
                }) {
                    return if refused { None } else { Some(total) };
                }
            }
            total.checked_add(count)
        })
}

fn scalar_callee(name: &str, arity: usize, declared: &Declared<'_, '_, '_>) -> Option<bool> {
    // Resolver priority is binding, native import, frozen compiler operations,
    // then ordinary declaration. Keep every other compiler operation closed.
    if let Some(op) = crate::byte_ops::by_name(name) {
        return Some(op.arity() == arity && primitive(&op.ast_return_type()));
    }
    if crate::string_ops::by_name(name).is_some()
        || crate::str_ops::by_name(name).is_some()
        || crate::vec_ops::by_name(name).is_some()
        || crate::box_ops::by_name(name).is_some()
        || crate::iterator_ops::by_name(name).is_some()
        || crate::host_io_ops::by_name(name).is_some()
        || crate::command_io_ops::by_name(name).is_some()
    {
        return Some(false);
    }
    let directory = declared.directory()?;
    let first = directory.partition_point(|row| row.0 < name);
    Some(
        directory
            .get(first)
            .is_some_and(|row| row.0 == name && row.1 == Some(arity))
            && directory.get(first + 1).is_none_or(|row| row.0 != name),
    )
}

fn complete_pattern<'a>(
    pattern: &'a MatchPattern,
    names: &mut Names<'a, ()>,
    depth: usize,
    scratch: &Scratch,
) -> Option<bool> {
    if depth >= 128 {
        return Some(false);
    }
    match pattern {
        MatchPattern::Binding { name, .. } => {
            names.entry(name, scratch)?;
        }
        MatchPattern::Variant { fields, .. } => {
            for field in fields {
                names.entry(&field.binding, scratch)?;
                if names.len() > MAX_NAMES {
                    return Some(false);
                }
            }
        }
        MatchPattern::Record { fields, .. } => {
            for field in fields {
                if !record_pattern(&field.pattern, names, depth + 1, scratch)? {
                    return Some(false);
                }
            }
        }
        MatchPattern::Or { alternatives, .. } => {
            for pattern in alternatives {
                if !complete_pattern(pattern, names, depth + 1, scratch)? {
                    return Some(false);
                }
            }
        }
        _ => {}
    }
    Some(names.len() <= MAX_NAMES)
}

fn record_pattern<'a>(
    pattern: &'a RecordMatchFieldPattern,
    names: &mut Names<'a, ()>,
    depth: usize,
    scratch: &Scratch,
) -> Option<bool> {
    if depth >= 128 {
        return Some(false);
    }
    match pattern {
        RecordMatchFieldPattern::Binding { name, .. } => {
            names.entry(name, scratch)?;
        }
        RecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                if !record_pattern(&field.pattern, names, depth + 1, scratch)? {
                    return Some(false);
                }
            }
        }
        RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    Some(names.len() <= MAX_NAMES)
}

#[cfg(test)]
mod tests;
