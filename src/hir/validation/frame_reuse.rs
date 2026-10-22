//! Empty typed continuation backing, owned by one immutable validation call.
use super::*;
#[cfg(test)]
mod before;
mod contracts;
#[cfg(test)]
mod contracts_before;
#[cfg(test)]
mod custody;
mod function;
#[cfg(test)]
mod function_before;
#[cfg(test)]
pub(in crate::hir::validation) mod source_inverse;
#[cfg(test)]
mod tests;

type Directories<'e> = (
    scope_checkpoint::Storage<Frame<'e>>,
    scope_checkpoint::Storage<scope_checkpoint::Scope>,
);

// The caller pays the new optional-owner argument temporary and callee formal
// before constructing Some/None or reborrowing a lease. This grants no result.
pub(super) fn argument() -> Result<(), Diagnostic> {
    scope_checkpoint::charge::reserve(
        std::mem::size_of::<Option<&mut Scratch<'_>>>().checked_mul(2),
    )
}

// Fresh adapters introduce another argument/result ABI layer. Acquire that
// layer before forming the forwarded call; ordinary direct calls retain their
// original parameter roles and pay only the added optional-owner transport.
pub(super) fn fresh_function_transport() -> Result<(), Diagnostic> {
    type Args<'a> = (
        &'a mut HirValidator<'a>,
        &'a ResolvedFunction,
        &'a FunctionExecutionId,
    );
    scope_checkpoint::charge::reserve(
        std::mem::size_of::<Args<'_>>()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Result<(), Diagnostic>>())),
    )
}
pub(super) fn fresh_expression_transport() -> Result<(), Diagnostic> {
    type Args<'a> = (
        &'a mut HirValidator<'a>,
        &'a FunctionExecutionId,
        &'a ResolvedExpr,
        &'a mut dyn ScopeWrite,
        path_label::Label<'a>,
        bool,
        Option<effect_keys::Allowed<'a>>,
    );
    scope_checkpoint::charge::reserve(
        std::mem::size_of::<Args<'_>>()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Result<(), Diagnostic>>())),
    )
}

pub(super) struct Scratch<'e> {
    frames: scope_checkpoint::Storage<Frame<'e>>,
    scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
}
impl<'e> Scratch<'e> {
    pub(super) fn new() -> Result<Self, Diagnostic> {
        // The new enclosing owner and its returned constructor carrier are
        // acquired before either empty directory is formed. Backing is still
        // acquired exclusively by Storage's complete actual-capacity payer.
        scope_checkpoint::charge::reserve(
            std::mem::size_of::<Self>()
                .checked_add(std::mem::size_of::<Result<Self, Diagnostic>>()),
        )?;
        Ok(Self {
            frames: scope_checkpoint::Storage::new(),
            scopes: scope_checkpoint::Storage::new(),
        })
    }
    pub(super) fn take(&mut self) -> Result<Directories<'e>, Diagnostic> {
        scope_checkpoint::charge::reserve(Some(0))?;
        // Authenticate even an already funded hit. Pay the replacement empty
        // pair, moved-out pair and returned transport before forming them.
        scope_checkpoint::charge::reserve(
            std::mem::size_of::<Directories<'e>>()
                .checked_mul(2)
                .and_then(|bytes| {
                    bytes.checked_add(std::mem::size_of::<Result<Directories<'e>, Diagnostic>>())
                }),
        )?;
        if self.frames.len() != 0 || self.scopes.len() != 0 {
            return Err(hir_error_static(
                "HIR continuation reuse requires empty directories",
            ));
        }
        Ok((
            std::mem::replace(&mut self.frames, scope_checkpoint::Storage::new()),
            std::mem::replace(&mut self.scopes, scope_checkpoint::Storage::new()),
        ))
    }
    pub(super) fn restore(
        &mut self,
        frames: scope_checkpoint::Storage<Frame<'e>>,
        scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
    ) -> Result<(), Diagnostic> {
        scope_checkpoint::charge::reserve(Some(0))?;
        if frames.len() != 0
            || scopes.len() != 0
            || self.frames.len() != 0
            || self.scopes.len() != 0
        {
            return Err(hir_error_static(
                "HIR continuation reuse requires drained directories",
            ));
        }
        self.frames = frames;
        self.scopes = scopes;
        Ok(())
    }
}
