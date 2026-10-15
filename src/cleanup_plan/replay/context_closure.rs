//! Inactive prerequisite: borrowed reachable namespace inventory, never replay authority.
//! Ordinary whole-program validation remains required. This does not implement
//! ReplayContext or select a physical storage profile.
use super::{replay_error, Diagnostic, ReplayBudget};
use crate::cleanup::ReplayContext;
use crate::hir::lexical_exit::ChildCursor;
use crate::hir::{
    DeclarationId, DeclarationKind, ResolvedExpr, ResolvedExprKind as K, ResolvedFunction,
    ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind as TK,
};

const DEPTH: usize = 514;
#[derive(Clone, Copy)]
enum Frame<'a> {
    Function(usize, usize),
    Expression(ChildCursor<'a>, bool),
}
/// Pointer-bound inventories in deterministic discovery order. No cloned IDs,
/// AST, type payloads, or attached cleanup/loan certificates enter this proof.
pub(super) struct Inventory<'a> {
    pub(super) functions: Vec<&'a ResolvedFunction>,
    pub(super) types: Vec<&'a ResolvedTypeDeclaration>,
}
struct Walk<'a, 'b> {
    source: &'a dyn ReplayContext,
    root: &'a ResolvedFunction,
    budget: &'b mut ReplayBudget,
    inventory: Inventory<'a>,
    function_state: Vec<u8>,
    type_state: Vec<u8>,
}
impl<'a, 'b> Walk<'a, 'b> {
    fn debit(&mut self, n: usize) -> Result<(), Diagnostic> {
        self.budget.charge(self.root, n, "inactive context closure")
    }
    fn error(&self) -> Diagnostic {
        replay_error(
            self.root,
            "inactive context closure cannot authenticate this reachable shape",
        )
    }
    fn function(&mut self, id: &DeclarationId) -> Result<usize, Diagnostic> {
        let source = self.source;
        let mut found = None;
        for (i, f) in source.functions().iter().enumerate() {
            self.debit(1)?;
            if f.id == *id && found.replace(i).is_some() {
                return Err(self.error());
            }
        }
        let i = found.ok_or_else(|| self.error())?;
        let f = source.functions().get(i).ok_or_else(|| self.error())?;
        self.debit(id.as_str().len().saturating_add(1))?;
        let d = self
            .source
            .declarations()
            .declaration(id)
            .ok_or_else(|| self.error())?;
        if d.kind != DeclarationKind::Function
            || d.name != f.name
            || d.owner.is_some()
            || !f.effects.is_empty()
            || f.yields.is_some()
            || self
                .source
                .declarations()
                .type_parameters(id)
                .is_some_and(|p| !p.is_empty())
        {
            return Err(self.error());
        }
        Ok(i)
    }
    fn nominal(&mut self, id: &DeclarationId) -> Result<usize, Diagnostic> {
        let source = self.source;
        let mut found = None;
        for (i, t) in source.types().iter().enumerate() {
            self.debit(1)?;
            if t.id == *id && found.replace(i).is_some() {
                return Err(self.error());
            }
        }
        found.ok_or_else(|| self.error())
    }
    fn ty(&mut self, ty: &'a ResolvedType) -> Result<(), Diagnostic> {
        let source = self.source;
        self.debit(1)?;
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => {
                let i = self.nominal(declaration)?;
                if self.type_state[i] == 0 {
                    self.type_state[i] = 1;
                    self.inventory
                        .types
                        .push(source.types().get(i).ok_or_else(|| self.error())?);
                }
                Ok(())
            }
            ResolvedType::Never
            | ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8 => Ok(()),
            ResolvedType::Nominal { .. }
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::Function { .. }
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
            | ResolvedType::StringMap => Err(self.error()),
        }
    }
    fn expression(&mut self, e: &'a ResolvedExpr) -> Result<Option<usize>, Diagnostic> {
        let source = self.source;
        self.ty(&e.ty)?;
        match &e.kind {
            K::Place(place) if place.projections.is_empty() => Ok(None),
            K::Place(_) => Err(self.error()),
            K::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(self.error());
                }
                let i = self.function(callee)?;
                let target = source.functions().get(i).ok_or_else(|| self.error())?;
                self.debit(args.len().saturating_add(1))?;
                self.ty(&target.return_type)?;
                if target.return_type != e.ty || target.params.len() != args.len() {
                    return Err(self.error());
                }
                for (a, p) in args.iter().zip(&target.params) {
                    self.ty(&p.ty)?;
                    self.ty(&a.ty)?;
                    if a.ty != p.ty || a.ownership != p.ownership {
                        return Err(self.error());
                    }
                }
                Ok(Some(i))
            }
            K::Block { statements, .. } => {
                self.debit(statements.len())?;
                for s in statements {
                    if matches!(s, crate::hir::ResolvedStatement::CopyDiscard { .. }) {
                        return Err(self.error());
                    }
                    self.ty(&s.binding().ty)?;
                }
                Ok(None)
            }
            // These leaves/operators carry no hidden callable/type relation.
            K::Int(_)
            | K::Int32(_)
            | K::Char(_)
            | K::Uint8(_)
            | K::Usize(_)
            | K::ArrayU8(_)
            | K::RepeatArrayU8 { .. }
            | K::Float32(_)
            | K::Float64(_)
            | K::Bool(_)
            | K::String(_)
            | K::BorrowedUtf8Literal(_)
            | K::Unary { .. }
            | K::Binary { .. }
            | K::If { .. }
            | K::LexicalExit { .. } => Ok(None),
            // Pattern/callable/import and generic metadata need their own complete
            // borrowed authentication; no child-only fallback is permission.
            K::VecConstructor { .. }
            | K::Closure { .. }
            | K::FunctionReference { .. }
            | K::Invoke { .. }
            | K::BorrowPlace { .. }
            | K::ByteRange { .. }
            | K::LiteralFormat { .. }
            | K::VecFieldRead { .. }
            | K::RowLoan { .. }
            | K::NativeRustImportCall(_)
            | K::HostCommandCall(_)
            | K::ConstructRecord { .. }
            | K::ConstructVariant { .. }
            | K::Match { .. }
            | K::Try { .. }
            | K::TryOption { .. }
            | K::UpdateRecord { .. }
            | K::Project { .. }
            | K::Upcast { .. }
            | K::Yield { .. } => Err(self.error()),
        }
    }
    fn type_closure(&mut self) -> Result<(), Diagnostic> {
        let mut next = 0;
        while next < self.inventory.types.len() {
            let t = self.inventory.types[next];
            next += 1;
            self.debit(t.id.as_str().len().saturating_add(1))?;
            let d = self
                .source
                .declarations()
                .declaration(&t.id)
                .ok_or_else(|| self.error())?;
            if d.owner.is_some()
                || d.name != t.name
                || !t.type_parameters.is_empty()
                || self
                    .source
                    .declarations()
                    .type_parameters(&t.id)
                    .is_none_or(|p| !p.is_empty())
            {
                return Err(self.error());
            }
            match &t.kind {
                TK::Record { fields } if d.kind == DeclarationKind::Record => {
                    let actual = self
                        .source
                        .declarations()
                        .record_fields(&t.id)
                        .ok_or_else(|| self.error())?;
                    self.fields(&t.id, fields, actual, DeclarationKind::Field)?;
                }
                TK::Variant { cases } if d.kind == DeclarationKind::Variant => {
                    let actual = self
                        .source
                        .declarations()
                        .variant_cases(&t.id)
                        .ok_or_else(|| self.error())?;
                    self.debit(cases.len())?;
                    if cases.len() != actual.len() {
                        return Err(self.error());
                    }
                    for (i, (c, a)) in cases.iter().zip(actual).enumerate() {
                        self.debit(
                            i.saturating_add(c.id.as_str().len())
                                .saturating_add(c.name.len())
                                .saturating_add(1),
                        )?;
                        if cases[..i].iter().any(|old| old.id == c.id) {
                            return Err(self.error());
                        }
                        if c.id != a.id
                            || c.name != a.name
                            || c.index as usize != i
                            || c.index != a.index
                        {
                            return Err(self.error());
                        }
                        let cd = self
                            .source
                            .declarations()
                            .declaration(&c.id)
                            .ok_or_else(|| self.error())?;
                        if cd.kind != DeclarationKind::VariantCase
                            || cd.owner.as_ref() != Some(&t.id)
                        {
                            return Err(self.error());
                        }
                        self.fields(&c.id, &c.fields, &a.fields, DeclarationKind::CaseField)?;
                    }
                }
                TK::Resource { .. } | TK::Class { .. } | TK::Record { .. } | TK::Variant { .. } => {
                    return Err(self.error());
                }
            }
        }
        self.reject_type_cycles()
    }
    fn fields(
        &mut self,
        owner: &DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldDeclaration],
        actual: &[crate::hir::ResolvedFieldDeclaration],
        kind: DeclarationKind,
    ) -> Result<(), Diagnostic> {
        self.debit(fields.len())?;
        if fields.len() != actual.len() {
            return Err(self.error());
        }
        for (i, (f, a)) in fields.iter().zip(actual).enumerate() {
            self.debit(
                i.saturating_add(f.id.as_str().len())
                    .saturating_add(f.name.len())
                    .saturating_add(1),
            )?;
            if fields[..i].iter().any(|old| old.id == f.id) {
                return Err(self.error());
            }
            self.ty(&f.ty)?;
            // Validate both types before structural equality: accepted nominal
            // types have no recursive argument backing.
            match &a.ty {
                ResolvedType::Nominal { arguments, .. } if !arguments.is_empty() => {
                    return Err(self.error());
                }
                ResolvedType::Function { .. } | ResolvedType::TypeParameter { .. } => {
                    return Err(self.error());
                }
                _ => {}
            }
            if f.id != a.id
                || f.name != a.name
                || f.index as usize != i
                || f.index != a.index
                || f.ty != a.ty
            {
                return Err(self.error());
            }
            let fd = self
                .source
                .declarations()
                .declaration(&f.id)
                .ok_or_else(|| self.error())?;
            if fd.kind != kind || fd.owner.as_ref() != Some(owner) {
                return Err(self.error());
            }
        }
        Ok(())
    }
    fn reject_type_cycles(&mut self) -> Result<(), Diagnostic> {
        let source = self.source;
        self.budget.reserve_array::<Option<(usize, usize)>>(
            self.root,
            DEPTH,
            "context type cycle cursor",
        )?;
        let mut frames = [None; DEPTH];
        for start in 0..self.inventory.types.len() {
            let i = self.nominal(&self.inventory.types[start].id)?;
            if self.type_state[i] == 3 {
                continue;
            }
            self.type_state[i] = 2;
            frames[0] = Some((i, 0));
            let mut depth = 1;
            while depth != 0 {
                self.debit(1)?;
                let (i, child) = frames[depth - 1].unwrap();
                let t = source.types().get(i).ok_or_else(|| self.error())?;
                // Charge the complete inspected prefix for this deliberately
                // conservative borrowed cycle scan; no hidden map is built.
                let width = match &t.kind {
                    TK::Record { fields } => fields.len(),
                    TK::Variant { cases } => {
                        self.debit(cases.len())?;
                        cases
                            .iter()
                            .try_fold(0usize, |n, c| n.checked_add(c.fields.len()))
                            .ok_or_else(|| self.error())?
                    }
                    _ => return Err(self.error()),
                };
                self.debit(width.saturating_add(1))?;
                let ty = match &t.kind {
                    TK::Record { fields } => fields.get(child).map(|f| &f.ty),
                    TK::Variant { cases } => cases
                        .iter()
                        .flat_map(|c| &c.fields)
                        .nth(child)
                        .map(|f| &f.ty),
                    _ => None,
                };
                let Some(ty) = ty else {
                    self.type_state[i] = 3;
                    depth -= 1;
                    continue;
                };
                frames[depth - 1] = Some((i, child + 1));
                if let ResolvedType::Nominal { declaration, .. } = ty {
                    let j = self.nominal(declaration)?;
                    if self.type_state[j] == 2 {
                        return Err(self.error());
                    }
                    if self.type_state[j] != 3 {
                        if depth == DEPTH {
                            return Err(self.error());
                        }
                        self.type_state[j] = 2;
                        frames[depth] = Some((j, 0));
                        depth += 1;
                    }
                }
            }
        }
        Ok(())
    }
}

pub(super) fn derive<'a>(
    source: &'a dyn ReplayContext,
    root: &'a ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Inventory<'a>, Diagnostic> {
    if !budget.selected() {
        return Err(replay_error(
            root,
            "context closure requires a physical reservation",
        ));
    }
    budget.reserve_array::<Walk<'_, '_>>(root, 1, "context inventory headers")?;
    budget.reserve_array::<Option<Frame<'_>>>(root, DEPTH, "context closure cursor")?;
    let mut walk = Walk {
        source,
        root,
        budget,
        inventory: Inventory {
            functions: Vec::new(),
            types: Vec::new(),
        },
        function_state: Vec::new(),
        type_state: Vec::new(),
    };
    walk.budget.reserve_vec(
        root,
        &mut walk.inventory.functions,
        source.functions().len(),
        "context function references",
    )?;
    walk.budget.reserve_vec(
        root,
        &mut walk.function_state,
        source.functions().len(),
        "context function states",
    )?;
    walk.budget.reserve_vec(
        root,
        &mut walk.inventory.types,
        source.types().len(),
        "context type references",
    )?;
    walk.budget.reserve_vec(
        root,
        &mut walk.type_state,
        source.types().len(),
        "context type states",
    )?;
    walk.function_state.resize(source.functions().len(), 0);
    walk.type_state.resize(source.types().len(), 0);
    let first = walk.function(&root.id)?;
    if !std::ptr::eq(
        source.functions().get(first).ok_or_else(|| walk.error())?,
        root,
    ) {
        return Err(walk.error());
    }
    let mut frames = [None; DEPTH];
    frames[0] = Some(Frame::Function(first, 0));
    let mut depth = 1;
    while depth != 0 {
        walk.debit(1)?;
        match frames[depth - 1].unwrap() {
            Frame::Function(i, n) => {
                let f = source.functions().get(i).ok_or_else(|| walk.error())?;
                if n == 0 {
                    if walk.function_state[i] == 1 {
                        return Err(walk.error());
                    }
                    walk.function_state[i] = 1;
                    walk.inventory.functions.push(f);
                    walk.ty(&f.return_type)?;
                    walk.debit(f.params.len())?;
                    for p in &f.params {
                        walk.ty(&p.ty)?;
                    }
                }
                let e = if n < f.requires.len() {
                    Some(&f.requires[n])
                } else if n == f.requires.len() {
                    Some(&f.body)
                } else {
                    f.ensures.get(n - f.requires.len() - 1)
                };
                let Some(e) = e else {
                    walk.function_state[i] = 2;
                    depth -= 1;
                    continue;
                };
                frames[depth - 1] = Some(Frame::Function(i, n + 1));
                if depth == DEPTH {
                    return Err(walk.error());
                }
                frames[depth] = Some(Frame::Expression(ChildCursor::new(e), false));
                depth += 1;
            }
            Frame::Expression(mut cursor, entered) => {
                if !entered {
                    frames[depth - 1] = Some(Frame::Expression(cursor, true));
                    if let Some(i) = walk.expression(cursor.expression())? {
                        if walk.function_state[i] == 1 {
                            return Err(walk.error());
                        }
                        if walk.function_state[i] == 0 {
                            if depth == DEPTH {
                                return Err(walk.error());
                            }
                            frames[depth] = Some(Frame::Function(i, 0));
                            depth += 1;
                        }
                    }
                    continue;
                }
                let child = cursor.next();
                frames[depth - 1] = Some(Frame::Expression(cursor, true));
                let Some(e) = child else {
                    depth -= 1;
                    continue;
                };
                if depth == DEPTH {
                    return Err(walk.error());
                }
                frames[depth] = Some(Frame::Expression(ChildCursor::new(e), false));
                depth += 1;
            }
        }
    }
    walk.type_closure()?;
    Ok(walk.inventory)
}

#[cfg(test)]
mod tests;
