//! Independent canonical replay retains each rebuilt origin once. Endpoint
//! ordinals are private access data, never source-plan or validation authority.
//! Caller-owned validation storage must fund the directory; this pure receiver
//! neither creates a physical ledger nor changes the conservative forecast.
use super::*;

#[derive(Clone)]
pub(super) struct Inventory {
    pub(super) ids: BTreeMap<(ExpressionId, LoanCause), LoanId>,
    pub(super) liveness: BTreeMap<(ExpressionId, LoanPointPhase, LoanId), usize>,
    pub(super) origins: Vec<Place>,
}

impl Inventory {
    pub(super) fn rebuild(program: &ResolvedProgram) -> Result<Self, Diagnostic> {
        #[cfg(test)]
        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(
            "hir::canonical_loans::Inventory::rebuild",
        );
        let mut inventory = Self {
            ids: BTreeMap::new(),
            liveness: BTreeMap::new(),
            origins: Vec::new(),
        };
        for function in program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            // Reconstruct independently from the exact current body. Attached
            // plans remain checked by the ordinary validator, not trusted here.
            // The entry baseline includes every earlier function's canonical
            // inventory. Its measured peak includes this function's planner.
            #[cfg(test)]
            let _allocation_phase = crate::test_allocation_observer::function_phase_guard(
                "hir::canonical_loans::Inventory::next_function",
            );
            inventory.append(crate::loan_plan::build_plan(program, function)?)?;
        }
        Ok(inventory)
    }

    fn append(&mut self, plan: crate::loan_plan::LoanPlan) -> Result<(), Diagnostic> {
        #[cfg(test)]
        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(
            "hir::canonical_loans::Inventory::append",
        );
        let base = self.origins.len();
        let origins = plan
            .loans
            .iter()
            .enumerate()
            .map(|(offset, loan)| {
                base.checked_add(offset)
                    .map(|ordinal| (loan.id, ordinal))
                    .ok_or_else(|| hir_error("canonical shared-loan origin directory overflows"))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        for endpoint in &plan.endpoints {
            let live = match endpoint.point.phase {
                LoanPointPhase::Before => &endpoint.live_before,
                LoanPointPhase::After => &endpoint.live_after,
            };
            for loan in live {
                let ordinal = origins.get(loan).copied().ok_or_else(|| {
                    hir_error("canonical shared-loan endpoint names an unknown loan")
                })?;
                if self
                    .liveness
                    .insert(
                        (
                            endpoint.point.expression.clone(),
                            endpoint.point.phase,
                            *loan,
                        ),
                        ordinal,
                    )
                    .is_some()
                {
                    return Err(hir_error("canonical shared-loan liveness is duplicated"));
                }
            }
        }
        for loan in plan.loans {
            if loan.start.phase != LoanPointPhase::Before
                || self.ids.insert((loan.site, loan.cause), loan.id).is_some()
            {
                return Err(hir_error(
                    "canonical shared-loan identity is ambiguous or malformed",
                ));
            }
            // Move the exact independently rebuilt Place, including every
            // projection and text holder; no endpoint clones its payload.
            self.origins.push(loan.origin);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// The ordinary compatibility constructor retains the saved owned directories.
// Production caller storage lends only immutable exact-subject lookup metadata.
#[derive(Clone)]
// The compatibility branch preserves its existing inline owned inventory;
// boxing it would add an unpriced allocation to old constructors.
#[allow(clippy::large_enum_variant)]
pub(super) enum Lookup<'a> {
    #[cfg(test)]
    Legacy(Inventory),
    Owned(std::rc::Rc<super::super::canonical_loan_storage::Storage<'a>>),
    UnfundedShared(&'a ResolvedProgram, std::rc::Rc<Inventory>),
    Borrowed(&'a super::super::canonical_loan_storage::Storage<'a>),
}
impl<'a> Lookup<'a> {
    pub(super) fn id(&self, key: &(ExpressionId, LoanCause)) -> Option<&LoanId> {
        match self {
            #[cfg(test)]
            Self::Legacy(i) => i.ids.get(key),
            Self::Owned(i) => i.id(key),
            Self::UnfundedShared(_, i) => i.ids.get(key),
            Self::Borrowed(i) => i.id(key),
        }
    }
    pub(super) fn live(&self, key: &(ExpressionId, LoanPointPhase, LoanId)) -> Option<&usize> {
        match self {
            #[cfg(test)]
            Self::Legacy(i) => i.liveness.get(key),
            Self::Owned(i) => i.live(key),
            Self::UnfundedShared(_, i) => i.liveness.get(key),
            Self::Borrowed(i) => i.live(key),
        }
    }
    pub(super) fn origin(&self, index: usize) -> Option<&Place> {
        match self {
            #[cfg(test)]
            Self::Legacy(i) => i.origins.get(index),
            Self::Owned(i) => i.origin(index),
            Self::UnfundedShared(_, i) => i.origins.get(index),
            Self::Borrowed(i) => i.origin(index),
        }
    }
    pub(super) fn shared(&self) -> Option<&'a super::super::canonical_loan_storage::Storage<'a>> {
        match self {
            #[cfg(test)]
            Self::Legacy(_) => None,
            Self::Owned(_) | Self::UnfundedShared(..) => None,
            Self::Borrowed(i) => Some(*i),
        }
    }
}

impl<'a> Lookup<'a> {
    pub(super) fn current(
        seed: &constructor_seed::ConstructorSeed<'a>,
    ) -> Result<Self, Diagnostic> {
        if crate::bounded_output::active_remaining().is_some() {
            return super::super::canonical_loan_storage::caller::prepare_shared(seed)
                .map(Self::Owned);
        }
        // Honest ordinary unobserved compatibility. No ledger/authority is
        // invented; later clones share existing backing rather than acquire it.
        let inventory = Inventory::rebuild(seed.program())?;
        Ok(Self::UnfundedShared(
            seed.program(),
            std::rc::Rc::new(inventory),
        ))
    }
    pub(super) fn is_current(&self, program: &ResolvedProgram) -> bool {
        match self {
            #[cfg(test)]
            Self::Legacy(_) => false,
            Self::Borrowed(storage) => storage.is_current(program),
            Self::Owned(storage) => storage.is_current(program),
            Self::UnfundedShared(subject, _) => std::ptr::eq(*subject, program),
        }
    }
    pub(super) fn share(&self) -> Option<Self> {
        match self {
            #[cfg(test)]
            Self::Legacy(_) => None,
            Self::Borrowed(_) => self.shared().map(Self::Borrowed),
            Self::Owned(storage) => Some(Self::Owned(std::rc::Rc::clone(storage))),
            Self::UnfundedShared(program, inventory) => {
                Some(Self::UnfundedShared(program, std::rc::Rc::clone(inventory)))
            }
        }
    }
}

#[cfg(test)]
mod constructors_before;

#[cfg(test)]
mod ownership_tests;
