//! Compiler-owned operations for the internal Owned Bounded Vec v1 profile.

use crate::ast::{Expr, ExprKind, Param, ParamMode, Span, Type};
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedParam, ResolvedType,
    ValueId,
};

mod wrappers;
pub(crate) use wrappers::*;
#[cfg(test)]
mod owned_payload_tests;

pub(crate) const WITH_CAPACITY_NAME: &str = "vec_with_capacity";
pub(crate) const PUSH_NAME: &str = "vec_push";
pub(crate) const LEN_NAME: &str = "vec_len";
pub(crate) const CAPACITY_NAME: &str = "vec_capacity";
pub(crate) const GET_NAME: &str = "vec_get";
pub(crate) const RESERVE_EXACT_NAME: &str = "vec_reserve_exact";
pub(crate) const SET_NAME: &str = "vec_set";
pub(crate) const CLEAR_NAME: &str = "vec_clear";
pub(crate) const WITH_CAPACITY_ID: &str = "core.vec.with-capacity";
pub(crate) const PUSH_ID: &str = "core.vec.push";
pub(crate) const LEN_ID: &str = "core.vec.len";
pub(crate) const CAPACITY_ID: &str = "core.vec.capacity";
pub(crate) const GET_ID: &str = "core.vec.get";
pub(crate) const RESERVE_EXACT_ID: &str = "core.vec.reserve-exact";
pub(crate) const SET_ID: &str = "core.vec.set";
pub(crate) const CLEAR_ID: &str = "core.vec.clear";
pub(crate) const MAX_CAPACITY: u64 = 8_192;
pub(crate) const OWNED_PAYLOAD_BYTES_PER_ELEMENT: u64 = 16;
pub(crate) const MAX_OWNED_PAYLOAD_BYTES: u64 = 131_072;
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
    ReserveExact,
    Set,
    Clear,
}

pub(crate) const ALL: [VecOp; 8] = [
    VecOp::WithCapacity,
    VecOp::Push,
    VecOp::Len,
    VecOp::Capacity,
    VecOp::Get,
    VecOp::ReserveExact,
    VecOp::Set,
    VecOp::Clear,
];

impl VecOp {
    pub(crate) const fn reopens_same_owner(self) -> bool {
        matches!(
            self,
            Self::Push | Self::ReserveExact | Self::Set | Self::Clear
        )
    }

    pub(crate) const fn returns_owner(self) -> bool {
        matches!(
            self,
            Self::WithCapacity | Self::Push | Self::ReserveExact | Self::Set | Self::Clear
        )
    }

    pub(crate) const fn admitted_in_while(self) -> bool {
        matches!(self, Self::Push | Self::Len | Self::Capacity | Self::Get)
    }

    pub(crate) const fn capacity_argument(self) -> Option<usize> {
        match self {
            Self::WithCapacity => Some(0),
            Self::ReserveExact => Some(1),
            _ => None,
        }
    }
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::WithCapacity => WITH_CAPACITY_NAME,
            Self::Push => PUSH_NAME,
            Self::Len => LEN_NAME,
            Self::Capacity => CAPACITY_NAME,
            Self::Get => GET_NAME,
            Self::ReserveExact => RESERVE_EXACT_NAME,
            Self::Set => SET_NAME,
            Self::Clear => CLEAR_NAME,
        }
    }
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::WithCapacity => WITH_CAPACITY_ID,
            Self::Push => PUSH_ID,
            Self::Len => LEN_ID,
            Self::Capacity => CAPACITY_ID,
            Self::Get => GET_ID,
            Self::ReserveExact => RESERVE_EXACT_ID,
            Self::Set => SET_ID,
            Self::Clear => CLEAR_ID,
        }
    }
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::WithCapacity => 1,
            Self::Push => 2,
            Self::Len | Self::Capacity => 1,
            Self::Get => 2,
            Self::ReserveExact => 2,
            Self::Set => 3,
            Self::Clear => 1,
        }
    }
    pub(crate) const fn param_ownership(self, index: usize) -> OwnershipMode {
        match (self, index) {
            (Self::Push | Self::ReserveExact | Self::Set | Self::Clear, 0) => OwnershipMode::Own,
            (Self::Len | Self::Capacity | Self::Get, 0) => OwnershipMode::Borrow,
            _ => OwnershipMode::Value,
        }
    }
    /// The element slot's ownership mode.
    ///
    /// An owned element transfers into the collection at the call's commit
    /// boundary instead of being copied. `Bytes` does, and so does the one
    /// admitted owned-record collection element (SPX-AI-019), which is the
    /// only `ResolvedType::Nominal` any admission path lets reach here:
    /// Copy scalars are primitive variants and a generic collection's element
    /// is a `ResolvedType::TypeParameter`, so neither is matched. An element
    /// outside every admitted profile is refused with a stable diagnostic in
    /// source and HIR before this function is consulted.
    pub(crate) const fn param_ownership_for(
        self,
        index: usize,
        element: &ResolvedType,
    ) -> OwnershipMode {
        if matches!(element, ResolvedType::Bytes | ResolvedType::Nominal { .. })
            && matches!((self, index), (Self::Push, 1) | (Self::Set, 2))
        {
            OwnershipMode::Own
        } else {
            self.param_ownership(index)
        }
    }
    pub(crate) fn resolved_return_type(self, element: &ResolvedType) -> ResolvedType {
        match self {
            Self::WithCapacity | Self::Push | Self::ReserveExact | Self::Set | Self::Clear => {
                resolved_vec(element.clone())
            }
            Self::Len | Self::Capacity => ResolvedType::Usize,
            Self::Get => element.clone(),
        }
    }
    pub(crate) fn ast_return_type(self, element: &Type) -> Type {
        match self {
            Self::WithCapacity | Self::Push | Self::ReserveExact | Self::Set | Self::Clear => {
                ast_vec(element.clone())
            }
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
            (Self::ReserveExact, 0) | (Self::Set, 0) | (Self::Clear, 0) => {
                *ty == resolved_vec(element.clone())
            }
            (Self::ReserveExact, 1) | (Self::Set, 1) => *ty == ResolvedType::Usize,
            (Self::Set, 2) => ty == element,
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
        RESERVE_EXACT_NAME => Some(VecOp::ReserveExact),
        SET_NAME => Some(VecOp::Set),
        CLEAR_NAME => Some(VecOp::Clear),
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
        RESERVE_EXACT_ID => Some(VecOp::ReserveExact),
        SET_ID => Some(VecOp::Set),
        CLEAR_ID => Some(VecOp::Clear),
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
/// Scalar admission remains frozen for generic wrappers. Bytes is admitted only
/// by the owning intrinsic Vec boundary.
pub(crate) fn ast_vec_element_is_admitted(ty: &Type) -> bool {
    ast_element_is_admitted(ty) || *ty == Type::Bytes
}
pub(crate) fn resolved_vec_element_is_admitted(ty: &ResolvedType) -> bool {
    resolved_element_is_admitted(ty) || *ty == ResolvedType::Bytes
}
pub(crate) fn ast_operation_element_is_admitted(op: VecOp, ty: &Type) -> bool {
    ast_element_is_admitted(ty) || (*ty == Type::Bytes && op != VecOp::Get)
}
pub(crate) fn resolved_operation_element_is_admitted(op: VecOp, ty: &ResolvedType) -> bool {
    resolved_element_is_admitted(ty) || (*ty == ResolvedType::Bytes && op != VecOp::Get)
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
    ast_params_with_owned_element(op, element, *element == Type::Bytes)
}

/// `ast_params`, with the element slot's transfer mode supplied by the caller.
///
/// The source verifier cannot decide from the spelling alone whether a
/// `Type::Named` element is the admitted owned-record collection element or a
/// generic wrapper's type parameter — both are `Named` with no arguments — so
/// the classifier's answer is threaded in rather than guessed here. Passing
/// `false` reproduces the pre-SPX-AI-019 Copy-element signature exactly.
pub(crate) fn ast_params_with_owned_element(
    op: VecOp,
    element: &Type,
    owned_element: bool,
) -> Vec<Param> {
    let types = match op {
        VecOp::WithCapacity => vec![Type::Usize],
        VecOp::Push => vec![ast_vec(element.clone()), element.clone()],
        VecOp::Len | VecOp::Capacity => vec![ast_vec(element.clone())],
        VecOp::Get => vec![ast_vec(element.clone()), Type::Usize],
        VecOp::ReserveExact => vec![ast_vec(element.clone()), Type::Usize],
        VecOp::Set => vec![ast_vec(element.clone()), Type::Usize, element.clone()],
        VecOp::Clear => vec![ast_vec(element.clone())],
    };
    types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| Param {
            name: format!("arg{index}"),
            mode: match if owned_element
                && matches!((op, index), (VecOp::Push, 1) | (VecOp::Set, 2))
            {
                OwnershipMode::Own
            } else {
                op.param_ownership(index)
            } {
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
        VecOp::ReserveExact => vec![resolved_vec(element.clone()), ResolvedType::Usize],
        VecOp::Set => vec![
            resolved_vec(element.clone()),
            ResolvedType::Usize,
            element.clone(),
        ],
        VecOp::Clear => vec![resolved_vec(element.clone())],
    };
    types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: format!("arg{index}"),
            ownership: op.param_ownership_for(index, element),
            ty,
            span: Span::default(),
        })
        .collect()
}

/// True when the source requests the owning Bytes Vec profile. An authored
/// `Vec` declaration remains ordinary source meaning.
pub(crate) fn program_uses_owned_payload(program: &crate::ast::Program) -> bool {
    if program.types.iter().any(|declaration| {
        declaration.name == "Vec" && declaration.stable_id != crate::prelude::VEC_ID
    }) {
        return false;
    }
    fn has_vec_bytes(ty: &Type) -> bool {
        matches!(ty, Type::Named { name, arguments }
            if name == "Vec" && matches!(arguments.as_slice(), [Type::Bytes]))
            || matches!(ty, Type::Named { arguments, .. } if arguments.iter().any(has_vec_bytes))
    }
    let function_uses = |function: &crate::ast::Function| {
        function.params.iter().any(|param| has_vec_bytes(&param.ty))
            || has_vec_bytes(&function.return_type)
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expr| {
                    let mut found = false;
                    expr.visit_call_instances(&mut |name, arguments, _| {
                        found |= matches!(by_name(name), Some(op) if op != VecOp::Get)
                            && matches!(arguments, [Type::Bytes]);
                    });
                    found
                })
    };
    program.functions.iter().any(function_uses)
}

pub(crate) fn resolved_program_uses_owned_payload(program: &crate::hir::ResolvedProgram) -> bool {
    if crate::iterator_ops::resolved_program_uses_owned_iterator(program) {
        return true;
    }
    fn has_vec_bytes(ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::VEC_ID
                && matches!(arguments.as_slice(), [ResolvedType::Bytes]))
            || matches!(ty, ResolvedType::Nominal { arguments, .. }
                if arguments.iter().any(has_vec_bytes))
    }
    let function_uses = |function: &crate::hir::ResolvedFunction| {
        function.params.iter().any(|param| has_vec_bytes(&param.ty))
            || has_vec_bytes(&function.return_type)
            || std::iter::once(&function.body)
                .chain(function.requires.iter())
                .chain(function.ensures.iter())
                .any(|root| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(root, &mut |callee, instance, arguments| {
                        found |= instance.is_none()
                            && matches!(by_id(callee.as_str()), Some(op) if op != VecOp::Get)
                            && matches!(arguments, [ResolvedType::Bytes]);
                    });
                    found
                })
    };
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(function_uses)
}

pub(crate) fn is_same_owner_reassignment_source(
    program: &crate::ast::Program,
    value: &Expr,
    name: &str,
    ty: &Type,
) -> bool {
    let ExprKind::Call {
        name: callee,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    let direct = by_name(callee).filter(|op| op.reopens_same_owner());
    let wrapper = program
        .functions
        .iter()
        .find(|function| function.name == *callee)
        .and_then(|function| source_wrapper(program, function))
        .filter(|op| op.reopens_same_owner());
    let op = direct.or(wrapper);
    let owner_type_matches = match type_arguments.as_slice() {
        [element] => ast_vec(element.clone()) == *ty,
        [] if direct.is_none() && wrapper.is_some() => matches!(
            ty,
            Type::Named { name, arguments }
                if name == "Vec"
                    && matches!(arguments.as_slice(), [element] if ast_element_is_admitted(element))
        ),
        _ => false,
    };
    op.is_some_and(|op| args.len() == op.arity())
        && owner_type_matches
        && matches!(&args[0].kind, ExprKind::Var(source) if source == name)
}

pub(crate) fn is_same_owner_reassignment_hir(
    program: &crate::hir::ResolvedProgram,
    value: &ResolvedExpr,
    owner: &ValueId,
) -> bool {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &value.kind
    else {
        return false;
    };
    let op = if instance.is_none() {
        by_id(callee.as_str()).filter(|op| op.reopens_same_owner())
    } else {
        instance.as_ref().and_then(|instance| {
            (crate::hir::FunctionInstanceId::derive(callee, type_arguments) == *instance)
                .then(|| {
                    program
                        .function_templates
                        .iter()
                        .find(|template| template.id == *callee)
                        .and_then(|template| hir_wrapper_in_program(program, template))
                        .filter(|op| op.reopens_same_owner())
                })
                .flatten()
        })
    };
    op.is_some_and(|op| args.len() == op.arity())
        && matches!(type_arguments.as_slice(), [argument] if resolved_vec_element_is_admitted(argument))
        && matches!(&args[0].kind, ResolvedExprKind::Place(place)
            if &place.root == owner && place.projections.is_empty())
}

pub(crate) fn is_same_owner_reassignment_hir_source(
    program: &crate::ast::Program,
    value: &ResolvedExpr,
    owner: &ValueId,
) -> bool {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &value.kind
    else {
        return false;
    };
    let op = if instance.is_none() {
        by_id(callee.as_str()).filter(|op| op.reopens_same_owner())
    } else {
        instance.as_ref().and_then(|instance| {
            (crate::hir::FunctionInstanceId::derive(callee, type_arguments) == *instance)
                .then(|| {
                    program
                        .functions
                        .iter()
                        .find(|function| function.stable_id == callee.as_str())
                        .and_then(|function| source_wrapper(program, function))
                        .filter(|op| op.reopens_same_owner())
                })
                .flatten()
        })
    };
    op.is_some_and(|op| args.len() == op.arity())
        && matches!(type_arguments.as_slice(), [argument] if resolved_vec_element_is_admitted(argument))
        && matches!(&args[0].kind, ResolvedExprKind::Place(place)
            if &place.root == owner && place.projections.is_empty())
}
