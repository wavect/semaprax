//! Budget-charged skeleton path materialization for the typed-HIR replay.
//!
//! Path observations are shared behind `Rc`, so sequencing a prefix with each
//! suffix copies pointers rather than deep-copying every identity and place.
//! Every charge and materialization count is unchanged.
use super::*;

pub(super) struct SkeletonWork<'a, 'b> {
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
    /// Owned String Loops v1 same-owner append operands that move, not clone.
    pub(super) string_owner_moves: BTreeSet<ExpressionId>,
    pub(super) string_condition_reads: BTreeSet<ExpressionId>,
}

impl<'a, 'b> SkeletonWork<'a, 'b> {
    pub(super) fn new(function: &'a ResolvedFunction, budget: &'b mut ReplayBudget) -> Self {
        Self {
            function,
            budget,
            string_owner_moves: crate::string_ops::same_owner_concat_operands(function),
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
        }
    }
}

impl SkeletonWork<'_, '_> {
    pub(super) fn charge(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.budget.charge_skeleton(self.function, units, phase)
    }

    pub(super) fn clone_owned<T: Clone>(
        &mut self,
        value: &T,
        phase: &str,
    ) -> Result<T, Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
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
        note_skeleton_materialization();
        Ok(path.clone())
    }

    pub(super) fn clone_observations(
        &mut self,
        observations: &Observations,
        phase: &str,
    ) -> Result<Observations, Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        Ok(observations.clone())
    }

    pub(super) fn extend_observations(
        &mut self,
        target: &mut Observations,
        observations: &Observations,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        target.extend(observations);
        Ok(())
    }

    pub(super) fn push_observation(
        &mut self,
        path: &mut ExprSkeletonPath,
        observation: SkeletonObservation,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        path.observations.push(observation.into());
        Ok(())
    }
}

pub(super) fn empty_expr_path() -> ExprSkeletonPath {
    ExprSkeletonPath {
        observations: Observations::default(),
        owned_source: None,
        failed: false,
        residual: false,
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
    prefix: Option<std::rc::Rc<ObservationDag>>,
    items: Vec<Observation>,
}

/// A set of observation sequences. `Seq` is `prefix` then `items`; `Cat`
/// concatenates its parts in order; `Alt` is the union of its members.
pub(super) enum ObservationDag {
    Seq(Option<std::rc::Rc<ObservationDag>>, Vec<Observation>),
    Cat(Vec<std::rc::Rc<ObservationDag>>),
    Alt(Vec<std::rc::Rc<ObservationDag>>),
}

impl Drop for ObservationDag {
    /// Release deep merge chains without recursion.
    fn drop(&mut self) {
        let mut pending = self.take_children();
        while let Some(child) = pending.pop() {
            if let Ok(mut node) = std::rc::Rc::try_unwrap(child) {
                pending.extend(node.take_children());
            }
        }
    }
}

impl ObservationDag {
    fn take_children(&mut self) -> Vec<std::rc::Rc<ObservationDag>> {
        match self {
            Self::Seq(prefix, _) => prefix.take().into_iter().collect(),
            Self::Cat(parts) | Self::Alt(parts) => std::mem::take(parts),
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
                let head = std::rc::Rc::new(ObservationDag::Seq(None, vec![observation]));
                self.prefix = Some(std::rc::Rc::new(ObservationDag::Cat(vec![head, prefix])));
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
            Some(head) => std::rc::Rc::new(ObservationDag::Cat(vec![head, suffix_prefix.clone()])),
        });
        self.items = suffix.items.clone();
    }

    /// The whole sequence set as one DAG node, or `None` for the empty sequence.
    pub(super) fn freeze(&self) -> Option<std::rc::Rc<ObservationDag>> {
        if self.items.is_empty() {
            return self.prefix.clone();
        }
        Some(std::rc::Rc::new(ObservationDag::Seq(
            self.prefix.clone(),
            self.items.clone(),
        )))
    }

    fn union(members: Vec<Observations>) -> Self {
        let members = members
            .iter()
            .map(|member| {
                member
                    .freeze()
                    .unwrap_or_else(|| std::rc::Rc::new(ObservationDag::Seq(None, Vec::new())))
            })
            .collect();
        Self {
            prefix: Some(std::rc::Rc::new(ObservationDag::Alt(members))),
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
        let mut order = BTreeMap::<(Option<CleanupPlace>, bool, bool), usize>::new();
        let mut groups = Vec::<(ExprSkeletonPath, Vec<Observations>)>::new();
        for path in paths {
            let key = (path.owned_source.clone(), path.failed, path.residual);
            match order.get(&key) {
                Some(index) => groups[*index].1.push(path.observations),
                None => {
                    order.insert(key, groups.len());
                    let observations = path.observations.clone();
                    groups.push((path, vec![observations]));
                }
            }
        }
        Ok(groups
            .into_iter()
            .map(|(mut path, members)| {
                if members.len() > 1 {
                    path.observations = Observations::union(members);
                }
                path
            })
            .collect())
    }

    pub(super) fn merged_suffixes<'s>(
        &mut self,
        paths: &'s [ExprSkeletonPath],
    ) -> Result<std::borrow::Cow<'s, [ExprSkeletonPath]>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(std::borrow::Cow::Borrowed(paths));
        }
        Ok(std::borrow::Cow::Owned(self.merged_paths(paths.to_vec())?))
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
        type CallKey = (Option<CleanupPlace>, bool, bool, Vec<(u32, CleanupPlace)>);
        let mut order = BTreeMap::<CallKey, usize>::new();
        let mut groups = Vec::<(CallSkeletonState, Vec<Observations>)>::new();
        for (path, commits) in states {
            let key = (
                path.owned_source.clone(),
                path.failed,
                path.residual,
                commits.clone(),
            );
            match order.get(&key) {
                Some(index) => groups[*index].1.push(path.observations),
                None => {
                    order.insert(key, groups.len());
                    let observations = path.observations.clone();
                    groups.push(((path, commits), vec![observations]));
                }
            }
        }
        Ok(groups
            .into_iter()
            .map(|((mut path, commits), members)| {
                if members.len() > 1 {
                    path.observations = Observations::union(members);
                }
                (path, commits)
            })
            .collect())
    }
}
