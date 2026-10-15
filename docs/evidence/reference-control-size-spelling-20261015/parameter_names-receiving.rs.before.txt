//! Same borrowed ordinals, membership and sort, on the original affine payer.
use super::{BorrowedNames, Ordinals, INLINE};
use crate::ast::TypeParameterDeclaration;
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::constructor_failure::{
    receiving::{Receiver, Setup},
    typed::Failure,
};
use std::mem::{size_of, size_of_val};

fn physical(requested: usize, owner: &mut Receiver) -> Failure {
    match owner.physical_request(requested) {
        Setup::Budget { .. } => refused(),
        other => other.into(),
    }
}
fn refused() -> Failure {
    Failure::one(Diagnostic::io_static(
        "SPX-G171",
        "generic parameter name index exceeds its actual backing reservation",
    ))
}
pub(in crate::workspace_graph::generic_type_import) fn new<'a>(
    parameters: &'a [TypeParameterDeclaration],
    owner: &mut Receiver,
) -> Result<BorrowedNames<'a>, Failure> {
    owner.entry::<BorrowedNames<'a>>()?;
    owner.reserve(
        size_of_val(&parameters)
            + 3 * size_of::<usize>() // count, payload and full request
            + size_of::<Ordinals>()
            + size_of::<Option<usize>>()
            // Both inline refusal returns and the exact-request Copy adapter
            // can be reached only after this acquisition has refused. Acquire
            // their actual declared controls before that boundary.
            + 2 * size_of::<Failure>()
            + size_of::<Setup>()
            + 2 * size_of::<usize>() // helper and physical_request arguments
            + 2 * size_of::<&mut Receiver>(),
    )?;
    let count = parameters.len();
    let ordinals = if count <= INLINE {
        owner.reserve(size_of::<BorrowedNames<'a>>() + size_of::<[usize; INLINE]>())?;
        let mut values = [0; INLINE];
        for (ordinal, value) in values[..count].iter_mut().enumerate() {
            *value = ordinal;
        }
        Ordinals::Inline { values, len: count }
    } else {
        let payload = count
            .checked_mul(size_of::<usize>())
            .ok_or_else(|| physical(usize::MAX, owner))?;
        let bytes = size_of::<BorrowedNames<'a>>()
            .checked_add(payload)
            .ok_or_else(|| physical(usize::MAX, owner))?;
        // The old full Self+payload request remains. New returned/control and
        // transient Vec initializer layouts are additional real storage.
        owner.reserve(size_of::<Vec<usize>>())?;
        if let Err(failure) = owner.reserve(bytes) {
            return Err(match failure {
                Setup::Budget { .. } => refused(),
                other => other.into(),
            });
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| physical(payload, owner))?;
        if values.capacity() != count {
            return Err(physical(payload, owner));
        }
        values.extend(0..count);
        Ordinals::Heap(values)
    };
    let mut result = BorrowedNames {
        parameters,
        ordinals,
        #[cfg(test)]
        comparisons: std::cell::Cell::new(0),
    };
    #[cfg(test)]
    let comparisons = &result.comparisons;
    result
        .ordinals
        .values_mut()
        .sort_unstable_by(|left, right| {
            #[cfg(test)]
            comparisons.set(comparisons.get() + 1);
            parameters[*left].name.cmp(&parameters[*right].name)
        });
    Ok(result)
}

#[cfg(test)]
pub(in crate::workspace_graph::generic_type_import) fn observation<'a>(
    names: &'a BorrowedNames<'_>,
) -> (*const TypeParameterDeclaration, usize, &'a [usize]) {
    (
        names.parameters.as_ptr(),
        names.comparisons.get(),
        names.ordinals.values(),
    )
}
