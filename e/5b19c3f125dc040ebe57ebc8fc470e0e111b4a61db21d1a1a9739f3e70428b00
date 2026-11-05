//! Exact caller-paid private ordered CFG scratch. No attached plan supplies
//! lookup or sizing authority; the public builder keeps its old BTree policy.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
#[cfg(test)]
mod live_receivers_tests;
#[cfg(test)]
mod planner_receivers_tests;
#[cfg(test)]
mod tests;
mod tree;

pub(super) struct Map<K, V> {
    root: Option<Box<tree::Node<K, V>>>,
    len: usize,
}
impl<K: Ord, V> Map<K, V> {
    pub(super) fn new(payer: &CfgReservation) -> Result<Self, Diagnostic> {
        payer
            .reserve(std::mem::size_of::<Self>())
            .map_err(|_| storage::marker())?;
        Ok(Self { root: None, len: 0 })
    }
    pub(super) fn get(&self, key: &K) -> Option<&V> {
        tree::get(self.root.as_deref(), key)
    }
    pub(super) fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
    fn insert(
        &mut self,
        key: K,
        value: V,
        payer: &CfgReservation,
    ) -> Result<Option<V>, Diagnostic> {
        payer.reserve(0).map_err(|_| storage::marker())?;
        let mut leaf = if self.get(&key).is_none() {
            let Some(bytes) = std::mem::size_of::<tree::Node<K, V>>().checked_mul(2) else {
                payer.arithmetic();
                return Err(storage::marker());
            };
            let _allocation_stage = crate::bounded_output::loan_allocation_stage_guard(
                crate::bounded_output::ReservationStage::LoanOrderedDirectoryNode,
            );
            // Box heap and literal initializer are paid before construction.
            payer.reserve(bytes).map_err(|_| storage::marker())?;
            Some(Box::new(tree::Node::leaf(key, value)))
        } else {
            return Ok(tree::replace(&mut self.root, &key, value));
        };
        // No allocation, payer call or failure follows the prepared leaf.
        tree::insert(&mut self.root, &mut leaf);
        self.len += 1;
        Ok(None)
    }
}
pub(super) struct Set<K>(Map<K, ()>);
impl<K: Ord> Set<K> {
    pub(super) fn new(payer: &CfgReservation) -> Result<Self, Diagnostic> {
        Map::new(payer).map(Self)
    }
    pub(super) fn contains(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }
    pub(super) fn is_empty(&self) -> bool {
        self.0.len == 0
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len
    }
    pub(super) fn iter(&self, payer: &CfgReservation) -> Result<tree::Keys<'_, K, ()>, Diagnostic> {
        payer
            .reserve(std::mem::size_of::<tree::Keys<'_, K, ()>>())
            .map_err(|_| storage::marker())?;
        Ok(tree::keys(self.0.root.as_deref()))
    }
    pub(super) fn remove(&mut self, key: &K, payer: &CfgReservation) -> Result<bool, Diagnostic> {
        payer.reserve(0).map_err(|_| storage::marker())?;
        let removed = tree::remove(&mut self.0.root, key).is_some();
        if removed {
            self.0.len -= 1;
        }
        Ok(removed)
    }
    pub(super) fn into_vec(mut self, payer: &CfgReservation) -> Result<Vec<K>, Diagnostic> {
        let mut values = vector(self.0.len, payer)?;
        tree::drain(self.0.root.take(), &mut values);
        Ok(values)
    }
}
pub(super) fn map_insert<K: Ord, V>(
    values: &mut Map<K, V>,
    key: K,
    value: V,
    payer: Option<&CfgReservation>,
) -> Result<Option<V>, Diagnostic> {
    values.insert(key, value, payer.ok_or_else(storage::marker)?)
}
pub(super) fn set_insert<K: Ord>(
    values: &mut Set<K>,
    key: K,
    payer: Option<&CfgReservation>,
) -> Result<bool, Diagnostic> {
    values
        .0
        .insert(key, (), payer.ok_or_else(storage::marker)?)
        .map(|old| old.is_none())
}
pub(super) fn vector<T>(capacity: usize, payer: &CfgReservation) -> Result<Vec<T>, Diagnostic> {
    payer
        .reserve(std::mem::size_of::<Vec<T>>())
        .map_err(|_| storage::marker())?;
    storage::vector(capacity, Some(payer))
}
pub(super) fn fresh<T>(payer: &CfgReservation) -> Result<Vec<T>, Diagnostic> {
    vector(0, payer)
}
pub(super) enum Nodes {
    Legacy(BTreeMap<LoanProgramPoint, u16>),
    Funded(Map<LoanProgramPoint, u16>),
    // One complete expression key, both actual before/after point identities.
    // The public point vector retains every complete LoanProgramPoint unchanged.
    PhasePairs(Map<ExpressionId, [u16; 2]>),
    PhaseRoots(Map<ExpressionId, Option<[u16; 2]>>),
}
impl Nodes {
    #[cfg(test)]
    pub(super) fn complete_len_for_test(&self) -> usize {
        match self {
            Self::Legacy(values) => values.len(),
            Self::Funded(values) => values.len,
            Self::PhasePairs(values) => values.len * 2,
            Self::PhaseRoots(values) => values.len * 2,
        }
    }
    pub(super) fn get(&self, key: &LoanProgramPoint) -> Option<&u16> {
        match self {
            Self::Legacy(values) => values.get(key),
            Self::Funded(values) => values.get(key),
            Self::PhasePairs(values) => values.get(&key.expression).map(|phases| {
                &phases[match key.phase {
                    LoanPointPhase::Before => 0,
                    LoanPointPhase::After => 1,
                }]
            }),
            Self::PhaseRoots(values) => {
                values
                    .get(&key.expression)
                    .and_then(Option::as_ref)
                    .map(|phases| {
                        &phases[match key.phase {
                            LoanPointPhase::Before => 0,
                            LoanPointPhase::After => 1,
                        }]
                    })
            }
        }
    }
}

pub(super) fn singleton_set<K: Ord>(
    key: K,
    payer: Option<&CfgReservation>,
) -> Result<Set<K>, Diagnostic> {
    let payer = payer.ok_or_else(storage::marker)?;
    let mut values = Set::new(payer)?;
    set_insert(&mut values, key, Some(payer))?;
    Ok(values)
}
pub(super) fn clone_set<K: Ord + Copy>(
    values: &Set<K>,
    payer: Option<&CfgReservation>,
) -> Result<Set<K>, Diagnostic> {
    let payer = payer.ok_or_else(storage::marker)?;
    let mut copy = Set::new(payer)?;
    for value in values.iter(payer)? {
        set_insert(&mut copy, *value, Some(payer))?;
    }
    Ok(copy)
}
