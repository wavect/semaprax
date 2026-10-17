//! Budget-charged skeleton path materialization for the typed-HIR replay.
//!
//! Path observations are shared behind `Rc`, so sequencing a prefix with each
//! suffix copies pointers rather than deep-copying every identity and place.
//! Existing work events remain; ordinary construction ordinals and exact memo
//! pointer checks add explicit work, and new ordinary DAG storage is prepaid.
use super::*;
mod dag_storage;
#[cfg(test)]
mod oracle_entry;
mod ordinary_dag;

pub(super) struct SkeletonWork<'a, 'b> {
    pub(super) continuations: Option<&'b mut continuation_storage::Storage<'a>>,
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
    pub(super) renewal_bindings: crate::cleanup_plan::renewal_profile::RenewalBindings<'a>,
    pub(super) active_copy_region: Option<ExpressionId>,
    dag_scratch: Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    ordinary_scratch: Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    next_ordinary_dag: usize,
    held_renewals: Option<&'a crate::cleanup_plan::renewal_profile::RenewalBindings<'a>>,
    ordinary_index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ordinary_program: Option<&'a dyn crate::cleanup::ReplayContext>,
    /// Authored same-owner append and direct literal-format String moves.
    pub(super) string_owner_moves: BTreeSet<ExpressionId>,
    pub(super) string_condition_reads: BTreeSet<ExpressionId>,
    /// Authenticated execution identity of a lexical-profile function.
    pub(super) owner: Option<crate::hir::FunctionExecutionId>,
}

impl<'a, 'b> SkeletonWork<'a, 'b> {
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
/// The enumerating replay keeps every path flat: `prefix` stays `None` and
/// `items` is exactly the former observation vector, with the same clone,
/// append and charge behavior. Only the factored replay merges paths that
/// carry identical cleanup state; their sequences then become one shared
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

/// Flat sequences compare by content; the enumerating comparison sorts only
/// flat paths. A merged prefix compares by identity.
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
    fn lexical_merge_normal(&self, path: &ExprSkeletonPath) -> bool {
        self.function.cleanup_plan.lexical_control.is_none()
            || (self.active_copy_region.is_some()
                && !path.failed
                && !path.residual
                && path.owned_source.is_none()
                && path.continuation == lexical_skeleton::Continuation::Normal)
    }

    /// Factored replay only: merge paths whose cleanup state is identical
    /// into one path carrying the union of their observation sequences. Every
    /// later skeleton step depends only on that state, so the union denotes
    /// exactly the paths enumeration would produce.
    pub(super) fn merged_paths(
        &mut self,
        paths: Vec<ExprSkeletonPath>,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(paths);
        }
        self.charge(paths.len(), "factored path-state merge")?;
        type Key = (
            Option<CleanupPlace>,
            bool,
            bool,
            lexical_skeleton::Continuation,
        );
        type Group = (ExprSkeletonPath, Vec<Observations>);
        let mut order = BTreeMap::<Key, usize>::new();
        let mut groups = Vec::<Group>::new();
        for mut path in paths {
            if !self.lexical_merge_normal(&path) {
                self.budget
                    .reserve_vec(self.function, &mut groups, 1, "isolated lexical path")?;
                groups.push((path, Vec::new()));
                continue;
            }
            let source = self.clone_payload(&path.owned_source, "factored merge source key")?;
            let key = (
                source,
                path.failed,
                path.residual,
                path.continuation.clone(),
            );
            match order.get(&key).copied() {
                Some(index) => {
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups[index].1,
                        1,
                        "factored merge members",
                    )?;
                    groups[index].1.push(path.observations);
                }
                None => {
                    self.budget.reserve_btree_node::<Key, usize>(
                        self.function,
                        "factored merge identity index",
                    )?;
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups,
                        1,
                        "factored merge groups",
                    )?;
                    self.budget.reserve_array::<Observations>(
                        self.function,
                        1,
                        "factored first merge member",
                    )?;
                    let observations = std::mem::take(&mut path.observations);
                    order.insert(key, groups.len());
                    groups.push((path, vec![observations]));
                }
            }
        }
        self.budget.reserve_array::<ExprSkeletonPath>(
            self.function,
            groups.len(),
            "factored merged paths",
        )?;
        let mut result = Vec::with_capacity(groups.len());
        for (mut path, mut members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.selected() {
                    self.union_observations(members)?
                } else {
                    self.ordinary_union(members)?
                };
            } else if let Some(observations) = members.pop() {
                // A singleton keeps its original prefix and Vec backing.
                path.observations = observations;
            }
            result.push(path);
        }
        Ok(result)
    }

    pub(super) fn merged_suffixes<'s>(
        &mut self,
        paths: &'s [ExprSkeletonPath],
    ) -> Result<std::borrow::Cow<'s, [ExprSkeletonPath]>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(std::borrow::Cow::Borrowed(paths));
        }
        self.budget.reserve_array::<ExprSkeletonPath>(
            self.function,
            paths.len(),
            "factored suffix path copies",
        )?;
        let mut owned = Vec::with_capacity(paths.len());
        for path in paths {
            owned.push(self.clone_payload(path, "factored suffix path backing")?);
        }
        Ok(std::borrow::Cow::Owned(self.merged_paths(owned)?))
    }

    /// Call states additionally merge only with identical commit lists.
    pub(super) fn merged_call_states(
        &mut self,
        states: Vec<CallSkeletonState>,
    ) -> Result<Vec<CallSkeletonState>, Diagnostic> {
        if !self.budget.merge_paths || states.len() < 2 {
            return Ok(states);
        }
        self.charge(states.len(), "factored call-state merge")?;
        type Key = (
            Option<CleanupPlace>,
            bool,
            bool,
            lexical_skeleton::Continuation,
            Vec<(u32, CleanupPlace)>,
        );
        type Group = (CallSkeletonState, Vec<Observations>);
        let mut order = BTreeMap::<Key, usize>::new();
        let mut groups = Vec::<Group>::new();
        for (mut path, commits) in states {
            if !self.lexical_merge_normal(&path)
                || (self.function.cleanup_plan.lexical_control.is_some() && !commits.is_empty())
            {
                self.budget.reserve_vec(
                    self.function,
                    &mut groups,
                    1,
                    "isolated lexical call state",
                )?;
                groups.push(((path, commits), Vec::new()));
                continue;
            }
            let source = self.clone_payload(&path.owned_source, "factored call source key")?;
            let key_commits = self.clone_payload(&commits, "factored call commit key")?;
            let key = (
                source,
                path.failed,
                path.residual,
                path.continuation.clone(),
                key_commits,
            );
            match order.get(&key).copied() {
                Some(index) => {
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups[index].1,
                        1,
                        "factored call merge members",
                    )?;
                    groups[index].1.push(path.observations);
                }
                None => {
                    self.budget.reserve_btree_node::<Key, usize>(
                        self.function,
                        "factored call identity index",
                    )?;
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups,
                        1,
                        "factored call merge groups",
                    )?;
                    self.budget.reserve_array::<Observations>(
                        self.function,
                        1,
                        "factored first call member",
                    )?;
                    let observations = std::mem::take(&mut path.observations);
                    order.insert(key, groups.len());
                    groups.push(((path, commits), vec![observations]));
                }
            }
        }
        self.budget.reserve_array::<CallSkeletonState>(
            self.function,
            groups.len(),
            "factored merged call states",
        )?;
        let mut result = Vec::with_capacity(groups.len());
        for ((mut path, commits), mut members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.selected() {
                    self.union_observations(members)?
                } else {
                    self.ordinary_union(members)?
                };
            } else if let Some(observations) = members.pop() {
                // A singleton keeps its original prefix and Vec backing.
                path.observations = observations;
            }
            result.push((path, commits));
        }
        Ok(result)
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
