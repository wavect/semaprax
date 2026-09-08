//! Raw-AST fallback only: exact ordinary scalar-return calls have no nominal
//! result identity and no owned-result cleanup temporary. Expression/callee
//! identities and all argument expressions keep their original charges.
//! Primitive unshadowed parameter reads retain expression and binding ValueId
//! slots; only their absent nominal/owned-result allowance is discounted.

use super::super::AuthoredDeclaration;
use crate::ast::{
    Expr, ExprKind, Function, MatchPattern, ModuleUseKind, Program, RecordMatchFieldPattern,
    Statement, Type,
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_NAMES: usize = 4096;

pub(super) fn discount(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> usize {
    let mut declared = BTreeMap::<&str, Option<usize>>::new();
    let mut insert = |name, function: &Function| {
        let signature = (function.type_parameters.is_empty() && primitive(&function.return_type))
            .then_some(function.params.len());
        if declared.contains_key(name) {
            declared.insert(name, None);
        } else {
            declared.insert(name, signature);
        }
    };
    for function in &program.functions {
        insert(function.name.as_str(), function);
    }
    for item in &program.module_uses {
        if item.kind != ModuleUseKind::Function {
            continue;
        }
        if let Some(target) = authored.get(item.persistent_id.as_str()) {
            if target.module == item.target_module {
                if let Some(function) = target.function {
                    insert(item.alias.as_str(), function);
                }
            }
        }
    }
    program
        .functions
        .iter()
        .map(|function| {
            let mut shadowed = BTreeSet::<&str>::new();
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
                                    | Statement::ForOwn { item, .. } => Some(item),
                                    _ => None,
                                };
                                if let Some(name) = name {
                                    shadowed.insert(name);
                                }
                            }
                        }
                        ExprKind::Closure { params, .. } => {
                            for param in params {
                                shadowed.insert(&param.name);
                            }
                        }
                        ExprKind::Match { arms, .. } => {
                            for arm in arms {
                                if !complete_pattern(&arm.pattern, &mut shadowed, 0) {
                                    return false;
                                }
                            }
                        }
                        _ => {}
                    }
                    shadowed.len() <= MAX_NAMES
                });
            }
            if !complete || shadowed.len() > MAX_NAMES {
                return 0;
            }
            // Parameters are in scope in contracts and the body. No declaration
            // anywhere may reuse a candidate name (including closure/pattern
            // scopes), so binding-first resolution is sufficient: no inference.
            let scalar_parameters = function
                .params
                .iter()
                .filter(|parameter| {
                    parameter.mode == crate::ast::ParamMode::Value
                        && primitive(&parameter.ty)
                        && !shadowed.contains(parameter.name.as_str())
                        && function
                            .params
                            .iter()
                            .filter(|other| other.name == parameter.name)
                            .count()
                            == 1
                })
                .map(|parameter| parameter.name.as_str())
                .collect::<BTreeSet<_>>();
            shadowed.extend(
                function
                    .params
                    .iter()
                    .map(|parameter| parameter.name.as_str()),
            );
            let mut count = super::local_identity::discount(
                function,
                program,
                &scalar_parameters,
                &declared,
                &shadowed,
            );
            for root in roots(function) {
                if !walk(root, &mut |expression| {
                    if let ExprKind::Var(name) = &expression.kind {
                        if scalar_parameters.contains(name.as_str()) {
                            count += 1;
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
                            && scalar_callee(name, args.len(), &declared)
                        {
                            count += 2;
                        }
                    }
                    true
                }) {
                    return 0;
                }
            }
            count
        })
        .sum()
}

pub(super) fn primitive(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}

pub(super) fn scalar_callee(
    name: &str,
    arity: usize,
    declared: &BTreeMap<&str, Option<usize>>,
) -> bool {
    // Resolver priority is binding, native import, frozen compiler operations,
    // then ordinary declaration. Keep every other compiler operation closed.
    if let Some(op) = crate::byte_ops::by_name(name) {
        return op.arity() == arity && primitive(&op.ast_return_type());
    }
    if crate::string_ops::by_name(name).is_some()
        || crate::str_ops::by_name(name).is_some()
        || crate::vec_ops::by_name(name).is_some()
        || crate::box_ops::by_name(name).is_some()
        || crate::iterator_ops::by_name(name).is_some()
        || crate::host_io_ops::by_name(name).is_some()
        || crate::command_io_ops::by_name(name).is_some()
    {
        return false;
    }
    declared
        .get(name)
        .is_some_and(|signature| *signature == Some(arity))
}

pub(super) fn roots(function: &Function) -> impl Iterator<Item = &Expr> {
    function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
}

/// Fixed scratch and conservative bailout: no new AST admission or unbounded
/// lookup inventory. Closure and pattern bindings shadow candidates throughout
/// the enclosing function, deliberately stronger than lexical visibility.
pub(super) fn walk<'a>(root: &'a Expr, visit: &mut impl FnMut(&'a Expr) -> bool) -> bool {
    let mut stack = [None; 129];
    stack[0] = Some((root, 0));
    let mut length = 1;
    while length != 0 {
        let (expression, child_index) = stack[length - 1].unwrap();
        if child_index == 0 && !visit(expression) {
            return false;
        }
        if let Some(child) = expression.child(child_index) {
            stack[length - 1] = Some((expression, child_index + 1));
            if length == stack.len() {
                return false;
            }
            stack[length] = Some((child, 0));
            length += 1;
        } else {
            length -= 1;
        }
    }
    true
}

pub(super) fn complete_pattern<'a>(
    pattern: &'a MatchPattern,
    names: &mut BTreeSet<&'a str>,
    depth: usize,
) -> bool {
    if depth >= 128 {
        return false;
    }
    match pattern {
        MatchPattern::Binding { name, .. } => {
            names.insert(name);
        }
        MatchPattern::Variant { fields, .. } => {
            for field in fields {
                names.insert(&field.binding);
                if names.len() > MAX_NAMES {
                    return false;
                }
            }
        }
        MatchPattern::Record { fields, .. } => {
            for field in fields {
                if !record_pattern(&field.pattern, names, depth + 1) {
                    return false;
                }
            }
        }
        MatchPattern::Or { alternatives, .. } => {
            for pattern in alternatives {
                if !complete_pattern(pattern, names, depth + 1) {
                    return false;
                }
            }
        }
        _ => {}
    }
    names.len() <= MAX_NAMES
}

fn record_pattern<'a>(
    pattern: &'a RecordMatchFieldPattern,
    names: &mut BTreeSet<&'a str>,
    depth: usize,
) -> bool {
    if depth >= 128 {
        return false;
    }
    match pattern {
        RecordMatchFieldPattern::Binding { name, .. } => {
            names.insert(name);
        }
        RecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                if !record_pattern(&field.pattern, names, depth + 1) {
                    return false;
                }
            }
        }
        RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    names.len() <= MAX_NAMES
}

#[cfg(test)]
mod tests {
    use super::discount;
    fn count(source: &str) -> usize {
        let program = crate::parse(source, std::path::Path::new("calls.spx")).unwrap();
        let programs = vec![program];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        discount(&programs[0], &authored)
    }
    #[test]
    fn identity_prebound_scalar_calls_keep_callee_and_opaque_results() {
        assert_eq!(count("module calls; @id(\"calls.scalar\") fn scalar()->i64{1} @id(\"calls.main\") fn main()->i64{scalar()}"), 2);
        assert_eq!(count("module calls; @id(\"calls.scalar\") fn scalar<T>(x:T)->i64{1} @id(\"calls.main\") fn main()->i64{scalar<i64>(0)}"), 0);
        assert_eq!(
            count("module calls; @id(\"calls.main\") fn main()->usize{byte_len(input)}"),
            2
        );
        assert_eq!(
            count("module calls; @id(\"calls.main\") fn main()->usize{byte_get(input,0usize)}"),
            0
        );
    }
    #[test]
    fn identity_prebound_scalar_call_binding_shadowing_is_conservative() {
        let header = "module calls; @id(\"calls.scalar\") fn scalar()->i64{1} ";
        for function in [
            "@id(\"calls.main\") fn main(scalar:fn()->i64)->i64{scalar()}",
            "@id(\"calls.main\") fn main()->i64{let scalar=fn()->i64{2};scalar()}",
            "@id(\"calls.main\") fn main()->i64{match true { scalar => scalar(), }}",
        ] {
            assert_eq!(count(&format!("{header}{function}")), 0);
        }
    }
    #[test]
    fn identity_prebound_scalar_call_import_requires_exact_provider_header() {
        let provider = crate::parse(
            "module provider; @id(\"provider.scalar\") fn scalar()->i64{1}",
            std::path::Path::new("provider.spx"),
        )
        .unwrap();
        let consumer = crate::parse("module consumer; use function @id(\"provider.scalar\") from provider as call; @id(\"consumer.main\") fn main()->i64{call()}", std::path::Path::new("consumer.spx")).unwrap();
        let programs = vec![provider, consumer];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        assert_eq!(discount(&programs[1], &authored), 2);
        let mut changed = programs[1].clone();
        changed.module_uses[0].persistent_id = "provider.missing".to_owned();
        assert_eq!(discount(&changed, &authored), 0);
    }
    #[test]
    fn identity_prebound_scalar_parameter_reads_keep_both_place_identities() {
        for ty in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
            let source =
                format!("module calls; @id(\"calls.read\") fn read(value:{ty})->{ty} {{value}}");
            assert_eq!(
                count(&source),
                1,
                "expression and binding ValueId remain charged for {ty}"
            );
        }
        assert_eq!(count("module calls; @id(\"calls.read\") fn read(value:i64)->i64 requires value>=0 ensures result>=value {value}"), 3);
        for signature in [
            "value:own Bytes",
            "value:borrow Slice<u8>",
            "value:fn()->i64",
            "value:T",
        ] {
            let source =
                format!("module calls; @id(\"calls.read\") fn read({signature})->i64 {{value}}");
            assert_eq!(count(&source), 0);
        }
    }

    #[test]
    fn identity_prebound_scalar_parameter_redeclarations_disable_discount() {
        for body in [
            "{let value=0; value}",
            "{match true {value=>value,}}",
            "{let callback=fn(value:i64)->i64{value}; value}",
            "{for value in items {0} value}",
        ] {
            let source =
                format!("module calls; @id(\"calls.read\") fn read(value:i64)->i64 {body}");
            assert_eq!(count(&source), 0, "shadowing remains conservative: {body}");
        }
    }
    #[test]
    fn identity_prebound_unique_scalar_locals_keep_place_identities() {
        for body in [
            "{let value=0; value}",
            "{let mut value=0; value=1; value}",
            "{let value:i64=opaque(); value}",
            "{let value=if true {1} else {2}; value}",
        ] {
            assert_eq!(
                count(&format!(
                    "module calls; @id(\"calls.main\") fn main()->i64 {body}"
                )),
                1,
                "{body}"
            );
        }
        assert_eq!(count("module calls; @id(\"calls.main\") fn main()->i64 {let first=1; let second=first; second+first}"), 3);
        assert_eq!(count("module calls; @id(\"calls.scalar\") fn scalar()->i64{1} @id(\"calls.main\") fn main()->i64 {let value=scalar();value}"), 3);
    }

    #[test]
    fn identity_prebound_local_scope_and_opaque_values_remain_charged() {
        for body in [
            "{let prior=value; let value=1; prior}",
            "{let block={let value=1; 0}; value}",
            "{let value=1; let block={let value=2; value}; value}",
            "{let value=opaque(); value}",
            "{let value:Bytes=opaque(); value}",
            "{let value:fn()->i64=opaque(); value}",
            "{let value=1; let callback=fn(value:i64)->i64{value}; value}",
            "{let value=1; match true {value=>value,}}",
        ] {
            assert_eq!(
                count(&format!(
                    "module calls; @id(\"calls.main\") fn main()->i64 {body}"
                )),
                0,
                "{body}"
            );
        }
    }
}
