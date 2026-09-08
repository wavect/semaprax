//! Independent HIR classifier for bounded nested owning generic relays.

use super::*;

pub(super) fn is_admitted(
    declarations: &DeclarationIndex,
    root: &ResolvedType,
    function_owner: &DeclarationId,
    parameter_count: usize,
) -> bool {
    enum Frame<'a> {
        Type(ResolvedType, usize),
        Fields(
            DeclarationId,
            &'a [ResolvedFieldDeclaration],
            Vec<ResolvedType>,
            usize,
            usize,
        ),
        Leave(String),
    }

    let is_parameter = |ty: &ResolvedType| {
        matches!(ty, ResolvedType::TypeParameter { owner, index }
            if owner == function_owner
                && usize::try_from(*index).is_ok_and(|index| index < parameter_count))
    };
    let mut frames = vec![Frame::Type(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut byte_leaves = 0usize;
    let mut visited_fields = 0usize;
    let mut saw_record = false;
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Type(ResolvedType::Function { .. }, _) => return false,
            Frame::Type(ResolvedType::Bytes, _) => {
                byte_leaves += 1;
                if byte_leaves > MAX_NESTED_OWNED_BYTE_LEAVES {
                    return false;
                }
            }
            Frame::Type(ref ty, _) if nested_record_copy_scalar_is_admitted(ty) => {}
            Frame::Type(ref ty, _) if is_parameter(ty) => {}
            Frame::Type(
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                },
                depth,
            ) => {
                if depth > MAX_NESTED_OWNED_RECORD_DEPTH
                    || declarations
                        .declaration(&declaration)
                        .is_none_or(|item| item.kind != DeclarationKind::Record)
                    || declarations
                        .type_parameters(&declaration)
                        .is_none_or(|parameters| parameters.len() != arguments.len())
                    || arguments.iter().any(|argument| {
                        *argument != ResolvedType::Bytes
                            && !nested_record_copy_scalar_is_admitted(argument)
                            && !is_parameter(argument)
                            && !matches!(argument, ResolvedType::Nominal { .. })
                    })
                {
                    return false;
                }
                let Some(fields) = declarations.record_fields(&declaration) else {
                    return false;
                };
                saw_record = true;
                let identity = ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments: arguments.clone(),
                }
                .identity_key();
                if !active.insert(identity.clone()) {
                    return false;
                }
                frames.push(Frame::Leave(identity));
                frames.push(Frame::Fields(declaration, fields, arguments, 0, depth));
            }
            Frame::Type(
                ResolvedType::Unit
                | ResolvedType::ArrayU8(_)
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::TypeParameter { .. },
                _,
            ) => return false,
            Frame::Type(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => unreachable!("admitted scalar handled above"),
            Frame::Fields(declaration, fields, arguments, index, depth) => {
                let Some(field) = fields.get(index) else {
                    continue;
                };
                visited_fields += 1;
                if visited_fields > MAX_NESTED_OWNED_RECORD_FIELDS {
                    return false;
                }
                frames.push(Frame::Fields(
                    declaration.clone(),
                    fields,
                    arguments.clone(),
                    index + 1,
                    depth,
                ));
                let Ok(field_ty) = substitute_type(&field.ty, &declaration, &arguments) else {
                    return false;
                };
                frames.push(Frame::Type(field_ty, depth + 1));
            }
            Frame::Leave(identity) => {
                active.remove(&identity);
            }
        }
    }
    saw_record && byte_leaves > 0
}
