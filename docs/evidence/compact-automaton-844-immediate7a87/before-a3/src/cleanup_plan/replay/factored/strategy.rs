//! Deferred lexical resources and exact post-authentication representation.
//! A resource plan or terminal census carries no equality/ownership authority.
//! Factored admission retains the independently rebuilt observation languages.
use super::*;

/// Selected from actual lexical HIR before an enumerating census may refuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum ResourceStrategy {
    EnumeratedUpper,
    DeferredLexical,
}
pub(in crate::cleanup_plan::replay) fn provisional(
    function: &ResolvedFunction,
) -> ResourceStrategy {
    if lexical_cert::requires_lexical(function) {
        ResourceStrategy::DeferredLexical
    } else {
        ResourceStrategy::EnumeratedUpper
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Number {
    Known(u64),
    ArithmeticOverflow,
}
impl Number {
    fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Known(left), Self::Known(right)) => left
                .checked_add(right)
                .map_or(Self::ArithmeticOverflow, Self::Known),
            _ => Self::ArithmeticOverflow,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalCounts {
    success: Number,
    failure: Number,
    latch: Number,
}
impl TerminalCounts {
    const ZERO: Self = Self {
        success: Number::Known(0),
        failure: Number::Known(0),
        latch: Number::Known(0),
    };
    fn add(self, other: Self) -> Self {
        Self {
            success: self.success.add(other.success),
            failure: self.failure.add(other.failure),
            latch: self.latch.add(other.latch),
        }
    }
    fn terminal(terminal: SkeletonTerminal) -> Self {
        let mut counts = Self::ZERO;
        *match terminal {
            SkeletonTerminal::Success => &mut counts.success,
            SkeletonTerminal::Failure => &mut counts.failure,
            SkeletonTerminal::LoopLatch => &mut counts.latch,
        } = Number::Known(1);
        counts
    }
    fn census(self) -> Census {
        match self.success.add(self.failure).add(self.latch) {
            Number::Known(total) if total <= MAX_REPLAY_PATHS as u64 => Census::Known {
                classes: self,
                total,
            },
            Number::Known(total) => Census::Exceeded {
                classes: self,
                total,
            },
            Number::ArithmeticOverflow => Census::ArithmeticOverflow { classes: self },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Census {
    Known { classes: TerminalCounts, total: u64 },
    Exceeded { classes: TerminalCounts, total: u64 },
    ArithmeticOverflow { classes: TerminalCounts },
}

#[derive(Clone, Copy)]
struct Cursor {
    node: usize,
    next: usize,
    counts: TerminalCounts,
}

/// Count all accepted routes without inspecting observation values. Duplicate
/// epsilon and labelled movements contribute separately. A cycle is refused,
/// not saturated; each overflowing terminal class remains explicitly unknown.
fn census(
    function: &ResolvedFunction,
    built: &BuiltComparison,
    root: usize,
    budget: &mut ReplayBudget,
) -> Result<Census, Diagnostic> {
    let nodes = &built.automaton.nodes;
    if root >= nodes.len() || built.sink >= nodes.len() {
        return Err(replay_error(
            function,
            "factored census has an unknown root or sink",
        ));
    }
    if !nodes[built.sink].moves().is_empty() || !nodes[built.sink].epsilon().is_empty() {
        return Err(replay_error(
            function,
            "exact census sink has outgoing movements",
        ));
    }
    let initialization = nodes
        .len()
        .checked_mul(2)
        .ok_or_else(|| replay_error(function, "exact census initialization work overflowed"))?;
    budget.charge(
        function,
        initialization,
        "exact census color and class initialization",
    )?;
    budget.reserve_array::<u8>(function, nodes.len(), "exact census node colors")?;
    budget.reserve_array::<TerminalCounts>(
        function,
        nodes.len(),
        "exact census terminal classes",
    )?;
    budget.reserve_array::<Cursor>(function, nodes.len(), "exact census traversal frontier")?;
    let mut colors = vec![0u8; nodes.len()];
    let mut counts = vec![TerminalCounts::ZERO; nodes.len()];
    let mut frontier = Vec::with_capacity(nodes.len());
    budget.charge(function, 2, "exact census root and release")?;
    colors[root] = 1;
    frontier.push(Cursor {
        node: root,
        next: 0,
        counts: TerminalCounts::ZERO,
    });
    while let Some(cursor) = frontier.last_mut() {
        let node = &nodes[cursor.node];
        let moves = node
            .epsilon()
            .len()
            .checked_add(node.moves().len())
            .ok_or_else(|| replay_error(function, "exact census movement count overflowed"))?;
        if cursor.next == moves {
            budget.charge(function, 1, "exact census completed node")?;
            counts[cursor.node] = cursor.counts;
            colors[cursor.node] = 2;
            frontier.pop();
            continue;
        }
        budget.charge(function, 1, "exact census route")?;
        let index = cursor.next;
        let target = if index < node.epsilon().len() {
            node.epsilon()[index]
        } else {
            let (label, target) = &node.moves()[index - node.epsilon().len()];
            if let Label::End(terminal) = label {
                if *target != built.sink {
                    return Err(replay_error(
                        function,
                        "exact census terminal differs from its sink",
                    ));
                }
                cursor.counts = cursor.counts.add(TerminalCounts::terminal(*terminal));
                cursor.next += 1;
                continue;
            }
            *target
        };
        let color = colors
            .get(target)
            .ok_or_else(|| replay_error(function, "exact census movement has an unknown target"))?;
        match color {
            0 => {
                if frontier.len() == frontier.capacity() {
                    return Err(replay_error(
                        function,
                        "exact census frontier exceeds node count",
                    ));
                }
                // The parent cursor keeps this edge pending until the exact
                // child count is memoized. Only active ancestors are gray.
                budget.charge(function, 2, "exact census child and release")?;
                colors[target] = 1;
                frontier.push(Cursor {
                    node: target,
                    next: 0,
                    counts: TerminalCounts::ZERO,
                });
            }
            1 => return Err(replay_error(function, "exact census reached a cycle")),
            2 => {
                cursor.counts = cursor.counts.add(counts[target]);
                cursor.next += 1;
            }
            _ => unreachable!("census owns its colors"),
        }
    }
    Ok(counts[root].census())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum Strategy {
    Enumerated,
    LexicalFactored,
    RefusedArithmeticOverflow,
}

struct Candidate {
    built: BuiltComparison,
    hir: Census,
    plan: Census,
    strategy: Strategy,
}
impl Candidate {
    #[cfg(test)]
    fn reconstruct(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        if function.cleanup_plan.lexical_control.is_none() {
            return Err(replay_error(
                function,
                "lexical strategy requires a lexical function",
            ));
        }
        let built = build_comparison(program, function, budget, None, None)?;
        Self::from_built(function, built, budget)
    }
    fn from_built(
        function: &ResolvedFunction,
        built: BuiltComparison,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let hir = census(function, &built, built.hir_root, budget)?;
        let plan = census(function, &built, built.plan_root, budget)?;
        let strategy = match (hir, plan) {
            (Census::ArithmeticOverflow { .. }, _) | (_, Census::ArithmeticOverflow { .. }) => {
                Strategy::RefusedArithmeticOverflow
            }
            (Census::Known { .. }, Census::Known { .. }) => Strategy::Enumerated,
            _ => Strategy::LexicalFactored,
        };
        // No equality or ownership authority follows from this census.
        // The backing remains owned and charged through the selected proof.
        Ok(Self {
            built,
            hir,
            plan,
            strategy,
        })
    }
}

#[cfg(test)]
#[path = "strategy/activation_tests.rs"]
mod activation_tests;
#[cfg(test)]
#[path = "strategy/coverage_tests.rs"]
mod coverage_tests;
#[cfg(test)]
#[path = "strategy/hostile_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "strategy/tests.rs"]
mod tests;

/// Called only after structural IDs, HIR/certificates, acyclicity and the
/// unchanged exact ownership-state replay have authenticated the lexical plan.
pub(in crate::cleanup_plan::replay) fn validate_authenticated(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    expression_count: usize,
) -> Result<(), Diagnostic> {
    let built = build_comparison(program, function, budget, helpers, None)?;
    let mut candidate = Candidate::from_built(function, built, budget)?;
    if candidate.strategy == Strategy::RefusedArithmeticOverflow {
        return Err(replay_error(
            function,
            "cleanup replay exact terminal multiplicity overflowed",
        ));
    }
    // Selected physical replay has no unmetered legacy census fallback.
    // Its independently closed provider/type profile is unchanged.
    if budget.selected() {
        candidate.strategy = Strategy::LexicalFactored;
    }
    #[cfg(test)]
    if let Some(forced) = forced_strategy() {
        candidate.strategy = forced;
    }
    match candidate.strategy {
        Strategy::Enumerated => {
            if budget.selected() {
                return Err(replay_error(
                    function,
                    "selected physical replay requires its metered factored strategy",
                ));
            }
            // The actual census attempt remains spent. The legacy ordinary
            // route keeps its exact path/work ceilings and enumerating proof.
            let upper =
                lexical_census::validate_size_budget_and_skeleton(program, function, || {
                    Ok(expression_count)
                })?;
            reserve_enumerated(function, budget, upper)?;
            validate_typed_control_skeleton_with_index(program, function, budget, helpers, None)
        }
        Strategy::LexicalFactored => {
            let BuiltComparison {
                automaton,
                sink,
                hir_root,
                plan_root,
            } = &candidate.built;
            let mut canonical = canonical_shared::Canonicalizer::new(automaton, *sink);
            let mut receiving =
                canonical_shared::closure_receiving::ClosureReceiving::acquire(function, budget)?;
            budget.reserve_array::<(usize, u64)>(function, 1, "lexical HIR weighted root")?;
            let hir =
                canonical.id_with_receiving(function, vec![(*hir_root, 1)], budget, &mut receiving);
            #[cfg(test)]
            instrumentation::phase(function, budget, "HIR canonicalization", None);
            let hir = hir?;
            budget.reserve_array::<(usize, u64)>(function, 1, "lexical plan weighted root")?;
            let plan = canonical.id_with_receiving(
                function,
                vec![(*plan_root, 1)],
                budget,
                &mut receiving,
            );
            #[cfg(test)]
            instrumentation::phase(function, budget, "plan canonicalization", None);
            let plan = plan?;
            match (hir, plan) {
                (Some(hir), Some(plan)) if hir == plan => Ok(()),
                (Some(_), Some(_)) => Err(skeleton_mismatch(function)),
                _ => Err(replay_error(
                    function,
                    "cleanup replay factored trace multiplicity overflowed",
                )),
            }
        }
        Strategy::RefusedArithmeticOverflow => unreachable!("overflow census already refused"),
    }
}

/// Add this function's enumerating reservation to the held program pool; never
/// overwrite a reservation belonging to another function and never refund.
pub(in crate::cleanup_plan::replay) fn reserve_enumerated(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    upper: usize,
) -> Result<(), Diagnostic> {
    let held = budget
        .skeleton_remaining
        .checked_add(upper)
        .ok_or_else(|| replay_error(function, "cleanup replay skeleton reservation overflowed"))?;
    let remaining = budget.remaining.checked_sub(upper).ok_or_else(|| {
        replay_error(
            function,
            "cleanup replay skeleton-work preflight exceeds the global budget",
        )
    })?;
    budget.skeleton_remaining = held;
    budget.remaining = remaining;
    Ok(())
}

#[cfg(test)]
thread_local! {
    static FORCED_STRATEGY: std::cell::Cell<Option<Strategy>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn forced_strategy() -> Option<Strategy> {
    FORCED_STRATEGY.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn with_strategy<T>(
    strategy: Strategy,
    operation: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<Strategy>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORCED_STRATEGY.with(|cell| cell.set(self.0));
        }
    }
    let restore = Restore(FORCED_STRATEGY.with(|cell| cell.replace(Some(strategy))));
    let result = operation();
    drop(restore);
    result
}
