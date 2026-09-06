//! Compiler-owned operations for the internal Owned Bounded Box v1 profile.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ValueId};

mod wrappers;
pub(crate) use wrappers::*;

pub(crate) const NEW_NAME: &str = "box_new";
pub(crate) const GET_NAME: &str = "box_get";
pub(crate) const INTO_INNER_NAME: &str = "box_into_inner";
pub(crate) const NEW_ID: &str = "core.box.new";
pub(crate) const GET_ID: &str = "core.box.get";
pub(crate) const INTO_INNER_ID: &str = "core.box.into-inner";
pub(crate) const STATUS_DOMAIN: &str = "semaprax.box.v1";
pub(crate) const ALLOCATION_FAILURE_CODE: u32 = 1;
pub(crate) const MAX_LIVE_ALLOCATIONS: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoxOp {
    New,
    Get,
    IntoInner,
}

pub(crate) const ALL: [BoxOp; 3] = [BoxOp::New, BoxOp::Get, BoxOp::IntoInner];

impl BoxOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::New => NEW_NAME,
            Self::Get => GET_NAME,
            Self::IntoInner => INTO_INNER_NAME,
        }
    }
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::New => NEW_ID,
            Self::Get => GET_ID,
            Self::IntoInner => INTO_INNER_ID,
        }
    }
    pub(crate) const fn param_ownership(self) -> OwnershipMode {
        match self {
            Self::New => OwnershipMode::Value,
            Self::Get => OwnershipMode::Borrow,
            Self::IntoInner => OwnershipMode::Own,
        }
    }
    pub(crate) fn resolved_param_type(self, element: &ResolvedType) -> ResolvedType {
        match self {
            Self::New => element.clone(),
            Self::Get | Self::IntoInner => resolved_box(element.clone()),
        }
    }
    pub(crate) fn ast_param_type(self, element: &Type) -> Type {
        match self {
            Self::New => element.clone(),
            Self::Get | Self::IntoInner => ast_box(element.clone()),
        }
    }
    pub(crate) fn resolved_return_type(self, element: &ResolvedType) -> ResolvedType {
        match self {
            Self::New => resolved_box(element.clone()),
            Self::Get | Self::IntoInner => element.clone(),
        }
    }
    pub(crate) fn ast_return_type(self, element: &Type) -> Type {
        match self {
            Self::New => ast_box(element.clone()),
            Self::Get | Self::IntoInner => element.clone(),
        }
    }
}

pub(crate) fn by_name(name: &str) -> Option<BoxOp> {
    ALL.into_iter().find(|op| op.name() == name)
}
pub(crate) fn by_id(id: &str) -> Option<BoxOp> {
    ALL.into_iter().find(|op| op.id() == id)
}
pub(crate) fn ast_element_is_admitted(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}
pub(crate) fn resolved_element_is_admitted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::Char
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
    )
}
pub(crate) fn ast_box(element: Type) -> Type {
    Type::Named {
        name: "Box".to_owned(),
        arguments: vec![element],
    }
}
pub(crate) fn resolved_box(element: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::BOX_ID),
        arguments: vec![element],
    }
}
pub(crate) fn ast_params(op: BoxOp, element: &Type) -> Vec<Param> {
    vec![Param {
        name: "arg0".to_owned(),
        mode: match op.param_ownership() {
            OwnershipMode::Own => ParamMode::Own,
            OwnershipMode::Borrow => ParamMode::Borrow,
            _ => ParamMode::Value,
        },
        ty: op.ast_param_type(element),
        span: Span::default(),
    }]
}
pub(crate) fn resolved_params(op: BoxOp, element: &ResolvedType) -> Vec<ResolvedParam> {
    vec![ResolvedParam {
        id: ValueId::intrinsic_parameter(op.id(), 0),
        name: "arg0".to_owned(),
        ownership: op.param_ownership(),
        ty: op.resolved_param_type(element),
        span: Span::default(),
    }]
}
pub(crate) fn is_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments } if declaration.as_str() == crate::prelude::BOX_ID && matches!(arguments.as_slice(), [element] if resolved_element_is_admitted(element)))
}
