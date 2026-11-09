//! Budget-charged skeleton path materialization for the typed-HIR replay.
//!
//! Path observations are shared behind `Rc`, so sequencing a prefix with each
//! suffix copies pointers rather than deep-copying every identity and place.
//! Existing work events remain; ordinary construction ordinals and exact memo
//! pointer checks add explicit work, and new ordinary DAG storage is prepaid.
use super::*;
pub(super) mod dag_storage;
pub(super) mod enumerated_prefix;
#[cfg(test)]
mod isolated_paths;
pub(super) mod merge_keys;
#[cfg(test)]
pub(super) mod merge_keys_878;
#[cfg(test)]
mod merge_keys_before;
#[cfg(test)]
pub(super) mod merge_keys_tests;
pub(super) mod merge_search;
#[cfg(test)]
mod oracle_entry;
mod ordinary_dag;
pub(super) mod prefix_fusion;
pub(super) mod receiving;

#[cfg(test)]
mod constructor_funding_tests {
    use super::*;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;

    fn program(source: &str) -> ResolvedProgram {
        let checked = crate::check(source, "constructor-funding.spx").unwrap();
        let canonical = crate::format::canonical(&checked);
        let checked = crate::check(&canonical, "constructor-funding.spx").unwrap();
        crate::hir::resolve(&checked).unwrap()
    }

    #[test]
    fn ordinary_constructor_matches_complete_previous_collectors_and_order() {
        for source in [
            "module constructor.simple; @id(\"main\") fn main()->i64 {0}",
            include_str!("../../../tests/owned_data/byte_helper_renewal.spx"),
            r#"module constructor.moving;
@id("main") fn main()->i64 {
 let mut text=string_from_i64(1);
 let mut count=0;
 while count<2 {text=string_concat(text,"x"); count=count+1; 0}
 string_len(text)
}"#,
        ] {
            let program = program(source);
            with_limit_usage_refusal(usize::MAX, || {
                let held = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
                for function in program.functions.iter().chain(
                    program
                        .function_instances
                        .iter()
                        .map(|instance| &instance.function),
                ) {
                    for index in [None, Some(&held)] {
                        let mut old_budget =
                            ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut new_budget =
                            ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let old = SkeletonWork::with_index_before_funding(
                            &program,
                            function,
                            &mut old_budget,
                            index,
                        )
                        .unwrap();
                        let new =
                            SkeletonWork::with_index(&program, function, &mut new_budget, index)
                                .unwrap();
                        assert_eq!(new.string_owner_moves, old.string_owner_moves);
                        assert_eq!(new.string_condition_reads, old.string_condition_reads);
                        assert_eq!(new.owner, old.owner);
                        assert_eq!(
                            new.renewal_bindings.has_helpers(),
                            old.renewal_bindings.has_helpers()
                        );
                        assert_eq!(
                            new.renewal_bindings.appends(),
                            old.renewal_bindings.appends()
                        );
                        crate::hir::function_value::walk(function, |expression| {
                            assert_eq!(
                                new.renewal_bindings
                                    .get(&expression.id)
                                    .map(|binding| &binding.id),
                                old.renewal_bindings
                                    .get(&expression.id)
                                    .map(|binding| &binding.id)
                            );
                            assert_eq!(
                                new.renewal_bindings.is_helper(&expression.id),
                                old.renewal_bindings.is_helper(&expression.id)
                            );
                        });
                        assert_eq!(new.budget.remaining, old.budget.remaining);
                        assert_eq!(new.budget.skeleton_remaining, old.budget.skeleton_remaining);
                    }
                }
            });
        }
    }

    #[test]
    fn ordinary_constructor_prepays_real_scratch_at_zero_short_exact_and_sticky_limits() {
        let program = program("module constructor.physical; @id(\"main\") fn main()->i64 {0}");
        let function = &program.functions[0];
        // Execute the full old constructor as a distinct historical witness.
        // It acquired traversal backing before a single reservation at zero.
        let ((accepted, old_heap), refused, paid, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let result =
                    SkeletonWork::with_index_before_funding(&program, function, &mut budget, None);
                let accepted = result.is_ok();
                drop(result);
                drop(budget);
                accepted
            })
        });
        assert!(accepted && !refused && paid == 0 && first.is_none());
        assert!(old_heap.allocated > 0 && old_heap.live == 0 && !old_heap.invalid);
        assert_eq!(old_heap.allocated, old_heap.freed);
        // The old constructor did not fail here. Compare all six Diagnostic
        // fields to the independently executed, byte-exact original bootstrap
        // that now lawfully selects the first failure before any collector.
        let (expected_zero, refused, paid, first) = with_limit_usage_refusal(0, || {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.prepare_builder(function).unwrap_err()
        });
        assert!(refused && paid == 0 && first.is_some());
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let result = SkeletonWork::with_index(&program, function, &mut budget, None);
                    let accepted = result.is_ok();
                    if limit == 0 {
                        assert_eq!(result.as_ref().err(), Some(&expected_zero));
                    }
                    drop(result);
                    if !accepted {
                        let after_first = crate::bounded_output::active_remaining();
                        assert!(
                            SkeletonWork::with_index(&program, function, &mut budget, None)
                                .is_err()
                        );
                        assert_eq!(crate::bounded_output::active_remaining(), after_first);
                    }
                    drop(budget);
                    accepted
                })
            })
        };
        let ((accepted, heap), refused, exact, first) = observe(usize::MAX);
        assert!(accepted && !refused && first.is_none());
        assert!(!heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
        assert!(heap.allocated <= exact);
        for limit in [0, exact - 1, exact] {
            let ((accepted, heap), refused, paid, first) = observe(limit);
            assert_eq!(accepted, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            assert!(!heap.invalid && heap.live == 0);
            assert_eq!(heap.allocated, heap.freed);
            assert!(
                heap.allocated <= paid,
                "limit={limit} exact={exact} paid={paid} heap={heap:?} first={first:?}"
            );
            if limit == 0 {
                assert_eq!(heap.allocated, 0);
            }
        }
    }
}

pub(super) struct SkeletonWork<'a, 'b> {
    pub(super) continuations: Option<&'b mut continuation_storage::Storage<'a>>,
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
    pub(super) renewal_bindings: crate::cleanup_plan::renewal_profile::RenewalBindings<'a>,
    pub(super) active_copy_region: Option<ExpressionId>,
    pub(super) dag_scratch:
        Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    pub(super) ordinary_scratch:
        Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    pub(super) next_ordinary_dag: usize,
    pub(super) held_renewals: Option<&'a crate::cleanup_plan::renewal_profile::RenewalBindings<'a>>,
    pub(super) ordinary_index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    pub(super) ordinary_program: Option<&'a dyn crate::cleanup::ReplayContext>,
    /// Authored same-owner append and direct literal-format String moves.
    pub(super) string_owner_moves: BTreeSet<ExpressionId>,
    pub(super) string_condition_reads: BTreeSet<ExpressionId>,
    /// Authenticated execution identity of a lexical-profile function.
    pub(super) owner: Option<crate::hir::FunctionExecutionId>,
}

impl<'a, 'b> SkeletonWork<'a, 'b> {
    /// Release test-owned observation scratch in the original field order.
    #[cfg(test)]
    pub(super) fn release_observation_scratch_for_test(&mut self) {
        drop(self.dag_scratch.take());
        drop(self.ordinary_scratch.take());
    }

    #[cfg(test)]
    pub(super) fn new(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
    ) -> Self {
        Self::with_index(program, function, budget, None).expect("unlimited test inventory")
    }
    pub(super) fn with_index(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ) -> Result<Self, Diagnostic> {
        #[cfg(test)]
        if let Some(lease) = budget.oracle_lease() {
            return Self::oracle_with_index(program, function, budget, index, lease);
        }
        use crate::hir::funded::FundedError;
        // The held payer owns refusal text and the exact clone census cursor
        // before any collector may acquire scratch, keys or tree nodes.
        budget.prepare_builder(function)?;
        budget.prepare_clone_cursor(function)?;
        // Concrete new fallible receiving sums, closures and explicit reserve
        // references, before those receivers are formed. No guessed heap fee.
        let controls = 2 * std::mem::size_of::<
            Result<
                crate::cleanup_plan::renewal_profile::RenewalBindings<'_>,
                FundedError<Diagnostic>,
            >,
        >() + 4 * std::mem::size_of::<
            Result<BTreeSet<ExpressionId>, FundedError<Diagnostic>>,
        >() + 2 * std::mem::size_of::<Result<bool, FundedError<Diagnostic>>>()
            + 2 * std::mem::size_of::<
                Result<
                    Result<crate::hir::FunctionExecutionId, crate::hir::lexical_exit::OwnerVerdict>,
                    FundedError<Diagnostic>,
                >,
            >()
            + 2 * std::mem::size_of::<(
                &mut ReplayBudget,
                &ResolvedFunction,
                &crate::bounded_output::classifier_string::ClassifierLedger,
            )>()
            + std::mem::size_of::<crate::bounded_output::classifier_string::ClassifierLedger>()
            + 10 * std::mem::size_of::<&mut dyn FnMut(usize) -> Result<(), Diagnostic>>();
        budget.reserve_array::<u8>(
            function,
            controls,
            "ordinary skeleton constructor receivers",
        )?;
        let ledger = crate::bounded_output::classifier_string::ClassifierLedger::capture();
        let mut reserve = |bytes| {
            if !ledger.current() {
                return Err(budget.budget_refusal(
                    function,
                    "ordinary replay setup payer is outside its current invocation",
                    "",
                ));
            }
            budget.reserve_bytes(function, bytes, "ordinary skeleton constructor inventory")
        };
        macro_rules! inventory {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    // Forward the already-selected Diagnostic and paid backing.
                    Err(FundedError::Refused(error)) => return Err(error),
                    Err(
                        FundedError::Layout(reason)
                        | FundedError::Census(reason)
                        | FundedError::Unfunded(reason),
                    ) => {
                        return Err(budget.budget_refusal(function, reason, ""));
                    }
                }
            };
        }
        let renewal_bindings = if index.is_some() {
            crate::cleanup_plan::renewal_profile::RenewalBindings::empty()
        } else {
            inventory!(
                crate::cleanup_plan::renewal_profile::bindings_with_index_funded(
                    program,
                    function,
                    None,
                    &mut reserve
                )
            )
        };
        let string_owner_moves = inventory!(
            crate::literal_format::checked_moving_string_operands_funded(function, &mut reserve)
        );
        let string_condition_reads = inventory!(
            crate::string_ops::conditions::function_reads_funded(function, &mut reserve)
        );
        let requires = inventory!(crate::hir::lexical_exit::requires_with_funded_cursor(
            function,
            &mut reserve
        ));
        let owner = if requires {
            inventory!(crate::hir::lexical_exit::owner_funded(
                program,
                function,
                &mut reserve
            ))
            .ok()
        } else {
            None
        };
        Ok(Self {
            function,
            budget,
            continuations: None,
            active_copy_region: None,
            dag_scratch: None,
            ordinary_scratch: None,
            next_ordinary_dag: 0,
            held_renewals: None,
            ordinary_index: index,
            ordinary_program: Some(program),
            renewal_bindings,
            string_owner_moves,
            string_condition_reads,
            owner,
        })
    }

    // Complete ordinary previous constructor, independent test control.
    #[cfg(test)]
    pub(super) fn with_index_before_funding(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ) -> Result<Self, Diagnostic> {
        #[cfg(test)]
        if let Some(lease) = budget.oracle_lease() {
            return Self::oracle_with_index(program, function, budget, index, lease);
        }
        Ok(Self {
            function,
            budget,
            continuations: None,
            active_copy_region: None,
            dag_scratch: None,
            ordinary_scratch: None,
            next_ordinary_dag: 0,
            held_renewals: None,
            ordinary_index: index,
            ordinary_program: Some(program),
            renewal_bindings: if index.is_some() {
                crate::cleanup_plan::renewal_profile::RenewalBindings::empty()
            } else {
                crate::cleanup_plan::renewal_profile::bindings_with_index(program, function, None)
            },
            string_owner_moves: crate::literal_format::checked_moving_string_operands(function)?,
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
            owner: if super::lexical_cert::requires_lexical(function) {
                crate::hir::lexical_exit::owner(program, function).ok()
            } else {
                None
            },
        })
    }

    #[cfg(test)]
    fn ordinary_with_index(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ) -> Result<Self, Diagnostic> {
        let prepared = ordinary_entry::prepare(program, function, budget, index.is_some())?;
        Ok(Self {
            function,
            budget,
            continuations: None,
            active_copy_region: None,
            dag_scratch: None,
            ordinary_scratch: None,
            next_ordinary_dag: 0,
            held_renewals: None,
            ordinary_index: index,
            ordinary_program: Some(program),
            renewal_bindings: prepared.renewals,
            string_owner_moves: prepared.moves,
            string_condition_reads: prepared.reads,
            owner: prepared.owner,
        })
    }

    pub(super) fn checked_with_index(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ) -> Result<Self, Diagnostic> {
        if !budget.selected() {
            return Self::with_index(program, function, budget, index);
        }
        selected_profile::validate(program, function, budget)?;
        let owner = selected_owner::owner(program, function, budget)?;
        let held_renewals = program.primitive_renewals(function).ok_or_else(|| {
            replay_error(
                function,
                "selected skeleton requires its independently rebuilt phase site inventory",
            )
        })?;
        // Literal formatting remains closed. Current String condition reads
        // are rebuilt under this same ledger before their index is published.
        let string_condition_reads = selected_profile::conditions::reads(function, budget)?;
        let string_owner_moves = selected_profile::conditions::moves(function, budget)?;
        Ok(Self {
            function,
            budget,
            continuations: None,
            renewal_bindings: crate::cleanup_plan::renewal_profile::RenewalBindings::empty(),
            active_copy_region: None,
            dag_scratch: None,
            ordinary_scratch: None,
            next_ordinary_dag: 0,
            held_renewals: Some(held_renewals),
            ordinary_index: None,
            ordinary_program: None,
            string_owner_moves,
            string_condition_reads,
            owner: Some(owner),
        })
    }

    pub(super) fn renewal_binding(
        &mut self,
        at: &ExpressionId,
    ) -> Result<Option<&'a crate::hir::ResolvedBinding>, Diagnostic> {
        if let Some(sites) = self.held_renewals {
            return Ok(sites.get(at));
        }
        if let (Some(index), Some(program)) = (self.ordinary_index, self.ordinary_program) {
            return ordinary_renewals::lookup(program, self.function, at, Some(index), self.budget)
                .map(|found| found.map(|(binding, _)| binding));
        }
        Ok(self.renewal_bindings.get(at))
    }
}

impl SkeletonWork<'_, '_> {
    pub(super) fn charge(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.budget.charge_skeleton(self.function, units, phase)
    }

    /// Preserve legacy sequencing/work counters for a previously uncharged
    /// move, while reserving selected vector growth before that move occurs.
    pub(super) fn push_existing_path(
        &mut self,
        target: &mut Vec<ExprSkeletonPath>,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.budget.reserve_vec(self.function, target, 1, phase)?;
        target.push(path);
        Ok(())
    }
    pub(super) fn extend_existing_paths(
        &mut self,
        target: &mut Vec<ExprSkeletonPath>,
        paths: Vec<ExprSkeletonPath>,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.budget
            .reserve_vec(self.function, target, paths.len(), phase)?;
        target.extend(paths);
        Ok(())
    }

    pub(super) fn clone_owned<T: Clone + ClonePayload>(
        &mut self,
        value: &T,
        phase: &str,
    ) -> Result<T, Diagnostic> {
        self.charge(1, phase)?;
        self.budget.reserve_clone(self.function, value, phase)?;
        note_skeleton_materialization();
        Ok(value.clone())
    }

    /// Meter an existing uncharged clone without changing its legacy work count.
    pub(super) fn clone_payload<T: Clone + ClonePayload>(
        &mut self,
        value: &T,
        phase: &str,
    ) -> Result<T, Diagnostic> {
        self.budget.reserve_clone(self.function, value, phase)?;
        Ok(value.clone())
    }

    pub(super) fn push_expr_path(
        &mut self,
        paths: &mut Vec<ExprSkeletonPath>,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        self.budget.reserve_vec(self.function, paths, 1, phase)?;
        paths.push(path);
        Ok(())
    }

    pub(super) fn push_skeleton_path(
        &mut self,
        paths: &mut Vec<SkeletonPath>,
        path: SkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        self.budget.reserve_vec(self.function, paths, 1, phase)?;
        paths.push(path);
        Ok(())
    }

    pub(super) fn singleton_path(
        &mut self,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        let mut paths = Vec::new();
        self.push_expr_path(&mut paths, path, phase)?;
        Ok(paths)
    }

    pub(super) fn clone_expr_path(
        &mut self,
        path: &ExprSkeletonPath,
        phase: &str,
    ) -> Result<ExprSkeletonPath, Diagnostic> {
        self.charge(1, phase)?;
        self.budget.reserve_clone(self.function, path, phase)?;
        note_skeleton_materialization();
        Ok(path.clone())
    }

    pub(super) fn clone_observations(
        &mut self,
        observations: &Observations,
        phase: &str,
    ) -> Result<Observations, Diagnostic> {
        self.charge(1, phase)?;
        self.budget
            .reserve_clone(self.function, observations, phase)?;
        note_skeleton_materialization();
        Ok(observations.clone())
    }

    #[cfg(test)]
    pub(super) fn extend_observations(
        &mut self,
        target: &mut Observations,
        observations: &Observations,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        if self.budget.selected() {
            if target.prefix.is_some() || observations.prefix.is_some() {
                if !self.budget.merge_paths {
                    return Err(replay_error(
                        self.function,
                        "selected lexical replay requires flat observations",
                    ));
                }
                return self.extend_factored_observations(target, observations);
            }
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                observations.items.len(),
                phase,
            )?;
        }
        if self.budget.selected() {
            target.extend(observations);
            Ok(())
        } else {
            self.ordinary_extend(target, observations)
        }
    }

    /// The caller owns this local prefix. Its already-paid tail can transfer
    /// into a sequence node; the borrowed suffix still needs its own copy.
    pub(super) fn extend_owned_observations(
        &mut self,
        mut target: Observations,
        observations: &Observations,
        phase: &str,
    ) -> Result<Observations, Diagnostic> {
        #[cfg(test)]
        if SAVED_EXTENSIONS.with(std::cell::Cell::get) {
            self.extend_observations(&mut target, observations, phase)?;
            return Ok(target);
        }
        self.charge(1, phase)?;
        note_skeleton_materialization();
        if self.budget.selected() {
            if target.prefix.is_some() || observations.prefix.is_some() {
                if !self.budget.merge_paths {
                    return Err(replay_error(
                        self.function,
                        "selected lexical replay requires flat observations",
                    ));
                }
                return self.extend_owned_factored_observations(target, observations);
            }
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                observations.items.len(),
                phase,
            )?;
            target.extend(observations);
            Ok(target)
        } else {
            self.ordinary_extend_owned(target, observations)
        }
    }

    pub(super) fn push_observation(
        &mut self,
        path: &mut ExprSkeletonPath,
        observation: SkeletonObservation,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        if self.budget.selected() && path.observations.prefix.is_some() && !self.budget.merge_paths
        {
            return Err(replay_error(
                self.function,
                "selected lexical replay requires flat observations",
            ));
        }
        self.budget
            .reserve_vec(self.function, &mut path.observations.items, 1, phase)?;
        self.budget
            .reserve_bytes(self.function, physical_clone::observation_rc_bytes(), phase)?;
        let observation = std::rc::Rc::new(observation);
        path.continuation.record(&observation);
        path.observations.push(observation);
        Ok(())
    }
    pub(super) fn insert_front_observation(
        &mut self,
        observations: &mut Observations,
        observation: SkeletonObservation,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.budget
            .reserve_bytes(self.function, physical_clone::observation_rc_bytes(), phase)?;
        if self.budget.selected() && observations.prefix.is_some() {
            if !self.budget.merge_paths {
                return Err(replay_error(
                    self.function,
                    "selected lexical replay requires flat observations",
                ));
            }
            return self.prepend_factored_observation(observations, observation.into());
        }
        self.budget
            .reserve_vec(self.function, &mut observations.items, 1, phase)?;
        if observations.prefix.is_some() {
            self.ordinary_prepend(observations, observation.into())
        } else {
            observations.insert_front(observation.into());
            Ok(())
        }
    }
}

pub(super) fn empty_expr_path() -> ExprSkeletonPath {
    ExprSkeletonPath {
        observations: Observations::default(),
        owned_source: None,
        failed: false,
        residual: false,
        continuation: lexical_skeleton::Continuation::Normal,
    }
}

type Observation = std::rc::Rc<SkeletonObservation>;

/// The observation sequences of one skeleton path.
///
/// Enumerating replay preserves every distinct path. The ordinary-only
/// `enumerated_prefix` route may store immutable sequence prefixes with
/// complete-content paid comparison; legacy/Selected/Oracle paths stay flat.
/// Only factored replay merges paths that carry identical cleanup state;
/// their sequences then become one shared
/// `ObservationDag` prefix denoting the set of all merged sequences, followed
/// by the flat `items` appended afterwards.
#[derive(Clone, Default)]
pub(super) struct Observations {
    pub(super) prefix: Option<std::rc::Rc<ObservationDag>>,
    pub(super) items: Vec<Observation>,
}

/// A set of observation sequences. `Seq` is `prefix` then `items`; `Cat`
/// concatenates its parts in order; `Alt` is the union of its members.
pub(super) struct ObservationDag {
    pub(super) ordinal: Option<usize>,
    pub(super) kind: ObservationDagKind,
    // Immutable structural bound, independent of any payer or scratch session.
    pending_bound: Option<usize>,
    scratch: Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
}

pub(super) enum ObservationDagKind {
    Seq(Option<std::rc::Rc<ObservationDag>>, Vec<Observation>),
    Cat(Vec<std::rc::Rc<ObservationDag>>),
    Alt(Vec<std::rc::Rc<ObservationDag>>),
}

impl Drop for ObservationDag {
    /// Selected DAG nodes share a precharged destruction frontier. No backing
    /// allocation occurs during their destruction, including deeply shared DAGs.
    fn drop(&mut self) {
        if !self.has_children() {
            return;
        }
        if let Some(scratch) = self.scratch.clone() {
            let mut scratch = scratch.borrow_mut();
            self.drain_children(&mut scratch.pending);
            while let Some(child) = scratch.pending.pop() {
                if let Ok(mut node) = std::rc::Rc::try_unwrap(child) {
                    node.drain_children(&mut scratch.pending);
                    // `node` now has no children, so its Drop never reborrows.
                }
            }
            return;
        }
        // Preserve the ordinary factored route's destructor behavior.
        let mut pending = self.take_children();
        while let Some(child) = pending.pop() {
            if let Ok(mut node) = std::rc::Rc::try_unwrap(child) {
                pending.extend(node.take_children());
            }
        }
    }
}

impl ObservationDag {
    pub(super) fn legacy(kind: ObservationDagKind) -> Self {
        Self {
            ordinal: None,
            pending_bound: kind.pending_bound(),
            kind,
            scratch: None,
        }
    }

    fn has_children(&self) -> bool {
        match &self.kind {
            ObservationDagKind::Seq(prefix, _) => prefix.is_some(),
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => !parts.is_empty(),
        }
    }

    fn drain_children(&mut self, pending: &mut Vec<std::rc::Rc<Self>>) {
        match &mut self.kind {
            ObservationDagKind::Seq(prefix, _) => {
                if let Some(child) = prefix.take() {
                    pending.push(child);
                }
            }
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => {
                pending.append(parts);
            }
        }
    }

    fn take_children(&mut self) -> Vec<std::rc::Rc<Self>> {
        match &mut self.kind {
            ObservationDagKind::Seq(prefix, _) => prefix.take().into_iter().collect(),
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => {
                std::mem::take(parts)
            }
        }
    }
}

impl Observations {
    pub(super) fn push(&mut self, observation: Observation) {
        self.items.push(observation);
    }

    pub(super) fn insert_front(&mut self, observation: Observation) {
        match self.prefix.take() {
            None => self.items.insert(0, observation),
            Some(prefix) => {
                let head = std::rc::Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
                    None,
                    vec![observation],
                )));
                self.prefix = Some(std::rc::Rc::new(ObservationDag::legacy(
                    ObservationDagKind::Cat(vec![head, prefix]),
                )));
            }
        }
    }

    /// Append every sequence of `suffix` to every sequence of `self`.
    pub(super) fn extend(&mut self, suffix: &Observations) {
        let Some(suffix_prefix) = &suffix.prefix else {
            self.items.extend_from_slice(&suffix.items);
            return;
        };
        self.prefix = Some(match self.freeze() {
            None => suffix_prefix.clone(),
            Some(head) => std::rc::Rc::new(ObservationDag::legacy(ObservationDagKind::Cat(vec![
                head,
                suffix_prefix.clone(),
            ]))),
        });
        self.items = suffix.items.clone();
    }

    /// The whole sequence set as one DAG node, or `None` for the empty sequence.
    pub(super) fn freeze(&self) -> Option<std::rc::Rc<ObservationDag>> {
        if self.items.is_empty() {
            return self.prefix.clone();
        }
        Some(std::rc::Rc::new(ObservationDag::legacy(
            ObservationDagKind::Seq(self.prefix.clone(), self.items.clone()),
        )))
    }

    #[cfg(test)]
    fn union(members: Vec<Observations>) -> Self {
        let members = members
            .iter()
            .map(|member| {
                member.freeze().unwrap_or_else(|| {
                    std::rc::Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
                        None,
                        Vec::new(),
                    )))
                })
            })
            .collect();
        Self {
            prefix: Some(std::rc::Rc::new(ObservationDag::legacy(
                ObservationDagKind::Alt(members),
            ))),
            items: Vec::new(),
        }
    }

    pub(super) fn parts(&self) -> (Option<&std::rc::Rc<ObservationDag>>, &[Observation]) {
        (self.prefix.as_ref(), &self.items)
    }

    fn identity(&self) -> (Option<usize>, &[Observation]) {
        (
            self.prefix
                .as_ref()
                .map(|prefix| std::rc::Rc::as_ptr(prefix) as usize),
            &self.items,
        )
    }
}

impl From<Vec<Observation>> for Observations {
    fn from(items: Vec<Observation>) -> Self {
        Self {
            prefix: None,
            items,
        }
    }
}

/// The flat tail. It is the complete sequence on every enumerating path,
/// where nothing is merged.
impl std::ops::Deref for Observations {
    type Target = [Observation];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl std::fmt::Debug for Observations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.prefix.is_some() {
            formatter.write_str("<merged> ")?;
        }
        formatter.debug_list().entries(&self.items).finish()
    }
}

/// Legacy flat sequences compare by content. Factored prefixes compare by
/// identity; ordinary enumerated history never uses this ordering and instead
/// enters `enumerated_prefix`'s paid complete-content comparator.
impl PartialEq for Observations {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for Observations {}

impl PartialOrd for Observations {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Observations {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.identity().cmp(&other.identity())
    }
}

impl SkeletonWork<'_, '_> {
    pub(in crate::cleanup_plan::replay) fn lexical_merge_normal(
        &self,
        path: &ExprSkeletonPath,
    ) -> bool {
        self.function.cleanup_plan.lexical_control.is_none()
            || (self.active_copy_region.is_some()
                && !path.failed
                && !path.residual
                && path.owned_source.is_none()
                && path.continuation == lexical_skeleton::Continuation::Normal)
    }
}

#[cfg(test)]
thread_local! {
    static SAVED_EXTENSIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
fn with_saved_extensions<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            SAVED_EXTENSIONS.with(|cell| cell.set(self.0));
        }
    }
    let _reset = Reset(SAVED_EXTENSIONS.with(|cell| cell.replace(true)));
    operation()
}

#[cfg(test)]
/// Entry inventories use the same finite byte payer as subsequent replay.
/// The original semantic helper ceilings and ordinary work events stay intact;
/// this receiver introduces neither another account nor a helper work quota.
mod ordinary_entry {
    use super::*;
    use crate::bounded_output::classifier_string::ClassifierLedger;
    use crate::hir::funded::FundedError;

    // Match the original owning field order for partial construction and Drop.
    pub(super) struct Prepared<'a> {
        pub(super) renewals: crate::cleanup_plan::renewal_profile::RenewalBindings<'a>,
        pub(super) moves: BTreeSet<ExpressionId>,
        pub(super) reads: BTreeSet<ExpressionId>,
        pub(super) owner: Option<crate::hir::FunctionExecutionId>,
    }

    pub(super) fn prepare<'a>(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
        indexed: bool,
    ) -> Result<Prepared<'a>, Diagnostic> {
        let ledger = ClassifierLedger::capture();
        let result = {
            let mut pay = |bytes| {
                if !ledger.current() {
                    return Err(budget.budget_refusal(
                        function,
                        "ordinary replay setup payer is outside its current invocation",
                        "",
                    ));
                }
                budget.reserve_bytes(function, bytes, "ordinary replay setup backing")
            };
            let receiving = std::mem::size_of::<Prepared<'_>>()
                + std::mem::size_of::<Result<Prepared<'_>, FundedError<Diagnostic>>>()
                + std::mem::size_of::<ClassifierLedger>()
                + std::mem::size_of_val(&pay);
            (|| {
                crate::hir::funded::charge(&mut pay, receiving)?;
                Ok(Prepared {
                    renewals: if indexed {
                        crate::cleanup_plan::renewal_profile::RenewalBindings::empty()
                    } else {
                        crate::cleanup_plan::renewal_profile::bindings_funded(
                            program, function, &mut pay,
                        )?
                    },
                    moves: crate::literal_format::checked_moving_string_operands_funded(
                        function, &mut pay,
                    )?,
                    reads: crate::string_ops::conditions::function_reads_funded(
                        function, &mut pay,
                    )?,
                    owner: if crate::hir::lexical_exit::requires_with_funded_cursor(
                        function, &mut pay,
                    )? {
                        crate::hir::lexical_exit::owner_funded(program, function, &mut pay)?.ok()
                    } else {
                        None
                    },
                })
            })()
        };
        match result {
            Ok(prepared) => Ok(prepared),
            Err(FundedError::Refused(error)) => Err(error),
            Err(
                FundedError::Layout(reason)
                | FundedError::Census(reason)
                | FundedError::Unfunded(reason),
            ) => Err(budget.budget_refusal(function, reason, "")),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const SUBJECTS: [&str; 3] = [
            "module setup.scalar; @id(\"f\") fn f()->i64 {0} @id(\"main\") fn main()->i64 {0}",
            r#"module setup.strings;
@id("f") fn f(text:string)->i64 {
 let mut owner=text; let mut count=0;
 while string_len(owner)>0 && string_len(owner)<4 {
  owner=string_concat(owner,"x"); count=count+1; 0
 }
 return count;
}
@id("main") fn main()->i64 {0}
"#,
            r#"module setup.renewals;
@id("f") fn f()->i64 {
 let mut bytes=bytes_zeroed(2usize);
 let mut values=vec_with_capacity<i64>(1usize); let mut n=0usize;
 while n<1usize {
  bytes=bytes_set(bytes,n,65u8); values=vec_push<i64>(values,1); n=n+1usize; 0
 }
 if byte_len(bytes_as_slice(bytes))==2usize && vec_len<i64>(values)==1usize {7}else{0}
}
@id("main") fn main()->i64 {0}
"#,
        ];

        #[test]
        fn complete_entry_keeps_every_inventory_work_and_physical_refusal() {
            for (subject, source) in SUBJECTS.into_iter().enumerate() {
                let parsed = crate::check(source, "ordinary-setup.spx").unwrap();
                let canonical = crate::format::canonical(&parsed);
                let parsed = crate::check(&canonical, "ordinary-setup.spx").unwrap();
                assert_eq!(crate::format::canonical(&parsed), canonical);
                let program = crate::hir::resolve(&parsed).unwrap();
                let function = program
                    .functions
                    .iter()
                    .find(|f| f.id.as_str() == "f")
                    .unwrap();
                let index = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
                let mut ids = Vec::new();
                crate::hir::function_value::walk(function, |expression| {
                    ids.push(expression.id.clone());
                });
                for indexed in [false, true] {
                    let index = indexed.then_some(&index);
                    // The entire original constructor remains the independent
                    // unobserved reference, outside the measured finite account.
                    let mut old_budget = ReplayBudget::unobserved();
                    let expected =
                        SkeletonWork::with_index(&program, function, &mut old_budget, index)
                            .unwrap();
                    if subject == 1 {
                        assert!(!expected.string_owner_moves.is_empty());
                        assert!(!expected.string_condition_reads.is_empty());
                        assert!(expected.owner.is_some());
                    }
                    if subject == 2 && !indexed {
                        assert!(ids
                            .iter()
                            .any(|id| expected.renewal_bindings.get(id).is_some()));
                    }
                    let observe = |available, floor| {
                        crate::bounded_output::with_limit_usage_refusal(available + floor, || {
                            assert!(crate::bounded_output::set_active_floor(floor));
                            crate::test_allocation_observer::measure(|| {
                                let mut budget =
                                    ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                                let result = (|| {
                                    let work = SkeletonWork::with_index(
                                        &program,
                                        function,
                                        &mut budget,
                                        index,
                                    )?;
                                    assert_eq!(
                                        work.string_owner_moves,
                                        expected.string_owner_moves
                                    );
                                    assert_eq!(
                                        work.string_condition_reads,
                                        expected.string_condition_reads
                                    );
                                    assert_eq!(work.owner, expected.owner);
                                    for id in &ids {
                                        assert_eq!(
                                            work.renewal_bindings.get(id),
                                            expected.renewal_bindings.get(id)
                                        );
                                    }
                                    assert_eq!(work.budget.remaining, MAX_REPLAY_WORK_UNITS);
                                    assert_eq!(
                                        work.budget.skeleton_remaining,
                                        MAX_REPLAY_WORK_UNITS
                                    );
                                    drop(work);
                                    Ok::<_, Diagnostic>(())
                                })();
                                let success = result.is_ok();
                                if !success {
                                    assert!(budget
                                        .reserve_bytes(function, 0, "sticky setup")
                                        .is_err());
                                }
                                drop(result);
                                drop(budget);
                                success
                            })
                        })
                    };
                    let ((ok, allocation), refused, exact, first) = observe(usize::MAX, 0);
                    assert!(ok && !refused && first.is_none() && exact > 0);
                    assert!(!allocation.invalid && allocation.live == 0);
                    assert_eq!(allocation.allocated, allocation.freed);
                    assert!(allocation.allocated <= exact);
                    for floor in [0, 37] {
                        for available in [0, exact - 1, exact] {
                            let ((ok, allocation), refused, used, receipt) =
                                observe(available, floor);
                            assert_eq!(ok, available == exact);
                            assert_eq!(refused, available != exact);
                            assert_eq!(receipt.is_some(), available != exact);
                            if let Some(receipt) = receipt {
                                assert_eq!(receipt.floor, floor);
                            }
                            assert!(used <= available);
                            assert!(!allocation.invalid && allocation.live == 0);
                            assert_eq!(allocation.allocated, allocation.freed);
                            assert!(allocation.allocated <= used);
                            if available == 0 {
                                assert_eq!(allocation.allocated, 0);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod incoming_constructor_control {
    use super::*;
    #[test]
    fn main_complete_prepared_receiver_matches_joined_actual_inventories() {
        let program = crate::hir::resolve(
            &crate::check(
                "module joined.setup; @id(\"main\") fn main()->i64 {return 0;}",
                "joined-setup.spx",
            )
            .unwrap(),
        )
        .unwrap();
        let function = &program.functions[0];
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut old_budget = ReplayBudget::new();
            let before =
                SkeletonWork::ordinary_with_index(&program, function, &mut old_budget, None)
                    .unwrap();
            let mut current_budget = ReplayBudget::new();
            let current =
                SkeletonWork::with_index(&program, function, &mut current_budget, None).unwrap();
            assert_eq!(current.string_owner_moves, before.string_owner_moves);
            assert_eq!(
                current.string_condition_reads,
                before.string_condition_reads
            );
            assert_eq!(current.owner, before.owner);
            crate::hir::function_value::walk(function, |expression| {
                assert_eq!(
                    current.renewal_bindings.get(&expression.id),
                    before.renewal_bindings.get(&expression.id)
                );
            });
        });
    }
}
