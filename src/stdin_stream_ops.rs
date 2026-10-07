//! Closed compiler-owned streaming stdin vocabulary and reader representation.
//!
//! A reader is a sealed nominal cleanup leaf, not an authored record or a file
//! descriptor. Target profiles must explicitly admit this vocabulary.

pub(crate) mod analysis;

use crate::ast::{Param, ParamMode, Span, Type, TypeDeclaration, TypeDeclarationKind};
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedHostCommandOperation, ResolvedParam, ResolvedType,
    TypeFacts, ValueId,
};

pub(crate) const READER_ID: &str = "core.stdin-stream.reader";
pub(crate) const DROP_ID: &str = "core.stdin-stream.drop";
pub(crate) const OPEN_ID: &str = "core.host.stdin-stream-open";
pub(crate) const NEXT_ID: &str = "core.host.stdin-stream-next";
pub(crate) const CHUNK_ID: &str = "core.stdin-stream.chunk";
pub(crate) const EOF_ID: &str = "core.stdin-stream.eof";
pub(crate) const CHUNK_BYTES: u64 = 4_096;
pub(crate) const PROFILE: &str = "language-command-io.stream.v1";
pub(crate) const INPUT_PROFILE: &str = "argv-utf8+stdin-stream.v1";

pub(crate) fn ast_reader() -> Type {
    Type::Named {
        name: "StdinReader".to_owned(),
        arguments: Vec::new(),
    }
}
pub(crate) fn reader() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(READER_ID),
        arguments: Vec::new(),
    }
}
pub(crate) fn ast_is_reader(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == "StdinReader" && arguments.is_empty())
}
pub(crate) fn is_reader(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments } if declaration.as_str() == READER_ID && arguments.is_empty())
}
pub(crate) fn declaration() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: READER_ID.to_owned(),
        explicit_id: true,
        name: "StdinReader".to_owned(),
        name_span: Span::default(),
        type_parameters: Vec::new(),
        kind: TypeDeclarationKind::Record { fields: Vec::new() },
        extends: None,
        invariants: None,
        span: Span::default(),
    }
}
pub(crate) fn type_facts(
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> Option<TypeFacts> {
    (declaration.as_str() == READER_ID && arguments.is_empty()).then(|| TypeFacts {
        copy: false,
        contains_resource: false,
        sized: true,
        needs_drop: true,
        layout_key: "stdin-stream-reader.v1".to_owned(),
    })
}
pub(crate) const fn is_host(op: ResolvedHostCommandOperation) -> bool {
    matches!(
        op,
        ResolvedHostCommandOperation::StdinStreamOpen
            | ResolvedHostCommandOperation::StdinStreamNext
    )
}
pub(crate) fn host_by_name(name: &str) -> Option<ResolvedHostCommandOperation> {
    match name {
        "stdin_stream_open" => Some(ResolvedHostCommandOperation::StdinStreamOpen),
        "stdin_stream_next" => Some(ResolvedHostCommandOperation::StdinStreamNext),
        _ => None,
    }
}
pub(crate) fn host_by_id(id: &str) -> Option<ResolvedHostCommandOperation> {
    match id {
        OPEN_ID => Some(ResolvedHostCommandOperation::StdinStreamOpen),
        NEXT_ID => Some(ResolvedHostCommandOperation::StdinStreamNext),
        _ => None,
    }
}
pub(crate) fn host_params(op: ResolvedHostCommandOperation) -> Vec<Param> {
    match op {
        ResolvedHostCommandOperation::StdinStreamOpen => Vec::new(),
        ResolvedHostCommandOperation::StdinStreamNext => vec![Param {
            name: "reader".to_owned(),
            mode: ParamMode::Own,
            ty: ast_reader(),
            span: Span::default(),
        }],
        _ => unreachable!("closed streaming operation table"),
    }
}
pub(crate) fn resolved_host_params(op: ResolvedHostCommandOperation) -> Vec<ResolvedParam> {
    host_params(op)
        .into_iter()
        .enumerate()
        .map(|(index, param)| ResolvedParam {
            id: ValueId::intrinsic_parameter(crate::command_io_ops::id(op), index),
            name: param.name,
            ownership: OwnershipMode::Own,
            ty: reader(),
            span: param.span,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PureOp {
    Chunk,
    Eof,
}
impl PureOp {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Chunk => CHUNK_ID,
            Self::Eof => EOF_ID,
        }
    }
    pub(crate) fn ast_result(self) -> Type {
        match self {
            Self::Chunk => Type::SliceU8,
            Self::Eof => Type::Bool,
        }
    }
    pub(crate) fn result(self) -> ResolvedType {
        match self {
            Self::Chunk => ResolvedType::SliceU8,
            Self::Eof => ResolvedType::Bool,
        }
    }
    pub(crate) fn params(self) -> Vec<Param> {
        vec![Param {
            name: "reader".to_owned(),
            mode: ParamMode::Borrow,
            ty: ast_reader(),
            span: Span::default(),
        }]
    }
    pub(crate) fn resolved_params(self) -> Vec<ResolvedParam> {
        vec![ResolvedParam {
            id: ValueId::intrinsic_parameter(self.id(), 0),
            name: "reader".to_owned(),
            ownership: OwnershipMode::Borrow,
            ty: reader(),
            span: Span::default(),
        }]
    }
}
pub(crate) fn pure_by_name(name: &str) -> Option<PureOp> {
    match name {
        "stdin_stream_chunk" => Some(PureOp::Chunk),
        "stdin_stream_eof" => Some(PureOp::Eof),
        _ => None,
    }
}
pub(crate) fn pure_by_id(id: &str) -> Option<PureOp> {
    match id {
        CHUNK_ID => Some(PureOp::Chunk),
        EOF_ID => Some(PureOp::Eof),
        _ => None,
    }
}
pub(crate) fn source_next_is_same_owner(
    program: &crate::ast::Program,
    name: &str,
    value: &crate::ast::Expr,
) -> bool {
    let crate::ast::ExprKind::Call {
        name: operation,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    type_arguments.is_empty()
        && matches!(args.as_slice(), [crate::ast::Expr { kind: crate::ast::ExprKind::Var(owner), .. }] if owner == name)
        && (operation == "stdin_stream_next"
            || program
                .functions
                .iter()
                .any(|function| function.name == *operation && ast_forward_signature(function)))
}
pub(crate) fn ast_forward_signature(function: &crate::ast::Function) -> bool {
    function.type_parameters.is_empty()
        && ast_is_reader(&function.return_type)
        && matches!(function.params.as_slice(), [param] if param.mode == ParamMode::Own && ast_is_reader(&param.ty))
        && function
            .effects
            .iter()
            .all(|effect| effect == crate::command_io_ops::STDIN_READ_EFFECT)
        && function.yields.is_none()
        && function.follows.is_none()
}
pub(crate) fn resolved_forward_signature(function: &crate::hir::ResolvedFunction) -> bool {
    is_reader(&function.return_type)
        && matches!(function.params.as_slice(), [param] if param.ownership == OwnershipMode::Own && is_reader(&param.ty))
        && function
            .effects
            .iter()
            .all(|effect| effect == crate::command_io_ops::STDIN_READ_EFFECT)
        && function.yields.is_none()
}
pub(crate) fn hir_reopen(value: &crate::hir::ResolvedExpr, owner: &ValueId) -> bool {
    use crate::hir::ResolvedExprKind;
    if !is_reader(&value.ty) || value.ownership != OwnershipMode::Own {
        return false;
    }
    let args = match &value.kind {
        ResolvedExprKind::HostCommandCall(call)
            if call.operation == ResolvedHostCommandOperation::StdinStreamNext =>
        {
            &call.args
        }
        ResolvedExprKind::Call {
            args,
            instance: None,
            type_arguments,
            ..
        } if type_arguments.is_empty() => args,
        _ => return false,
    };
    matches!(args.as_slice(), [argument] if is_reader(&argument.ty) && argument.ownership == OwnershipMode::Own && matches!(&argument.kind, ResolvedExprKind::Place(place) if &place.root == owner && place.projections.is_empty()))
}
pub(crate) fn owner_reopens(
    function: &crate::hir::ResolvedFunction,
) -> std::collections::BTreeMap<crate::hir::ExpressionId, crate::hir::ExpressionId> {
    crate::string_ops::same_owner_concat_appends(function)
        .into_iter()
        .chain(reader_reopens(function))
        .collect()
}

pub(crate) fn reader_reopens(
    function: &crate::hir::ResolvedFunction,
) -> std::collections::BTreeMap<crate::hir::ExpressionId, crate::hir::ExpressionId> {
    let mut pending = vec![&function.body];
    let mut reopens = std::collections::BTreeMap::new();
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let crate::hir::ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if hir_reopen(value, &binding.id) {
                        let args = match &value.kind {
                            crate::hir::ResolvedExprKind::HostCommandCall(call) => &call.args,
                            crate::hir::ResolvedExprKind::Call { args, .. } => args,
                            _ => unreachable!(),
                        };
                        reopens.insert(args[0].id.clone(), value.id.clone());
                    }
                }
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    reopens
}

pub(crate) fn resolved_type_uses(ty: &ResolvedType) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if declaration.as_str() == READER_ID {
                    return true;
                }
                pending.extend(arguments);
            }
            ResolvedType::Function { parameters, result } => {
                pending.push(result);
                pending.extend(parameters);
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn ast_type_uses(ty: &Type) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            Type::Named { name, arguments } => {
                if name == "StdinReader" {
                    return true;
                }
                pending.extend(arguments);
            }
            Type::Function { parameters, result } => {
                pending.push(result);
                pending.extend(parameters);
            }
            _ => {}
        }
    }
    false
}
pub(crate) fn ast_expression_uses(expression: &crate::ast::Expr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            crate::ast::ExprKind::Call {
                name,
                type_arguments,
                ..
            } => {
                if matches!(
                    name.as_str(),
                    "stdin_stream_open"
                        | "stdin_stream_next"
                        | "stdin_stream_chunk"
                        | "stdin_stream_eof"
                ) || type_arguments.iter().any(ast_type_uses)
                {
                    return true;
                }
            }
            crate::ast::ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                ..
            }
            | crate::ast::ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                ..
            } => {
                if type_name == "StdinReader" || type_arguments.iter().any(ast_type_uses) {
                    return true;
                }
            }
            crate::ast::ExprKind::Closure {
                params,
                return_type,
                ..
            } => {
                if ast_type_uses(return_type) || params.iter().any(|param| ast_type_uses(&param.ty))
                {
                    return true;
                }
            }
            _ => {}
        }
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            pending.push(child);
            index += 1;
        }
    }
    false
}
pub(crate) fn program_uses(program: &crate::ast::Program) -> bool {
    fn function_uses(function: &crate::ast::Function) -> bool {
        ast_type_uses(&function.return_type)
            || function.params.iter().any(|param| ast_type_uses(&param.ty))
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(ast_expression_uses)
    }
    program.functions.iter().any(function_uses)
        || program
            .types
            .iter()
            .any(|declaration| match &declaration.kind {
                TypeDeclarationKind::Record { fields } => {
                    fields.iter().any(|field| ast_type_uses(&field.ty))
                }
                TypeDeclarationKind::Class {
                    fields, methods, ..
                } => {
                    fields.iter().any(|field| ast_type_uses(&field.ty))
                        || methods.iter().any(function_uses)
                }
                TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .any(|field| ast_type_uses(&field.ty)),
                TypeDeclarationKind::Resource { .. } => false,
            })
}

pub(crate) fn resolved_expression_uses(expression: &crate::hir::ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if resolved_type_uses(&expression.ty)
            || matches!(&expression.kind, crate::hir::ResolvedExprKind::HostCommandCall(call) if is_host(call.operation))
            || matches!(&expression.kind, crate::hir::ResolvedExprKind::BorrowPlace { operation, .. } if operation.as_str() == CHUNK_ID)
            || matches!(&expression.kind, crate::hir::ResolvedExprKind::Call { callee, .. } if pure_by_id(callee.as_str()).is_some())
        {
            return true;
        }
        if let crate::hir::ResolvedExprKind::Closure { body, .. } = &expression.kind {
            pending.push(body);
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
pub(crate) fn resolved_function_uses(function: &crate::hir::ResolvedFunction) -> bool {
    resolved_type_uses(&function.return_type)
        || function
            .params
            .iter()
            .any(|param| resolved_type_uses(&param.ty))
        || function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(resolved_expression_uses)
}
pub(crate) fn resolved_program_uses(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(resolved_function_uses)
}

pub(crate) fn contract_bytes() -> Vec<u8> {
    use std::fmt::Write as _;
    let legacy =
        String::from_utf8(crate::prelude::contract_bytes_v9()).expect("compiler prelude is UTF-8");
    let mut contract = legacy.replacen(crate::prelude::SCHEMA_V9, crate::prelude::SCHEMA_V10, 1);
    writeln!(contract, "opaque {READER_ID} StdinReader copy:false contains_resource:false needs_drop:true lifecycle:{DROP_ID}").unwrap();
    writeln!(
        contract,
        "operation {OPEN_ID} stdin_stream_open ()->own:StdinReader effect:process.stdin.read"
    )
    .unwrap();
    writeln!(contract, "operation {NEXT_ID} stdin_stream_next (own:StdinReader)->own:StdinReader effect:process.stdin.read").unwrap();
    writeln!(
        contract,
        "operation {CHUNK_ID} stdin_stream_chunk (borrow:StdinReader)->borrow:Slice<u8>"
    )
    .unwrap();
    writeln!(
        contract,
        "operation {EOF_ID} stdin_stream_eof (borrow:StdinReader)->value:bool"
    )
    .unwrap();
    writeln!(
        contract,
        "profile {PROFILE} input:{INPUT_PROFILE} chunk_bytes:{CHUNK_BYTES} opens_per_path:1"
    )
    .unwrap();
    contract.push_str("rule next_reuses_buffer_and_returns_same_owner\nrule chunk_loan_ends_before_next\nrule short_read_is_not_eof\nrule no_cumulative_stream_input_byte_limit\n");
    contract.into_bytes()
}

pub(crate) fn digest_text() -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}

#[cfg(test)]
mod tests;
