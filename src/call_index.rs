use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ExpressionId, FunctionInstanceId, IdentityOrigin, ResolvedExpr,
    ResolvedExprKind, ResolvedProgram, ResolvedType,
};

#[cfg(test)]
thread_local! {
    static CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn reset_capacity_high_water() {
    CAPACITY_HIGH_WATER.with(|water| water.set(0));
}
#[cfg(test)]
fn capacity_high_water() -> usize {
    CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}
#[cfg(test)]
fn note_capacity_high_water(bytes: usize) {
    CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum PersistentCallableKind {
    Function,
    FunctionTemplate,
}

impl PersistentCallableKind {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::FunctionTemplate => "function_template",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallRegion {
    Requires,
    Body,
    Ensures,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PersistentCallSite {
    pub(crate) expression: ExpressionId,
    pub(crate) owner: DeclarationId,
    pub(crate) owner_kind: PersistentCallableKind,
    pub(crate) owner_origin: IdentityOrigin,
    pub(crate) region: CallRegion,
    pub(crate) callee: DeclarationId,
    pub(crate) type_arguments: Vec<ResolvedType>,
    pub(crate) instance: Option<FunctionInstanceId>,
}

pub(crate) struct PersistentCallIndex {
    sites_by_expression: BTreeMap<String, PersistentCallSite>,
    calls_by_owner: BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    callers_by_callee: BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    kinds_by_owner: BTreeMap<DeclarationId, PersistentCallableKind>,
    origins_by_owner: BTreeMap<DeclarationId, IdentityOrigin>,
}

impl PersistentCallIndex {
    pub(crate) fn build(program: &ResolvedProgram) -> Result<Self, Diagnostic> {
        hir::validate(program)?;
        let mut index = Self {
            sites_by_expression: BTreeMap::new(),
            calls_by_owner: BTreeMap::new(),
            callers_by_callee: BTreeMap::new(),
            kinds_by_owner: BTreeMap::new(),
            origins_by_owner: BTreeMap::new(),
        };
        for function in &program.functions {
            index.add_owner(
                program,
                &function.id,
                PersistentCallableKind::Function,
                &function.requires,
                &function.body,
                &function.ensures,
            )?;
        }
        for template in &program.function_templates {
            index.add_owner(
                program,
                &template.id,
                PersistentCallableKind::FunctionTemplate,
                &template.requires,
                &template.body,
                &template.ensures,
            )?;
        }

        let owners = index
            .kinds_by_owner
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        for callees in index.calls_by_owner.values_mut() {
            callees.retain(|callee| owners.contains(callee));
        }
        index.callers_by_callee = owners
            .iter()
            .cloned()
            .map(|owner| (owner, BTreeSet::new()))
            .collect();
        for (caller, callees) in &index.calls_by_owner {
            for callee in callees {
                if owners.contains(callee) {
                    index
                        .callers_by_callee
                        .get_mut(callee)
                        .expect("known callable has a reverse-index entry")
                        .insert(caller.clone());
                }
            }
        }
        Ok(index)
    }

    pub(crate) fn site(&self, expression: &str) -> Option<&PersistentCallSite> {
        self.sites_by_expression.get(expression)
    }

    pub(crate) fn calls_by_owner(&self) -> &BTreeMap<DeclarationId, BTreeSet<DeclarationId>> {
        &self.calls_by_owner
    }

    pub(crate) fn callers_by_callee(&self) -> &BTreeMap<DeclarationId, BTreeSet<DeclarationId>> {
        &self.callers_by_callee
    }

    pub(crate) fn kind(&self, owner: &DeclarationId) -> Option<PersistentCallableKind> {
        self.kinds_by_owner.get(owner).copied()
    }

    pub(crate) fn origin(&self, owner: &DeclarationId) -> Option<IdentityOrigin> {
        self.origins_by_owner.get(owner).copied()
    }

    fn add_owner(
        &mut self,
        program: &ResolvedProgram,
        owner: &DeclarationId,
        kind: PersistentCallableKind,
        requires: &[ResolvedExpr],
        body: &ResolvedExpr,
        ensures: &[ResolvedExpr],
    ) -> Result<(), Diagnostic> {
        let declaration = program.declarations.declaration(owner).ok_or_else(|| {
            call_index_error(format!(
                "callable `{owner}` is absent from declaration metadata"
            ))
        })?;
        if self.kinds_by_owner.insert(owner.clone(), kind).is_some()
            || self
                .origins_by_owner
                .insert(owner.clone(), declaration.identity_origin)
                .is_some()
            || self
                .calls_by_owner
                .insert(owner.clone(), BTreeSet::new())
                .is_some()
        {
            return Err(call_index_error(format!(
                "duplicate callable owner `{owner}`"
            )));
        }
        for expression in requires {
            self.visit_expr(
                owner,
                kind,
                declaration.identity_origin,
                CallRegion::Requires,
                expression,
            )?;
        }
        self.visit_expr(
            owner,
            kind,
            declaration.identity_origin,
            CallRegion::Body,
            body,
        )?;
        for expression in ensures {
            self.visit_expr(
                owner,
                kind,
                declaration.identity_origin,
                CallRegion::Ensures,
                expression,
            )?;
        }
        Ok(())
    }

    fn visit_expr(
        &mut self,
        owner: &DeclarationId,
        owner_kind: PersistentCallableKind,
        owner_origin: IdentityOrigin,
        region: CallRegion,
        expression: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedExpr),
            Children(&'a ResolvedExpr, usize),
        }
        const { assert!(std::mem::size_of::<Frame<'static>>() == 16) };
        fn child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
            match &expression.kind {
                ResolvedExprKind::Call { args, .. } => args.get(index),
                ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Project { base: value, .. }
                | ResolvedExprKind::Try { operand: value, .. }
                | ResolvedExprKind::TryOption { operand: value, .. } => {
                    (index == 0).then_some(value)
                }
                ResolvedExprKind::Binary { left, right, .. } => {
                    [left.as_ref(), right.as_ref()].get(index).copied()
                }
                ResolvedExprKind::Block { statements, tail } => statements
                    .get(index)
                    .map(|statement| statement.value())
                    .or_else(|| (index == statements.len()).then_some(tail)),
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => [
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref(),
                ]
                .get(index)
                .copied(),
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    fields.get(index).map(|field| &field.value)
                }
                ResolvedExprKind::Match { scrutinee, arms } => {
                    if index == 0 {
                        Some(scrutinee)
                    } else {
                        arms.get(index - 1).map(|arm| &arm.value)
                    }
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    if index == 0 {
                        Some(base)
                    } else {
                        fields.get(index - 1).map(|field| &field.value)
                    }
                }
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::Place(_) => None,
            }
        }

        let mut frames = vec![Frame::Enter(expression)];
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_capacity_high_water(
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + self
                        .sites_by_expression
                        .iter()
                        .map(|(key, site)| {
                            std::mem::size_of::<(String, PersistentCallSite)>()
                                + key.capacity()
                                + site.type_arguments.capacity()
                                    * std::mem::size_of::<ResolvedType>()
                        })
                        .sum::<usize>()
                    + self
                        .calls_by_owner
                        .values()
                        .map(|values| values.len() * std::mem::size_of::<DeclarationId>())
                        .sum::<usize>(),
            );
            match frame {
                Frame::Enter(expression) => {
                    if let ResolvedExprKind::Call {
                        callee,
                        type_arguments,
                        instance,
                        ..
                    } = &expression.kind
                    {
                        let site = PersistentCallSite {
                            expression: expression.id.clone(),
                            owner: owner.clone(),
                            owner_kind,
                            owner_origin,
                            region,
                            callee: callee.clone(),
                            type_arguments: type_arguments.clone(),
                            instance: instance.clone(),
                        };
                        if self
                            .sites_by_expression
                            .insert(expression.id.as_str().to_owned(), site)
                            .is_some()
                        {
                            return Err(call_index_error(format!(
                                "call expression `{}` has multiple source owners",
                                expression.id
                            )));
                        }
                        self.calls_by_owner
                            .get_mut(owner)
                            .expect("registered owner remains indexed")
                            .insert(callee.clone());
                    }
                    frames.push(Frame::Children(expression, 0));
                }
                Frame::Children(expression, index) => {
                    if let Some(next) = child(expression, index) {
                        frames.push(Frame::Children(expression, index + 1));
                        frames.push(Frame::Enter(next));
                    }
                }
            }
        }
        Ok(())
    }
}

fn call_index_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-G003", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_lookup_requires_the_exact_indexed_id() {
        let source = r#"module call.index;
@id("helper.answer") fn answer()->i64{42}
@id("app.main") fn main()->i64{answer()}
"#;
        let program = crate::parse(source, std::path::Path::new("call-index.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        reset_capacity_high_water();
        let index = PersistentCallIndex::build(&resolved).unwrap();
        assert!(capacity_high_water() > 0);
        let expression = index.sites_by_expression.keys().next().unwrap().clone();

        assert_eq!(
            index.site(&expression).unwrap().expression.as_str(),
            expression
        );
        assert!(index.site(&format!("{expression}.suffix")).is_none());
    }
}
