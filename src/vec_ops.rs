//! Compiler-owned operations for the internal Owned Bounded Vec v1 profile.

use crate::ast::{Expr, ExprKind, Param, ParamMode, Span, Type};
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedParam, ResolvedType,
    ValueId,
};

pub(crate) const WITH_CAPACITY_NAME: &str = "vec_with_capacity";
pub(crate) const PUSH_NAME: &str = "vec_push";
pub(crate) const LEN_NAME: &str = "vec_len";
pub(crate) const CAPACITY_NAME: &str = "vec_capacity";
pub(crate) const GET_NAME: &str = "vec_get";
pub(crate) const WITH_CAPACITY_ID: &str = "core.vec.with-capacity";
pub(crate) const PUSH_ID: &str = "core.vec.push";
pub(crate) const LEN_ID: &str = "core.vec.len";
pub(crate) const CAPACITY_ID: &str = "core.vec.capacity";
pub(crate) const GET_ID: &str = "core.vec.get";
pub(crate) const MAX_CAPACITY: u64 = 8_192;
pub(crate) const STATUS_DOMAIN: &str = "semaprax.vec.v1";
pub(crate) const PUSH_FULL_CODE: u32 = 1;
pub(crate) const GET_OUT_OF_BOUNDS_CODE: u32 = 2;
pub(crate) const ALLOCATION_FAILURE_CODE: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VecOp {
    WithCapacity,
    Push,
    Len,
    Capacity,
    Get,
}

impl VecOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::WithCapacity => WITH_CAPACITY_NAME,
            Self::Push => PUSH_NAME,
            Self::Len => LEN_NAME,
            Self::Capacity => CAPACITY_NAME,
            Self::Get => GET_NAME,
        }
    }
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::WithCapacity => WITH_CAPACITY_ID,
            Self::Push => PUSH_ID,
            Self::Len => LEN_ID,
            Self::Capacity => CAPACITY_ID,
            Self::Get => GET_ID,
        }
    }
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::WithCapacity => 1,
            Self::Push => 2,
            Self::Len | Self::Capacity => 1,
            Self::Get => 2,
        }
    }
    pub(crate) const fn param_ownership(self, index: usize) -> OwnershipMode {
        match (self, index) {
            (Self::Push, 0) => OwnershipMode::Own,
            (Self::Len | Self::Capacity | Self::Get, 0) => OwnershipMode::Borrow,
            _ => OwnershipMode::Value,
        }
    }
    pub(crate) fn resolved_return_type(self, element: &ResolvedType) -> ResolvedType {
        match self {
            Self::WithCapacity | Self::Push => resolved_vec(element.clone()),
            Self::Len | Self::Capacity => ResolvedType::Usize,
            Self::Get => element.clone(),
        }
    }
    pub(crate) fn ast_return_type(self, element: &Type) -> Type {
        match self {
            Self::WithCapacity | Self::Push => ast_vec(element.clone()),
            Self::Len | Self::Capacity => Type::Usize,
            Self::Get => element.clone(),
        }
    }
    pub(crate) fn accepts_resolved(
        self,
        index: usize,
        ty: &ResolvedType,
        element: &ResolvedType,
    ) -> bool {
        match (self, index) {
            (Self::WithCapacity, 0) => *ty == ResolvedType::Usize,
            (Self::Push, 0) | (Self::Len | Self::Capacity | Self::Get, 0) => {
                *ty == resolved_vec(element.clone())
            }
            (Self::Push, 1) => ty == element,
            (Self::Get, 1) => *ty == ResolvedType::Usize,
            _ => false,
        }
    }
}

pub(crate) fn by_name(name: &str) -> Option<VecOp> {
    match name {
        WITH_CAPACITY_NAME => Some(VecOp::WithCapacity),
        PUSH_NAME => Some(VecOp::Push),
        LEN_NAME => Some(VecOp::Len),
        CAPACITY_NAME => Some(VecOp::Capacity),
        GET_NAME => Some(VecOp::Get),
        _ => None,
    }
}
pub(crate) fn by_id(id: &str) -> Option<VecOp> {
    match id {
        WITH_CAPACITY_ID => Some(VecOp::WithCapacity),
        PUSH_ID => Some(VecOp::Push),
        LEN_ID => Some(VecOp::Len),
        CAPACITY_ID => Some(VecOp::Capacity),
        GET_ID => Some(VecOp::Get),
        _ => None,
    }
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
pub(crate) fn ast_vec(element: Type) -> Type {
    Type::Named {
        name: "Vec".to_owned(),
        arguments: vec![element],
    }
}
pub(crate) fn resolved_vec(element: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::VEC_ID),
        arguments: vec![element],
    }
}
pub(crate) fn ast_params(op: VecOp, element: &Type) -> Vec<Param> {
    let types = match op {
        VecOp::WithCapacity => vec![Type::Usize],
        VecOp::Push => vec![ast_vec(element.clone()), element.clone()],
        VecOp::Len | VecOp::Capacity => vec![ast_vec(element.clone())],
        VecOp::Get => vec![ast_vec(element.clone()), Type::Usize],
    };
    types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| Param {
            name: format!("arg{index}"),
            mode: match op.param_ownership(index) {
                OwnershipMode::Own => ParamMode::Own,
                OwnershipMode::Borrow => ParamMode::Borrow,
                _ => ParamMode::Value,
            },
            ty,
            span: Span::default(),
        })
        .collect()
}
pub(crate) fn resolved_params(op: VecOp, element: &ResolvedType) -> Vec<ResolvedParam> {
    let types = match op {
        VecOp::WithCapacity => vec![ResolvedType::Usize],
        VecOp::Push => vec![resolved_vec(element.clone()), element.clone()],
        VecOp::Len | VecOp::Capacity => vec![resolved_vec(element.clone())],
        VecOp::Get => vec![resolved_vec(element.clone()), ResolvedType::Usize],
    };
    types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: format!("arg{index}"),
            ownership: op.param_ownership(index),
            ty,
            span: Span::default(),
        })
        .collect()
}

pub(crate) fn is_same_owner_push_source(value: &Expr, name: &str, ty: &Type) -> bool {
    let ExprKind::Call {
        name: callee,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    by_name(callee) == Some(VecOp::Push)
        && type_arguments.len() == 1
        && args.len() == 2
        && ast_vec(type_arguments[0].clone()) == *ty
        && matches!(&args[0].kind, ExprKind::Var(source) if source == name)
}

pub(crate) fn is_same_owner_push_hir(value: &ResolvedExpr, owner: &ValueId) -> bool {
    matches!(
        &value.kind,
        ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
            if by_id(callee.as_str()) == Some(VecOp::Push)
                && type_arguments.len() == 1
                && args.len() == 2
                && matches!(&args[0].kind, ResolvedExprKind::Place(place) if &place.root == owner && place.projections.is_empty())
    )
}
