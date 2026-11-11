//! Forward the live schema payer to the complete original renewal decisions.
use super::paid_type_facts::Payer;
use super::{Diagnostic, ExpressionId, ResolvedFunction};
use crate::cleanup::ReplayContext;
use crate::hir::funded::{self, FundedError, Reserve};
use crate::hir::{ResolvedBinding, TypeFactsPayer};
use std::collections::BTreeMap;
use std::mem::{size_of, size_of_val};

pub(super) enum Kind {
    Byte,
    Text,
    Vector,
    Iterator,
}

pub(super) fn includes(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    payer: &mut Payer<'_>,
    kind: Kind,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    let _phase = crate::test_allocation_observer::function_phase_guard(match kind {
        Kind::Byte => "schema-byte-renewal",
        Kind::Text => "schema-text-renewal",
        Kind::Vector => "schema-vector-renewal",
        Kind::Iterator => "schema-iterator-renewals",
    });
    type Found<'a> = BTreeMap<ExpressionId, &'a ResolvedBinding>;
    type ResultMap<'a> = Result<Found<'a>, FundedError<Diagnostic>>;
    type ResultFlag = Result<bool, FundedError<Diagnostic>>;
    payer.for_subject(function)?;
    let result = {
        let mut reserve = |bytes| payer.bytes(bytes);
        let receiving = size_of::<(
            [(&dyn ReplayContext, &ResolvedFunction, &mut Payer<'_>, Kind); 2],
            [Result<bool, Diagnostic>; 2],
            [ResultFlag; 2],
        )>() + size_of_val(&reserve);
        (|| {
            funded::charge(&mut reserve, receiving)?;
            match kind {
                Kind::Iterator => {
                    funded::charge(
                        &mut reserve,
                        size_of::<
                            [(
                                &dyn ReplayContext,
                                &ResolvedFunction,
                                Reserve<'_, Diagnostic>,
                            ); 2],
                        >(),
                    )?;
                    crate::hir::iterator_loop::schema_requires_renewal_funded(
                        program,
                        function,
                        &mut reserve,
                    )
                }
                kind => {
                    funded::charge(
                        &mut reserve,
                        size_of::<(
                            [(&ResolvedFunction, Reserve<'_, Diagnostic>); 2],
                            [ResultMap<'_>; 2],
                            &Found<'_>,
                            bool,
                        )>(),
                    )?;
                    let map = match kind {
                        Kind::Byte => {
                            crate::byte_ops::same_owner_set_bindings_funded(function, &mut reserve)
                        }
                        Kind::Text => {
                            crate::string_ops::replacement::bindings_funded(function, &mut reserve)
                        }
                        Kind::Vector => {
                            funded::charge(&mut reserve, size_of::<[&dyn ReplayContext; 2]>())?;
                            crate::hir::vec_loop_renewal::bindings_in_funded(
                                program,
                                function,
                                &mut reserve,
                            )
                        }
                        Kind::Iterator => unreachable!("iterator returns its own selection"),
                    }?;
                    Ok(!map.is_empty())
                }
            }
        })()
    };
    match result {
        Ok(selected) => Ok(selected),
        Err(FundedError::Refused(error)) => Err(error),
        // These collectors handle their own original census verdict before
        // returning a map. A different refusal must remain observable.
        Err(
            FundedError::Layout(reason)
            | FundedError::Census(reason)
            | FundedError::Unfunded(reason),
        ) => Err(payer.construction_refusal(reason)),
    }
}
