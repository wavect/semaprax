//! Ordinary update authentication on the exact schema invocation payer.
use super::*;
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::replay::paid_type_facts::Payer;
use crate::hir::{self, DeclarationId, DeclarationKind, TypeFactsPayer};
use std::mem::size_of;

pub(in crate::cleanup_plan::replay) fn contains(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    let _phase = crate::test_allocation_observer::function_phase_guard("schema-record-update");
    payer.for_subject(function)?;
    payer.bytes(size_of::<(
        [(&dyn ReplayContext, &ResolvedFunction, &mut Payer<'_>); 2],
        Vec<&ResolvedExpr>,
        [std::slice::Iter<'_, ResolvedExpr>; 2],
        [Option<usize>; 2],
        [usize; 3],
        [Result<bool, Diagnostic>; 2],
    )>())?;
    let roots = function
        .requires
        .len()
        .checked_add(function.ensures.len())
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| payer.construction_refusal("record update selector roots overflowed"))?;
    let mut pending = Vec::new();
    payer.grow(&mut pending, roots)?;
    pending.extend(&function.requires);
    pending.push(&function.body);
    pending.extend(&function.ensures);
    let mut nested = false;
    loop {
        payer.bytes(size_of::<(
            [&mut Vec<&ResolvedExpr>; 2],
            [Option<&ResolvedExpr>; 2],
            [(&dyn ReplayContext, &ResolvedExpr, &mut Payer<'_>); 2],
            [Result<bool, Diagnostic>; 2],
            [bool; 3],
            usize,
        )>())?;
        let Some(expression) = pending.pop() else {
            return Ok(nested);
        };
        // Non-short-circuit OR retains the first diagnostic after a match.
        nested |= authenticate_paid(program, expression, payer)?;
        let mut index = 0usize;
        loop {
            payer.bytes(size_of::<(
                [(&ResolvedExpr, usize); 2],
                [Option<&ResolvedExpr>; 2],
                [usize; 2],
                [(&mut Vec<&ResolvedExpr>, &ResolvedExpr); 2],
            )>())?;
            let Some(child) = super::super::super::replay_expression_child(expression, index)
            else {
                break;
            };
            payer.grow(&mut pending, 1)?;
            pending.push(child);
            index += 1;
        }
    }
}

fn authenticate_paid(
    program: &dyn ReplayContext,
    expression: &ResolvedExpr,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    let ResolvedExprKind::UpdateRecord {
        base,
        record,
        fields,
    } = &expression.kind
    else {
        return Ok(false);
    };
    payer.bytes(size_of::<(
        [&ResolvedType; 4],
        [&DeclarationId; 2],
        &[ResolvedType],
        BTreeSet<&DeclarationId>,
        std::slice::Iter<'_, hir::ResolvedFieldInitializer>,
        [Result<bool, Diagnostic>; 3],
    )>())?;
    if expression.ty != base.ty {
        return Err(payer.replay_failure("nested record update replay changes its result shape"));
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &base.ty
    else {
        return Err(payer.replay_failure("record update replay base is not nominal"));
    };
    if declaration != record {
        return Ok(false);
    }
    let nested = nested_owned_bytes(program, &base.ty, payer)?;
    let generic_flat = !arguments.is_empty()
        && flat_owned_bytes(program.declarations(), declaration, arguments, payer)?;
    if !nested && !generic_flat {
        return Ok(false);
    }
    let declared = program
        .declarations()
        .record_fields(record)
        .ok_or_else(|| payer.replay_failure("nested update has no inventory"))?;
    let mut seen = BTreeSet::new();
    for initializer in fields {
        payer.bytes(size_of::<(
            [&hir::ResolvedFieldInitializer; 2],
            std::slice::Iter<'_, hir::ResolvedFieldDeclaration>,
            Option<&hir::ResolvedFieldDeclaration>,
            [&DeclarationId; 4],
            [bool; 4],
            ResolvedType,
            [Result<ResolvedType, Diagnostic>; 2],
            [Result<bool, Diagnostic>; 2],
            OwnershipMode,
        )>())?;
        if seen.contains(&initializer.field) {
            return Err(payer.replay_failure("nested update replay repeats a replacement field"));
        }
        payer.node::<&DeclarationId, ()>()?;
        seen.insert(&initializer.field);
        let field = declared
            .iter()
            .find(|field| field.id == initializer.field)
            .ok_or_else(|| payer.replay_failure("nested update replay has a foreign field"))?;
        let ty = payer.substitute_update_type(&field.ty, record, arguments)?;
        let ownership = if payer.update_type_needs_drop(program, &ty)? {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        if initializer.value.ty != ty || initializer.value.ownership != ownership {
            return Err(payer.replay_failure(
                "nested update replay replacement type or ownership is not canonical",
            ));
        }
    }
    Ok(nested)
}

fn nested_owned_bytes(
    program: &dyn ReplayContext,
    root: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    type Frame = (ResolvedType, usize, BTreeSet<DeclarationId>);
    payer.bytes(size_of::<(
        [(&dyn ReplayContext, &ResolvedType, &mut Payer<'_>); 2],
        Vec<Frame>,
        Frame,
        [Result<ResolvedType, Diagnostic>; 2],
        (usize, usize, bool),
    )>())?;
    let mut pending = Vec::new();
    payer.grow(&mut pending, 1)?;
    pending.push((payer.clone_update_type(root)?, 0, BTreeSet::new()));
    let (mut fields, mut leaves, mut nested) = (0usize, 0usize, false);
    loop {
        payer.bytes(size_of::<(
            [&mut Vec<Frame>; 2],
            [Option<Frame>; 2],
            [usize; 6],
            [bool; 7],
            [&DeclarationId; 3],
            Option<&[hir::ResolvedFieldDeclaration]>,
            std::iter::Rev<std::slice::Iter<'_, hir::ResolvedFieldDeclaration>>,
        )>())?;
        let Some((ty, depth, mut ancestors)) = pending.pop() else {
            return Ok(nested);
        };
        if depth > DEPTH_LIMIT {
            return Err(payer.replay_failure("nested update replay exceeds its depth bound"));
        }
        if matches!(ty, ResolvedType::Bytes | ResolvedType::String) {
            leaves += 1;
            nested |= depth > 1 || ty == ResolvedType::String;
            if leaves > LEAF_LIMIT {
                return Err(
                    payer.replay_failure("nested update replay exceeds its owned-leaf bound")
                );
            }
            continue;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            continue;
        };
        if ancestors.contains(&declaration) {
            return Ok(false);
        }
        let Some(declared) = program.declarations().record_fields(&declaration) else {
            continue;
        };
        fields = fields
            .checked_add(declared.len())
            .ok_or_else(|| payer.replay_failure("nested update work overflowed"))?;
        if fields > FIELD_LIMIT {
            return Err(payer.replay_failure("nested update replay exceeds its field-work bound"));
        }
        // Declaration identities share their existing text on Clone. Only
        // the new ordered backing and the receiving handles need payment.
        payer.node::<DeclarationId, ()>()?;
        ancestors.insert(declaration.clone());
        for field in declared.iter().rev() {
            payer.bytes(size_of::<(
                [&hir::ResolvedFieldDeclaration; 2],
                [(
                    &ResolvedType,
                    &DeclarationId,
                    &[ResolvedType],
                    &mut Payer<'_>,
                ); 2],
                [Result<ResolvedType, Diagnostic>; 2],
                [Frame; 2],
                [&BTreeSet<DeclarationId>; 2],
                [usize; 3],
            )>())?;
            let ty = payer.substitute_update_type(&field.ty, &declaration, &arguments)?;
            for _ in &ancestors {
                payer.node::<DeclarationId, ()>()?;
            }
            let next = ancestors.clone();
            payer.grow(&mut pending, 1)?;
            pending.push((ty, depth + 1, next));
        }
    }
}

// The original generic-flat predicate uses substituted values only to check
// for Bytes and scalar leaves. Loan the scalar argument instead of cloning it.
// Compound templates still check their complete parameter sequence before
// returning the original negative classification.
fn flat_owned_bytes(
    declarations: &hir::DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    payer.bytes(size_of::<(
        [(
            &hir::DeclarationIndex,
            &DeclarationId,
            &[ResolvedType],
            &mut Payer<'_>,
        ); 2],
        Option<&[hir::ResolvedFieldDeclaration]>,
        std::slice::Iter<'_, hir::ResolvedFieldDeclaration>,
        Vec<&ResolvedType>,
        [bool; 5],
    )>())?;
    if declarations
        .declaration(declaration)
        .is_none_or(|item| item.kind != DeclarationKind::Record)
        || declarations
            .type_parameters(declaration)
            .is_none_or(|params| params.len() != arguments.len())
        || arguments
            .iter()
            .any(|arg| *arg != ResolvedType::Bytes && !hir::is_scalar_resolved_type(arg))
    {
        return Ok(false);
    }
    let Some(fields) = declarations.record_fields(declaration) else {
        return Ok(false);
    };
    let mut pending = Vec::new();
    let (mut saw_bytes, mut all_admitted) = (false, true);
    for field in fields {
        payer.bytes(size_of::<(
            [&hir::ResolvedFieldDeclaration; 2],
            [&ResolvedType; 3],
            [Option<&ResolvedType>; 2],
            [bool; 4],
        )>())?;
        let ty = match &field.ty {
            ResolvedType::TypeParameter { owner, index } => {
                let Some(ty) = argument(owner, *index, declaration, arguments) else {
                    return Ok(false);
                };
                ty
            }
            ty @ (ResolvedType::Nominal { .. } | ResolvedType::Function { .. }) => {
                if !validate_compound(ty, declaration, arguments, &mut pending, payer)? {
                    return Ok(false);
                }
                all_admitted = false;
                continue;
            }
            ty => ty,
        };
        saw_bytes |= *ty == ResolvedType::Bytes;
        all_admitted &= *ty == ResolvedType::Bytes || hir::is_scalar_resolved_type(ty);
    }
    Ok(saw_bytes && all_admitted)
}

fn argument<'a>(
    owner: &DeclarationId,
    index: u32,
    expected: &DeclarationId,
    arguments: &'a [ResolvedType],
) -> Option<&'a ResolvedType> {
    if owner != expected {
        return None;
    }
    arguments.get(usize::try_from(index).ok()?)
}

fn validate_compound<'a>(
    root: &'a ResolvedType,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
    pending: &mut Vec<&'a ResolvedType>,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    payer.grow(pending, 1)?;
    pending.push(root);
    loop {
        payer.bytes(size_of::<(
            [&mut Vec<&ResolvedType>; 2],
            [Option<&ResolvedType>; 2],
            [(&DeclarationId, u32, &DeclarationId, &[ResolvedType]); 2],
            [usize; 3],
            [bool; 3],
        )>())?;
        let Some(ty) = pending.pop() else {
            return Ok(true);
        };
        match ty {
            ResolvedType::TypeParameter { owner, index }
                if argument(owner, *index, declaration, arguments).is_none() =>
            {
                return Ok(false);
            }
            ResolvedType::Nominal { arguments, .. } => {
                payer.grow(pending, arguments.len())?;
                pending.extend(arguments.iter().rev());
            }
            ResolvedType::Function { parameters, result } => {
                let slots = parameters
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| payer.construction_refusal("update type cursor overflowed"))?;
                payer.grow(pending, slots)?;
                pending.push(result);
                pending.extend(parameters.iter().rev());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
    use crate::cleanup_plan::replay::ReplayBudget;
    use crate::test_allocation_observer::measure;

    const SOURCE: &str = r#"
module funded.update;
@id("update.leaf") record Leaf { @id("update.leaf.payload") payload: Bytes, }
@id("update.root") record Root {
  @id("update.root.leaf") leaf: Leaf,
  @id("update.root.sequence") sequence: i64,
}
@id("update.copy") fn replace(value: own Root) -> Root {
  value with { sequence: 9 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

    #[test]
    fn complete_record_scan_preserves_first_diagnostic_and_drops_its_backing() {
        let parsed = crate::check(SOURCE, "funded-record-update.spx").unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let original = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "update.copy")
            .unwrap();
        for fault in 0..4 {
            let mut function = original.clone();
            let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
                panic!("update fixture body is a block");
            };
            let ResolvedExprKind::UpdateRecord { fields, .. } = &mut tail.kind else {
                panic!("update fixture tail is a record replacement");
            };
            match fault {
                0 => {}
                1 => fields[0].field = DeclarationId::new("update.root.foreign"),
                2 => fields.push(fields[0].clone()),
                3 => fields[0].value.ownership = OwnershipMode::Borrow,
                _ => unreachable!(),
            }
            let expected = super::super::function_contains(&program, &function);
            assert_eq!(expected.is_ok(), fault == 0);
            let run = |cap, floor| {
                with_limit_usage_refusal(cap, || {
                    assert!(set_active_floor(floor));
                    let mut budget = ReplayBudget::new();
                    contains(
                        &program,
                        &function,
                        &mut Payer::capture(&function, &mut budget)?,
                    )
                })
            };
            let (actual, refused, paid, first) = run(usize::MAX, 0);
            assert_eq!(actual, expected);
            assert!(!refused && first.is_none());
            let (exact, refused, exact_paid, first) = run(paid + 17, 17);
            assert_eq!(exact, expected);
            assert_eq!(exact_paid, paid);
            assert!(!refused && first.is_none());
            let (short, refused, _, first) = run(paid + 16, 17);
            assert!(short.is_err() && refused && first.is_some());
            let ((status, heap), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    Payer::capture(&function, &mut budget)
                        .and_then(|mut payer| contains(&program, &function, &mut payer))
                        .is_ok()
                })
            });
            assert_eq!(status, expected.is_ok());
            assert!(!refused && first.is_none() && !heap.invalid);
            assert_eq!(heap.live, 0);
            assert_eq!(heap.allocated, heap.freed);
        }
    }
}
