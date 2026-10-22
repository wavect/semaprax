//! Explicit-payer funded assembly of the per-function renewal lookup. Same census
//! order and `extend` precedence as `bindings_with_index`; every receiver and
//! map merge is debited on the caller's payer first and its refusal returned
//! unchanged. Nothing reads the production ACTIVE ledger.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

pub(crate) fn bindings_with_index_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    reserve: Reserve<'_, E>,
) -> Result<RenewalBindings<'a>, FundedError<E>> {
    bindings_with_index_funded_with_work(program, function, index, reserve, &mut |_| Ok(()))
}

/// Same assembly; the standalone helper census additionally reports its
/// admission work units on `work` while its own ceilings run unchanged.
pub(crate) fn bindings_with_index_funded_with_work<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    reserve: Reserve<'_, E>,
    work: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<RenewalBindings<'a>, FundedError<E>> {
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
    Ok(RenewalBindings {
        legacy: found,
        appends: BTreeMap::new(),
        helpers,
    })
}

/// Funded single-site helper lookup for the standalone (no held index) route;
/// a census work/depth verdict is "no site", every other failure propagates.
/// Held-index and primitive routes allocate nothing and stay the originals.
#[cfg(test)]
pub(crate) fn helper_binding_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
    reserve: Reserve<'_, E>,
) -> Result<Option<&'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    match crate::loop_calls::byte_writer::checked_bindings_funded(program, function, reserve) {
        Ok(mut sites) => Ok(sites.remove(at)),
        Err(FundedError::Census(_)) => Ok(None),
        Err(failure) => Err(failure),
    }
}

/// Funded single-site twin of `binding_with_index`, same precedence: primitive
/// renewals, helper site, string replacement, bytes, vec, iterator. A route
/// that allocates (standalone helper census, each collector) is paid before it
/// acquires anything and only runs when every earlier route found nothing. The
/// census work/depth verdict is the original absence of a helper site; payer,
/// physical and account refusals propagate and are never read as absence.
#[cfg(test)]
pub(crate) fn binding_with_index_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
    index: Option<&crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    reserve: Reserve<'_, E>,
) -> Result<Option<&'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    if let Some(sites) = program.primitive_renewals(function) {
        return Ok(sites.get(at));
    }
    let helper = match index {
        Some(index)
            if program
                .full_program()
                .is_some_and(|program| index.matches_program(program)) =>
        {
            index
                .function_sites(function)
                .and_then(|sites| sites.get(at))
        }
        Some(_) => None,
        None => helper_binding_funded(program, function, at, reserve)?,
    };
    if helper.is_some() {
        return Ok(helper);
    }
    if let Some(found) =
        crate::string_ops::replacement::bindings_funded(function, reserve)?.remove(at)
    {
        return Ok(Some(found));
    }
    if let Some(found) =
        crate::byte_ops::same_owner_set_bindings_funded(function, reserve)?.remove(at)
    {
        return Ok(Some(found));
    }
    if let Some(found) =
        crate::hir::vec_loop_renewal::bindings_in_funded(program, function, reserve)?.remove(at)
    {
        return Ok(Some(found));
    }
    crate::hir::iterator_loop::renewal_binding_funded(program, function, at, reserve)
}

#[cfg(test)]
pub(crate) fn bindings_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<RenewalBindings<'a>, FundedError<E>> {
    bindings_with_index_funded(program, function, None, reserve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::assert_every_boundary;

    const HELPERS: &str = include_str!("../../../tests/owned_data/byte_helper_renewal.spx");
    const LOOPS: &str = r#"module test.renewal_funded;
@id("app.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let mut values=vec_with_capacity<i64>(1usize);
 let mut index=0usize;
 while index<1usize {
  buffer=bytes_set(buffer,index,65u8);
  values=vec_push<i64>(values,1);
  index=index+1usize;
  0
 }
 if byte_len(bytes_as_slice(buffer))==2usize && vec_len<i64>(values)==1usize {7}else{0}
}
"#;

    type Snapshot = (Vec<(String, String)>, Vec<(String, Option<String>, bool)>);

    /// Legacy entries plus the helper lookup at every known site.
    fn snapshot(sites: &RenewalBindings<'_>, candidates: &[ExpressionId]) -> Snapshot {
        let legacy = sites
            .legacy
            .iter()
            .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
            .collect();
        let lookups = candidates
            .iter()
            .map(|at| {
                (
                    at.as_str().to_owned(),
                    sites.get(at).map(|binding| binding.id.as_str().to_owned()),
                    sites.is_helper(at),
                )
            })
            .collect();
        (legacy, lookups)
    }

    fn candidates(
        program: &crate::hir::ResolvedProgram,
        function: &ResolvedFunction,
    ) -> Vec<ExpressionId> {
        let mut ids: std::collections::BTreeSet<_> =
            bindings(program, function).legacy.into_keys().collect();
        ids.extend(
            crate::loop_calls::byte_writer::checked_bindings(program, function)
                .unwrap()
                .into_iter()
                .map(|(at, _)| at.clone()),
        );
        ids.into_iter().collect()
    }

    #[test]
    fn funded_assembly_matches_the_ordinary_receiver_on_every_family() {
        let mut total = 0;
        for (source, name) in [(HELPERS, "funded-helpers.spx"), (LOOPS, "funded-loops.spx")] {
            let checked = crate::check(source, name).unwrap();
            let program = crate::hir::resolve(&checked).unwrap();
            for function in &program.functions {
                let ids = candidates(&program, function);
                let old = snapshot(&bindings(&program, function), &ids);
                total += old.0.len() + old.1.len();
                let (value, calls, _) = assert_every_boundary(|reserve| {
                    let sites = bindings_funded(&program, function, reserve)?;
                    assert!(sites.appends().is_empty());
                    assert_eq!(
                        sites.has_helpers(),
                        bindings(&program, function).has_helpers()
                    );
                    Ok(snapshot(&sites, &ids))
                });
                assert_eq!(value, old, "{}", function.id.as_str());
                assert!(calls > 0);
            }
        }
        assert!(total > 4, "the corpus must exercise renewal sites");
    }

    #[test]
    fn funded_assembly_keeps_duplicate_identity_precedence() {
        let checked = crate::check(LOOPS, "funded-duplicate.spx").unwrap();
        let mut program = crate::hir::resolve(&checked).unwrap();
        let byte_at = crate::byte_ops::same_owner_set_bindings(&program.functions[0])
            .keys()
            .next()
            .unwrap()
            .clone();
        let function = &mut program.functions[0];
        let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("main block")
        };
        let crate::hir::ResolvedStatement::While { body, .. } = statements
            .iter_mut()
            .find(|statement| matches!(statement, crate::hir::ResolvedStatement::While { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
            panic!("loop block")
        };
        for statement in statements {
            if let crate::hir::ResolvedStatement::Assign { binding, value, .. } = statement {
                if binding.name == "values" {
                    value.id = byte_at.clone();
                }
            }
        }
        let function = &program.functions[0];
        let ids = candidates(&program, function);
        let old = snapshot(&bindings(&program, function), &ids);
        let (value, _, _) = assert_every_boundary(|reserve| {
            Ok(snapshot(
                &bindings_funded(&program, function, reserve)?,
                &ids,
            ))
        });
        assert_eq!(value, old);
        assert_eq!(
            bindings(&program, function).get(&byte_at).unwrap().name,
            "buffer"
        );
    }

    #[test]
    fn funded_assembly_preserves_held_index_brand_and_rejects_foreign_or_stale_indexes() {
        let checked = crate::check(HELPERS, "funded-index.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let index = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let ids = candidates(&program, function);
        let old = snapshot(&bindings_with_index(&program, function, Some(&index)), &ids);
        assert!(old.1.iter().any(|(_, _, helper)| *helper));
        let (value, _, _) = assert_every_boundary(|reserve| {
            Ok(snapshot(
                &bindings_with_index_funded(&program, function, Some(&index), reserve)?,
                &ids,
            ))
        });
        assert_eq!(value, old);
        // A foreign program's index and a cloned (stale) function publish no helper sites.
        let foreign = program.clone();
        let other = crate::loop_calls::byte_writer::ModuleIndex::new(&foreign).unwrap();
        let stale = function.clone();
        let mut payer = funded::payer::Payer::unlimited();
        let wrong =
            bindings_with_index_funded(&program, function, Some(&other), &mut |b| payer.pay(b))
                .unwrap();
        let cloned =
            bindings_with_index_funded(&program, &stale, Some(&index), &mut |b| payer.pay(b))
                .unwrap();
        for at in &ids {
            assert_eq!(
                wrong.is_helper(at),
                bindings_with_index(&program, function, Some(&other)).is_helper(at)
            );
            assert!(!cloned.is_helper(at));
        }
    }

    #[test]
    fn funded_single_site_helper_lookup_matches_the_standalone_original() {
        let checked = crate::check(HELPERS, "funded-helper-site.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let sites = candidates(&program, function);
        assert!(!sites.is_empty());
        for at in &sites {
            let old = helper_binding_with_index(&program, function, at, None)
                .map(|b| b.id.as_str().to_owned());
            let (value, calls, _) = assert_every_boundary(|reserve| {
                Ok(helper_binding_funded(&program, function, at, reserve)?
                    .map(|b| b.id.as_str().to_owned()))
            });
            assert_eq!(value, old);
            assert!(calls > 0);
        }
    }

    #[test]
    fn census_verdict_is_empty_helpers_but_payer_refusal_is_never_empty() {
        let checked = crate::check(HELPERS, "funded-census-verdict.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut deep = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
            panic!("main block")
        };
        for _ in 0..600 {
            let inner = tail.clone();
            tail.kind = crate::hir::ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: inner,
            };
        }
        let old = bindings(&program, &deep);
        assert!(
            !old.has_helpers(),
            "original normal receiver: empty inventory"
        );
        // Every ordinal: either the original empty inventory (census verdict)
        // or the payer's own refusal, never a silent empty result for a refusal.
        let (value, calls, _) = assert_every_boundary(|reserve| {
            let sites = bindings_funded(&program, &deep, reserve)?;
            Ok((sites.legacy.len(), sites.has_helpers()))
        });
        assert_eq!(value, (old.legacy.len(), false));
        assert!(calls > 0);
    }

    type Held<'p> = Option<&'p crate::loop_calls::byte_writer::ModuleIndex<'p>>;

    fn single_site_parity(
        program: &crate::hir::ResolvedProgram,
        function: &ResolvedFunction,
        ids: &[ExpressionId],
        held: Held<'_>,
    ) -> usize {
        let mut found = 0;
        for at in ids {
            let old = binding_with_index(program, function, at, held)
                .map(|binding| binding.id.as_str().to_owned());
            let (value, calls, _) = assert_every_boundary(|reserve| {
                Ok(
                    binding_with_index_funded(program, function, at, held, reserve)?
                        .map(|binding| binding.id.as_str().to_owned()),
                )
            });
            assert_eq!(value, old, "{} at {}", function.id.as_str(), at.as_str());
            assert!(calls > 0);
            found += usize::from(old.is_some());
        }
        found
    }

    #[test]
    fn funded_single_site_binding_matches_the_original_precedence_on_every_family() {
        let mut found = 0;
        for (source, name) in [
            (HELPERS, "funded-site-helpers.spx"),
            (LOOPS, "funded-site-loops.spx"),
        ] {
            let checked = crate::check(source, name).unwrap();
            let program = crate::hir::resolve(&checked).unwrap();
            let index = crate::loop_calls::byte_writer::ModuleIndex::new(&program).ok();
            for function in &program.functions {
                let ids = candidates(&program, function);
                found += single_site_parity(&program, function, &ids, None);
                found += single_site_parity(&program, function, &ids, index.as_ref());
            }
        }
        assert!(found > 4, "the corpus must exercise single-site lookups");
    }

    #[test]
    fn funded_single_site_binding_keeps_exact_route_precedence_for_duplicate_identities() {
        let checked = crate::check(LOOPS, "funded-site-duplicate.spx").unwrap();
        let mut program = crate::hir::resolve(&checked).unwrap();
        let byte_at = crate::byte_ops::same_owner_set_bindings(&program.functions[0])
            .keys()
            .next()
            .unwrap()
            .clone();
        let crate::hir::ResolvedExprKind::Block { statements, .. } =
            &mut program.functions[0].body.kind
        else {
            panic!("main block")
        };
        let crate::hir::ResolvedStatement::While { body, .. } = statements
            .iter_mut()
            .find(|statement| matches!(statement, crate::hir::ResolvedStatement::While { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
            panic!("loop block")
        };
        for statement in statements {
            if let crate::hir::ResolvedStatement::Assign { binding, value, .. } = statement {
                if binding.name == "values" {
                    value.id = byte_at.clone();
                }
            }
        }
        let function = &program.functions[0];
        let ids = candidates(&program, function);
        assert!(single_site_parity(&program, function, &ids, None) > 0);
        // Bytes outrank vec, exactly as in the original chain.
        assert_eq!(
            binding_with_index(&program, function, &byte_at, None)
                .unwrap()
                .name,
            "buffer"
        );
        let mut payer = funded::payer::Payer::unlimited();
        let site =
            binding_with_index_funded(&program, function, &byte_at, None, &mut |b| payer.pay(b))
                .unwrap();
        assert_eq!(site.unwrap().name, "buffer");
    }

    #[test]
    fn funded_single_site_census_verdict_is_absence_but_refusals_propagate() {
        let checked = crate::check(HELPERS, "funded-site-census.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let main = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let ids = candidates(&program, main);
        let mut deep = main.clone();
        let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut deep.body.kind else {
            panic!("main block")
        };
        for _ in 0..600 {
            let inner = tail.clone();
            tail.kind = crate::hir::ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: inner,
            };
        }
        assert!(!bindings(&program, &deep).has_helpers());
        // Census verdict: no helper site; the later routes still answer as the original.
        single_site_parity(&program, &deep, &ids, None);
    }

    #[test]
    fn funded_assembly_never_touches_the_production_ledger() {
        let checked = crate::check(HELPERS, "funded-assembly-active.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let (calls, refused, active) = crate::bounded_output::with_limit_usage(0, || {
            let mut payer = funded::payer::Payer::unlimited();
            bindings_funded(&program, function, &mut |bytes| payer.pay(bytes)).unwrap();
            payer.calls
        });
        assert!(calls > 0 && !refused);
        assert_eq!(active, 0);
    }
}

#[path = "funded_receiving.rs"]
mod receiving;
pub(crate) use receiving::bindings_into;
