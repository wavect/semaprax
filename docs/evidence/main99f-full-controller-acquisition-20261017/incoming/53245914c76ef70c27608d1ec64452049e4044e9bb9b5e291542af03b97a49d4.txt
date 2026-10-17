//! Saved complete pre-move merges and union construction; independent clone oracle.
use super::*;

impl SkeletonWork<'_, '_> {
    pub(super) fn tail_merge_control(
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
        for path in paths {
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
                    let observations =
                        self.clone_payload(&path.observations, "factored first member tail")?;
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
        for (mut path, members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.selected() {
                    self.saved_union_observations(members)?
                } else {
                    self.saved_ordinary_union(members)?
                };
            }
            result.push(path);
        }
        Ok(result)
    }

    pub(super) fn saved_merged_call_states(
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
        for (path, commits) in states {
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
                    let observations =
                        self.clone_payload(&path.observations, "factored first call tail")?;
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
        for ((mut path, commits), members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.selected() {
                    self.saved_union_observations(members)?
                } else {
                    self.saved_ordinary_union(members)?
                };
            }
            result.push((path, commits));
        }
        Ok(result)
    }
    fn saved_ordinary_freeze(
        &mut self,
        observations: &Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix.clone());
        }
        self.budget.reserve_clone(
            self.function,
            &observations.items,
            "ordinary DAG sequence tail",
        )?;
        self.ordinary_dag(ObservationDagKind::Seq(
            observations.prefix.clone(),
            observations.items.clone(),
        ))
        .map(Some)
    }

    pub(super) fn saved_ordinary_union(
        &mut self,
        members: Vec<Observations>,
    ) -> Result<Observations, Diagnostic> {
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            members.len(),
            "ordinary DAG alternatives",
        )?;
        let mut nodes = Vec::with_capacity(members.len());
        for member in &members {
            let node = match self.saved_ordinary_freeze(member)? {
                Some(node) => node,
                None => self.ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))?,
            };
            nodes.push(node);
        }
        Ok(Observations {
            prefix: Some(self.ordinary_dag(ObservationDagKind::Alt(nodes))?),
            items: Vec::new(),
        })
    }

    pub(super) fn saved_freeze_observations(
        &mut self,
        observations: &Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix.clone());
        }
        self.budget
            .reserve_clone(self.function, observations, "factored sequence tail clone")?;
        let kind = ObservationDagKind::Seq(observations.prefix.clone(), observations.items.clone());
        self.charged_dag(kind).map(Some)
    }

    pub(in crate::cleanup_plan::replay) fn saved_union_observations(
        &mut self,
        members: Vec<Observations>,
    ) -> Result<Observations, Diagnostic> {
        self.charge(members.len(), "factored observation alternatives")?;
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            members.len(),
            "factored alternative members",
        )?;
        let mut alternatives = Vec::with_capacity(members.len());
        for member in members {
            let node = match self.saved_freeze_observations(&member)? {
                Some(node) => node,
                None => self.charged_dag(ObservationDagKind::Seq(None, Vec::new()))?,
            };
            // Preserve duplicate members: this is a multiset, never a set.
            alternatives.push(node);
        }
        Ok(Observations {
            prefix: Some(self.charged_dag(ObservationDagKind::Alt(alternatives))?),
            items: Vec::new(),
        })
    }
}
