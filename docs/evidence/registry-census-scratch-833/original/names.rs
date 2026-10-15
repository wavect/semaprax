//! Complete borrowed identifier census for the plain-module private-layout MVP.
//! Unsupported declaration surfaces refuse inference; they never receive a partial census.
use super::super::super::{graph_error, reserve_builder_structure};
use crate::ast::*;
use crate::diagnostic::Diagnostic;

const PREFIX: &str = "_spx_layout_";
const DEPTH: usize = 513;
const WORK: usize = 1_048_576;

pub(super) fn first(caller: &Program) -> Result<u64, Vec<Diagnostic>> {
    if !caller.interfaces.is_empty()
        || !caller.protocols.is_empty()
        || !caller.implementations.is_empty()
        || !caller.session_protocols.is_empty()
        || !caller.agents.is_empty()
        || caller.types.iter().any(|t| {
            matches!(
                t.kind,
                TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. }
            )
        })
        || caller.functions.iter().any(|f| f.follows.is_some())
    {
        return Err(refusal());
    }
    let mut census = Census::new()?;
    census.name(&caller.module)?;
    for item in &caller.module_uses {
        census.name(&item.alias)?;
        census.name(&item.persistent_id)?;
        census.name(&item.target_module)?;
    }
    for name in &caller.permits {
        census.name(name)?;
    }
    for declaration in &caller.types {
        census.name(&declaration.name)?;
        census.name(&declaration.stable_id)?;
        for p in &declaration.type_parameters {
            census.name(&p.name)?;
        }
        if let Some(ty) = &declaration.extends {
            census.ty(ty)?;
        }
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                for f in fields {
                    census.field(f)?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    census.name(&case.name)?;
                    census.name(&case.stable_id)?;
                    for f in &case.fields {
                        census.field(f)?;
                    }
                }
            }
            TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => {
                return Err(refusal())
            }
        }
        for e in declaration.invariants() {
            census.expr(e)?;
        }
    }
    for f in &caller.functions {
        census.name(&f.name)?;
        census.name(&f.stable_id)?;
        for p in &f.type_parameters {
            census.name(&p.name)?;
        }
        for p in &f.params {
            census.name(&p.name)?;
            census.ty(&p.ty)?;
        }
        census.ty(&f.return_type)?;
        for name in &f.effects {
            census.name(name)?;
        }
        if let Some(y) = &f.yields {
            census.ty(&y.request_type)?;
            census.ty(&y.response_type)?;
        }
        for e in f
            .requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
        {
            census.expr(e)?;
        }
    }
    census.next.ok_or_else(refusal)
}

fn refusal() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G172",
        "private layout identifier census exceeds its bounded plain-module profile",
    )]
}

struct Census<'a> {
    expressions: Vec<(&'a Expr, usize)>,
    types: Vec<(&'a Type, usize)>,
    patterns: Vec<(Pattern<'a>, usize)>,
    remaining: usize,
    next: Option<u64>,
}
#[derive(Clone, Copy)]
enum Pattern<'a> {
    Match(&'a MatchPattern),
    Record(&'a RecordMatchFieldPattern),
}

impl<'a> Census<'a> {
    fn new() -> Result<Self, Vec<Diagnostic>> {
        let bytes = DEPTH
            .checked_mul(
                std::mem::size_of::<(&Expr, usize)>()
                    + std::mem::size_of::<(&Type, usize)>()
                    + std::mem::size_of::<(Pattern<'_>, usize)>(),
            )
            .ok_or_else(refusal)?;
        reserve_builder_structure(bytes)?;
        let expressions = Vec::with_capacity(DEPTH);
        let types = Vec::with_capacity(DEPTH);
        let patterns = Vec::with_capacity(DEPTH);
        if expressions.capacity() != DEPTH
            || types.capacity() != DEPTH
            || patterns.capacity() != DEPTH
        {
            return Err(refusal());
        }
        Ok(Self {
            expressions,
            types,
            patterns,
            remaining: WORK,
            next: Some(0),
        })
    }
    fn charge(&mut self, n: usize) -> Result<(), Vec<Diagnostic>> {
        self.remaining = self.remaining.checked_sub(n).ok_or_else(refusal)?;
        Ok(())
    }
    fn name(&mut self, name: &str) -> Result<(), Vec<Diagnostic>> {
        self.charge(name.len().checked_add(1).ok_or_else(refusal)?)?;
        if let Some(digits) = name.strip_prefix(PREFIX) {
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                let number = digits.parse::<u64>().map_err(|_| refusal())?;
                let next = number.checked_add(1).ok_or_else(refusal)?;
                self.next = self.next.map(|old| old.max(next));
            }
        }
        Ok(())
    }
    fn field(&mut self, f: &'a FieldDeclaration) -> Result<(), Vec<Diagnostic>> {
        self.name(&f.name)?;
        self.name(&f.stable_id)?;
        self.ty(&f.ty)
    }
    fn ty(&mut self, root: &'a Type) -> Result<(), Vec<Diagnostic>> {
        self.types.push((root, 0));
        while let Some((ty, index)) = self.types.pop() {
            self.charge(1)?;
            if index == 0 {
                if let Type::Named { name, .. } = ty {
                    self.name(name)?;
                }
            }
            let child = match ty {
                Type::Named { arguments, .. } => arguments.get(index),
                Type::Function { parameters, result } => parameters
                    .get(index)
                    .or_else(|| (index == parameters.len()).then_some(result.as_ref())),
                Type::Never
                | Type::I64
                | Type::I32
                | Type::Char
                | Type::U8
                | Type::Usize
                | Type::ArrayU8(_)
                | Type::F32
                | Type::F64
                | Type::Bool
                | Type::String
                | Type::Bytes
                | Type::Str
                | Type::SliceU8
                | Type::StringMap
                | Type::OnceFunction
                | Type::OnceFunctionI64
                | Type::OnceFunctionI64Pair
                | Type::MutFunctionI64 => None,
            };
            if let Some(child) = child {
                if self.types.len() + 2 > DEPTH {
                    return Err(refusal());
                }
                self.types.push((ty, index + 1));
                self.types.push((child, 0));
            }
        }
        Ok(())
    }
    fn pattern(&mut self, root: &'a MatchPattern) -> Result<(), Vec<Diagnostic>> {
        self.patterns.push((Pattern::Match(root), 0));
        while let Some((pattern, index)) = self.patterns.pop() {
            self.charge(1)?;
            if index == 0 {
                match pattern {
                    Pattern::Match(MatchPattern::Variant {
                        type_name,
                        case_name,
                        fields,
                        ..
                    }) => {
                        self.name(type_name)?;
                        self.name(case_name)?;
                        for f in fields {
                            self.name(&f.name)?;
                            self.name(&f.binding)?;
                        }
                    }
                    Pattern::Match(MatchPattern::Record {
                        type_name, fields, ..
                    })
                    | Pattern::Record(RecordMatchFieldPattern::Record {
                        type_name, fields, ..
                    }) => {
                        self.name(type_name)?;
                        for f in fields {
                            self.name(&f.name)?;
                        }
                    }
                    Pattern::Match(MatchPattern::Binding { name, .. })
                    | Pattern::Record(RecordMatchFieldPattern::Binding { name, .. }) => {
                        self.name(name)?
                    }
                    Pattern::Match(
                        MatchPattern::Wildcard { .. }
                        | MatchPattern::Literal { .. }
                        | MatchPattern::Or { .. },
                    )
                    | Pattern::Record(RecordMatchFieldPattern::Wildcard { .. }) => {}
                }
            }
            let child = match pattern {
                Pattern::Match(MatchPattern::Or { alternatives, .. }) => {
                    alternatives.get(index).map(Pattern::Match)
                }
                Pattern::Match(MatchPattern::Record { fields, .. })
                | Pattern::Record(RecordMatchFieldPattern::Record { fields, .. }) => {
                    fields.get(index).map(|f| Pattern::Record(&f.pattern))
                }
                _ => None,
            };
            if let Some(child) = child {
                if self.patterns.len() + 2 > DEPTH {
                    return Err(refusal());
                }
                self.patterns.push((pattern, index + 1));
                self.patterns.push((child, 0));
            }
        }
        Ok(())
    }
    fn expr(&mut self, root: &'a Expr) -> Result<(), Vec<Diagnostic>> {
        self.expressions.push((root, 0));
        while let Some((e, index)) = self.expressions.pop() {
            // Indexed source child lookup still scans a block/match prefix.
            // Charge that actual bound, rather than pretending it is a cursor.
            self.charge(match &e.kind {
                ExprKind::Block { statements, .. } => statements.len() + 1,
                ExprKind::Match { arms, .. } => arms.len() + 1,
                _ => 1,
            })?;
            if index == 0 {
                match &e.kind {
                    ExprKind::VecConstructor { element, .. } => self.ty(element)?,
                    ExprKind::Var(name) => self.name(name)?,
                    ExprKind::Closure {
                        params,
                        return_type,
                        ..
                    } => {
                        for p in params {
                            self.name(&p.name)?;
                            self.ty(&p.ty)?;
                        }
                        self.ty(return_type)?;
                    }
                    ExprKind::Call {
                        name,
                        type_arguments,
                        ..
                    } => {
                        self.name(name)?;
                        for ty in type_arguments {
                            self.ty(ty)?;
                        }
                    }
                    ExprKind::MethodCall {
                        method,
                        type_arguments,
                        ..
                    } => {
                        self.name(method)?;
                        for ty in type_arguments {
                            self.ty(ty)?;
                        }
                    }
                    ExprKind::SuperMethod { method, .. } => self.name(method)?,
                    ExprKind::ConstructRecord {
                        type_name,
                        type_arguments,
                        fields,
                        ..
                    }
                    | ExprKind::ConstructVariant {
                        type_name,
                        type_arguments,
                        fields,
                        ..
                    } => {
                        self.name(type_name)?;
                        for ty in type_arguments {
                            self.ty(ty)?;
                        }
                        for f in fields {
                            self.name(&f.name)?;
                        }
                        if let ExprKind::ConstructVariant { case_name, .. } = &e.kind {
                            self.name(case_name)?;
                        }
                    }
                    ExprKind::UpdateRecord { fields, .. } => {
                        for f in fields {
                            self.name(&f.name)?;
                        }
                    }
                    ExprKind::Project { field, .. } => self.name(field)?,
                    ExprKind::Match { arms, .. } => {
                        for arm in arms {
                            self.pattern(&arm.pattern)?;
                        }
                    }
                    ExprKind::Block { statements, .. } => {
                        for s in statements {
                            match s {
                                Statement::Let { name, declared, .. } => {
                                    self.name(name)?;
                                    if let Some(ty) = declared {
                                        self.ty(ty)?;
                                    }
                                }
                                Statement::Assign { name, field, .. } => {
                                    self.name(name)?;
                                    if let Some(f) = field {
                                        self.name(&f.name)?;
                                    }
                                }
                                Statement::For { item, .. } | Statement::ForOwn { item, .. } => {
                                    self.name(item)?
                                }
                                Statement::ForBorrow { item, index, .. } => {
                                    self.name(item)?;
                                    if let Some(i) = index {
                                        self.name(&i.name)?;
                                    }
                                }
                                Statement::Unsafe { .. } | Statement::While { .. } => {}
                            }
                        }
                    }
                    ExprKind::LexicalExit { .. }
                    | ExprKind::Unary { .. }
                    | ExprKind::Binary { .. }
                    | ExprKind::If { .. }
                    | ExprKind::Try { .. }
                    | ExprKind::Yield { .. }
                    | ExprKind::Int(_)
                    | ExprKind::Int32(_)
                    | ExprKind::Char(_)
                    | ExprKind::Uint8(_)
                    | ExprKind::Usize(_)
                    | ExprKind::ArrayU8(_)
                    | ExprKind::RepeatArrayU8 { .. }
                    | ExprKind::Float32(_)
                    | ExprKind::Float64(_)
                    | ExprKind::Bool(_)
                    | ExprKind::String(_)
                    | ExprKind::BorrowedUtf8Literal(_) => {}
                }
            }
            if let Some(child) = e.child(index) {
                if self.expressions.len() + 2 > DEPTH {
                    return Err(refusal());
                }
                self.expressions.push((e, index + 1));
                self.expressions.push((child, 0));
            }
        }
        Ok(())
    }
}
