//! Direct ordinary output producer; legacy returning producer remains intact.
use super::*;
pub(crate) fn bindings_into<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    output: &mut RenewalBindings<'a>,
    reserve: Reserve<'_, E>,
    work: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<(), FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut found = crate::hir::iterator_loop::renewal_bindings_funded(program, function, reserve)?;
    let vec_sites = crate::hir::vec_loop_renewal::bindings_in_funded(program, function, reserve)?;
    funded::extend_map(&mut found, vec_sites, reserve)?;
    let byte_sites = crate::byte_ops::same_owner_set_bindings_funded(function, reserve)?;
    funded::extend_map(&mut found, byte_sites, reserve)?;
    let text_sites = crate::string_ops::replacement::bindings_funded(function, reserve)?;
    funded::extend_map(&mut found, text_sites, reserve)?;
    let helpers = match index.and_then(|index| index.function_sites(function)) {
        Some(sites) => HelperSites::Borrowed(sites),
        None if index.is_some() => HelperSites::Owned(Default::default()),
        // The ordinary receiver turns only the census work/depth verdict into
        // no helper sites; payer refusals and physical layout/allocation
        // failures are never swallowed.
        None => match crate::loop_calls::byte_writer::checked_bindings_funded_with_work(
            program, function, reserve, work,
        ) {
            Ok(sites) => HelperSites::Owned(sites),
            Err(FundedError::Census(_)) => HelperSites::Owned(Default::default()),
            Err(failure) => return Err(failure),
        },
    };
    output.legacy = found;
    output.appends = BTreeMap::new();
    output.helpers = helpers;
    Ok(())
}
