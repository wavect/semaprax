//! One existing replay payer for classifier constructors and their receiving
//! lifetime. No work allowance, fallback quota or fact authority is introduced.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::hir::{DeclarationIndex, TypeFactsPayer};

pub(super) struct Payer<'a> {
    function: &'a ResolvedFunction,
    budget: &'a mut ReplayBudget,
    ledger: ClassifierLedger,
}
impl Payer<'_> {
    fn current(&mut self) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if let Some(lease) = self.budget.oracle_lease() {
            if matches!(allocation::authority_for_function(self.function), Ok(Some(held)) if held == lease)
            {
                return Ok(());
            }
            lease.fail_physical("classifier account payer differs from its live invocation");
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "oracle invocation authority refused a classifier payer",
            ));
        }
        if self.ledger.current() {
            return Ok(());
        }
        Err(self.budget.budget_refusal(
            self.function,
            "cleanup replay classifier payer is outside its current invocation",
            "",
        ))
    }
}
impl TypeFactsPayer for Payer<'_> {
    #[cfg(test)]
    fn classifier_sink_is_explicit(&self) -> bool {
        self.budget.oracle_lease().is_some()
    }

    #[cfg(test)]
    fn classifier_reserve_sink(&mut self, bytes: usize) -> Result<bool, Diagnostic> {
        self.current()?;
        if self.budget.oracle_lease().is_none() {
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "explicit classifier sink requires its account payer",
            ));
        }
        // Logical text and replacement backing share the SAME finite account,
        // just as the ordinary paid receiver reserves both against its ledger.
        // Account exhaustion is a typed sticky refusal, never partial key text.
        self.budget
            .reserve_bytes(self.function, bytes, "type-fact classifier logical text")?;
        Ok(true)
    }
    // Claude-authored account-only override: ordinary paths do no added work,
    // ledger access or preparation. Zero still checks the exact live authority.
    #[cfg(test)]
    fn oracle_work(&mut self, units: usize, phase: &'static str) -> Result<(), Diagnostic> {
        if self.budget.oracle_lease().is_none() {
            return Ok(());
        }
        self.current()?;
        self.budget.charge_oracle_work(self.function, units, phase)
    }
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.current()?;
        self.budget
            .reserve_bytes(self.function, bytes, "type-fact classifier backing")
    }
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        self.current()?;
        self.budget.reserve_vec(
            self.function,
            values,
            additional,
            "type-fact classifier vector",
        )
    }
    fn node<K, V>(&mut self) -> Result<(), Diagnostic> {
        self.current()?;
        self.budget
            .reserve_btree_node::<K, V>(self.function, "type-fact classifier ordered tree")
    }
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        self.current()?;
        #[cfg(test)]
        self.oracle_work(1, "classifier clone receive")?;
        let owns_backing = match ty {
            ResolvedType::Nominal { arguments, .. } => !arguments.is_empty(),
            ResolvedType::Function { .. } => true,
            _ => false,
        };
        if !owns_backing {
            self.bytes(0)?;
            return Ok(ty.clone());
        }
        // The original classifier has no new depth/census quota. Use a real
        // paid ancestor cursor instead of converting a fixed optimization
        // census bound into admission refusal for a wider/deeper current type.
        self.bytes(std::mem::size_of::<Vec<(&ResolvedType, usize)>>())?;
        let mut pending = Vec::new();
        self.grow(&mut pending, 1)?;
        pending.push((ty, 0usize));
        while let Some((current, child)) = pending.last().copied() {
            #[cfg(test)]
            self.oracle_work(1, "classifier clone census frame")?;
            if child == 0 {
                #[cfg(test)]
                self.oracle_work(1, "classifier cloned type node")?;
                let bytes = match current {
                    ResolvedType::Nominal { arguments, .. } => arguments
                        .len()
                        .checked_mul(std::mem::size_of::<ResolvedType>()),
                    ResolvedType::Function { parameters, .. } => parameters
                        .len()
                        .checked_mul(std::mem::size_of::<ResolvedType>())
                        .and_then(|n| n.checked_add(std::mem::size_of::<ResolvedType>())),
                    _ => Some(0),
                }
                .ok_or_else(|| {
                    self.construction_refusal("cleanup replay classifier clone layout overflowed")
                })?;
                self.bytes(bytes)?;
            }
            let next = match current {
                ResolvedType::Nominal { arguments, .. } => arguments.get(child),
                ResolvedType::Function { parameters, result } => {
                    if child == parameters.len() {
                        Some(result.as_ref())
                    } else {
                        parameters.get(child)
                    }
                }
                _ => None,
            };
            if let Some(next) = next {
                pending.last_mut().expect("classifier ancestor present").1 += 1;
                self.grow(&mut pending, 1)?;
                pending.push((next, 0));
            } else {
                pending.pop();
            }
        }
        Ok(ty.clone())
    }
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic> {
        self.bytes(std::mem::size_of::<ResolvedType>())?;
        Ok(Box::new(ty))
    }
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic {
        #[cfg(test)]
        if let Some(lease) = self.budget.oracle_lease() {
            lease.fail_physical(message);
        }
        self.budget.budget_refusal(self.function, message, "")
    }
}

pub(super) fn flags(
    index: &DeclarationIndex,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<Option<crate::hir::TypeFlags>, Diagnostic> {
    #[cfg(test)]
    if SAVED.with(std::cell::Cell::get) {
        if let Some(lease) = budget.oracle_lease() {
            lease.fail_physical("independent saved classifier is not an account-funded receiver");
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "saved classifier receiver has no oracle-account payer",
            ));
        }
        return Ok(index.type_flags(ty));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::flags(index, function, ty, budget);
    }
    index.type_flags_paid(
        ty,
        Payer {
            function,
            budget,
            ledger: ClassifierLedger::capture(),
        },
    )
}

mod droppability;
pub(super) fn needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    if SAVED.with(std::cell::Cell::get) {
        if let Some(lease) = budget.oracle_lease() {
            lease.fail_physical("independent saved classifier is not an account-funded receiver");
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "saved classifier receiver has no oracle-account payer",
            ));
        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::needs_drop(program, function, ty, budget);
    }
    let mut payer = Payer {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
    };
    match droppability::needs_drop_paid(program, ty, &mut payer) {
        Ok(value) => Ok(value),
        Err(droppability::Failure::Physical(error)) => Err(error),
        Err(droppability::Failure::Semantic(message)) => Err(droppability::replay_diagnostic(
            function, &message, &mut payer,
        )?),
    }
}
/// Raw ordinary inventory diagnostic adapter. The classifier's semantic String
/// has already acquired backing from this same payer; moving it into Diagnostic
/// must not append the independent-replay prefix or allocate a second message.
/// This is constructor plumbing only, not a schema-selection/admission route.
pub(super) fn inventory_needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    inventory_text::current(payer)?;
    match droppability::needs_drop_paid(program, ty, payer) {
        Ok(value) => Ok(value),
        Err(droppability::Failure::Physical(error)) => Err(error),
        Err(droppability::Failure::Semantic(message)) => Err(Diagnostic::io("SPX-H006", message)),
    }
}

#[cfg(test)]
thread_local! { static SAVED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
fn with_saved_classifier<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SAVED.with(|saved| saved.set(self.0));
        }
    }
    let _restore = Restore(SAVED.with(|saved| saved.replace(true)));
    run()
}
#[cfg(test)]
pub(super) mod tests;
#[cfg(test)]
mod zero_tests;

#[cfg(test)]
mod oracle_payer_tests {
    use super::*;
    use crate::cleanup_plan::replay::allocation::{enter, OracleRefusal, OracleScope};

    fn program() -> crate::hir::ResolvedProgram {
        crate::hir::resolve(
            &crate::check(
                r#"module classifier.oracle; @id("main") fn main()->i64 {7}"#,
                "classifier-oracle.spx",
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn explicit_classifier_payer_requires_live_subject_and_keeps_first_physical_failure() {
        let program = program();
        let function = &program.functions[0];
        let scope = OracleScope::open(1 << 20).unwrap();
        let lease = scope.lease();
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        let mut payer = Payer {
            function,
            budget: &mut budget,
            ledger: ClassifierLedger::capture(),
        };
        assert_eq!(payer.bytes(0), Ok(()));
        let _ = payer.construction_refusal("first classifier physical failure");
        let _ = payer.construction_refusal("second classifier physical failure");
        assert_eq!(
            scope.receipt().physical_failure,
            Some("first classifier physical failure")
        );
        assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
        assert_eq!(invocation.exit(), Ok(()));
        drop(scope);
        let scope = OracleScope::open(1 << 20).unwrap();
        let mut budget =
            ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, scope.lease());
        let mut payer = Payer {
            function,
            budget: &mut budget,
            ledger: ClassifierLedger::capture(),
        };
        assert!(
            payer.bytes(0).is_err(),
            "an account without its live invocation is refused"
        );
        assert!(scope.receipt().physical_failure.is_some());
        assert_eq!(scope.receipt().charged, 0);
    }

    #[test]
    fn complete_saved_receiver_cannot_be_misrepresented_as_account_funded() {
        let program = program();
        let function = &program.functions[0];
        let scope = OracleScope::open(1 << 20).unwrap();
        let lease = scope.lease();
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        let result = with_saved_classifier(|| {
            needs_drop(&program, function, &ResolvedType::I64, &mut budget)
        });
        assert!(result.is_err());
        assert!(scope.receipt().physical_failure.is_some());
        assert_eq!(scope.receipt().charged, 0);
        assert_eq!(invocation.exit(), Ok(()));
    }
}

#[cfg(test)]
mod oracle_logical_tests;

#[cfg(test)]
mod oracle_work_tests;

#[cfg(test)]
mod inventory_tests;

mod inventory_substitute;

mod inventory_clone;

#[cfg(test)]
mod inventory_substitute_tests;

mod inventory_text;

mod inventory_profile;

mod inventory_metadata;

mod inventory_nested;

mod inventory_encoded;

mod inventory_ordered;

mod inventory_flags;

mod inventory_raw_key;

mod inventory_drop;

mod inventory_lifecycle;

pub(super) mod inventory_summary;

#[cfg(test)]
pub(super) mod oracle_inventory;
