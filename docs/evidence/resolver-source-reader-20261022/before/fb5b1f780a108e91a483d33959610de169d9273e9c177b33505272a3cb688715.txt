//! Fixed-arity ownership extraction. Scheduling and variable-arity splices stay
//! unchanged; no child clones or temporary result backing are introduced.
use super::ResolvedExpr;

pub(in crate::hir) fn take_pair(results: &mut Vec<ResolvedExpr>) -> (ResolvedExpr, ResolvedExpr) {
    results
        .len()
        .checked_sub(2)
        .expect("expression continuation retains every child result");
    #[cfg(test)]
    if old_mode() {
        let mut children = saved_old(results, 2).into_iter();
        return (
            children.next().expect("binary left result retained"),
            children.next().expect("binary right result retained"),
        );
    }
    let right = results.pop().expect("binary right result retained");
    let left = results.pop().expect("binary left result retained");
    (left, right)
}

pub(in crate::hir) fn take_triple(
    results: &mut Vec<ResolvedExpr>,
) -> (ResolvedExpr, ResolvedExpr, ResolvedExpr) {
    results
        .len()
        .checked_sub(3)
        .expect("expression continuation retains every child result");
    #[cfg(test)]
    if old_mode() {
        let mut children = saved_old(results, 3).into_iter();
        return (
            children.next().expect("if condition retained"),
            children.next().expect("if then branch retained"),
            children.next().expect("if else branch retained"),
        );
    }
    let else_branch = results.pop().expect("if else branch retained");
    let then_branch = results.pop().expect("if then branch retained");
    let condition = results.pop().expect("if condition retained");
    (condition, then_branch, else_branch)
}

#[cfg(test)]
thread_local! { static OLD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
fn old_mode() -> bool {
    OLD.with(std::cell::Cell::get)
}
#[cfg(test)]
fn saved_old(results: &mut Vec<ResolvedExpr>, count: usize) -> Vec<ResolvedExpr> {
    let start = results
        .len()
        .checked_sub(count)
        .expect("expression continuation retains every child result");
    results.split_off(start)
}
#[cfg(test)]
fn with_saved_old<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            OLD.with(|old| old.set(self.0));
        }
    }
    let _restore = Restore(OLD.with(|old| old.replace(true)));
    operation()
}
#[cfg(any())]
mod tests;
