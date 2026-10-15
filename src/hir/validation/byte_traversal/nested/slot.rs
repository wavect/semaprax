//! A paid receiving carrier lazily holds one independently replayed census.
//! Selection reuses the existing complete v1 region check; it creates no extra
//! cursor scan, temporary owned error text or unpriced selector workspace.
use super::{
    Census, Diagnostic, Failure, Refused, Region, ResolvedExpr, ResolvedFunction, Storage,
};
pub(crate) struct Slot<'a> {
    context: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    proof: Option<Census<'a>>,
    storage: Storage,
}
pub(crate) fn controls(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::active_remaining().is_some() {
        #[cfg(test)]
        super::tests::record_request(bytes);
        if !crate::bounded_output::reserve_active_required(bytes) {
            #[cfg(test)]
            super::tests::record_failure();
            return Err(Failure::Storage(Refused::Budget).diagnostic());
        }
    }
    Ok(())
}
impl<'a> Slot<'a> {
    pub(crate) fn new(
        context: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
    ) -> Result<Self, Diagnostic> {
        let storage = Storage::acquire().map_err(|error| Failure::Storage(error).diagnostic())?;
        storage
            .reserve(std::mem::size_of::<Self>() + std::mem::size_of::<Result<Self, Diagnostic>>())
            .map_err(|error| Failure::Storage(error).diagnostic())?;
        Ok(Self {
            context,
            function,
            proof: None,
            storage,
        })
    }
    pub(crate) fn region(
        &mut self,
        context: &dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        expression: &'a ResolvedExpr,
    ) -> Result<Option<Region<'a>>, Diagnostic> {
        self.storage
            .reserve(0)
            .map_err(|error| Failure::Storage(error).diagnostic())?;
        if !std::ptr::eq(context, self.context) || !std::ptr::eq(function, self.function) {
            return Err(Failure::semantic(
                "borrowed byte traversal census function differs from its current receiver",
            )
            .diagnostic());
        }
        if let Some(proof) = &self.proof {
            return proof.region(context, function, expression);
        }
        match super::super::region(expression) {
            Err(error) if nested_refusal(&error) => {
                let proof = Census::acquire(self.context, self.function)?;
                let region = proof.region(context, function, expression)?;
                self.proof = Some(proof);
                Ok(region)
            }
            original => original,
        }
    }
    pub(crate) fn region_for(
        &mut self,
        context: &dyn crate::cleanup::ReplayContext,
        expression: &'a ResolvedExpr,
    ) -> Result<Option<Region<'a>>, Diagnostic> {
        self.region(context, self.function, expression)
    }
}
pub(in crate::hir) fn nested_refusal(error: &Diagnostic) -> bool {
    error.code == "SPX-H006"
        && error.message == "borrowed byte traversal is nested in another traversal"
}
