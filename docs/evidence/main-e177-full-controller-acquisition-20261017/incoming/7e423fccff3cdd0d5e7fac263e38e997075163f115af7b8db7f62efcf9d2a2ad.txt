//! Borrowed source-phase proof memo. Each defining module/provider is part of
//! the exact key; no cache survives the immutable source invocation.
use super::*;
use crate::ast::program_source::ProgramSource;
use crate::diagnostic::Diagnostic;
use std::cell::RefCell;
type Entry<'a> = (&'a dyn ProgramSource, &'a Function, bool);
pub(crate) struct SourceProofs<'a> {
    entries: Vec<Entry<'a>>,
    cursor: Option<Box<source::Cursor<'a>>>,
    work: usize,
    failed: bool,
}
impl Default for SourceProofs<'_> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            cursor: None,
            work: WORK,
            failed: false,
        }
    }
}
fn error() -> Diagnostic {
    Diagnostic::io(
        "SPX-G171",
        "byte helper source proof index exceeds admitted scratch or work",
    )
}
impl<'a> SourceProofs<'a> {
    pub(crate) fn reserve_carrier(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(error());
        }
        if !crate::bounded_output::reserve_active_required(bytes) {
            self.failed = true;
            return Err(error());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove(
        &mut self,
        owner: &'a dyn ProgramSource,
        target: &'a Function,
        lookup: impl Fn(&'a dyn ProgramSource, &str) -> Option<(&'a dyn ProgramSource, &'a Function)>,
        param: impl Fn(&dyn ProgramSource, ParamMode, &Type) -> bool,
        result: impl Fn(&dyn ProgramSource, &Type) -> bool,
        lookup_work: usize,
    ) -> Result<bool, Diagnostic> {
        if self.failed {
            return Err(error());
        }
        let result = self.checked(owner, target, lookup, param, result, lookup_work);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    #[allow(clippy::too_many_arguments)]
    fn checked(
        &mut self,
        owner: &'a dyn ProgramSource,
        target: &'a Function,
        lookup: impl Fn(&'a dyn ProgramSource, &str) -> Option<(&'a dyn ProgramSource, &'a Function)>,
        param: impl Fn(&dyn ProgramSource, ParamMode, &Type) -> bool,
        result: impl Fn(&dyn ProgramSource, &Type) -> bool,
        lookup_work: usize,
    ) -> Result<bool, Diagnostic> {
        self.work = self
            .work
            .checked_sub(self.entries.len().checked_add(1).ok_or_else(error)?)
            .ok_or_else(error)?;
        if let Some((_, _, proved)) = self
            .entries
            .iter()
            .find(|(p, f, _)| p.subject().same(owner.subject()) && std::ptr::eq(*f, target))
        {
            return Ok(*proved);
        }
        if self.entries.len() == self.entries.capacity() {
            let capacity = self
                .entries
                .capacity()
                .checked_mul(2)
                .ok_or_else(error)?
                .max(1);
            let mut bytes = capacity
                .checked_mul(std::mem::size_of::<Entry<'a>>())
                .ok_or_else(error)?;
            if self.entries.is_empty() {
                bytes = bytes
                    .checked_add(std::mem::size_of::<Self>())
                    .ok_or_else(error)?;
            }
            if !crate::bounded_output::reserve_active_required(bytes) {
                return Err(error());
            }
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(capacity)
                .map_err(|_| error())?;
            if replacement.capacity() != capacity {
                return Err(error());
            }
            replacement.extend_from_slice(&self.entries);
            self.entries = replacement;
        }
        if self.cursor.is_none() {
            if !crate::bounded_output::reserve_active_required(std::mem::size_of::<
                source::Cursor<'a>,
            >()) {
                return Err(error());
            }
            self.cursor = Some(Box::new(source::Cursor::default()));
        }
        let proved = source::prove_with_cursor(
            owner,
            target,
            &lookup,
            &param,
            &result,
            lookup_work,
            self.cursor.as_deref_mut().ok_or_else(error)?,
        )
        .is_some();
        self.entries.push((owner, target, proved));
        Ok(proved)
    }
    pub(crate) fn failure(&self) -> Option<Diagnostic> {
        self.failed.then(error)
    }
    #[cfg(test)]
    pub(crate) fn owned_capacity_bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<Entry<'a>>()
            + self
                .cursor
                .as_ref()
                .map_or(0, |_| std::mem::size_of::<source::Cursor<'a>>())
    }
}
pub(crate) struct SourceIndex<'a> {
    program: &'a dyn ProgramSource,
    proofs: RefCell<SourceProofs<'a>>,
}
impl<'a> SourceIndex<'a> {
    pub(crate) fn new(program: &'a dyn ProgramSource) -> Self {
        Self {
            program,
            proofs: RefCell::new(SourceProofs::default()),
        }
    }
    /// Signature frontier before target() can inspect transitive source bodies
    /// or allocate proof state. Reuse callers share this exact prerequisite.
    pub(crate) fn may_require_body_proof(function: &Function) -> bool {
        function.return_type == Type::Bytes
            && function
                .params
                .iter()
                .filter(|p| p.mode == ParamMode::Own && p.ty == Type::Bytes)
                .count()
                == 1
    }
    pub(crate) fn target(&self, function: &Function) -> bool {
        if !Self::may_require_body_proof(function) {
            return false;
        }
        let Some(function) = self
            .program
            .functions()
            .iter()
            .find(|candidate| std::ptr::eq(*candidate, function))
        else {
            return false;
        };
        if !function
            .params
            .iter()
            .all(|p| ast_param(self.program, p.mode, &p.ty))
        {
            return false;
        }
        let mut proofs = self.proofs.borrow_mut();
        if proofs.entries.is_empty() && !proofs.failed {
            let carrier = std::mem::size_of::<Self>() - std::mem::size_of::<SourceProofs<'a>>();
            if proofs.reserve_carrier(carrier).is_err() {
                return false;
            }
        }
        proofs
            .prove(
                self.program,
                function,
                |owner, name| {
                    owner
                        .functions()
                        .iter()
                        .find(|f| f.name == name)
                        .map(|f| (owner, f))
                },
                ast_param,
                super::super::ast_result_admitted,
                self.program
                    .functions()
                    .len()
                    .saturating_add(self.program.module_uses().len()),
            )
            .unwrap_or(false)
    }
    pub(crate) fn assignment(&self, binding: &str, value: &Expr) -> bool {
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &value.kind
        else {
            return false;
        };
        let Some(target) = self.program.functions().iter().find(|f| f.name == *name) else {
            return false;
        };
        type_arguments.is_empty()
            && args.len() == target.params.len()
            && self.target(target)
            && target.params.iter().zip(args).all(|(p, arg)| {
                p.mode != ParamMode::Own
                    || matches!(&arg.kind, ExprKind::Var(name) if name == binding)
            })
    }
    pub(crate) fn failure(&self) -> Option<Diagnostic> {
        self.proofs.borrow().failure()
    }
    #[cfg(test)]
    pub(crate) fn owned_capacity_bytes(&self) -> usize {
        self.proofs.borrow().owned_capacity_bytes()
    }
}
#[cfg(test)]
#[path = "source_index/tests.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod tests;

pub(crate) trait SourceHelpers {
    fn target(&self, function: &Function) -> bool;
    fn assignment(&self, binding: &str, value: &Expr) -> bool;
    #[cfg(test)]
    fn owned_capacity_bytes(&self) -> usize;
}
impl SourceHelpers for SourceIndex<'_> {
    fn target(&self, function: &Function) -> bool {
        self.target(function)
    }
    fn assignment(&self, binding: &str, value: &Expr) -> bool {
        self.assignment(binding, value)
    }
    #[cfg(test)]
    fn owned_capacity_bytes(&self) -> usize {
        self.owned_capacity_bytes()
    }
}

// ORIGINAL_OBSERVATION_BEGIN
#[cfg(test)]
impl SourceProofs<'_> {
    pub(crate) fn scope_observation(&self) -> (usize, usize) {
        (self.entries.len(), self.work)
    }
}
// ORIGINAL_OBSERVATION_END
