//! Renewal site identity is derived from HIR, independently of attached transitions.
pub(crate) mod child_refusal;
#[cfg(test)]
mod funded;
pub(crate) mod ordinary;
pub(crate) mod profile_refusal;
mod selected;
mod selected_inline;
#[cfg(test)]
use crate::hir::ResolvedProgram;
use crate::hir::{ExpressionId, ResolvedBinding, ResolvedFunction};
#[cfg(test)]
pub(crate) use funded::{
    binding_with_index_funded, bindings_funded, bindings_with_index_funded,
    bindings_with_index_funded_with_work,
};
#[cfg(test)]
pub(crate) use selected::bindings_with_reservation;
#[cfg(test)]
pub(crate) use selected::bindings_with_reservation_and_work;
pub(crate) use selected::selected_string_append;
pub(crate) use selected_inline::{
    bindings_with_reservation_and_work_inline, selected_string_append_inline,
};
use std::collections::BTreeMap;

/// A held phase index authenticates exact program and function membership.
/// Standalone callers still derive their own fresh helper census.
pub(crate) fn binding_with_index<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
    index: Option<&crate::loop_calls::byte_writer::ModuleIndex<'a>>,
) -> Option<&'a ResolvedBinding> {
    if let Some(sites) = program.primitive_renewals(function) {
        return sites.get(at);
    }
    helper_binding_with_index(program, function, at, index)
        .or_else(|| crate::string_ops::replacement::binding(function, at))
        .or_else(|| crate::byte_ops::same_owner_set_binding(function, at))
        .or_else(|| crate::hir::vec_loop_renewal::binding_in(program, function, at))
        .or_else(|| crate::hir::iterator_loop::renewal_binding(program, function, at))
}
pub(crate) fn helper_binding_with_index<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
    index: Option<&crate::loop_calls::byte_writer::ModuleIndex<'a>>,
) -> Option<&'a ResolvedBinding> {
    if let Some(sites) = program.primitive_renewals(function) {
        return sites.helpers.get(at);
    }
    match index {
        Some(index)
            if program
                .full_program()
                .is_some_and(|program| index.matches_program(program)) =>
        {
            index.function_sites(function)?.get(at)
        }
        Some(_) => None,
        None => crate::loop_calls::byte_writer::bindings(program, function).remove(at),
    }
}

pub(crate) struct RenewalBindings<'a> {
    legacy: BTreeMap<ExpressionId, &'a ResolvedBinding>,
    appends: BTreeMap<ExpressionId, ExpressionId>,
    helpers: HelperSites<'a>,
}
enum HelperSites<'a> {
    Owned(crate::loop_calls::byte_writer::HelperBindings<'a>),
    Borrowed(&'a crate::loop_calls::byte_writer::HelperBindings<'a>),
}
impl<'a> HelperSites<'a> {
    fn get(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        match self {
            Self::Owned(sites) => sites.get(at),
            Self::Borrowed(sites) => sites.get(at),
        }
    }
}
impl<'a> RenewalBindings<'a> {
    pub(crate) fn empty() -> Self {
        Self {
            legacy: BTreeMap::new(),
            appends: BTreeMap::new(),
            helpers: HelperSites::Owned(Default::default()),
        }
    }
    pub(crate) fn appends(&self) -> &BTreeMap<ExpressionId, ExpressionId> {
        &self.appends
    }
    pub(crate) fn has_helpers(&self) -> bool {
        match &self.helpers {
            HelperSites::Owned(sites) => !sites.is_empty(),
            HelperSites::Borrowed(sites) => !sites.is_empty(),
        }
    }
    pub(crate) fn is_helper(&self, at: &ExpressionId) -> bool {
        self.helpers.get(at).is_some()
    }
    pub(crate) fn get(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        self.helpers
            .get(at)
            .or_else(|| self.legacy.get(at).copied())
    }
}

/// Derive the per-function renewal lookup once for cleanup skeleton replay.
/// Legacy `extend` order remains unchanged. Borrowed helper membership wins
/// first, matching `binding`, without allocating new opaque tree nodes.
#[cfg(test)]
pub(crate) fn bindings<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
) -> RenewalBindings<'a> {
    bindings_with_index(program, function, None)
}
pub(crate) fn bindings_with_index<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
) -> RenewalBindings<'a> {
    let mut found = crate::hir::iterator_loop::renewal_bindings(program, function);
    found.extend(crate::hir::vec_loop_renewal::bindings_in(program, function));
    found.extend(crate::byte_ops::same_owner_set_bindings(function));
    found.extend(crate::string_ops::replacement::bindings(function));
    RenewalBindings {
        legacy: found,
        appends: BTreeMap::new(),
        helpers: match index.and_then(|index| index.function_sites(function)) {
            Some(sites) => HelperSites::Borrowed(sites),
            None if index.is_some() => HelperSites::Owned(Default::default()),
            None => HelperSites::Owned(crate::loop_calls::byte_writer::bindings(program, function)),
        },
    }
}

/// Called only after the site has independently passed renewal binding admission.
/// The already-retained RHS selects private provenance; it grants no new site.
pub(super) fn owned_rhs_provenance(expression: &crate::hir::ResolvedExpr) -> bool {
    matches!(&expression.kind, crate::hir::ResolvedExprKind::Call { callee, args, .. }
        if crate::vec_ops::by_id(callee.as_str()).is_some_and(|op| op.reopens_same_owner())
            && args.iter().skip(1).any(|arg| arg.ownership == crate::hir::OwnershipMode::Own))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{ResolvedExprKind, ResolvedStatement};

    fn legacy_lookup<'a>(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        at: &ExpressionId,
    ) -> Option<&'a ResolvedBinding> {
        crate::string_ops::replacement::binding(function, at)
            .or_else(|| crate::byte_ops::same_owner_set_binding(function, at))
            .or_else(|| crate::hir::vec_loop_renewal::binding_in(program, function, at))
            .or_else(|| crate::hir::iterator_loop::renewal_binding(program, function, at))
    }

    #[test]
    fn renewal_binding_index_preserves_lookup_priority_for_duplicate_ids() {
        let source = crate::check(
            r#"module test.renewal_binding_index;
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
"#,
            "renewal-binding-index.spx",
        )
        .unwrap();
        let mut program = crate::hir::resolve(&source).unwrap();
        let byte_at = {
            let function = program
                .functions
                .iter()
                .find(|function| function.id.as_str() == "app.main")
                .unwrap();
            crate::byte_ops::same_owner_set_bindings(function)
                .keys()
                .next()
                .unwrap()
                .clone()
        };
        let vec_at = {
            let function = program
                .functions
                .iter()
                .find(|function| function.id.as_str() == "app.main")
                .unwrap();
            crate::hir::vec_loop_renewal::bindings_in(&program, function)
                .keys()
                .next()
                .unwrap()
                .clone()
        };
        assert_ne!(byte_at, vec_at);

        {
            let function = program
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "app.main")
                .unwrap();
            let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
                panic!("main block")
            };
            let body = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ResolvedStatement::While { body, .. } => Some(body),
                    _ => None,
                })
                .unwrap();
            let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
                panic!("loop block")
            };
            let vec_rhs = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ResolvedStatement::Assign { binding, value, .. }
                        if binding.name == "values" =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
                .unwrap();
            vec_rhs.id = byte_at.clone();
        }

        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let index = bindings(&program, function);
        let mut candidate_ids = crate::string_ops::replacement::bindings(function)
            .into_keys()
            .collect::<std::collections::BTreeSet<_>>();
        candidate_ids.extend(crate::byte_ops::same_owner_set_bindings(function).into_keys());
        candidate_ids
            .extend(crate::hir::vec_loop_renewal::bindings_in(&program, function).into_keys());
        candidate_ids
            .extend(crate::hir::iterator_loop::renewal_bindings(&program, function).into_keys());
        for at in candidate_ids {
            assert_eq!(
                index.get(&at).map(|binding| &binding.id),
                legacy_lookup(&program, function, &at).map(|binding| &binding.id),
                "profile precedence changed at {at:?}"
            );
        }
        assert_eq!(index.get(&byte_at).unwrap().name, "buffer");
    }
    #[test]
    fn held_helper_index_replays_repeated_renewals_without_fresh_cursor_backing() {
        let source = include_str!("../../tests/owned_data/byte_helper_renewal.spx");
        let ast = crate::check(source, "held-renewal.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let index = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let sites = index.function_sites(function).unwrap();
        let bindings: Vec<_> = crate::loop_calls::byte_writer::checked_bindings(&program, function)
            .unwrap()
            .into_iter()
            .collect();
        assert!(!bindings.is_empty());
        let (valid, refused, bytes) = crate::bounded_output::with_limit_usage(0, || {
            (0..64).all(|_| {
                bindings.iter().all(|(at, binding)| {
                    sites.get(at) == Some(*binding)
                        && binding_with_index(&program, function, at, Some(&index))
                            == Some(*binding)
                        && helper_binding_with_index(&program, function, at, Some(&index))
                            == Some(*binding)
                })
            })
        });
        assert!(valid);
        assert!(!refused);
        assert_eq!(bytes, 0);
        let foreign = program.clone();
        let other = crate::loop_calls::byte_writer::ModuleIndex::new(&foreign).unwrap();
        for (at, _) in bindings {
            assert!(helper_binding_with_index(&program, function, at, Some(&other)).is_none());
            assert!(
                helper_binding_with_index(&program, &function.clone(), at, Some(&index)).is_none()
            );
        }
    }
}
