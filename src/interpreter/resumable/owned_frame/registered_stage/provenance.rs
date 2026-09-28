//! Private allocation continuity witnesses. These Weak facts hold no backing
//! owner and grant no restore, journal, or cleanup authority.
use super::*;
use std::sync::Weak;

pub(super) struct OwnedAllocationProvenanceV2 {
    next: u32,
    witnesses: BTreeMap<u32, Weak<[u8]>>,
}
impl OwnedAllocationProvenanceV2 {
    /// Called only immediately after this module's checked fresh staging.
    pub(super) fn fresh(root: &Value) -> Result<Self, Diagnostic> {
        let live =
            inventory(&[root]).ok_or_else(|| rejected("fresh allocation inventory differs"))?;
        let next =
            u32::try_from(live.len()).map_err(|_| rejected("allocation namespace exhausted"))?;
        if live.keys().copied().ne(1..=next) {
            return Err(rejected("fresh allocation namespace differs"));
        }
        Ok(Self {
            next,
            witnesses: live,
        })
    }
    pub(super) fn seed(&self, roots: &[&Value]) -> Result<u32, Diagnostic> {
        self.validate(roots)
            .then_some(self.next)
            .ok_or_else(|| rejected("retained allocation witnesses differ"))
    }
    pub(super) fn validate(&self, roots: &[&Value]) -> bool {
        let Some(actual) = inventory(roots) else {
            return false;
        };
        actual.iter().all(|(id, backing)| {
            *id <= self.next
                && self
                    .witnesses
                    .get(id)
                    .is_some_and(|w| Weak::ptr_eq(w, backing))
        }) && self.witnesses.iter().all(|(id, w)| {
            w.strong_count() == 0 || actual.get(id).is_some_and(|a| Weak::ptr_eq(w, a))
        })
    }
    /// This caller owns an actual evaluator seeded with `seed()`. Its counter
    /// may advance on a failed frame even when a new allocation already died;
    /// keep that high-water permanently, registering only actual retained roots.
    /// Only the checked effect-result owner calls this after result acceptance
    /// and acknowledged physical Decision disposal. This witness operation
    /// itself supplies no journal, effect, result or restoration authority.
    pub(super) fn mint_accepted_bytes(
        &mut self,
        roots: &[&Value],
        payload: Vec<u8>,
    ) -> Result<Value, (Vec<u8>, Diagnostic)> {
        if payload.len() > 1024 || !self.validate(roots) {
            return Err((
                payload,
                rejected("accepted allocation inventory/capacity differs"),
            ));
        }
        let Some(next) = self.next.checked_add(1) else {
            return Err((payload, rejected("allocation namespace exhausted")));
        };
        let bytes: Arc<[u8]> = Arc::from(payload);
        self.witnesses.insert(next, Arc::downgrade(&bytes));
        self.next = next;
        Ok(Value::Bytes(OwnedBytesValue {
            allocation: next,
            bytes,
        }))
    }

    pub(super) fn record_frame(&mut self, roots: &[&Value], next: u32) -> Result<(), Diagnostic> {
        if next < self.next {
            return Err(rejected("allocation high-water regressed"));
        }
        let actual =
            inventory(roots).ok_or_else(|| rejected("evaluated allocation inventory differs"))?;
        if actual.iter().any(|(id, w)| {
            *id > next
                || (*id <= self.next
                    && !self
                        .witnesses
                        .get(id)
                        .is_some_and(|old| Weak::ptr_eq(old, w)))
        }) || self.witnesses.iter().any(|(id, w)| {
            w.strong_count() != 0 && !actual.get(id).is_some_and(|a| Weak::ptr_eq(w, a))
        }) {
            return Err(rejected("evaluated allocation continuity differs"));
        }
        self.next = next;
        for (id, backing) in actual {
            self.witnesses.entry(id).or_insert(backing);
        }
        Ok(())
    }
}
fn inventory(roots: &[&Value]) -> Option<BTreeMap<u32, Weak<[u8]>>> {
    let mut output = BTreeMap::new();
    for root in roots {
        let fields = match root {
            Value::Record(v) if Arc::strong_count(v) == 1 => &v.fields,
            Value::Variant(v) if Arc::strong_count(v) == 1 => &v.fields,
            _ => return None,
        };
        for value in fields.values() {
            if let Value::Bytes(v) = value {
                if Arc::strong_count(&v.bytes) != 1
                    || v.allocation == 0
                    || output
                        .insert(v.allocation, Arc::downgrade(&v.bytes))
                        .is_some()
                {
                    return None;
                }
            } else if super::super::super::clone_scalar(value).is_none() {
                return None;
            }
        }
    }
    Some(output)
}
#[cfg(test)]
mod tests;
