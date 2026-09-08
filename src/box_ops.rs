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
    pub(crate) const fn param_ownership_for(self, element: &ResolvedType) -> OwnershipMode {
        if matches!(self, Self::New) && matches!(element, ResolvedType::Bytes) {
            OwnershipMode::Own
        } else {
            self.param_ownership()
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
/// The scalar carrier profile remains the only generic/wrapper admission.
/// The additive v5 profile admits Bytes only at the owning intrinsic boundary.
pub(crate) fn ast_box_element_is_admitted(ty: &Type) -> bool {
    ast_element_is_admitted(ty) || *ty == Type::Bytes
}
pub(crate) fn resolved_box_element_is_admitted(ty: &ResolvedType) -> bool {
    resolved_element_is_admitted(ty) || *ty == ResolvedType::Bytes
}
pub(crate) fn ast_operation_element_is_admitted(op: BoxOp, ty: &Type) -> bool {
    ast_element_is_admitted(ty) || (*ty == Type::Bytes && op != BoxOp::Get)
}
pub(crate) fn resolved_operation_element_is_admitted(op: BoxOp, ty: &ResolvedType) -> bool {
    resolved_element_is_admitted(ty) || (*ty == ResolvedType::Bytes && op != BoxOp::Get)
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
        mode: match if matches!(op, BoxOp::New) && *element == Type::Bytes {
            OwnershipMode::Own
        } else {
            op.param_ownership()
        } {
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
        ownership: op.param_ownership_for(element),
        ty: op.resolved_param_type(element),
        span: Span::default(),
    }]
}
pub(crate) fn is_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments } if declaration.as_str() == crate::prelude::BOX_ID && matches!(arguments.as_slice(), [element] if resolved_box_element_is_admitted(element)))
}

/// True when checked meaning needs the v5 owned-Bytes Box runtime contract.
/// An authored `Box` declaration is never reclassified by its spelling alone.
pub(crate) fn program_uses_owned_payload(program: &crate::ast::Program) -> bool {
    if program.types.iter().any(|declaration| {
        declaration.name == "Box" && declaration.stable_id != crate::prelude::BOX_ID
    }) {
        return false;
    }
    fn has_box_bytes(ty: &Type) -> bool {
        matches!(ty, Type::Named { name, arguments } if name == "Box" && matches!(arguments.as_slice(), [Type::Bytes]))
            || matches!(ty, Type::Named { arguments, .. } if arguments.iter().any(has_box_bytes))
    }
    fn function_uses(function: &crate::ast::Function) -> bool {
        function.params.iter().any(|param| has_box_bytes(&param.ty))
            || has_box_bytes(&function.return_type)
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| {
                    let mut found = false;
                    expression.visit_call_instances(&mut |name, arguments, _| {
                        found |= matches!(by_name(name), Some(BoxOp::New | BoxOp::IntoInner))
                            && matches!(arguments, [Type::Bytes]);
                    });
                    found
                })
    }
    program.functions.iter().any(function_uses)
        || program
            .types
            .iter()
            .any(|declaration| match &declaration.kind {
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                    fields.iter().any(|field| has_box_bytes(&field.ty))
                }
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .any(|field| has_box_bytes(&field.ty)),
                crate::ast::TypeDeclarationKind::Resource { .. } => false,
            })
}

/// HIR equivalent used by target selection after source verification has bound
/// `Box` to the compiler-owned declaration identity.
pub(crate) fn resolved_program_uses_owned_payload(program: &crate::hir::ResolvedProgram) -> bool {
    fn has_box_bytes(ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::BOX_ID
                && matches!(arguments.as_slice(), [ResolvedType::Bytes]))
            || matches!(ty, ResolvedType::Nominal { arguments, .. }
                if arguments.iter().any(has_box_bytes))
    }
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            function.params.iter().any(|param| has_box_bytes(&param.ty))
                || has_box_bytes(&function.return_type)
                || std::iter::once(&function.body)
                    .chain(function.requires.iter())
                    .chain(function.ensures.iter())
                    .any(|root| {
                        let mut found = false;
                        crate::hir::visit_resolved_calls(
                            root,
                            &mut |callee, instance, arguments| {
                                found |= instance.is_none()
                                    && matches!(
                                        by_id(callee.as_str()),
                                        Some(BoxOp::New | BoxOp::IntoInner)
                                    )
                                    && matches!(arguments, [ResolvedType::Bytes]);
                            },
                        );
                        found
                    })
        })
}

#[cfg(test)]
mod owned_payload_tests;
