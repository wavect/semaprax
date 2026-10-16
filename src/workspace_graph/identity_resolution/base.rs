//! Actual core invocation base lease forwarded through selective and fresh routes.
use super::*;
use crate::hir::identity_resolution::NominalReceiverFailure;
use crate::hir::prelude_bundle::InvocationFailure;
use crate::project::incremental::paid_resolution::base::BaseSelectiveFailure;
pub(in crate::workspace_graph) fn resolve_module_with_core<'s>(
    core: &mut crate::workspace_graph::expected_projection::CoreProviderProofs<'_>,
    frontend: Option<&FrontendPass>,
    path: &str,
    synthetic: &'s Input<'_>,
    private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
) -> Result<Parts, ModuleFailure> {
    // This existing core captured the original active ledger. A foreign report
    // owner cannot authorize acquisition or reuse of its retained base payload.
    core.bind_base_owner();
    resolve_module_with_base(&mut core.base_prelude, frontend, path, synthetic, private)
}
pub(in crate::workspace_graph) fn resolve_module_with_base<'s>(
    invocation: &mut crate::hir::prelude_bundle::Invocation<Refused>,
    frontend: Option<&FrontendPass>,
    path: &str,
    synthetic: &'s Input<'_>,
    private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
) -> Result<Parts, ModuleFailure> {
    // Every non-hit module acquires exactly once, on the same Rc<Budget> the
    // caller's resolver-cost interval measures; no fresh quota is created.
    let mut owner = if frontend.is_some_and(FrontendPass::retains_checked_hir) {
        staged!(SelectiveResolution, IdentityReservation::acquire())
    } else {
        staged!(FreshResolution, IdentityReservation::acquire())
    }
    .map_err(ModuleFailure::Setup)?;
    #[cfg(test)]
    observe_acquired(&owner);
    // Both routes execute inside the same linked-source authentication scope.
    // Evaluating a selective Result before these guards would reject retained
    // imported wrapper bodies before the guards could authenticate them.
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(|| {
            let selective = staged!(
                SelectiveResolution,
                frontend.and_then(|cache| {
                    synthetic
                        .owned()
                        .and_then(|input| {
                            cache.resolve_functions_paid_with_base(
                                path, input, private, invocation, &mut owner,
                            )
                        })
                        .or_else(|| {
                            synthetic.borrowed().and_then(|view| {
                                if cache.permits_borrowed_cold_source(path) {
                                    cache.resolve_borrowed_cold_source_paid_with_base(
                                        path, view, private, invocation, &mut owner,
                                    )
                                } else {
                                    cache.resolve_borrowed_source_paid_with_base(
                                        path, view, private, invocation, &mut owner,
                                    )
                                }
                            })
                        })
                })
            );
            if let Some(selective) = selective {
                selective.map(|resolved| resolved.into_parts())
            } else {
                staged!(FreshResolution, {
                    synthetic
                        .resolve_paid_with_base(private, invocation, &mut owner)
                        .map(|(resolved, work)| {
                            let selected = work.static_text();
                            (resolved, BTreeMap::new(), 0, selected)
                        })
                        .map_err(BaseSelectiveFailure::Resolve)
                })
            }
        })
    })
    .map_err(|failure| match failure {
        BaseSelectiveFailure::Project(report) => ModuleFailure::Report(report),
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::SourceStop(abort)) => {
            ModuleFailure::SourceStop(abort)
        }
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Resolve(Failure::Source(report))) => {
            ModuleFailure::Report(report)
        }
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Resolve(Failure::Semantic(
            error,
        ))) => ModuleFailure::Report(vec![error]),
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Resolve(Failure::Identity(_))) => {
            ModuleFailure::Report(owner.publish())
        }
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Base(InvocationFailure::Bytes(
            _,
        ))) => ModuleFailure::Report(owner.publish()),
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Base(InvocationFailure::Work(_))) => {
            ModuleFailure::Report(owner.publish_base_prelude_work())
        }
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Base(
            InvocationFailure::ExtensionWork(_),
        )) => ModuleFailure::Report(owner.publish_prelude_extension_work()),
        BaseSelectiveFailure::Resolve(NominalReceiverFailure::Base(
            InvocationFailure::Producer(_),
        )) => ModuleFailure::Report(owner.publish_base_prelude_producer()),
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod affinity_tests;

#[cfg(test)]
mod vector_tests;

pub(in crate::workspace_graph) mod cache_billing;
