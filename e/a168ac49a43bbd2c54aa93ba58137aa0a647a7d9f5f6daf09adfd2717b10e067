//! Invocation-local borrowed callee inventory. Sorting moves references only;
//! bodies, signatures and declaration identities remain owned by the held provider.
use super::*;
use crate::cleanup::ReplayContext;

pub(super) struct Context<'a> {
    source: &'a dyn ReplayContext,
    targets: Vec<&'a ResolvedFunction>,
}
impl<'a> Context<'a> {
    #[cfg(test)]
    pub(super) fn storage_identity(&self) -> *const &'a ResolvedFunction {
        self.targets.as_ptr()
    }
    #[cfg(test)]
    pub(super) fn storage_members(&self) -> &[&'a ResolvedFunction] {
        &self.targets
    }

    pub(super) fn new(
        source: &'a dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        if !budget.selected()
            || !source.function_templates().is_empty()
            || !source.function_instances().is_empty()
        {
            return Err(budget.budget_refusal(
                function,
                "selected callee index requires a reserved monomorphic context",
                "",
            ));
        }
        budget.reserve_array::<Self>(function, 1, "selected callee index owner")?;
        let mut targets = Vec::new();
        budget.reserve_vec(
            function,
            &mut targets,
            source.function_count(),
            "selected borrowed callee references",
        )?;
        for index in 0..source.function_count() {
            let target = source.function_at(index).ok_or_else(|| {
                budget.budget_refusal(function, "selected callee member index is incomplete", "")
            })?;
            budget.charge(function, 1, "selected callee inventory member")?;
            targets.push(target);
        }
        // Fallible in-place heapsort: no sort scratch allocation and no work
        // after the first exhausted comparison debit. Original source order stays intact.
        for root in (0..targets.len() / 2).rev() {
            sift(
                &mut targets,
                root,
                source.function_count(),
                function,
                budget,
            )?;
        }
        for end in (1..targets.len()).rev() {
            targets.swap(0, end);
            sift(&mut targets, 0, end, function, budget)?;
        }
        for pair in targets.windows(2) {
            let work = pair[0]
                .id
                .as_str()
                .len()
                .min(pair[1].id.as_str().len())
                .checked_add(1)
                .ok_or_else(|| {
                    budget.budget_refusal(
                        function,
                        "selected callee uniqueness work overflowed",
                        "",
                    )
                })?;
            budget.charge(function, work, "selected callee unique identity")?;
            if pair[0].id == pair[1].id {
                return Err(budget.budget_refusal(
                    function,
                    "selected callee inventory repeats a declaration identity",
                    "",
                ));
            }
        }
        Ok(Self { source, targets })
    }
}
fn less(
    left: &ResolvedFunction,
    right: &ResolvedFunction,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let work = left
        .id
        .as_str()
        .len()
        .min(right.id.as_str().len())
        .checked_add(1)
        .ok_or_else(|| {
            budget.budget_refusal(function, "selected callee comparison work overflowed", "")
        })?;
    budget.charge(function, work, "selected callee identity comparison")?;
    Ok(left.id < right.id)
}
fn sift(
    targets: &mut [&ResolvedFunction],
    mut root: usize,
    end: usize,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    while let Some(child) = root
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .filter(|n| *n < end)
    {
        let mut greatest = child;
        if child + 1 < end && less(targets[child], targets[child + 1], function, budget)? {
            greatest += 1;
        }
        if !less(targets[root], targets[greatest], function, budget)? {
            break;
        }
        targets.swap(root, greatest);
        root = greatest;
    }
    Ok(())
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
        if instance.is_some() {
            return None;
        }
        self.targets
            .binary_search_by(|target| target.id.cmp(callee))
            .ok()
            .map(|at| self.targets[at])
    }
}

#[cfg(test)]
mod tests {
    use super::{Context, ReplayContext};
    use crate::cleanup_plan::replay::{
        selected_context, AdmissionMode, AllocationPayer, ReplayBudget,
    };
    use crate::cleanup_plan::ReplayPhysicalReservation;
    fn program() -> crate::hir::ResolvedProgram {
        let source = crate::check("module callees; @id(\"z.last\") fn last()->i64 {7} @id(\"app.main\") fn main()->i64 {last()} @id(\"mid.third\") fn third()->i64 {9}", "selected-callees.spx").unwrap();
        crate::hir::resolve(&source).unwrap()
    }
    fn budget(bytes: usize) -> ReplayBudget {
        let mut budget = ReplayBudget::new();
        budget.admission = AdmissionMode::SelectedRetained;
        budget.payer = AllocationPayer::SelectedReservation;
        budget.physical = Some(Box::new(ReplayPhysicalReservation::new(bytes)));
        budget
    }
    #[test]
    fn exact_borrowed_inventory_preserves_bodies_order_and_one_short_refuses() {
        let program = program();
        let function = &program.functions[0];
        let mut measured = budget(usize::MAX);
        let indexed = Context::new(&program, function, &mut measured).unwrap();
        for target in &program.functions {
            assert!(std::ptr::eq(
                indexed.resolve_call_target(&target.id, None).unwrap(),
                target
            ));
        }
        assert_eq!(
            indexed
                .functions()
                .get(0)
                .expect("current replay ordinal")
                .id
                .as_str(),
            "z.last"
        );
        assert!(indexed.targets.windows(2).all(|p| p[0].id < p[1].id));
        let charged = measured.physical.as_ref().unwrap().charged();
        let mut exact = budget(charged);
        Context::new(&program, function, &mut exact).unwrap();
        assert_eq!(exact.physical.as_ref().unwrap().charged(), charged);
        let mut short = budget(charged - 1);
        assert_eq!(
            Context::new(&program, function, &mut short)
                .err()
                .unwrap()
                .code,
            "SPX-H006"
        );
        assert!(short.physical.as_mut().unwrap().reserve(0).is_err());
    }
    #[test]
    fn duplicate_declaration_and_stale_function_context_never_supply_authority() {
        let mut program = program();
        let main = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let mut reserved = budget(usize::MAX);
        let primitive = super::super::primitive_fixture::Context::new(&program);
        let indexed = Context::new(&primitive, &main, &mut reserved).unwrap();
        let error = selected_context::Context::new(&indexed, &main, &mut reserved)
            .err()
            .unwrap();
        assert!(
            error.message.contains("not held by its current context"),
            "equal bytes and identity do not replace current membership"
        );
        drop(indexed);
        let duplicate = program.functions[0].id.clone();
        program.functions[1].id = duplicate;
        assert_eq!(
            Context::new(&program, &program.functions[0], &mut budget(usize::MAX))
                .err()
                .unwrap()
                .code,
            "SPX-H006"
        );
    }
    #[test]
    fn exhausted_sort_work_never_publishes_a_partial_lookup_inventory() {
        let program = program();
        let function = &program.functions[0];
        let mut reserved = budget(usize::MAX);
        reserved.remaining = program.functions.len();
        let error = Context::new(&program, function, &mut reserved)
            .err()
            .unwrap();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("callee identity comparison"));
        assert_eq!(reserved.remaining, 0);
        // Sorting only touched borrowed references in unpublished local storage.
        assert_eq!(program.functions[0].id.as_str(), "z.last");
    }
}
