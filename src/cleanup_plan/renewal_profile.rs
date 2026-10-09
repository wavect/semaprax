//! Renewal site identity is derived from HIR, independently of attached transitions.
use crate::hir::{ExpressionId, ResolvedBinding, ResolvedFunction, ResolvedProgram};
use std::collections::BTreeMap;

pub(crate) fn binding<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    crate::string_ops::replacement::binding(function, at)
        .or_else(|| crate::byte_ops::same_owner_set_binding(function, at))
        .or_else(|| crate::hir::vec_loop_renewal::binding(function, at))
        .or_else(|| crate::hir::iterator_loop::renewal_binding(program, function, at))
}

/// Derive the per-function renewal lookup once for cleanup skeleton replay.
/// `extend` order mirrors `binding`'s first-match order, including duplicate
/// or forged expression IDs across profile maps.
pub(crate) fn bindings<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
) -> BTreeMap<ExpressionId, &'a ResolvedBinding> {
    let mut found = crate::hir::iterator_loop::renewal_bindings(program, function);
    found.extend(crate::hir::vec_loop_renewal::bindings(function));
    found.extend(crate::byte_ops::same_owner_set_bindings(function));
    found.extend(crate::string_ops::replacement::bindings(function));
    found
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
            .or_else(|| crate::hir::vec_loop_renewal::binding(function, at))
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
            crate::hir::vec_loop_renewal::bindings(function)
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
        candidate_ids.extend(crate::hir::vec_loop_renewal::bindings(function).into_keys());
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
}
