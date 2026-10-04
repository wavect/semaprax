//! Exact compiler-owned immutable `List<i64>` constructor and destructor lane.
//! The source type is algebraic even though its physical carrier is opaque.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LIST_ID: &str = "core.list";
pub(crate) const STEP_ID: &str = "core.list-step";
pub(crate) const NIL_CASE_ID: &str = "core.list-step.nil";
pub(crate) const CONS_CASE_ID: &str = "core.list-step.cons";
pub(crate) const HEAD_ID: &str = "core.list-step.cons.head";
pub(crate) const TAIL_ID: &str = "core.list-step.cons.tail";
pub(crate) const NIL_ID: &str = "core.list.nil";
pub(crate) const CONS_ID: &str = "core.list.cons";
pub(crate) const UNCONS_ID: &str = "core.list.uncons";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ListOp {
    Nil,
    Cons,
    Uncons,
}

pub(crate) const ALL: [ListOp; 3] = [ListOp::Nil, ListOp::Cons, ListOp::Uncons];

impl ListOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Nil => "list_nil",
            Self::Cons => "list_cons",
            Self::Uncons => "list_uncons",
        }
    }

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Nil => NIL_ID,
            Self::Cons => CONS_ID,
            Self::Uncons => UNCONS_ID,
        }
    }

    pub(crate) const fn argument_count(self) -> usize {
        match self {
            Self::Nil => 0,
            Self::Cons => 2,
            Self::Uncons => 1,
        }
    }

    pub(crate) fn ast_params(self) -> Vec<Param> {
        let list = ast_list();
        let make = |name: &str, mode, ty| Param {
            name: name.into(),
            mode,
            ty,
            span: Span::default(),
        };
        match self {
            Self::Nil => vec![],
            Self::Cons => vec![
                make("head", ParamMode::Value, Type::I64),
                make("tail", ParamMode::Value, list),
            ],
            Self::Uncons => vec![make("list", ParamMode::Value, list)],
        }
    }

    pub(crate) fn resolved_params(self) -> Vec<ResolvedParam> {
        self.ast_params()
            .into_iter()
            .enumerate()
            .map(|(index, param)| ResolvedParam {
                id: ValueId::intrinsic_parameter(self.id(), index),
                name: param.name,
                ownership: OwnershipMode::Value,
                ty: if self == Self::Cons && index == 0 {
                    ResolvedType::I64
                } else {
                    resolved_list()
                },
                span: Span::default(),
            })
            .collect()
    }

    pub(crate) fn ast_return_type(self) -> Type {
        if self == Self::Uncons {
            ast_step()
        } else {
            ast_list()
        }
    }

    pub(crate) fn resolved_return_type(self) -> ResolvedType {
        if self == Self::Uncons {
            resolved_step()
        } else {
            resolved_list()
        }
    }
}

pub(crate) fn by_name(name: &str) -> Option<ListOp> {
    ALL.into_iter().find(|op| op.name() == name)
}

pub(crate) fn by_id(id: &str) -> Option<ListOp> {
    ALL.into_iter().find(|op| op.id() == id)
}

pub(crate) fn ast_list() -> Type {
    Type::Named {
        name: "List".into(),
        arguments: vec![Type::I64],
    }
}

pub(crate) fn ast_step() -> Type {
    Type::Named {
        name: "ListStep".into(),
        arguments: vec![Type::I64],
    }
}

pub(crate) fn resolved_list() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(LIST_ID),
        arguments: vec![ResolvedType::I64],
    }
}

pub(crate) fn resolved_step() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(STEP_ID),
        arguments: vec![ResolvedType::I64],
    }
}

pub(crate) fn is_list(ty: &ResolvedType) -> bool {
    *ty == resolved_list()
}

pub(crate) fn is_step(ty: &ResolvedType) -> bool {
    *ty == resolved_step()
}

pub(crate) fn step_shape(index: &crate::hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    if !is_step(ty) {
        return false;
    }
    let owner = DeclarationId::new(STEP_ID);
    let Some(cases) = index.variant_cases(&owner) else {
        return false;
    };
    let parameter = ResolvedType::TypeParameter {
        owner: owner.clone(),
        index: 0,
    };
    matches!(cases, [nil, cons]
        if nil.id.as_str() == NIL_CASE_ID && nil.name == "Nil" && nil.index == 0 && nil.fields.is_empty()
        && cons.id.as_str() == CONS_CASE_ID && cons.name == "Cons" && cons.index == 1
        && matches!(cons.fields.as_slice(), [head, tail]
            if head.id.as_str() == HEAD_ID && head.name == "head" && head.index == 0 && head.ty == parameter
            && tail.id.as_str() == TAIL_ID && tail.name == "tail" && tail.index == 1
            && tail.ty == ResolvedType::Nominal { declaration: DeclarationId::new(LIST_ID), arguments: vec![parameter] }))
}

pub(crate) fn validate_declarations(
    program: &crate::hir::ResolvedProgram,
) -> Result<(), crate::diagnostic::Diagnostic> {
    use crate::hir::DeclarationKind;
    for id in [LIST_ID, STEP_ID] {
        let key = DeclarationId::new(id);
        let Some(declaration) = program.declarations.declaration(&key) else {
            continue;
        };
        let valid = declaration.identity_origin == crate::hir::IdentityOrigin::CompilerOwned
            && program
                .declarations
                .type_parameters(&key)
                .is_some_and(|parameters| parameters.len() == 1)
            && if id == LIST_ID {
                declaration.kind == DeclarationKind::Record
                    && declaration.name == "List"
                    && program
                        .declarations
                        .record_fields(&key)
                        .is_some_and(|fields| fields.is_empty())
            } else {
                declaration.kind == DeclarationKind::Variant
                    && declaration.name == "ListStep"
                    && step_shape(&program.declarations, &resolved_step())
            };
        if !valid {
            return Err(crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "immutable list prelude declaration is not canonical",
            ));
        }
    }
    Ok(())
}

pub(crate) fn is_step_tail_field(
    owner: &DeclarationId,
    case: &DeclarationId,
    field: &crate::hir::ResolvedFieldDeclaration,
) -> bool {
    owner.as_str() == STEP_ID
        && case.as_str() == CONS_CASE_ID
        && field.id.as_str() == TAIL_ID
        && field.index == 1
        && field.name == "tail"
        && field.ty
            == ResolvedType::Nominal {
                declaration: DeclarationId::new(LIST_ID),
                arguments: vec![ResolvedType::TypeParameter {
                    owner: owner.clone(),
                    index: 0,
                }],
            }
}

pub(crate) fn ast_is_list(ty: &Type) -> bool {
    *ty == ast_list() || *ty == ast_step()
}

pub(crate) fn program_uses_list(program: &crate::ast::Program) -> bool {
    fn type_uses(ty: &Type) -> bool {
        match ty {
            Type::Named { name, arguments } => {
                matches!(name.as_str(), "List" | "ListStep") || arguments.iter().any(type_uses)
            }
            Type::Function { parameters, result } => {
                parameters.iter().any(type_uses) || type_uses(result)
            }
            _ => false,
        }
    }
    fn function_uses(function: &crate::ast::Function) -> bool {
        type_uses(&function.return_type)
            || function.params.iter().any(|param| type_uses(&param.ty))
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| {
                    let mut found = false;
                    expression.visit_calls(&mut |name, _| found |= by_name(name).is_some());
                    found
                })
    }
    program.functions.iter().any(function_uses)
        || program.types.iter().any(|declaration| {
            matches!(&declaration.kind, crate::ast::TypeDeclarationKind::Class { methods, .. }
                if methods.iter().any(function_uses))
        })
}

pub(crate) fn resolved_program_uses_list(program: &crate::hir::ResolvedProgram) -> bool {
    let id = DeclarationId::new(LIST_ID);
    program.types.iter().any(|declaration| declaration.id == id)
        && program
            .declarations
            .declaration(&id)
            .is_some_and(|indexed| {
                indexed.identity_origin == crate::hir::IdentityOrigin::CompilerOwned
            })
}
