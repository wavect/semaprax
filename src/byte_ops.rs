//! Compiler-owned operations over non-escaping borrowed byte slices.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LEN_NAME: &str = "byte_len";
pub(crate) const GET_NAME: &str = "byte_get";
pub(crate) const LEN_ID: &str = "core.bytes.len";
pub(crate) const GET_ID: &str = "core.bytes.get";
pub(crate) const MAX_EXTERNAL_ROOT_BYTES: u64 = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteOp {
    Len,
    Get,
}

impl ByteOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Len => LEN_NAME,
            Self::Get => GET_NAME,
        }
    }
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Len => LEN_ID,
            Self::Get => GET_ID,
        }
    }
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::Len => 1,
            Self::Get => 2,
        }
    }
    pub(crate) fn param_types(self) -> &'static [ResolvedType] {
        match self {
            Self::Len => &[ResolvedType::SliceU8],
            Self::Get => &[ResolvedType::SliceU8, ResolvedType::Usize],
        }
    }
    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            Self::Len => ResolvedType::Usize,
            Self::Get => ResolvedType::Nominal {
                declaration: DeclarationId::new(crate::prelude::OPTION_ID),
                arguments: vec![ResolvedType::U8],
            },
        }
    }
    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            Self::Len => Type::Usize,
            Self::Get => Type::Named {
                name: "Option".to_owned(),
                arguments: vec![Type::U8],
            },
        }
    }
}

pub(crate) fn by_name(name: &str) -> Option<ByteOp> {
    match name {
        LEN_NAME => Some(ByteOp::Len),
        GET_NAME => Some(ByteOp::Get),
        _ => None,
    }
}
pub(crate) fn by_id(id: &str) -> Option<ByteOp> {
    match id {
        LEN_ID => Some(ByteOp::Len),
        GET_ID => Some(ByteOp::Get),
        _ => None,
    }
}

pub(crate) fn resolved_params(op: ByteOp) -> Vec<ResolvedParam> {
    op.param_types()
        .iter()
        .enumerate()
        .map(|(index, ty)| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: if index == 0 { "value" } else { "index" }.to_owned(),
            ownership: if index == 0 {
                OwnershipMode::Borrow
            } else {
                OwnershipMode::Value
            },
            ty: ty.clone(),
            span: Span::default(),
        })
        .collect()
}

pub(crate) fn ast_params(op: ByteOp) -> Vec<Param> {
    op.param_types()
        .iter()
        .enumerate()
        .map(|(index, ty)| Param {
            name: if index == 0 { "value" } else { "index" }.to_owned(),
            mode: if index == 0 {
                ParamMode::Borrow
            } else {
                ParamMode::Value
            },
            ty: match ty {
                ResolvedType::SliceU8 => Type::SliceU8,
                ResolvedType::Usize => Type::Usize,
                _ => unreachable!(),
            },
            span: Span::default(),
        })
        .collect()
}
