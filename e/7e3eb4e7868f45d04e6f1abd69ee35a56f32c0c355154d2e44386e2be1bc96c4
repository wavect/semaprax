use super::super::*;

impl SkeletonWork<'_, '_> {
    pub(super) fn saved_merged_paths(
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
}


// Complete incoming optimized method retained under cfg(test), not yet compiled.
// Shipping activation remains pending.
impl SkeletonWork<'_, '_> {
    #[allow(dead_code)] // retained original algorithm; activation is explicitly pending
    pub(super) fn incoming_merged_paths(
        &mut self,
        paths: Vec<ExprSkeletonPath>,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(paths);
        }
        self.charge(paths.len(), "factored path-state merge")?;
        if super::super::can_reuse(self, &paths) {
            self.budget.reserve_array::<u8>(
                self.function,
                isolated_paths::RECEIVING_BYTES,
                "isolated path receiving owners",
            )?;
            return Ok(paths);
        }
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
}
