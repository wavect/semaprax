//! Budget-charged skeleton path materialization for the typed-HIR replay.
//!
//! Path observations are shared behind `Rc`, so sequencing a prefix with each
//! suffix copies pointers rather than deep-copying every identity and place.
//! Every charge and materialization count is unchanged.
use super::*;

pub(super) struct SkeletonWork<'a, 'b> {
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
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
        observations: &[std::rc::Rc<SkeletonObservation>],
        phase: &str,
    ) -> Result<Vec<std::rc::Rc<SkeletonObservation>>, Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        Ok(observations.to_vec())
    }

    pub(super) fn extend_observations(
        &mut self,
        target: &mut Vec<std::rc::Rc<SkeletonObservation>>,
        observations: &[std::rc::Rc<SkeletonObservation>],
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        target.extend_from_slice(observations);
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
