//! Complete recursive-type census before shared acquisition.
use super::*;
use crate::cleanup_plan::ReplayClonePayload;
fn capacity<E>(
    root: &ResolvedType,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<usize, E> {
    fn push<'a, E>(
        stack: &mut Vec<&'a ResolvedType>,
        ty: &'a ResolvedType,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        if stack.len() == stack.capacity() {
            let capacity = stack.capacity().saturating_mul(2).max(4);
            reserve(
                size_of::<Vec<&ResolvedType>>()
                    .saturating_add(capacity.saturating_mul(size_of::<&ResolvedType>())),
            )?;
            let mut next = Vec::with_capacity(capacity);
            assert_eq!(next.capacity(), capacity);
            next.append(stack);
            *stack = next;
        }
        stack.push(ty);
        Ok(())
    }
    // Leaves require neither a census heap nor last-owner Drop scratch.
    if !matches!(root, ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
        && !matches!(root, ResolvedType::Function { .. })
    {
        return Ok(0);
    }
    let mut stack = Vec::new();
    push(&mut stack, root, reserve)?;
    let mut highwater = 0;
    while let Some(ty) = stack.pop() {
        match ty {
            ResolvedType::Nominal { arguments, .. } => {
                for child in arguments {
                    push(&mut stack, child, reserve)?;
                }
            }
            ResolvedType::Function { parameters, result } => {
                for child in parameters {
                    push(&mut stack, child, reserve)?;
                }
                push(&mut stack, result, reserve)?;
            }
            _ => {}
        }
        highwater = highwater.max(stack.len());
    }
    Ok(highwater)
}
impl NominalElement for ResolvedFieldDeclaration {
    fn clone_legacy_unfunded(&self) -> Self {
        super::legacy::clone::field(self)
    }
    fn drop_capacity<E>(
        &self,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<usize, E> {
        capacity(&self.ty, reserve)
    }
    fn drain_types(&mut self, dispose: &mut impl FnMut(ResolvedType)) {
        dispose(std::mem::replace(&mut self.ty, ResolvedType::Unit));
    }
    fn clone_bytes(&self) -> Option<usize> {
        self.name.len().checked_add(self.ty.clone_payload_bytes()?)
    }
}
impl NominalElement for ResolvedVariantCaseDeclaration {
    fn clone_legacy_unfunded(&self) -> Self {
        super::legacy::clone::case(self)
    }
    fn drop_capacity<E>(
        &self,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<usize, E> {
        let mut n = 0;
        for field in &self.fields {
            n = n.max(field.drop_capacity(reserve)?);
        }
        Ok(n)
    }
    fn drain_types(&mut self, dispose: &mut impl FnMut(ResolvedType)) {
        for field in &mut self.fields {
            field.drain_types(dispose);
        }
    }
    fn clone_bytes(&self) -> Option<usize> {
        self.fields.iter().try_fold(
            self.name.len().checked_add(
                self.fields
                    .len()
                    .checked_mul(size_of::<ResolvedFieldDeclaration>())?,
            )?,
            |n, f| n.checked_add(f.clone_bytes()?),
        )
    }
}
impl NominalElement for ResolvedTypeParameterDeclaration {
    fn drop_capacity<E>(&self, _: &mut impl FnMut(usize) -> Result<(), E>) -> Result<usize, E> {
        Ok(0)
    }
    fn drain_types(&mut self, _: &mut impl FnMut(ResolvedType)) {}
    fn clone_bytes(&self) -> Option<usize> {
        Some(self.name.len())
    }
}
