//! Complete frozen ordinary substituters retained independently for tests.
use super::*;

pub(crate) fn substitute_type(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<ResolvedType, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
        FinishCallable(usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                ResolvedType::Function { parameters, result } => {
                    frames.push(Frame::FinishCallable(parameters.len()));
                    frames.push(Frame::Enter(result));
                    frames.extend(parameters.iter().rev().map(Frame::Enter));
                }
                ResolvedType::Never => resolved.push(ResolvedType::Never),
                ResolvedType::Unit => resolved.push(ResolvedType::Unit),
                ResolvedType::I64 => resolved.push(ResolvedType::I64),
                ResolvedType::I32 => resolved.push(ResolvedType::I32),
                ResolvedType::Char => resolved.push(ResolvedType::Char),
                ResolvedType::U8 => resolved.push(ResolvedType::U8),
                ResolvedType::Usize => resolved.push(ResolvedType::Usize),
                ResolvedType::ArrayU8(length) => resolved.push(ResolvedType::ArrayU8(*length)),
                ResolvedType::F32 => resolved.push(ResolvedType::F32),
                ResolvedType::F64 => resolved.push(ResolvedType::F64),
                ResolvedType::Bool => resolved.push(ResolvedType::Bool),
                ResolvedType::String => resolved.push(ResolvedType::String),
                ResolvedType::OnceFunction => resolved.push(ResolvedType::OnceFunction),
                ResolvedType::OnceFunctionI64 => resolved.push(ResolvedType::OnceFunctionI64),
                ResolvedType::MutFunctionI64 => resolved.push(ResolvedType::MutFunctionI64),
                ResolvedType::OnceFunctionI64Pair => {
                    resolved.push(ResolvedType::OnceFunctionI64Pair)
                }
                ResolvedType::Bytes => resolved.push(ResolvedType::Bytes),
                ResolvedType::Str => resolved.push(ResolvedType::Str),
                ResolvedType::SliceU8 => resolved.push(ResolvedType::SliceU8),
                ResolvedType::StringMap => resolved.push(ResolvedType::StringMap),
                ResolvedType::TypeParameter {
                    owner: parameter_owner,
                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(format!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    resolved.push(
                        arguments
                            .get(usize::try_from(*index).map_err(|_| {
                                hir_error(format!("type parameter index {index} does not fit usize"))
                            })?)
                            .cloned()
                            .ok_or_else(|| {
                                hir_error(format!(
                                    "type template for `{owner}` references missing parameter {index}"
                                ))
                            })?,
                    );
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    frames.push(Frame::Finish(declaration, arguments.len()));
                    frames.extend(arguments.iter().rev().map(Frame::Enter));
                }
            },
            Frame::FinishCallable(count) => {
                let result = resolved
                    .pop()
                    .ok_or_else(|| hir_error("incomplete callable substitution"))?;
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("incomplete callable parameters"))?;
                let parameters = resolved.drain(split..).collect();
                resolved.push(ResolvedType::Function {
                    parameters,
                    result: Box::new(result),
                });
            }
            Frame::Finish(declaration, count) => {
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("type substitution traversal is incomplete"))?;
                let nested = resolved.drain(split..).collect();
                resolved.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments: nested,
                });
            }
        }
    }
    if resolved.len() != 1 {
        return Err(hir_error("type substitution traversal did not settle"));
    }
    Ok(resolved
        .pop()
        .expect("substitution result count checked above"))
}

pub(super) fn substitute_source_function_type(
    function: &crate::ast::Function,
    arguments: &[Type],
    template: &Type,
) -> Option<Type> {
    enum Frame<'a> {
        Enter(&'a Type),
        Finish(&'a str, usize),
        FinishCallable(usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                Type::Function { parameters, result } => {
                    frames.push(Frame::FinishCallable(parameters.len()));
                    frames.push(Frame::Enter(result));
                    frames.extend(parameters.iter().rev().map(Frame::Enter));
                }
                Type::Never => resolved.push(Type::Never),
                Type::I64 => resolved.push(Type::I64),
                Type::I32 => resolved.push(Type::I32),
                Type::Char => resolved.push(Type::Char),
                Type::U8 => resolved.push(Type::U8),
                Type::Usize => resolved.push(Type::Usize),
                Type::ArrayU8(length) => resolved.push(Type::ArrayU8(*length)),
                Type::F32 => resolved.push(Type::F32),
                Type::F64 => resolved.push(Type::F64),
                Type::Bool => resolved.push(Type::Bool),
                Type::String => resolved.push(Type::String),
                Type::OnceFunction => resolved.push(Type::OnceFunction),
                Type::OnceFunctionI64 => resolved.push(Type::OnceFunctionI64),
                Type::OnceFunctionI64Pair => resolved.push(Type::OnceFunctionI64Pair),
                Type::MutFunctionI64 => resolved.push(Type::MutFunctionI64),
                Type::Bytes => resolved.push(Type::Bytes),
                Type::Str => resolved.push(Type::Str),
                Type::SliceU8 => resolved.push(Type::SliceU8),
                Type::StringMap => resolved.push(Type::StringMap),
                Type::Named {
                    name,
                    arguments: nested,
                } => {
                    if nested.is_empty() {
                        if let Some(index) = function
                            .type_parameters
                            .iter()
                            .position(|parameter| parameter.name == *name)
                        {
                            resolved.push(arguments.get(index)?.clone());
                            continue;
                        }
                    }
                    frames.push(Frame::Finish(name, nested.len()));
                    frames.extend(nested.iter().rev().map(Frame::Enter));
                }
            },
            Frame::FinishCallable(count) => {
                let result = resolved.pop()?;
                let split = resolved.len().checked_sub(count)?;
                let parameters = resolved.drain(split..).collect();
                resolved.push(Type::Function {
                    parameters,
                    result: Box::new(result),
                });
            }
            Frame::Finish(name, count) => {
                let split = resolved.len().checked_sub(count)?;
                let arguments = resolved.drain(split..).collect();
                resolved.push(Type::Named {
                    name: name.to_owned(),
                    arguments,
                });
            }
        }
    }
    (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
}
