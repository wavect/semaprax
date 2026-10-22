//! Authenticated authored ordinary calls. This proof owns no heap allocation.
//! Scratch exhaustion, incomplete traversal and ambiguous headers keep the old
//! charge. Synthetic/default/generic trees never enter this census.
use super::{AuthoredDeclaration, call_identity};
use crate::ast::{
    Expr, ExprKind, MatchPattern, ModuleUseKind, Program, RecordMatchFieldPattern, Statement,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Proof {
    pub(super) roots: usize,
    pub(super) bytes: usize,
}
impl Proof {
    fn charge<T>(&mut self, value: &T) {
        self.bytes = self
            .bytes
            .saturating_add(std::mem::size_of_val(value))
            .saturating_add(std::mem::size_of::<&mut Self>())
            .saturating_add(std::mem::size_of::<&T>());
    }
}

/// The names are deliberately function-wide: even a binding in a disjoint arm
/// excludes its spelling. Bounded scratch avoids an unpriced tree/index.
struct Names<'a> {
    entries: [Option<&'a str>; 128],
    len: usize,
}
impl<'a> Names<'a> {
    fn insert(&mut self, name: &'a str, proof: &mut Proof) -> bool {
        proof.charge(&(&*self, name));
        if self.len == self.entries.len() {
            return false;
        }
        self.entries[self.len] = Some(name);
        self.len += 1;
        true
    }
    fn contains(&self, name: &str, proof: &mut Proof) -> bool {
        proof.charge(&(self, name));
        // One scalar cursor. No iterator, closure environment, Some(name)
        // construction or Option equality pipeline is formed by this search.
        proof.bytes = proof.bytes.saturating_add(std::mem::size_of::<usize>());
        let mut index = 0;
        loop {
            // Actual cursor/length comparison values and its boolean result,
            // including the terminal probe on an absent spelling.
            proof.bytes = proof
                .bytes
                .saturating_add(2 * std::mem::size_of::<usize>() + std::mem::size_of::<bool>());
            if index == self.len {
                proof.charge(&false);
                return false;
            }
            // Array Index actual/formal receivers and indices, returned option
            // loan. Matching a borrowed place forms no Option<&str> value.
            proof.bytes = proof.bytes.saturating_add(
                2 * std::mem::size_of::<&[Option<&str>; 128]>()
                    + 2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<&Option<&str>>(),
            );
            if let Some(item) = &self.entries[index] {
                // The &&str pattern binding, str::eq's actual/formal fat loans,
                // and its returned boolean all belong to this reached probe.
                proof.bytes = proof.bytes.saturating_add(
                    std::mem::size_of::<&&str>()
                        + 4 * std::mem::size_of::<&str>()
                        + std::mem::size_of::<bool>(),
                );
                if str::eq(*item, name) {
                    proof.charge(&true);
                    return true;
                }
            }
            proof.bytes = proof.bytes.saturating_add(std::mem::size_of::<usize>());
            index += 1;
        }
    }

    fn pattern(&mut self, pattern: &'a MatchPattern, depth: usize, proof: &mut Proof) -> bool {
        proof.charge(&(&*self, pattern, depth));
        if depth == 128 {
            return false;
        }
        match pattern {
            MatchPattern::Binding { name, .. } => self.insert(name, proof),
            MatchPattern::Variant { fields, .. } => {
                fields.iter().all(|f| self.insert(&f.binding, proof))
            }
            MatchPattern::Record { fields, .. } => fields
                .iter()
                .all(|f| self.record(&f.pattern, depth + 1, proof)),
            MatchPattern::Or { alternatives, .. } => alternatives
                .iter()
                .all(|p| self.pattern(p, depth + 1, proof)),
            _ => true,
        }
    }
    fn record(
        &mut self,
        pattern: &'a RecordMatchFieldPattern,
        depth: usize,
        proof: &mut Proof,
    ) -> bool {
        proof.charge(&(&*self, pattern, depth));
        if depth == 128 {
            return false;
        }
        match pattern {
            RecordMatchFieldPattern::Binding { name, .. } => self.insert(name, proof),
            RecordMatchFieldPattern::Record { fields, .. } => fields
                .iter()
                .all(|f| self.record(&f.pattern, depth + 1, proof)),
            RecordMatchFieldPattern::Wildcard { .. } => true,
        }
    }
}

fn compiler_dispatch(name: &str, proof: &mut Proof) -> bool {
    let result = crate::vec_ops::field_unique::literal_call(name)
        || name == crate::vec_ops::field_update::NAME
        || name == crate::vec_field::NAME
        || name == crate::literal_format::NAME
        || crate::string_ops::by_name(name).is_some()
        || crate::str_ops::by_name(name).is_some()
        || crate::vec_ops::by_name(name).is_some()
        || crate::box_ops::by_name(name).is_some()
        || crate::iterator_ops::by_name(name).is_some()
        || crate::list_ops::by_name(name).is_some()
        || crate::map_ops::by_name(name).is_some()
        || crate::byte_ops::by_name(name).is_some()
        || crate::host_io_ops::by_name(name).is_some()
        || crate::command_io_ops::by_name(name).is_some();
    proof.charge(&(name, result));
    result
}

fn target(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    imports: bool,
    name: &str,
    arity: usize,
    proof: &mut Proof,
) -> bool {
    proof.charge(&(program, authored, imports, name, arity));
    if compiler_dispatch(name, proof) {
        return false;
    }
    for interface in &program.interfaces {
        proof.charge(&interface);
        for import in &interface.imports {
            proof.charge(&import);
            if import.name == name {
                return false;
            }
        }
    }
    let mut found = None;
    let mut ambiguous = false;
    for function in &program.functions {
        proof.charge(&function);
        if function.name == name {
            ambiguous |= found.is_some();
            found = Some(function);
        }
    }
    for item in program.module_uses.iter().filter(|_| imports) {
        proof.charge(&item);
        if item.alias != name {
            continue;
        }
        // Any competing nonfunction alias also closes the proof.
        if item.kind != ModuleUseKind::Function {
            return false;
        }
        let Some(header) = authored.get(item.persistent_id.as_str()) else {
            return false;
        };
        proof.charge(&header);
        if header.module != item.target_module {
            return false;
        }
        let Some(function) = header.function else {
            return false;
        };
        proof.charge(&function);
        if function.stable_id != item.persistent_id {
            return false;
        }
        ambiguous |= found.is_some();
        found = Some(function);
    }
    proof.charge(&(found, ambiguous));
    let admitted = !ambiguous
        && found.is_some_and(|f| f.type_parameters.is_empty() && f.params.len() == arity);
    proof.charge(&admitted);
    admitted
}

pub(super) fn count(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    imports: bool,
) -> Proof {
    let mut proof = Proof::default();
    proof.charge(&(program, authored, imports));
    proof.bytes = proof.bytes.saturating_add(std::mem::size_of::<Proof>());
    for function in &program.functions {
        proof.charge(&function);
        // Generic owner bodies can be materialized repeatedly; no discount.
        if !function.type_parameters.is_empty() {
            continue;
        }
        let mut names = Names {
            entries: [None; 128],
            len: 0,
        };
        proof.charge(&names);
        let mut complete = function.params.iter().all(|p| {
            proof.charge(&p);
            names.insert(&p.name, &mut proof)
        });
        for root in call_identity::roots(function) {
            // Fixed walker scratch is retained for this complete pass, including
            // failed proofs. Each visited formal/reference is charged too.
            proof.bytes = proof
                .bytes
                .saturating_add(std::mem::size_of::<[Option<(&Expr, usize)>; 129]>());
            complete &= call_identity::walk(root, &mut |expression| {
                proof.charge(&expression);
                match &expression.kind {
                    ExprKind::Block { statements, .. } => statements.iter().all(|s| match s {
                        Statement::Let { name, .. } | Statement::Assign { name, .. } => {
                            names.insert(name, &mut proof)
                        }
                        Statement::For { item, .. } | Statement::ForOwn { item, .. } => {
                            names.insert(item, &mut proof)
                        }
                        Statement::ForBorrow { item, index, .. } => {
                            names.insert(item, &mut proof)
                                && index
                                    .as_ref()
                                    .is_none_or(|i| names.insert(&i.name, &mut proof))
                        }
                        _ => true,
                    }),
                    ExprKind::Closure { params, .. } => params.iter().all(|p| {
                        proof.charge(&p);
                        names.insert(&p.name, &mut proof)
                    }),
                    ExprKind::Match { arms, .. } => arms
                        .iter()
                        .all(|a| names.pattern(&a.pattern, 0, &mut proof)),
                    _ => true,
                }
            });
        }
        proof.charge(&complete);
        if !complete {
            continue;
        }
        let before = proof.roots;
        proof.charge(&before);
        for root in call_identity::roots(function) {
            if !visit(
                root, false, true, 0, &names, program, authored, imports, &mut proof,
            ) {
                proof.roots = before;
                break;
            }
        }
    }
    proof
}

#[allow(clippy::too_many_arguments)]
fn visit(
    expression: &Expr,
    free: bool,
    outside: bool,
    depth: usize,
    names: &Names<'_>,
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    imports: bool,
    proof: &mut Proof,
) -> bool {
    proof.charge(&(
        expression, free, outside, depth, names, program, authored, imports,
    ));
    if depth == 128 {
        return false;
    }
    if let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    {
        if free
            && outside
            && type_arguments.is_empty()
            && !names.contains(name, proof)
            && target(program, authored, imports, name, args.len(), proof)
        {
            proof.roots = proof.roots.saturating_add(1);
        }
    }
    let outside = outside && !matches!(expression.kind, ExprKind::Closure { .. });
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        let free = match &expression.kind {
            ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::Call { .. } => true,
            ExprKind::Block { tail, .. } => std::ptr::eq(child, tail.as_ref()),
            _ => false,
        };
        proof.charge(&(index, child, free, outside));
        if !visit(
            child,
            free,
            outside,
            depth + 1,
            names,
            program,
            authored,
            imports,
            proof,
        ) {
            return false;
        }
        index += 1;
    }
    let complete = true;
    proof.charge(&complete);
    complete
}
