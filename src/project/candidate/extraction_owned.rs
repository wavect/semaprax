//! Correspondence for a nested lexical scope, not cleanup-plan rewriting.
use super::*;
use crate::hir::{ResolvedExpr, ResolvedFunction, ValueId};

pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    let programs = parse_revision(before)?;
    let target = text(request, "target")?;
    let selected =
        expression::authored_selection(before, &programs, target, text(request, "expression_id")?)?;
    let mut original_types = types::Types::new(before, &programs[selected.owner], target)?;
    let (captures, _, extended, owner_capture) =
        capture_plan(before, target, &selected, &mut original_types)?;
    if !extended && !owner_capture {
        return Ok(());
    }
    let rebuilt = parse_revision(after)?;
    let helper_id = text(request, "new_id")?;
    let helper = function(after, helper_id)?;
    let source = rebuilt
        .iter()
        .find(|p| {
            p.path == programs[selected.owner].path && p.module == programs[selected.owner].module
        })
        .ok_or_else(|| invalid("extraction helper source owner changed"))?;
    let mut helper_types = types::Types::new(after, source, helper_id)?;
    if helper.params.len() != captures.len()
        || !helper.requires.is_empty()
        || !helper.ensures.is_empty()
        || helper.return_type != selected.expression.ty
        || helper.body.ownership != selected.expression.ownership
        || helper.body.ty != selected.expression.ty
    {
        return Err(correspondence(
            owner_capture,
            "extraction helper changed its result boundary or contracts",
        ));
    }
    let _ = helper_types.result(&helper.return_type, helper.body.ownership)?;
    let inner = if extended {
        let ResolvedExprKind::Block {
            statements,
            tail: inner,
        } = &helper.body.kind
        else {
            return Err(invalid("extraction helper has no root block wrapper"));
        };
        if !statements.is_empty() || !matches!(inner.kind, ResolvedExprKind::Block { .. }) {
            return Err(invalid(
                "extraction owning scope must remain nested inside an empty root",
            ));
        }
        inner.as_ref()
    } else if matches!(selected.expression.kind, ResolvedExprKind::Block { .. }) {
        &helper.body
    } else {
        // A helper whose authored body is not itself a block reappears inside
        // the function's root block once the projection is re-parsed, so the
        // correspondence descends that one synthetic level.
        let ResolvedExprKind::Block {
            statements,
            tail: inner,
        } = &helper.body.kind
        else {
            return Err(invalid("extraction helper has no root block wrapper"));
        };
        if !statements.is_empty() {
            return Err(invalid(
                "extraction helper root block gained authored statements",
            ));
        }
        inner.as_ref()
    };
    let mut ids = Ids::default();
    for (capture, parameter) in captures.iter().zip(&helper.params) {
        let binding = selected
            .scope
            .iter()
            .find(|b| b.id == capture.id)
            .ok_or_else(|| {
                invalid("extraction capture disappeared from its authenticated scope")
            })?;
        let expected_ownership =
            if capture.mode == ParamMode::Own || matches!(&capture.ty, Type::String) {
                OwnershipMode::Own
            } else {
                OwnershipMode::Value
            };
        if parameter.name != capture.name
            || parameter.ty != *binding.ty
            || parameter.ownership != expected_ownership
        {
            return Err(correspondence(
                owner_capture,
                "extraction helper changed an authenticated capture boundary",
            ));
        }
        helper_types.internal(&parameter.ty, parameter.ownership)?;
        ids.insert(binding.id, parameter.id.as_str())?;
    }
    pair(selected.expression, inner, &mut ids, &mut helper_types)?;
    let plan = &helper.cleanup_plan;
    if !plan.entry_state.conditional_owned_parameters.is_empty()
        || !helper.loan_plan.loans.is_empty()
    {
        return Err(invalid(
            "extraction helper introduced crossing ownership or loans",
        ));
    }
    if owner_capture {
        let owner_parameter = captures
            .iter()
            .zip(&helper.params)
            .find(|(capture, _)| {
                capture.mode == ParamMode::Own || matches!(&capture.ty, Type::String)
            })
            .map(|(_, parameter)| parameter)
            .ok_or_else(|| owner_replay("extraction helper lost its owning parameter"))?;
        // A String owner carries no runtime free in this profile, so the checked
        // plan holds no live owned parameter for it. Every other owner must
        // arrive as the one exact whole live owner.
        let released_by_scope = matches!(owner_parameter.ty, ResolvedType::String)
            && plan.entry_state.live_owned_parameters.is_empty();
        if !released_by_scope
            && (plan.entry_state.live_owned_parameters.len() != 1
                || plan.entry_state.live_owned_parameters[0].storage
                    != crate::cleanup_plan::StorageId::Value(owner_parameter.id.clone())
                || !plan.entry_state.live_owned_parameters[0]
                    .projections
                    .is_empty())
        {
            return Err(owner_replay(
                "extraction helper does not receive one exact whole live owner",
            ));
        }
    } else if !plan.entry_state.live_owned_parameters.is_empty() {
        return Err(invalid("extraction helper introduced an owning parameter"));
    }
    let mut owned_result_commits = 0usize;
    let mut scalar_result_commits = 0usize;
    for exit in &plan.exits {
        if let crate::cleanup_plan::ExitContinuation::CommitResult { source } = &exit.continuation {
            match source {
                crate::cleanup_plan::CleanupResultSource::Owned { storage } => {
                    owned_result_commits += 1;
                    if storage.storage != crate::cleanup_plan::StorageId::ProvisionalResult
                        || !storage.projections.is_empty()
                    {
                        return Err(invalid(
                            "extraction owned result does not publish the whole provisional result",
                        ));
                    }
                }
                crate::cleanup_plan::CleanupResultSource::Scalar { .. } => {
                    scalar_result_commits += 1;
                }
            }
        }
    }
    match helper.body.ownership {
        OwnershipMode::Value if owned_result_commits != 0 => {
            return Err(invalid(
                "extraction Copy result gained an owned publication",
            ));
        }
        OwnershipMode::Own if owned_result_commits == 0 || scalar_result_commits != 0 => {
            return Err(invalid(
                "extraction owned result lacks exact owned publication",
            ));
        }
        OwnershipMode::Borrow | OwnershipMode::Shared => {
            return Err(invalid("extraction helper exposes a loan as its result"));
        }
        _ => {}
    }
    let roots = plan
        .regions
        .iter()
        .filter(|r| r.parent.is_none())
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(invalid("extraction helper cleanup root is ambiguous"));
    }
    // An owning capture and an owned result both land in the root region, so
    // their types are admitted at the boundaries checked above rather than on
    // the Copy-only path.
    let mut owned_boundary = helper
        .params
        .iter()
        .filter(|parameter| parameter.ownership == OwnershipMode::Own)
        .map(|parameter| &parameter.ty)
        .collect::<Vec<_>>();
    if helper.body.ownership == OwnershipMode::Own {
        owned_boundary.push(&helper.return_type);
    }
    // Inspect slots in their original order. The rebuilt plan remains the
    // authority; this correspondence never filters or repairs it.
    for storage in &roots[0].slots {
        let slot = plan
            .slots
            .iter()
            .find(|slot| &slot.storage == storage)
            .ok_or_else(|| invalid("extraction helper root storage has no checked slot"))?;
        if owned_boundary.contains(&&slot.ty) {
            continue;
        }
        helper_types.check(&slot.ty)?;
    }
    Ok(())
}

fn correspondence(owner_capture: bool, message: &'static str) -> Vec<Diagnostic> {
    if owner_capture {
        owner_replay(message)
    } else {
        invalid(message)
    }
}

fn function<'a>(revision: &'a ProjectRevision, id: &str) -> Result<&'a ResolvedFunction> {
    let mut found = None;
    for module in revision.semantic.image_modules() {
        for function in module.functions().iter().filter(|f| f.id.as_str() == id) {
            if found.replace(function).is_some() {
                return Err(invalid("extraction helper checked identity is ambiguous"));
            }
        }
    }
    found.ok_or_else(|| invalid("extraction helper checked identity is missing"))
}

#[derive(Default)]
struct Ids {
    values: BTreeMap<String, String>,
    destinations: BTreeSet<String>,
    patterns: usize,
}
impl Ids {
    fn insert(&mut self, old: &str, new: &str) -> Result<()> {
        if self.values.len() >= MAX_NODES + MAX_CAPTURES {
            return Err(limit("extraction value correspondence exceeds its bound"));
        }
        if self.values.contains_key(old) || !self.destinations.insert(new.to_owned()) {
            return Err(invalid("extraction value correspondence is not bijective"));
        }
        self.values.insert(old.to_owned(), new.to_owned());
        Ok(())
    }
    fn root(&self, old: &ValueId, new: &ValueId) -> Result<()> {
        if self.values.get(old.as_str()).map(String::as_str) != Some(new.as_str()) {
            return Err(invalid("extraction changed an authenticated lexical value"));
        }
        Ok(())
    }
    fn binding(&mut self, old: &ResolvedBinding, new: &ResolvedBinding) -> Result<()> {
        if old.name != new.name || old.ty != new.ty || old.ownership != new.ownership {
            return Err(invalid(
                "extraction changed an internal binding type or ownership",
            ));
        }
        self.insert(old.id.as_str(), new.id.as_str())
    }
    fn pattern(
        &mut self,
        old: &ResolvedMatchPattern,
        new: &ResolvedMatchPattern,
        depth: usize,
    ) -> Result<()> {
        pattern_budget(depth, &mut self.patterns)?;
        use ResolvedMatchPattern as P;
        match (old, new) {
            (P::Wildcard, P::Wildcard) => Ok(()),
            (P::Literal(a), P::Literal(b)) if a == b => Ok(()),
            (P::Binding(a), P::Binding(b)) => self.binding(a, b),
            (P::Or(a), P::Or(b)) if a.len() == b.len() => {
                for (a, b) in a.iter().zip(b) {
                    self.pattern(a, b, depth + 1)?;
                }
                Ok(())
            }
            (
                P::Variant {
                    variant: a,
                    case: ac,
                    fields: af,
                },
                P::Variant {
                    variant: b,
                    case: bc,
                    fields: bf,
                },
            ) if a == b && ac == bc && af.len() == bf.len() => {
                for (a, b) in af.iter().zip(bf) {
                    if a.field != b.field {
                        return Err(invalid("extraction changed a pattern field identity"));
                    }
                    self.binding(&a.binding, &b.binding)?;
                }
                Ok(())
            }
            (
                P::Record {
                    record: a,
                    instance: at,
                    fields: af,
                },
                P::Record {
                    record: b,
                    instance: bt,
                    fields: bf,
                },
            ) if a == b && at == bt => self.fields(af, bf, depth + 1),
            _ => Err(invalid("extraction changed a checked pattern identity")),
        }
    }
    fn fields(
        &mut self,
        old: &[hir::ResolvedRecordMatchPatternField],
        new: &[hir::ResolvedRecordMatchPatternField],
        depth: usize,
    ) -> Result<()> {
        if old.len() != new.len() {
            return Err(invalid("extraction changed a pattern field inventory"));
        }
        for (a, b) in old.iter().zip(new) {
            pattern_budget(depth, &mut self.patterns)?;
            if a.field != b.field {
                return Err(invalid("extraction changed a pattern field identity"));
            }
            use ResolvedRecordMatchFieldPattern as F;
            match (&a.pattern, &b.pattern) {
                (F::Wildcard, F::Wildcard) => {}
                (F::Binding(a), F::Binding(b)) => self.binding(a, b)?,
                (
                    F::Record {
                        record: a,
                        instance: at,
                        fields: af,
                    },
                    F::Record {
                        record: b,
                        instance: bt,
                        fields: bf,
                    },
                ) if a == b && at == bt => self.fields(af, bf, depth + 1)?,
                _ => return Err(invalid("extraction changed a nested pattern")),
            }
        }
        Ok(())
    }
}

fn pair(
    old: &ResolvedExpr,
    new: &ResolvedExpr,
    ids: &mut Ids,
    types: &mut types::Types<'_>,
) -> Result<()> {
    let mut pending = vec![(old, new, 0usize)];
    let mut pairs = Vec::new();
    while let Some((a, b, depth)) = pending.pop() {
        if pairs.len() >= MAX_NODES || depth > MAX_DEPTH {
            return Err(limit("extraction HIR correspondence exceeds its bound"));
        }
        if a.ty != b.ty
            || a.ownership != b.ownership
            || std::mem::discriminant(&a.kind) != std::mem::discriminant(&b.kind)
        {
            return Err(invalid(
                "extraction changed a checked expression type or ownership",
            ));
        }
        types.internal(&b.ty, b.ownership)?;
        use ResolvedExprKind as E;
        let same = match (&a.kind, &b.kind) {
            (
                E::Call {
                    callee: a,
                    type_arguments: at,
                    instance: ai,
                    ..
                },
                E::Call {
                    callee: b,
                    type_arguments: bt,
                    instance: bi,
                    ..
                },
            ) => a == b && at == bt && ai == bi,
            (
                E::ConstructRecord {
                    record: a,
                    fields: af,
                },
                E::ConstructRecord {
                    record: b,
                    fields: bf,
                },
            )
            | (
                E::UpdateRecord {
                    record: a,
                    fields: af,
                    ..
                },
                E::UpdateRecord {
                    record: b,
                    fields: bf,
                    ..
                },
            ) => a == b && af.iter().map(|f| &f.field).eq(bf.iter().map(|f| &f.field)),
            (
                E::ConstructVariant {
                    variant: a,
                    case: ac,
                    fields: af,
                },
                E::ConstructVariant {
                    variant: b,
                    case: bc,
                    fields: bf,
                },
            ) => a == b && ac == bc && af.iter().map(|f| &f.field).eq(bf.iter().map(|f| &f.field)),
            (E::Project { field: a, .. }, E::Project { field: b, .. }) => a == b,
            (E::Block { statements: a, .. }, E::Block { statements: b, .. }) => {
                if a.len() != b.len() {
                    return Err(invalid("extraction changed block statements"));
                }
                for (a, b) in a.iter().zip(b) {
                    match (a, b) {
                        (
                            ResolvedStatement::Let {
                                binding: a,
                                mutable: am,
                                ..
                            },
                            ResolvedStatement::Let {
                                binding: b,
                                mutable: bm,
                                ..
                            },
                        ) if am == bm => ids.binding(a, b)?,
                        (
                            ResolvedStatement::Assign { field: a, .. },
                            ResolvedStatement::Assign { field: b, .. },
                        ) if a == b => {}
                        (ResolvedStatement::While { .. }, ResolvedStatement::While { .. }) => {}
                        _ => return Err(invalid("extraction changed checked statement semantics")),
                    }
                }
                true
            }
            (
                E::Match {
                    mode: a, arms: aa, ..
                },
                E::Match {
                    mode: b, arms: ba, ..
                },
            ) => {
                if a != b || aa.len() != ba.len() {
                    return Err(invalid("extraction changed match semantics"));
                }
                for (a, b) in aa.iter().zip(ba) {
                    if a.guard.is_some() != b.guard.is_some() {
                        return Err(invalid("extraction changed match guards"));
                    }
                    ids.pattern(&a.pattern, &b.pattern, 0)?;
                }
                true
            }
            // These external ABI/effect call forms are not ordinary direct
            // call identities. Keep them outside this new owning lane.
            (E::NativeRustImportCall(_), _) | (E::HostCommandCall(_), _) => false,
            _ => true, // Exact authored AST replay binds operators/literals.
        };
        if !same {
            return Err(invalid(
                "extraction changed an authenticated operation identity",
            ));
        }
        let mut ac = Vec::new();
        let mut bc = Vec::new();
        hir::push_resolved_expression_children_in_authored_order(a, &mut ac);
        hir::push_resolved_expression_children_in_authored_order(b, &mut bc);
        if ac.len() != bc.len() {
            return Err(invalid("extraction changed checked expression children"));
        }
        pending.extend(ac.into_iter().zip(bc).map(|(a, b)| (a, b, depth + 1)));
        pairs.push((a, b));
    }
    for (a, b) in pairs {
        match (&a.kind, &b.kind) {
            (ResolvedExprKind::Place(a), ResolvedExprKind::Place(b)) => {
                ids.root(&a.root, &b.root)?;
                if a.projections != b.projections {
                    return Err(invalid("extraction changed a checked place projection"));
                }
            }
            (
                ResolvedExprKind::Block { statements: a, .. },
                ResolvedExprKind::Block { statements: b, .. },
            ) => {
                for (a, b) in a.iter().zip(b) {
                    if let (
                        ResolvedStatement::Assign { binding: a, .. },
                        ResolvedStatement::Assign { binding: b, .. },
                    ) = (a, b)
                    {
                        ids.root(&a.id, &b.id)?;
                        if a.ty != b.ty || a.ownership != b.ownership {
                            return Err(invalid("extraction changed assignment ownership"));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{with_authenticated_project, ProjectCandidate, SemanticChange};

    // Build a real candidate before corrupting only its private HIR copy.
    // These assertions exercise identities that unchanged source-shaped
    // types/operators alone cannot distinguish. Authored, not executed here.
    fn rebuilt_pair_rejects_tampering(target: &str, change_callee: bool) {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/calculator-project/semaprax.toml");
        let base = with_authenticated_project(&manifest, |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap();
        let old = function(base.revision(), target).unwrap();
        let request = json!({"kind":"extract_function","target":target,"expression_id":old.body.id.as_str(),
            "new_id":"calculator.correspondence","new_name":"extracted_correspondence"});
        let change = SemanticChange::new(base.revision().project_revision(), &request).unwrap();
        let candidate = base.apply(base.candidate_digest(), &change).unwrap();
        let helper = function(candidate.revision(), "calculator.correspondence").unwrap();
        let old_programs = parse_revision(base.revision()).unwrap();
        let selected = expression::authored_selection(
            base.revision(),
            &old_programs,
            target,
            old.body.id.as_str(),
        )
        .unwrap();
        let mut original_types =
            types::Types::new(base.revision(), &old_programs[selected.owner], target).unwrap();
        let (captures, _, _, _) =
            capture_plan(base.revision(), target, &selected, &mut original_types).unwrap();
        let new_programs = parse_revision(candidate.revision()).unwrap();
        let source = new_programs
            .iter()
            .find(|p| p.path == old_programs[selected.owner].path)
            .unwrap();
        let compare = |body: &ResolvedExpr| {
            let mut ids = Ids::default();
            for (capture, parameter) in captures.iter().zip(&helper.params) {
                ids.insert(&capture.id, parameter.id.as_str()).unwrap();
            }
            let mut types =
                types::Types::new(candidate.revision(), source, "calculator.correspondence")
                    .unwrap();
            pair(selected.expression, body, &mut ids, &mut types)
        };
        compare(&helper.body).unwrap();
        let mut tampered = helper.body.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut tampered.kind else {
            panic!("fixture block");
        };
        if change_callee {
            let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
                panic!("fixture call");
            };
            *callee = helper.id.clone();
        } else {
            let ResolvedExprKind::Binary { left, right, .. } = &mut tail.kind else {
                panic!("fixture binary");
            };
            let ResolvedExprKind::Place(other) = &right.kind else {
                panic!("fixture right place");
            };
            let ResolvedExprKind::Place(place) = &mut left.kind else {
                panic!("fixture left place");
            };
            place.root = other.root.clone();
        }
        let errors = compare(&tampered).unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-G225"), "{errors:?}");
    }

    #[test]
    fn rebuilt_helper_rejects_same_typed_lexical_root_substitution() {
        rebuilt_pair_rejects_tampering("calculator.add", false);
    }

    #[test]
    fn rebuilt_helper_rejects_changed_stable_callee_identity() {
        rebuilt_pair_rejects_tampering("calculator.app.main", true);
    }
}
