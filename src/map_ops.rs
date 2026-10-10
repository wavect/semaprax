//! Additive typed Map and Set catalogue; v1 String Collections stays monomorphic.
use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const MAP_ID: &str = "core.collection.map.v2";
pub(crate) const SET_ID: &str = "core.collection.set.v2";
pub(crate) const DROP_ID: &str = "core.collection.drop.v2";
pub(crate) const STATUS_DOMAIN: &str = "semaprax.map.v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MapOp {
    New,
    Add,
    Set,
    Remove,
    GetOr,
    Has,
    Len,
    KeyAt,
    ValueAt,
    SetNew,
    SetInsert,
    SetRemove,
    SetHas,
    SetLen,
    SetKeyAt,
}

impl MapOp {
    pub(crate) const ALL: [Self; 15] = [
        Self::New,
        Self::Add,
        Self::Set,
        Self::Remove,
        Self::GetOr,
        Self::Has,
        Self::Len,
        Self::KeyAt,
        Self::ValueAt,
        Self::SetNew,
        Self::SetInsert,
        Self::SetRemove,
        Self::SetHas,
        Self::SetLen,
        Self::SetKeyAt,
    ];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::New => "map_new",
            Self::Add => "map_add",
            Self::Set => "map_set",
            Self::Remove => "map_remove",
            Self::GetOr => "map_get_or",
            Self::Has => "map_has",
            Self::Len => "map_len",
            Self::KeyAt => "map_key_at",
            Self::ValueAt => "map_value_at",
            Self::SetNew => "set_new",
            Self::SetInsert => "set_insert",
            Self::SetRemove => "set_remove",
            Self::SetHas => "set_has",
            Self::SetLen => "set_len",
            Self::SetKeyAt => "set_key_at",
        }
    }
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::New => "core.collection.map.new.v2",
            Self::Add => "core.collection.map.add.v2",
            Self::Set => "core.collection.map.set.v2",
            Self::Remove => "core.collection.map.remove.v2",
            Self::GetOr => "core.collection.map.get_or.v2",
            Self::Has => "core.collection.map.has.v2",
            Self::Len => "core.collection.map.len.v2",
            Self::KeyAt => "core.collection.map.key_at.v2",
            Self::ValueAt => "core.collection.map.value_at.v2",
            Self::SetNew => "core.collection.set.new.v2",
            Self::SetInsert => "core.collection.set.insert.v2",
            Self::SetRemove => "core.collection.set.remove.v2",
            Self::SetHas => "core.collection.set.has.v2",
            Self::SetLen => "core.collection.set.len.v2",
            Self::SetKeyAt => "core.collection.set.key_at.v2",
        }
    }
    pub(crate) fn is_set(self) -> bool {
        matches!(
            self,
            Self::SetNew
                | Self::SetInsert
                | Self::SetRemove
                | Self::SetHas
                | Self::SetLen
                | Self::SetKeyAt
        )
    }
    pub(crate) fn arity(self) -> usize {
        match self {
            Self::New | Self::SetNew | Self::Len | Self::SetLen => 1,
            Self::Add | Self::Set | Self::GetOr => 3,
            _ => 2,
        }
    }
    pub(crate) fn reopens(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Set | Self::Remove | Self::SetInsert | Self::SetRemove
        )
    }
    pub(crate) fn returns_collection(self) -> bool {
        self.reopens() || matches!(self, Self::New | Self::SetNew)
    }
    pub(crate) fn type_arity(self) -> usize {
        if self.is_set() {
            1
        } else {
            2
        }
    }
    pub(crate) fn ast_type(self, arguments: &[Type]) -> Option<Type> {
        if arguments.len() != self.type_arity()
            || !ast_key(&arguments[0])
            || (!self.is_set() && !ast_value(&arguments[1]))
            || (self == Self::Add && arguments[1] != Type::I64)
        {
            return None;
        }
        if !self.is_set() && arguments == [Type::String, Type::I64] {
            return Some(Type::StringMap);
        }
        Some(Type::Named {
            name: if self.is_set() { "Set" } else { "Map" }.into(),
            arguments: arguments.to_vec(),
        })
    }
    pub(crate) fn ast_signature(self, arguments: &[Type]) -> Option<(Vec<Param>, Type)> {
        let collection = self.ast_type(arguments)?;
        let key = arguments[0].clone();
        let value = if self.is_set() {
            Type::Bool
        } else {
            arguments[1].clone()
        };
        let mut params = Vec::new();
        if matches!(self, Self::New | Self::SetNew) {
            params.push(param("capacity", Type::Usize, ParamMode::Value));
        } else {
            params.push(param(
                "collection",
                collection.clone(),
                if self.reopens() {
                    ParamMode::Own
                } else {
                    ParamMode::Borrow
                },
            ));
            if !matches!(self, Self::Len | Self::SetLen) {
                if matches!(self, Self::KeyAt | Self::ValueAt | Self::SetKeyAt) {
                    params.push(param("index", Type::Usize, ParamMode::Value));
                } else {
                    params.push(param(
                        "key",
                        key.clone(),
                        if key == Type::String {
                            ParamMode::Borrow
                        } else {
                            ParamMode::Value
                        },
                    ));
                }
            }
            if matches!(self, Self::Add | Self::Set | Self::GetOr) {
                params.push(param(
                    "value",
                    value.clone(),
                    if value == Type::String {
                        ParamMode::Borrow
                    } else {
                        ParamMode::Value
                    },
                ));
            }
        }
        let result = if self.returns_collection() {
            collection
        } else if matches!(self, Self::Len | Self::SetLen) {
            Type::Usize
        } else if matches!(self, Self::Has | Self::SetHas) {
            Type::Bool
        } else if matches!(self, Self::KeyAt | Self::SetKeyAt) {
            key
        } else {
            value
        };
        Some((params, result))
    }
    pub(crate) fn resolved_arguments(self, collection: &ResolvedType) -> Option<Vec<ResolvedType>> {
        let (key, value, set) = parts(collection)?;
        if self.is_set() != set || (self == Self::Add && value != &ResolvedType::I64) {
            return None;
        }
        Some(if set {
            vec![key.clone()]
        } else {
            vec![key.clone(), value.clone()]
        })
    }
    pub(crate) fn resolved_signature(
        self,
        arguments: &[ResolvedType],
    ) -> Option<(Vec<ResolvedParam>, ResolvedType)> {
        let ast = arguments
            .iter()
            .map(resolved_scalar_ast)
            .collect::<Option<Vec<_>>>()?;
        let (params, result) = self.ast_signature(&ast)?;
        let params = params
            .into_iter()
            .enumerate()
            .map(|(index, param)| ResolvedParam {
                id: ValueId::parameter(
                    &crate::hir::FunctionExecutionId::Monomorphic(DeclarationId::new(self.id())),
                    index,
                ),
                name: param.name,
                ownership: match param.mode {
                    ParamMode::Own => OwnershipMode::Own,
                    ParamMode::Borrow => OwnershipMode::Borrow,
                    _ => OwnershipMode::Value,
                },
                ty: ast_resolved(&param.ty).expect("typed collection signature is closed"),
                span: param.span,
            })
            .collect();
        Some((params, ast_resolved(&result)?))
    }
}
fn param(name: &str, ty: Type, mode: ParamMode) -> Param {
    Param {
        name: name.into(),
        ty,
        mode,
        span: Span::default(),
    }
}
pub(crate) fn by_name(name: &str) -> Option<MapOp> {
    MapOp::ALL.into_iter().find(|op| op.name() == name)
}
pub(crate) fn by_id(id: &str) -> Option<MapOp> {
    MapOp::ALL.into_iter().find(|op| op.id() == id)
}
pub(crate) fn is_reserved_operation_id(id: &str) -> bool {
    by_id(id).is_some()
        || matches!(
            id,
            DROP_ID | "core.collection.wasm.checked.v2" | "core.collection.wasm.drop.v2"
        )
}
pub(crate) fn reserved_ids() -> [&'static str; 20] {
    let mut ids = [
        MAP_ID,
        SET_ID,
        DROP_ID,
        "core.collection.wasm.checked.v2",
        "core.collection.wasm.drop.v2",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
    ];
    for (slot, op) in ids[5..].iter_mut().zip(MapOp::ALL) {
        *slot = op.id();
    }
    ids
}
pub(crate) fn by_generic_name(name: &str, arguments: &[Type]) -> Option<MapOp> {
    by_name(name).filter(|op| op.is_set() || !arguments.is_empty())
}
pub(crate) fn ast_key(ty: &Type) -> bool {
    matches!(ty, Type::String | Type::I64 | Type::Bool)
}
pub(crate) fn ast_value(ty: &Type) -> bool {
    matches!(
        ty,
        Type::String
            | Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}
pub(crate) fn ast_collection(ty: &Type) -> bool {
    match ty {
        Type::StringMap => true,
        Type::Named { name, arguments } if name == "Map" => {
            arguments.len() == 2 && ast_key(&arguments[0]) && ast_value(&arguments[1])
        }
        Type::Named { name, arguments } if name == "Set" => {
            arguments.len() == 1 && ast_key(&arguments[0])
        }
        _ => false,
    }
}
pub(crate) fn parts(ty: &ResolvedType) -> Option<(&ResolvedType, &ResolvedType, bool)> {
    match ty {
        ResolvedType::StringMap => Some((&ResolvedType::String, &ResolvedType::I64, false)),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == MAP_ID
            && arguments.len() == 2
            && resolved_scalar_ast(&arguments[0]).is_some_and(|t| ast_key(&t))
            && resolved_scalar_ast(&arguments[1]).is_some_and(|t| ast_value(&t)) =>
        {
            Some((&arguments[0], &arguments[1], false))
        }
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == SET_ID
            && arguments.len() == 1
            && resolved_scalar_ast(&arguments[0]).is_some_and(|t| ast_key(&t)) =>
        {
            Some((&arguments[0], &ResolvedType::Bool, true))
        }
        _ => None,
    }
}
pub(crate) fn is_collection(ty: &ResolvedType) -> bool {
    parts(ty).is_some()
}
pub(crate) fn is_typed_collection(ty: &ResolvedType) -> bool {
    is_collection(ty) && ty != &ResolvedType::StringMap
}
pub(crate) fn is_declaration(id: &str) -> bool {
    matches!(id, MAP_ID | SET_ID)
}
pub(crate) fn resolved_scalar_ast(ty: &ResolvedType) -> Option<Type> {
    Some(match ty {
        ResolvedType::String => Type::String,
        ResolvedType::I64 => Type::I64,
        ResolvedType::I32 => Type::I32,
        ResolvedType::U8 => Type::U8,
        ResolvedType::Usize => Type::Usize,
        ResolvedType::Char => Type::Char,
        ResolvedType::F32 => Type::F32,
        ResolvedType::F64 => Type::F64,
        ResolvedType::Bool => Type::Bool,
        _ => return None,
    })
}
pub(crate) fn ast_resolved(ty: &Type) -> Option<ResolvedType> {
    Some(match ty {
        Type::String => ResolvedType::String,
        Type::I64 => ResolvedType::I64,
        Type::I32 => ResolvedType::I32,
        Type::U8 => ResolvedType::U8,
        Type::Usize => ResolvedType::Usize,
        Type::Char => ResolvedType::Char,
        Type::F32 => ResolvedType::F32,
        Type::F64 => ResolvedType::F64,
        Type::Bool => ResolvedType::Bool,
        Type::StringMap => ResolvedType::StringMap,
        Type::Named { name, arguments } if ast_collection(ty) => ResolvedType::Nominal {
            declaration: DeclarationId::new(if name == "Map" { MAP_ID } else { SET_ID }),
            arguments: arguments
                .iter()
                .map(ast_resolved)
                .collect::<Option<Vec<_>>>()?,
        },
        _ => return None,
    })
}
pub(crate) fn program_uses(program: &crate::ast::Program) -> bool {
    program.functions.iter().any(|function| {
        function
            .params
            .iter()
            .any(|param| ast_collection(&param.ty))
            || ast_collection(&function.return_type)
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| {
                    let mut found = false;
                    expression.visit_call_instances(&mut |name, types, _| {
                        found |= by_generic_name(name, types).is_some() || name == "map_remove"
                    });
                    found
                })
    }) || program.types.iter().any(|decl| match &decl.kind {
        crate::ast::TypeDeclarationKind::Record { fields }
        | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
            fields.iter().any(|field| ast_collection(&field.ty))
        }
        _ => false,
    })
}

pub(crate) fn resolved_function_uses(function: &crate::hir::ResolvedFunction) -> bool {
    is_typed_collection(&function.return_type)
        || function.params.iter().any(|p| is_typed_collection(&p.ty))
        || function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(|root| {
                let mut found = false;
                crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                    found |= by_id(callee.as_str()).is_some()
                });
                found
            })
}

pub(crate) fn resolved_program_uses(program: &crate::hir::ResolvedProgram) -> bool {
    program.functions.iter().any(resolved_function_uses)
        || program
            .function_instances
            .iter()
            .any(|i| resolved_function_uses(&i.function))
        || program.types.iter().any(|decl| match &decl.kind {
            crate::hir::ResolvedTypeDeclarationKind::Record { fields }
            | crate::hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                fields.iter().any(|f| is_typed_collection(&f.ty))
            }
            _ => false,
        })
}
pub(crate) fn validate_declarations(
    program: &crate::hir::ResolvedProgram,
) -> Result<(), crate::diagnostic::Diagnostic> {
    for decl in &program.types {
        if !is_declaration(decl.id.as_str()) {
            continue;
        }
        let set = decl.id.as_str() == SET_ID;
        let names: &[&str] = if set { &["K"] } else { &["K", "V"] };
        if program
            .declarations
            .declaration(&decl.id)
            .is_none_or(|d| d.identity_origin != crate::hir::IdentityOrigin::CompilerOwned)
            || decl.name != (if set { "Set" } else { "Map" })
            || decl.type_parameters.len() != names.len()
            || decl
                .type_parameters
                .iter()
                .zip(names)
                .enumerate()
                .any(|(i, (p, n))| p.name != *n || p.index != i as u32)
            || !matches!(&decl.kind,crate::hir::ResolvedTypeDeclarationKind::Record{fields} if fields.is_empty())
        {
            return Err(crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "compiler-owned collection declaration is not canonical",
            ));
        }
    }
    Ok(())
}
impl MapOp {
    pub(crate) fn legacy(self, types: &[ResolvedType]) -> Option<crate::string_ops::StringOp> {
        if types != [ResolvedType::String, ResolvedType::I64] {
            return None;
        }
        use crate::string_ops::StringOp as S;
        Some(match self {
            Self::New => S::MapNew,
            Self::Add => S::MapAdd,
            Self::Set => S::MapSet,
            Self::Remove => S::MapRemove,
            Self::GetOr => S::MapGetOr,
            Self::Has => S::MapHas,
            Self::Len => S::MapLen,
            Self::KeyAt => S::MapKeyAt,
            Self::ValueAt => S::MapValueAt,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
