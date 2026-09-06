//! Independent source classifier for bounded nested owning generic relays.

use super::*;

pub(super) fn is_admitted(
    types: &TypeTable<'_>,
    root: &Type,
    function_parameters: &HashSet<&str>,
) -> bool {
    enum Frame<'a> {
        Type(Type, usize),
        Fields(
            &'a TypeDeclaration,
            &'a [FieldDeclaration],
            Vec<Type>,
            usize,
            usize,
        ),
        Leave(String),
    }

    let mut frames = vec![Frame::Type(root.clone(), 1)];
    let mut active = HashSet::new();
    let mut byte_leaves = 0usize;
    let mut visited_fields = 0usize;
    let mut saw_record = false;
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Type(Type::Bytes, _) => {
                byte_leaves += 1;
                if byte_leaves > MAX_NESTED_OWNED_BYTE_LEAVES {
                    return false;
                }
            }
            Frame::Type(ref ty, _) if owned_byte_record_copy_field_is_admitted(ty) => {}
            Frame::Type(Type::Named { name, arguments }, _)
                if arguments.is_empty() && function_parameters.contains(name.as_str()) => {}
            Frame::Type(Type::Named { name, arguments }, depth) => {
                if depth > MAX_NESTED_OWNED_RECORD_DEPTH {
                    return false;
                }
                let Some(declaration) = types.declaration(&name) else {
                    return false;
                };
                let TypeDeclarationKind::Record { fields } = &declaration.kind else {
                    return false;
                };
                if arguments.len() != declaration.type_parameters.len()
                    || arguments.iter().any(|argument| {
                        *argument != Type::Bytes
                            && !owned_byte_record_copy_field_is_admitted(argument)
                            && !matches!(argument, Type::Named { .. })
                    })
                {
                    return false;
                }
                saw_record = true;
                let identity = Type::Named {
                    name: name.clone(),
                    arguments: arguments.clone(),
                }
                .to_string();
                if !active.insert(identity.clone()) {
                    return false;
                }
                frames.push(Frame::Leave(identity));
                frames.push(Frame::Fields(declaration, fields, arguments, 0, depth));
            }
            Frame::Type(
                Type::I64
                | Type::I32
                | Type::Char
                | Type::U8
                | Type::Usize
                | Type::F32
                | Type::F64
                | Type::Bool,
                _,
            ) => unreachable!("admitted scalar handled above"),
            Frame::Type(Type::ArrayU8(_) | Type::String | Type::Str | Type::SliceU8, _) => {
                return false;
            }
            Frame::Fields(declaration, fields, arguments, index, depth) => {
                let Some(field) = fields.get(index) else {
                    continue;
                };
                visited_fields += 1;
                if visited_fields > MAX_NESTED_OWNED_RECORD_FIELDS {
                    return false;
                }
                frames.push(Frame::Fields(
                    declaration,
                    fields,
                    arguments.clone(),
                    index + 1,
                    depth,
                ));
                let Some(field_ty) =
                    TypeTable::substitute_variant_type(declaration, &arguments, &field.ty)
                else {
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
