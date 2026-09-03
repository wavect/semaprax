//! Exact owning provider-use authentication for signature replacement.
//!
//! This module admits up to eight independent narrow rewrites: each old owner
//! may occur exactly once, as the unprojected root of the compiler builtin
//! compiler-owned view operation for that owner kind.

use super::*;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    BytesSlice,
    StringStr,
}

impl Kind {
    pub(super) fn source_matches(self, parameter: &Param) -> bool {
        match self {
            Self::BytesSlice => parameter.mode == ParamMode::Own && parameter.ty == Type::Bytes,
            Self::StringStr => parameter.mode == ParamMode::Value && parameter.ty == Type::String,
        }
    }

    fn checked_matches(self, ownership: OwnershipMode, ty: &ResolvedType) -> bool {
        ownership == OwnershipMode::Own
            && match self {
                Self::BytesSlice => *ty == ResolvedType::Bytes,
                Self::StringStr => *ty == ResolvedType::String,
            }
    }

    pub(super) const fn result_type(self) -> Type {
        match self {
            Self::BytesSlice => Type::SliceU8,
            Self::StringStr => Type::Str,
        }
    }

    pub(super) const fn operation_name(self) -> &'static str {
        match self {
            Self::BytesSlice => crate::byte_ops::BYTES_AS_SLICE_NAME,
            Self::StringStr => crate::byte_ops::STRING_AS_STR_NAME,
        }
    }

    const fn operation_id(self) -> &'static str {
        match self {
            Self::BytesSlice => crate::byte_ops::BYTES_AS_SLICE_ID,
            Self::StringStr => crate::byte_ops::STRING_AS_STR_ID,
        }
    }
}

pub(super) fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G469", message)]
}

pub(super) fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G478", message)]
}

pub(super) fn duplicate(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G477", message)]
}

pub(super) fn alias(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G479", message)]
}

pub(super) struct AuthenticatedOwnerViews {
    replacements: Vec<(Param, String, Kind)>,
}

impl AuthenticatedOwnerViews {
    /// Apply the already authenticated batch to a private copy and publish the
    /// provider only after every bounded rewrite traversal succeeds.
    pub(super) fn rewrite_all(self, function: &mut Function) -> Result<()> {
        let mut rewritten = function.clone();
        for (owner, replacement, kind) in self.replacements {
            rewrite_source(&mut rewritten, &owner, &replacement, kind)?;
        }
        *function = rewritten;
        Ok(())
    }
}

pub(super) fn authenticate_all(
    revision: &ProjectRevision,
    function: &Function,
    original_params: &[Param],
    replacements: &[(usize, String, Kind)],
) -> Result<AuthenticatedOwnerViews> {
    let mut checked = None;
    for module in revision.semantic.image_modules() {
        for candidate in module
            .functions()
            .iter()
            .filter(|candidate| candidate.id.as_str() == function.stable_id)
        {
            if checked.replace(candidate).is_some() {
                return Err(invalid(
                    "owner-to-view replacement has ambiguous checked provider identity",
                ));
            }
        }
    }
    let checked = checked.ok_or_else(|| {
        invalid("owner-to-view replacement has no authenticated checked provider")
    })?;
    let mut authenticated = Vec::with_capacity(replacements.len());
    for (owner_index, replacement, kind) in replacements {
        let owner = original_params.get(*owner_index).ok_or_else(|| {
            invalid("owner-to-view replacement source parameter inventory disagrees")
        })?;
        let parameter = checked.params.get(*owner_index).ok_or_else(|| {
            invalid("owner-to-view replacement checked parameter inventory disagrees")
        })?;
        if parameter.name != owner.name
            || !kind.source_matches(owner)
            || !kind.checked_matches(parameter.ownership, &parameter.ty)
        {
            return Err(invalid(
                "owner-to-view replacement source and checked owner disagree",
            ));
        }
        if checked
            .requires
            .iter()
            .chain(&checked.ensures)
            .any(|contract| uses_root(contract, &parameter.id))
        {
            return Err(invalid(
                "owner-to-view replacement does not admit owner references in contracts",
            ));
        }
        let mut uses = 0usize;
        let mut pending = vec![&checked.body];
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                ResolvedExprKind::BorrowPlace { operation, place }
                    if place.root == parameter.id =>
                {
                    if operation.as_str() != kind.operation_id() || !place.projections.is_empty() {
                        return Err(invalid(
                            "owner-to-view replacement requires its exact unprojected compiler-owned view use",
                        ));
                    }
                    uses += 1;
                }
                ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. }
                    if place.root == parameter.id =>
                {
                    return Err(invalid(
                        "owner-to-view replacement owner has a non-view provider use",
                    ));
                }
                _ => push_children(expression, &mut pending),
            }
        }
        if uses != 1 {
            return Err(invalid(
                "owner-to-view replacement requires exactly one authenticated owner view use",
            ));
        }
        authenticated.push((owner.clone(), replacement.clone(), *kind, uses));
    }
    authenticate_sources(function, authenticated)
}

fn authenticate_sources(
    function: &Function,
    authenticated: Vec<(Param, String, Kind, usize)>,
) -> Result<AuthenticatedOwnerViews> {
    // Source/HIR parity for the complete batch is authenticated before any
    // provider mutation. A late owner mismatch cannot expose a partial rewrite.
    for (owner, _, kind, uses) in &authenticated {
        authenticate_source(function, owner, *kind, *uses)?;
    }
    Ok(AuthenticatedOwnerViews {
        replacements: authenticated
            .into_iter()
            .map(|(owner, replacement, kind, _)| (owner, replacement, kind))
            .collect(),
    })
}

fn authenticate_source(
    function: &Function,
    owner: &Param,
    kind: Kind,
    authenticated_uses: usize,
) -> Result<()> {
    let mut inspected = function.clone();
    let mut nodes = 0usize;
    let mut source_calls = 0usize;
    let mut source_places = 0usize;
    super::super::walk_function(&mut inspected, &mut nodes, &mut |expression| {
        if matches!(&expression.kind, ExprKind::Var(name) if name == &owner.name) {
            source_places += 1;
        }
        if matches!(&expression.kind, ExprKind::Call { name, type_arguments, args }
            if name == kind.operation_name()
                && type_arguments.is_empty()
                && matches!(args.as_slice(), [Expr { kind: ExprKind::Var(name), .. }] if name == &owner.name))
        {
            source_calls += 1;
        }
        Ok(())
    })?;
    if source_calls != authenticated_uses || source_places != authenticated_uses {
        return Err(invalid(
            "owner-to-view replacement source uses do not match authenticated builtin evidence",
        ));
    }
    Ok(())
}

fn rewrite_source(
    function: &mut Function,
    owner: &Param,
    replacement: &str,
    kind: Kind,
) -> Result<()> {
    let mut rewrite_nodes = 0usize;
    super::super::walk_function(function, &mut rewrite_nodes, &mut |expression| {
        if matches!(&expression.kind, ExprKind::Call { name, type_arguments, args }
            if name == kind.operation_name()
                && type_arguments.is_empty()
                && matches!(args.as_slice(), [Expr { kind: ExprKind::Var(name), .. }] if name == &owner.name))
        {
            expression.kind = ExprKind::Var(replacement.to_owned());
        }
        Ok(())
    })
}

fn push_children<'a>(expression: &'a ResolvedExpr, pending: &mut Vec<&'a ResolvedExpr>) {
    crate::hir::push_resolved_expression_children_in_authored_order(expression, pending);
}

fn uses_root(expression: &ResolvedExpr, root: &crate::hir::ValueId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match &node.kind {
            ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. }
                if &place.root == root =>
            {
                return true;
            }
            _ => push_children(node, &mut pending),
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_source_builtin_use_rewrites_without_admitting_an_owner_alias() {
        let mut program = crate::parse(
            "module sample; @id(\"sample.read\") fn read(input: own Bytes)->usize { byte_len(bytes_as_slice(input)) }",
            "sample.spx",
        )
        .unwrap();
        let owner = program.functions[0].params[0].clone();
        authenticate_source(&program.functions[0], &owner, Kind::BytesSlice, 1).unwrap();
        rewrite_source(&mut program.functions[0], &owner, "view", Kind::BytesSlice).unwrap();
        let source = crate::format::canonical(&program);
        assert!(source.contains("byte_len(view)"));
        assert!(!source.contains("bytes_as_slice(input)"));
    }

    #[test]
    fn source_shape_rejects_any_additional_owner_occurrence() {
        let program = crate::parse(
            "module sample; @id(\"sample.read\") fn read(input: own Bytes)->Bytes { let view = bytes_as_slice(input); input }",
            "sample.spx",
        )
        .unwrap();
        let owner = program.functions[0].params[0].clone();
        let errors =
            authenticate_source(&program.functions[0], &owner, Kind::BytesSlice, 1).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-G469"));
    }

    #[test]
    fn later_source_mismatch_cannot_partially_rewrite_an_earlier_owner() {
        let program = crate::parse(
            "module sample; @id(\"sample.read\") fn read(left: own Bytes, right: own Bytes)->usize { byte_len(bytes_as_slice(left)) + byte_len(bytes_as_slice(right)) + byte_len(bytes_as_slice(right)) }",
            "sample.spx",
        )
        .unwrap();
        let before = crate::format::canonical(&program);
        let function = &program.functions[0];
        let evidence = vec![
            (
                function.params[0].clone(),
                "left_view".to_owned(),
                Kind::BytesSlice,
                1,
            ),
            (
                function.params[1].clone(),
                "right_view".to_owned(),
                Kind::BytesSlice,
                1,
            ),
        ];
        let errors = match authenticate_sources(function, evidence) {
            Ok(_) => panic!("later owner source mismatch was admitted"),
            Err(errors) => errors,
        };
        assert!(errors.iter().any(|error| error.code == "SPX-G469"));
        assert_eq!(crate::format::canonical(&program), before);
    }
}
