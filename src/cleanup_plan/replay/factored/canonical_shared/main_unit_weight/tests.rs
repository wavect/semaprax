//! Independent full-old algebra and actual allocation/drop controls.
use super::*;

fn fixture() -> ResolvedFunction {
    crate::cleanup_plan::replay::factored::lexical_plan_tests::fixture(crate::cleanup_plan::replay::factored::lexical_plan_tests::WHILE).1
}

pub(super) fn automaton(function: &ResolvedFunction) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::new();
    let mut automaton = Automaton::default();
    let sink = automaton.node(function, &mut budget).unwrap();
    let mut roots = Vec::new();
    for terminal in [
        SkeletonTerminal::Success,
        SkeletonTerminal::Failure,
        SkeletonTerminal::LoopLatch,
    ] {
        roots.push(
            automaton
                .end(function, &mut budget, sink, terminal)
                .unwrap(),
        );
    }
    let duplicate = automaton.node(function, &mut budget).unwrap();
    for _ in 0..2 {
        automaton
            .epsilon(function, &mut budget, duplicate, roots[0])
            .unwrap();
    }
    roots.push(duplicate);
    for targets in [[roots[0], roots[1]], [roots[1], roots[0]]] {
        let root = automaton.node(function, &mut budget).unwrap();
        for target in targets {
            automaton
                .epsilon(function, &mut budget, root, target)
                .unwrap();
        }
        roots.push(root);
    }
    let cycle = automaton.node(function, &mut budget).unwrap();
    automaton
        .movement(
            function,
            &mut budget,
            cycle,
            Label::End(SkeletonTerminal::Success),
            cycle,
        )
        .unwrap();
    roots.push(cycle);
    (automaton, sink, roots)
}

thread_local! {
    static ACTUAL_ENTRIES: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}
struct ActualEntries(Option<(usize, usize)>);
impl ActualEntries {
    fn open(limit: usize) -> Self {
        Self(ACTUAL_ENTRIES.with(|state| state.replace(Some((limit, 0)))))
    }
    fn calls(&self) -> usize {
        ACTUAL_ENTRIES.with(|state| state.get().unwrap().1)
    }
}
impl Drop for ActualEntries {
    fn drop(&mut self) {
        ACTUAL_ENTRIES.with(|state| state.set(self.0));
    }
}
pub(super) fn note_actual_entry(budget: &mut ReplayBudget) {
    ACTUAL_ENTRIES.with(|state| {
        if let Some((limit, calls)) = state.get() {
            if calls == 0 {
                budget.remaining = limit;
            }
            state.set(Some((limit, calls + 1)));
        }
    });
}
