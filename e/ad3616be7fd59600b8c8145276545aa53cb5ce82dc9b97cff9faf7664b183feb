//! One combined zero/single edge slot; branching alone acquires both Vec headers.
//! The slot retains actual usize targets, including hostile usize::MAX inputs.
use super::{Diagnostic, Label, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

#[derive(Default)]
pub(super) enum Node {
    #[default]
    Empty,
    Movement((Label, usize)),
    Epsilon(usize),
    Many(Box<Many>),
}
pub(super) struct Many {
    pub(super) moves: Vec<(Label, usize)>,
    pub(super) epsilon: Vec<usize>,
}
impl Node {
    pub(super) fn moves(&self) -> &[(Label, usize)] {
        match self {
            Self::Movement(edge) => std::slice::from_ref(edge),
            Self::Many(many) => &many.moves,
            Self::Empty | Self::Epsilon(_) => &[],
        }
    }
    pub(super) fn epsilon(&self) -> &[usize] {
        match self {
            Self::Epsilon(target) => std::slice::from_ref(target),
            Self::Many(many) => &many.epsilon,
            Self::Empty | Self::Movement(_) => &[],
        }
    }
    // The caller pays this BEFORE evaluating the new helper receiver/arguments.
    // Five direct Diagnostic Results: caller payment, promotion return, Box
    // reserve_array, and both Vec reserves. Full old reserve_vec paths remain.
    pub(super) const PROMOTION_CONTROLS: usize = 5 * size_of::<Result<(), Diagnostic>>()
        + 2 * size_of::<Many>()
        + 2 * size_of::<Box<Many>>()
        + size_of::<Self>()
        + 2 * size_of::<Vec<(Label, usize)>>()
        + 2 * size_of::<Vec<usize>>()
        + size_of::<Self>() // mem::replace Empty initializer
        + size_of::<Self>() // returned old enum receiving local
        + size_of::<(Label, usize)>()
        + 2 * size_of::<&mut Self>()
        + 2 * size_of::<&mut Many>()
        + 2 * size_of::<&mut Box<Many>>()
        + size_of::<&ResolvedFunction>()
        + size_of::<&mut ReplayBudget>()
        + 2 * size_of::<bool>();
    pub(super) fn promote(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        movement: bool,
    ) -> Result<(), Diagnostic> {
        // All fallible acquisition completes before the existing inline value
        // moves. Box/header/carrier payment is at the caller frontier above.
        budget.reserve_array::<Many>(function, 1, "factored compact node owner")?;
        let mut many = Box::new(Many {
            moves: Vec::new(),
            epsilon: Vec::new(),
        });
        budget.reserve_vec(
            function,
            &mut many.moves,
            usize::from(matches!(self, Self::Movement(_))) + usize::from(movement),
            "factored automaton moves",
        )?;
        budget.reserve_vec(
            function,
            &mut many.epsilon,
            usize::from(matches!(self, Self::Epsilon(_))) + usize::from(!movement),
            "factored automaton epsilon moves",
        )?;
        match std::mem::replace(self, Self::Empty) {
            Self::Movement(edge) => many.moves.push(edge),
            Self::Epsilon(target) => many.epsilon.push(target),
            Self::Empty | Self::Many(_) => unreachable!("only an occupied inline slot promotes"),
        }
        *self = Self::Many(many);
        Ok(())
    }
}

#[cfg(test)]
#[path = "compact_nodes/before/mod.rs"]
pub(super) mod before;
