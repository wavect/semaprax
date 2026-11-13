//! Guarded complete-profile lease. Production Context keeps its old route.
use super::*;
pub(in crate::cleanup_plan::replay) struct Context<'a> {
    source: &'a dyn ReplayContext,
    profile: schema::borrowed::Profile<'a>,
    identity: Identity,
}

/// Non-zero-sized private receiver brand; ordinary/forwarding providers cannot
/// construct one. Its borrowed address is stable for the lifetime of each lease.
pub(crate) struct Identity {
    _private: u8,
}
impl<'a> Context<'a> {
    pub(in crate::cleanup_plan::replay) fn new(
        source: &'a dyn ReplayContext,
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let profile = schema::borrowed::build(source, function, budget)?;
        let selected = profile.schema(source, function, budget)?;
        // Current typed facts and independently rebuilt inventory are authority;
        // the attached plan/inventory must agree before lending the renewal lease.
        budget.charge(function, 1, "borrowed lease attached schema header")?;
        if !schema::borrowed::same_schema(function, selected, function.cleanup_plan.schema, budget)?
        {
            return Err(replay_error(
                function,
                "borrowed lease cleanup schema disagrees",
            ));
        }
        profile.validate_inventory(source, function, budget)?;
        budget.reserve_array::<Self>(function, 1, "guarded borrowed replay lease")?;
        Ok(Self {
            source,
            profile,
            identity: Identity { _private: 0 },
        })
    }
}

impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        self.source.declarations()
    }
    fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
        self.source.types()
    }
    fn type_count(&self) -> usize {
        self.source.type_count()
    }
    fn type_at(&self, index: usize) -> Option<&crate::hir::ResolvedTypeDeclaration> {
        self.source.type_at(index)
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        self.source.interfaces()
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        self.source.functions()
    }
    fn function_count(&self) -> usize {
        self.source.function_count()
    }
    fn function_at(&self, index: usize) -> Option<&ResolvedFunction> {
        self.source.function_at(index)
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        self.source.function_templates()
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        self.source.function_instances()
    }
    fn resolve_call_target(
        &self,
        callee: &crate::hir::DeclarationId,
        instance: Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        self.source.resolve_call_target(callee, instance)
    }
    fn selected_flat_records(&self) -> bool {
        true
    }
    fn borrowed_helper_lease<'a>(
        &'a self,
        function: &ResolvedFunction,
    ) -> Option<crate::cleanup_plan::ReplayBorrowedLease<'a>> {
        self.profile
            .matches(self.source, function)
            .then_some(Lease {
                identity: &self.identity,
                source: self.source,
                profile: &self.profile,
            })
    }
    fn borrowed_helper_identity(&self) -> Option<&Identity> {
        Some(&self.identity)
    }
    fn primitive_renewals<'a>(
        &'a self,
        function: &ResolvedFunction,
    ) -> Option<&'a crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        self.profile.renewals(self.source, function)
    }
}

/// Opaque current-context receiver. Only this private Context can mint it;
/// forwarding a lease through another provider cannot mint its receiver brand.
/// Trait-object vtable addresses are not stable identity across codegen units.
pub(crate) struct Lease<'a> {
    identity: &'a Identity,
    source: &'a dyn ReplayContext,
    profile: &'a schema::borrowed::Profile<'a>,
}
impl Lease<'_> {
    pub(in crate::cleanup_plan::replay) fn validate(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "borrowed lease exact receiving context")?;
        if !context
            .borrowed_helper_identity()
            .is_some_and(|identity| std::ptr::eq(self.identity, identity))
            || !self.profile.matches(self.source, function)
        {
            return Err(replay_error(
                function,
                "borrowed profile belongs to another receiving context",
            ));
        }
        Ok(())
    }
    pub(in crate::cleanup_plan::replay) fn schema(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<&'static str, Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile.schema(self.source, function, budget)
    }
    pub(in crate::cleanup_plan::replay) fn call_is_total(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.validate(context, function, budget)?;
        Ok(matches!(
            self.profile
                .call(self.source, function, expression, budget)?,
            schema::borrowed::Call::Builtin(
                schema::borrowed::operands::Builtin::Get
                    | schema::borrowed::operands::Builtin::Len
                    | schema::borrowed::operands::Builtin::Str(crate::str_ops::StrOp::LenBytes)
            )
        ))
    }
    pub(in crate::cleanup_plan::replay) fn call_by_id(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<(&ResolvedExpr, schema::borrowed::Call<'_>), Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile.call_by_id(self.source, function, id, budget)
    }
    pub(in crate::cleanup_plan::replay) fn call_params(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        work: &mut SkeletonWork<'_, '_>,
    ) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
        self.validate(context, function, work.budget)?;
        use schema::borrowed::operands::Builtin;
        match self
            .profile
            .call(self.source, function, expression, work.budget)?
        {
            schema::borrowed::Call::User(target) => {
                work.budget.reserve_array::<crate::hir::ResolvedParam>(
                    function,
                    target.params.len(),
                    "borrowed actual callee parameter clone",
                )?;
                for parameter in &target.params {
                    work.charge(1, "borrowed actual parameter backing visit")?;
                    work.budget.reserve_bytes(
                        function,
                        parameter.name.len(),
                        "borrowed actual parameter name backing",
                    )?;
                    work.charge(1, "borrowed actual parameter clone header")?;
                    for _ in parameter.name.bytes() {
                        work.charge(1, "borrowed actual parameter name clone byte")?;
                    }
                    work.charge(1, "borrowed scalar or view parameter type clone")?;
                }
                Ok(target.params.clone())
            }
            schema::borrowed::Call::Builtin(Builtin::FromU8) => {
                let op = crate::string_ops::StringOp::I64FromU8;
                typed_storage::borrowed_static_params(
                    op.id(),
                    op.param_types(),
                    op.param_names(),
                    |i, _| op.param_ownership(i),
                    work,
                )
            }
            schema::borrowed::Call::Builtin(operation @ (Builtin::Get | Builtin::Len)) => {
                let op = match operation {
                    Builtin::Get => crate::byte_ops::ByteOp::Get,
                    _ => crate::byte_ops::ByteOp::Len,
                };
                typed_storage::borrowed_static_params(
                    op.id(),
                    op.param_types(),
                    op.param_names(),
                    |i, _| op.param_ownership(i),
                    work,
                )
            }
            schema::borrowed::Call::Builtin(Builtin::Str(op)) => {
                typed_storage::borrowed_static_params(
                    op.id(),
                    op.param_types(),
                    op.param_names(),
                    |_, _| OwnershipMode::Borrow,
                    work,
                )
            }
        }
    }
    pub(in crate::cleanup_plan::replay) fn no_drop(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        ty: &ResolvedType,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile.no_drop(self.source, function, ty, budget)
    }
    pub(in crate::cleanup_plan::replay) fn match_shape(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile
            .match_shape(self.source, function, expression, budget)
    }
    pub(in crate::cleanup_plan::replay) fn inventory(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile
            .validate_inventory(self.source, function, budget)?;
        budget.charge(function, 1, "borrowed plan empty ownership headers")?;
        let plan = &function.cleanup_plan;
        if !plan.slots.is_empty()
            || !plan.entry_state.live_owned_parameters.is_empty()
            || !plan.entry_state.conditional_owned_parameters.is_empty()
        {
            return Err(replay_error(
                function,
                "nonowning helper plan claims owned storage",
            ));
        }
        Ok(())
    }
}
