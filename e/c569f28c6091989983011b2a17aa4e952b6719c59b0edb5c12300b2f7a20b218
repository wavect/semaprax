//! Infallible legacy COW cloning, without recursive type-stack growth or payer.
use super::*;
fn ty(root: &ResolvedType) -> ResolvedType {
    if !matches!(root,ResolvedType::Nominal{arguments,..} if !arguments.is_empty())
        && !matches!(root, ResolvedType::Function { .. })
    {
        return root.clone();
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Nominal(crate::hir::DeclarationId, usize),
        Function(usize),
    }
    let mut frames = vec![Frame::Enter(root)];
    let mut values = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(value) => match value {
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    frames.push(Frame::Nominal(declaration.clone(), arguments.len()));
                    for child in arguments.iter().rev() {
                        frames.push(Frame::Enter(child));
                    }
                }
                ResolvedType::Function { parameters, result } => {
                    frames.push(Frame::Function(parameters.len()));
                    frames.push(Frame::Enter(result));
                    for child in parameters.iter().rev() {
                        frames.push(Frame::Enter(child));
                    }
                }
                leaf => values.push(leaf.clone()),
            },
            Frame::Nominal(declaration, count) => {
                let split = values
                    .len()
                    .checked_sub(count)
                    .expect("legacy nominal child count");
                let arguments = values.drain(split..).collect();
                values.push(ResolvedType::Nominal {
                    declaration,
                    arguments,
                });
            }
            Frame::Function(count) => {
                let result = Box::new(values.pop().expect("legacy function result"));
                let split = values
                    .len()
                    .checked_sub(count)
                    .expect("legacy function parameter count");
                let parameters = values.drain(split..).collect();
                values.push(ResolvedType::Function { parameters, result });
            }
        }
    }
    assert_eq!(values.len(), 1, "legacy complete cloned type");
    values.pop().unwrap()
}
pub(in super::super) fn field(value: &ResolvedFieldDeclaration) -> ResolvedFieldDeclaration {
    ResolvedFieldDeclaration {
        id: value.id.clone(),
        name: value.name.clone(),
        index: value.index,
        ty: ty(&value.ty),
        span: value.span,
    }
}
pub(in super::super) fn case(
    value: &ResolvedVariantCaseDeclaration,
) -> ResolvedVariantCaseDeclaration {
    ResolvedVariantCaseDeclaration {
        id: value.id.clone(),
        name: value.name.clone(),
        index: value.index,
        fields: value.fields.iter().map(field).collect(),
        span: value.span,
    }
}

pub(in super::super) fn fields_equal(
    left: &[ResolvedFieldDeclaration],
    right: &[ResolvedFieldDeclaration],
) -> bool {
    fn same_type(left: &ResolvedType, right: &ResolvedType) -> bool {
        fn nested(ty: &ResolvedType) -> bool {
            matches!(
                ty,
                ResolvedType::Nominal { .. } | ResolvedType::Function { .. }
            )
        }
        if !nested(left) && !nested(right) {
            return left == right;
        }
        let mut pending = vec![(left, right)];
        while let Some((left, right)) = pending.pop() {
            match (left, right) {
                (
                    ResolvedType::Nominal {
                        declaration: a,
                        arguments: aa,
                    },
                    ResolvedType::Nominal {
                        declaration: b,
                        arguments: bb,
                    },
                ) => {
                    if a != b || aa.len() != bb.len() {
                        return false;
                    }
                    pending.extend(aa.iter().zip(bb).rev());
                }
                (
                    ResolvedType::Function {
                        parameters: aa,
                        result: a,
                    },
                    ResolvedType::Function {
                        parameters: bb,
                        result: b,
                    },
                ) => {
                    if aa.len() != bb.len() {
                        return false;
                    }
                    pending.push((a, b));
                    pending.extend(aa.iter().zip(bb).rev());
                }
                _ => {
                    if nested(left) || nested(right) || left != right {
                        return false;
                    }
                }
            }
        }
        true
    }
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.id == b.id
                && a.name == b.name
                && a.index == b.index
                && a.span == b.span
                && same_type(&a.ty, &b.ty)
        })
}
