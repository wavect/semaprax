//! Explicit unfunded compatibility acquisition. No ambient payer is queried.
use super::*;
impl<T: NominalElement> NominalPayload<T> {
    pub(super) fn detach_legacy_unfunded(&mut self) {
        let Storage::Shared(owner) = &mut self.storage else {
            unreachable!("legacy shared owner")
        };
        assert!(owner.mode == OwnerMode::LegacyUnfunded);
        let values = if let Some(owner) = Arc::get_mut(owner) {
            std::mem::take(&mut owner.values)
        } else {
            owner
                .values
                .iter()
                .map(NominalElement::clone_legacy_unfunded)
                .collect()
        };
        self.storage = Storage::Owned(values);
    }
    pub(crate) fn from_legacy_vec_unfunded(values: &mut Vec<T>) -> Self {
        let mut capacity = 0;
        for value in values.iter() {
            let required = value.drop_capacity(&mut |_| Ok::<_, std::convert::Infallible>(()));
            let required = match required {
                Ok(n) => n,
                Err(impossible) => match impossible {},
            };
            capacity = capacity.max(required);
        }
        // These are ordinary infallible legacy allocations, not a controlled
        // budget acquisition. In particular an existing active account pays 0.
        let scratch = Vec::with_capacity(capacity);
        Self {
            storage: Storage::Shared(Arc::new(Owner {
                mode: OwnerMode::LegacyUnfunded,
                values: std::mem::take(values),
                scratch,
            })),
        }
    }
}
#[cfg(test)]
mod tests;

pub(super) mod clone;

#[cfg(test)]
mod source_tests;

#[cfg(test)]
#[path = "legacy/source_tests_main8a.rs"]
mod source_tests_main8a;

#[cfg(test)]
#[path = "legacy/source_tests_maina3.rs"]
mod source_tests_maina3;
