//! Resolved high-level intermediate representation.
//!
//! The parsed AST keeps the names humans wrote. HIR replaces every nominal,
//! callable, and value reference with a deterministic identity. Backends should
//! consume this layer as the language grows rather than repeating name lookup.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;

use crate::ast::{
    BinaryOp, Expr, ExprKind, ImportFailure, MatchPattern, ParamMode, Program,
    ResourceLifecycleKind, Span, Statement, Type, TypeDeclarationKind, UnaryOp,
};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;
use crate::source_verify;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeclarationId(String);

impl DeclarationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionInstanceId(String);

impl FunctionInstanceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionExecutionId {
    Monomorphic(DeclarationId),
    Generic(FunctionInstanceId),
}

impl FunctionExecutionId {
    fn diagnostic_text(&self) -> &str {
        match self {
            Self::Monomorphic(id) => id.as_str(),
            Self::Generic(id) => id.as_str(),
        }
    }

    pub fn identity_key(&self) -> String {
        match self {
            Self::Monomorphic(declaration) => format!(
                "semaprax.function-execution.v1:monomorphic:{}:{}",
                declaration.as_str().len(),
                declaration
            ),
            Self::Generic(instance) => format!(
                "semaprax.function-execution.v1:generic:{}:{}",
                instance.as_str().len(),
                instance
            ),
        }
    }

    pub fn instance(&self) -> Option<&FunctionInstanceId> {
        match self {
            Self::Monomorphic(_) => None,
            Self::Generic(instance) => Some(instance),
        }
    }

    pub fn monomorphic_declaration(&self) -> Option<&DeclarationId> {
        match self {
            Self::Monomorphic(declaration) => Some(declaration),
            Self::Generic(_) => None,
        }
    }
}

impl fmt::Display for FunctionExecutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.diagnostic_text())
    }
}

impl fmt::Display for FunctionInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(String);

impl ValueId {
    fn parameter(function: &FunctionExecutionId, index: usize) -> Self {
        Self(scoped_identity(function, "value:param", &index.to_string()))
    }

    fn local(function: &FunctionExecutionId, path: &str) -> Self {
        Self(scoped_identity(function, "value:local", path))
    }

    fn result(function: &FunctionExecutionId) -> Self {
        Self(scoped_identity(function, "value:result", ""))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExpressionId(String);

impl ExpressionId {
    fn new(function: &FunctionExecutionId, path: &str) -> Self {
        Self(scoped_identity(function, "expression", path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn scoped_identity(owner: &FunctionExecutionId, kind: &str, path: &str) -> String {
    match owner {
        FunctionExecutionId::Monomorphic(owner) => format!(
            "declaration:{}:{}:{kind}:{}:{path}",
            owner.as_str().len(),
            owner,
            path.len()
        ),
        FunctionExecutionId::Generic(_) => {
            let owner = owner.identity_key();
            format!(
                "function-execution:{}:{}:{kind}:{}:{path}",
                owner.len(),
                owner,
                path.len()
            )
        }
    }
}

impl fmt::Display for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Resource,
    ResourceDrop,
    Record,
    Field,
    Variant,
    VariantCase,
    CaseField,
    Interface,
    Import,
    Function,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityOrigin {
    Explicit,
    Automatic,
    CompilerOwned,
}

impl IdentityOrigin {
    pub fn text(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Automatic => "automatic",
            Self::CompilerOwned => "compiler_owned",
        }
    }

    pub fn is_persistent(self) -> bool {
        matches!(self, Self::Explicit | Self::CompilerOwned)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: DeclarationId,
    pub name: String,
    pub kind: DeclarationKind,
    pub identity_origin: IdentityOrigin,
    pub owner: Option<DeclarationId>,
}

/// A deterministic, display-name-to-identity index.
///
/// Types and values occupy distinct namespaces so future record/variant type
/// declarations can coexist with functions without ambiguous lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    declarations: BTreeMap<DeclarationId, Declaration>,
    types_by_name: BTreeMap<String, DeclarationId>,
    functions_by_name: BTreeMap<String, DeclarationId>,
    fields_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    record_fields: BTreeMap<DeclarationId, Vec<ResolvedFieldDeclaration>>,
    cases_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    variant_cases: BTreeMap<DeclarationId, Vec<ResolvedVariantCaseDeclaration>>,
    case_fields: BTreeMap<DeclarationId, Vec<ResolvedFieldDeclaration>>,
    type_parameters: BTreeMap<DeclarationId, Vec<ResolvedTypeParameterDeclaration>>,
    imports_by_key: BTreeMap<String, DeclarationId>,
    type_facts_by_id: BTreeMap<String, TypeFacts>,
}

impl DeclarationIndex {
    pub fn declaration(&self, id: &DeclarationId) -> Option<&Declaration> {
        self.declarations.get(id)
    }

    pub fn type_id(&self, name: &str) -> Option<&DeclarationId> {
        self.types_by_name.get(name)
    }

    pub fn function_id(&self, name: &str) -> Option<&DeclarationId> {
        self.functions_by_name.get(name)
    }

    pub fn field_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.fields_by_owner_name
            .get(&(owner.clone(), name.to_owned()))
    }

    pub fn record_fields(&self, owner: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.record_fields.get(owner).map(Vec::as_slice)
    }

    pub fn case_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.cases_by_owner_name
            .get(&(owner.clone(), name.to_owned()))
    }

    pub fn variant_cases(
        &self,
        owner: &DeclarationId,
    ) -> Option<&[ResolvedVariantCaseDeclaration]> {
        self.variant_cases.get(owner).map(Vec::as_slice)
    }

    pub fn case_fields(&self, case: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.case_fields.get(case).map(Vec::as_slice)
    }

    pub fn type_parameters(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&[ResolvedTypeParameterDeclaration]> {
        self.type_parameters.get(declaration).map(Vec::as_slice)
    }

    pub fn import_id(&self, key: &str) -> Option<&DeclarationId> {
        self.imports_by_key.get(key)
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &Declaration> {
        self.declarations.values()
    }

    /// Computes the recursive semantic facts shared by ownership and backends.
    ///
    /// `None` is reserved for unresolved type parameters and future malformed
    /// HIR. Every type produced for today's verified language has facts.
    pub fn type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.type_facts_by_id
            .get(&ty.identity_key())
            .cloned()
            .or_else(|| self.recompute_type_facts(ty))
    }

    fn compute_type_facts(
        &self,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        memo: &mut BTreeMap<String, TypeFacts>,
    ) -> Option<TypeFacts> {
        let identity = ty.identity_key();
        if let Some(facts) = memo.get(&identity) {
            return Some(facts.clone());
        }
        match ty {
            ResolvedType::I64 => Some(TypeFacts {
                copy: true,
                contains_resource: false,
                sized: true,
                needs_drop: false,
                layout_key: "scalar:i64".to_owned(),
            }),
            ResolvedType::Bool => Some(TypeFacts {
                copy: true,
                contains_resource: false,
                sized: true,
                needs_drop: false,
                layout_key: "scalar:bool".to_owned(),
            }),
            ResolvedType::TypeParameter { .. } => None,
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                let declaration = self.declaration(declaration)?;
                let facts = match declaration.kind {
                    DeclarationKind::Resource if arguments.is_empty() => Some(TypeFacts {
                        copy: false,
                        contains_resource: true,
                        sized: true,
                        needs_drop: true,
                        layout_key: format!("resource:{}", ty.identity_key()),
                    }),
                    DeclarationKind::Record => {
                        let parameters = self.type_parameters.get(&declaration.id)?;
                        if arguments.len() != parameters.len()
                            || arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            })
                            || !visiting.insert(declaration.id.clone())
                        {
                            return None;
                        }
                        let fields = self.record_fields.get(&declaration.id)?;
                        let mut copy = true;
                        let mut contains_resource = false;
                        let mut sized = true;
                        let mut needs_drop = false;
                        let mut encoded_fields = String::new();
                        for field in fields {
                            let field_ty =
                                substitute_type(&field.ty, &declaration.id, arguments).ok()?;
                            let facts = self.compute_type_facts(&field_ty, visiting, memo)?;
                            copy &= facts.copy;
                            contains_resource |= facts.contains_resource;
                            sized &= facts.sized;
                            needs_drop |= facts.needs_drop;
                            write!(
                                encoded_fields,
                                "{}:{}:{}:{}",
                                field.id.as_str().len(),
                                field.id,
                                facts.layout_key.len(),
                                facts.layout_key
                            )
                            .expect("writing to a string cannot fail");
                        }
                        visiting.remove(&declaration.id);
                        Some(TypeFacts {
                            copy,
                            contains_resource,
                            sized,
                            needs_drop,
                            layout_key: format!(
                                "record:{}:{}:{}:{}",
                                declaration.id.as_str().len(),
                                declaration.id,
                                fields.len(),
                                encoded_fields
                            ),
                        })
                    }
                    DeclarationKind::Variant => {
                        let parameters = self.type_parameters.get(&declaration.id)?;
                        if arguments.len() != parameters.len()
                            || arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            })
                            || !visiting.insert(declaration.id.clone())
                        {
                            return None;
                        }
                        let cases = self.variant_cases.get(&declaration.id)?;
                        let mut encoded_cases = String::new();
                        for case in cases {
                            write!(
                                encoded_cases,
                                "{}:{}:{}:",
                                case.id.as_str().len(),
                                case.id,
                                case.fields.len()
                            )
                            .expect("writing to a string cannot fail");
                            for field in &case.fields {
                                let field_ty =
                                    substitute_type(&field.ty, &declaration.id, arguments).ok()?;
                                let facts = self.compute_type_facts(&field_ty, visiting, memo)?;
                                if !facts.copy || facts.contains_resource || facts.needs_drop {
                                    return None;
                                }
                                write!(
                                    encoded_cases,
                                    "{}:{}:{}:{}",
                                    field.id.as_str().len(),
                                    field.id,
                                    facts.layout_key.len(),
                                    facts.layout_key
                                )
                                .expect("writing to a string cannot fail");
                            }
                        }
                        visiting.remove(&declaration.id);
                        Some(TypeFacts {
                            copy: true,
                            contains_resource: false,
                            sized: true,
                            needs_drop: false,
                            layout_key: format!(
                                "variant:{}:{}:{}:{}",
                                declaration.id.as_str().len(),
                                declaration.id,
                                cases.len(),
                                encoded_cases
                            ),
                        })
                    }
                    DeclarationKind::Resource
                    | DeclarationKind::ResourceDrop
                    | DeclarationKind::Field
                    | DeclarationKind::VariantCase
                    | DeclarationKind::CaseField
                    | DeclarationKind::Interface
                    | DeclarationKind::Import
                    | DeclarationKind::Function => None,
                }?;
                memo.insert(identity, facts.clone());
                Some(facts)
            }
        }
    }

    fn populate_type_facts(&mut self) -> bool {
        let mut memo = BTreeMap::new();
        for ty in [ResolvedType::I64, ResolvedType::Bool] {
            let Some(facts) = self.compute_type_facts(&ty, &mut BTreeSet::new(), &mut memo) else {
                return false;
            };
            memo.insert(ty.identity_key(), facts);
        }
        let declarations = self.types_by_name.values().cloned().collect::<Vec<_>>();
        for declaration in declarations {
            if self
                .type_parameters
                .get(&declaration)
                .is_some_and(|parameters| !parameters.is_empty())
            {
                continue;
            }
            let ty = ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            };
            if self
                .compute_type_facts(&ty, &mut BTreeSet::new(), &mut memo)
                .is_none()
            {
                return false;
            }
        }
        self.type_facts_by_id = memo;
        true
    }

    fn recompute_type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.compute_type_facts(ty, &mut BTreeSet::new(), &mut BTreeMap::new())
    }

    fn from_verified(program: &Program) -> Result<Self, Diagnostic> {
        let mut index = Self::default();
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                TypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            index.insert_top_level(
                declaration.name.clone(),
                DeclarationId::new(declaration.stable_id.clone()),
                kind,
                if crate::prelude::is_compiler_owned_id(&declaration.stable_id) {
                    IdentityOrigin::CompilerOwned
                } else if declaration.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let parameters = declaration
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("type `{}` has too many parameters", declaration.name),
                                declaration.span,
                            )
                            .at_path(&program.path)
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        for interface in &program.interfaces {
            let interface_id = DeclarationId::new(interface.stable_id.clone());
            index.insert_top_level(
                interface.name.clone(),
                interface_id.clone(),
                DeclarationKind::Interface,
                IdentityOrigin::Explicit,
            );
            for import in &interface.imports {
                let import_id = DeclarationId::new(import.stable_id.clone());
                index
                    .imports_by_key
                    .insert(import.stable_id.clone(), import_id.clone());
                index.insert_owned_declaration(
                    interface_id.clone(),
                    import.name.clone(),
                    import_id,
                    DeclarationKind::Import,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
                continue;
            };
            if let [lifecycle] = lifecycles.as_slice() {
                let lifecycle_id = lifecycle
                    .stable_id
                    .as_ref()
                    .expect("verified lifecycle has an explicit identity");
                index.insert_owned_declaration(
                    DeclarationId::new(declaration.stable_id.clone()),
                    "drop".to_owned(),
                    DeclarationId::new(lifecycle_id.clone()),
                    DeclarationKind::ResourceDrop,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for function in &program.functions {
            let owner = DeclarationId::new(function.stable_id.clone());
            index.insert_top_level(
                function.name.clone(),
                owner.clone(),
                DeclarationKind::Function,
                if function.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let parameters = function
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("function `{}` has too many parameters", function.name),
                                function.span,
                            )
                            .at_path(&program.path)
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Record { fields } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_fields = Vec::with_capacity(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = index
                    .resolve_source_type(&field.ty, Some(&owner))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "SPX-H001",
                            format!("unresolved field type `{}`", field.ty),
                            field.span,
                        )
                        .at_path(&program.path)
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("record `{}` has too many fields", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                let resolved = ResolvedFieldDeclaration {
                    id: DeclarationId::new(field.stable_id.clone()),
                    name: field.name.clone(),
                    index: field_index,
                    ty,
                    span: field.span,
                };
                index.insert_field(
                    owner.clone(),
                    resolved.clone(),
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                resolved_fields.push(resolved);
            }
            index.record_fields.insert(owner, resolved_fields);
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_cases = Vec::with_capacity(cases.len());
            for (case_ordinal, case) in cases.iter().enumerate() {
                let case_id = DeclarationId::new(case.stable_id.clone());
                let case_index = u32::try_from(case_ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("variant `{}` has too many cases", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                index.insert_case(
                    owner.clone(),
                    case.name.clone(),
                    case_id.clone(),
                    if crate::prelude::is_compiler_owned_id(&case.stable_id) {
                        IdentityOrigin::CompilerOwned
                    } else if case.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                let mut resolved_fields = Vec::with_capacity(case.fields.len());
                for (field_ordinal, field) in case.fields.iter().enumerate() {
                    let ty = index
                        .resolve_source_type(&field.ty, Some(&owner))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "SPX-H001",
                                format!("unresolved case field type `{}`", field.ty),
                                field.span,
                            )
                            .at_path(&program.path)
                        })?;
                    let field_index = u32::try_from(field_ordinal).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!(
                                "variant case `{}::{}` has too many fields",
                                declaration.name, case.name
                            ),
                            case.span,
                        )
                        .at_path(&program.path)
                    })?;
                    let resolved = ResolvedFieldDeclaration {
                        id: DeclarationId::new(field.stable_id.clone()),
                        name: field.name.clone(),
                        index: field_index,
                        ty,
                        span: field.span,
                    };
                    index.insert_case_field(
                        case_id.clone(),
                        resolved.clone(),
                        if crate::prelude::is_compiler_owned_id(&field.stable_id) {
                            IdentityOrigin::CompilerOwned
                        } else if field.explicit_id {
                            IdentityOrigin::Explicit
                        } else {
                            IdentityOrigin::Automatic
                        },
                    );
                    resolved_fields.push(resolved);
                }
                index
                    .case_fields
                    .insert(case_id.clone(), resolved_fields.clone());
                resolved_cases.push(ResolvedVariantCaseDeclaration {
                    id: case_id,
                    name: case.name.clone(),
                    index: case_index,
                    fields: resolved_fields,
                    span: case.span,
                });
            }
            index.variant_cases.insert(owner, resolved_cases);
        }
        if !index.populate_type_facts() {
            return Err(Diagnostic::error(
                "SPX-T217",
                "record declarations contain an illegal by-value recursive layout",
                Span::default(),
            )
            .at_path(&program.path));
        }
        Ok(index)
    }

    fn insert_top_level(
        &mut self,
        name: String,
        id: DeclarationId,
        kind: DeclarationKind,
        identity_origin: IdentityOrigin,
    ) {
        match kind {
            DeclarationKind::Resource | DeclarationKind::Record | DeclarationKind::Variant => {
                self.types_by_name.insert(name.clone(), id.clone());
            }
            DeclarationKind::Function => {
                self.functions_by_name.insert(name.clone(), id.clone());
            }
            DeclarationKind::Interface => {}
            DeclarationKind::ResourceDrop
            | DeclarationKind::Import
            | DeclarationKind::Field
            | DeclarationKind::VariantCase
            | DeclarationKind::CaseField => {
                unreachable!("owned declarations use owner-scoped insertion")
            }
        }
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind,
                identity_origin,
                owner: None,
            },
        );
    }

    fn insert_owned_declaration(
        &mut self,
        owner: DeclarationId,
        name: String,
        id: DeclarationId,
        kind: DeclarationKind,
        identity_origin: IdentityOrigin,
    ) {
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_field(
        &mut self,
        owner: DeclarationId,
        field: ResolvedFieldDeclaration,
        identity_origin: IdentityOrigin,
    ) {
        self.fields_by_owner_name
            .insert((owner.clone(), field.name.clone()), field.id.clone());
        self.declarations.insert(
            field.id.clone(),
            Declaration {
                id: field.id,
                name: field.name,
                kind: DeclarationKind::Field,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_case(
        &mut self,
        owner: DeclarationId,
        name: String,
        id: DeclarationId,
        identity_origin: IdentityOrigin,
    ) {
        self.cases_by_owner_name
            .insert((owner.clone(), name.clone()), id.clone());
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind: DeclarationKind::VariantCase,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_case_field(
        &mut self,
        owner: DeclarationId,
        field: ResolvedFieldDeclaration,
        identity_origin: IdentityOrigin,
    ) {
        self.fields_by_owner_name
            .insert((owner.clone(), field.name.clone()), field.id.clone());
        self.declarations.insert(
            field.id.clone(),
            Declaration {
                id: field.id,
                name: field.name,
                kind: DeclarationKind::CaseField,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn resolve_source_type(
        &self,
        ty: &Type,
        parameter_owner: Option<&DeclarationId>,
    ) -> Option<ResolvedType> {
        match ty {
            Type::I64 => Some(ResolvedType::I64),
            Type::Bool => Some(ResolvedType::Bool),
            Type::Named { name, arguments } => {
                if arguments.is_empty() {
                    if let Some(owner) = parameter_owner {
                        if let Some(parameter) = self
                            .type_parameters(owner)?
                            .iter()
                            .find(|parameter| parameter.name == *name)
                        {
                            return Some(ResolvedType::TypeParameter {
                                owner: owner.clone(),
                                index: parameter.index,
                            });
                        }
                    }
                }
                let declaration = self.type_id(name)?.clone();
                Some(ResolvedType::Nominal {
                    declaration,
                    arguments: arguments
                        .iter()
                        .map(|argument| self.resolve_source_type(argument, parameter_owner))
                        .collect::<Option<Vec<_>>>()?,
                })
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedType {
    I64,
    Bool,
    TypeParameter {
        owner: DeclarationId,
        index: u32,
    },
    Nominal {
        declaration: DeclarationId,
        arguments: Vec<ResolvedType>,
    },
}

impl ResolvedType {
    pub fn nominal_id(&self) -> Option<&DeclarationId> {
        match self {
            Self::Nominal { declaration, .. } => Some(declaration),
            Self::I64 | Self::Bool | Self::TypeParameter { .. } => None,
        }
    }

    /// A name-independent key suitable as an input to future layout hashing.
    pub fn identity_key(&self) -> String {
        match self {
            Self::I64 => "i64".to_owned(),
            Self::Bool => "bool".to_owned(),
            Self::TypeParameter { owner, index } => {
                format!("parameter:{}:{}:{index}", owner.as_str().len(), owner)
            }
            Self::Nominal {
                declaration,
                arguments,
            } => {
                let argument_count = arguments.len();
                let encoded_arguments =
                    arguments
                        .iter()
                        .fold(String::new(), |mut output, argument| {
                            let key = argument.identity_key();
                            write!(output, "{}:{key}", key.len())
                                .expect("writing to a string cannot fail");
                            output
                        });
                format!(
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    argument_count,
                    encoded_arguments
                )
            }
        }
    }
}

impl FunctionInstanceId {
    pub fn derive(template: &DeclarationId, arguments: &[ResolvedType]) -> Self {
        let mut encoded_arguments = String::new();
        for argument in arguments {
            let key = argument.identity_key();
            write!(encoded_arguments, "{}:{key}", key.len())
                .expect("writing to a string cannot fail");
        }
        Self(format!(
            "semaprax.function-instance.v1:{}:{}:{}:{}",
            template.as_str().len(),
            template,
            arguments.len(),
            encoded_arguments
        ))
    }
}

/// Substitute one concrete generic instantiation into a declaration-owned
/// type template. Consumers share this helper so payload validation, type
/// facts, layouts, and backends cannot disagree about parameter identity.
pub(crate) fn substitute_type(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<ResolvedType, Diagnostic> {
    match template {
        ResolvedType::I64 => Ok(ResolvedType::I64),
        ResolvedType::Bool => Ok(ResolvedType::Bool),
        ResolvedType::TypeParameter {
            owner: parameter_owner,
            index,
        } => {
            if parameter_owner != owner {
                return Err(hir_error(format!(
                    "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                )));
            }
            arguments
                .get(usize::try_from(*index).map_err(|_| {
                    hir_error(format!("type parameter index {index} does not fit usize"))
                })?)
                .cloned()
                .ok_or_else(|| {
                    hir_error(format!(
                        "type template for `{owner}` references missing parameter {index}"
                    ))
                })
        }
        ResolvedType::Nominal {
            declaration,
            arguments: nested,
        } => Ok(ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: nested
                .iter()
                .map(|argument| substitute_type(argument, owner, arguments))
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }
}

fn substitute_source_function_type(
    function: &crate::ast::Function,
    arguments: &[Type],
    template: &Type,
) -> Option<Type> {
    match template {
        Type::I64 => Some(Type::I64),
        Type::Bool => Some(Type::Bool),
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
                    return arguments.get(index).cloned();
                }
            }
            Some(Type::Named {
                name: name.clone(),
                arguments: nested
                    .iter()
                    .map(|nested| substitute_source_function_type(function, arguments, nested))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
    }
}

fn specialize_source_function(
    function: &crate::ast::Function,
    arguments: &[Type],
) -> Option<crate::ast::Function> {
    let mut specialized = function.clone();
    specialized.type_parameters.clear();
    for param in &mut specialized.params {
        param.ty = substitute_source_function_type(function, arguments, &param.ty)?;
    }
    specialized.return_type =
        substitute_source_function_type(function, arguments, &function.return_type)?;
    Some(specialized)
}

fn materialize_function_template(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
) -> Result<ResolvedFunction, Diagnostic> {
    let instance = FunctionInstanceId::derive(&template.id, arguments);
    let execution = FunctionExecutionId::Generic(instance);
    let mut values = BTreeMap::new();
    let params = template
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let id = ValueId::parameter(&execution, index);
            values.insert(parameter.id.clone(), id.clone());
            Ok(ResolvedParam {
                id,
                name: parameter.name.clone(),
                ownership: parameter.ownership,
                ty: substitute_type(&parameter.ty, &template.id, arguments)?,
                span: parameter.span,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let result_id = ValueId::result(&execution);
    let return_type = substitute_type(&template.return_type, &template.id, arguments)?;
    let requires = template
        .requires
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            materialize_template_expr(
                template,
                arguments,
                &execution,
                expression,
                &values,
                &format!("requires.{index}"),
            )
        })
        .collect::<Result<_, _>>()?;
    let body = materialize_template_expr(
        template,
        arguments,
        &execution,
        &template.body,
        &values,
        "body",
    )?;
    let mut ensures_values = values;
    ensures_values.insert(template.result_id.clone(), result_id.clone());
    let ensures = template
        .ensures
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            materialize_template_expr(
                template,
                arguments,
                &execution,
                expression,
                &ensures_values,
                &format!("ensures.{index}"),
            )
        })
        .collect::<Result<_, _>>()?;
    Ok(ResolvedFunction {
        id: template.id.clone(),
        name: template.name.clone(),
        params,
        result_id,
        return_type,
        effects: template.effects.clone(),
        requires,
        ensures,
        body,
        cleanup: CleanupInventory::unresolved(),
        cleanup_plan: CleanupPlan::unresolved(),
        span: template.span,
    })
}

fn resolved_scalar_substitutions(parameter_count: usize) -> Vec<Vec<ResolvedType>> {
    debug_assert!((1..=2).contains(&parameter_count));
    (0..(1_usize << parameter_count))
        .map(|bits| {
            (0..parameter_count)
                .map(|index| {
                    if bits & (1 << index) == 0 {
                        ResolvedType::I64
                    } else {
                        ResolvedType::Bool
                    }
                })
                .collect()
        })
        .collect()
}

fn same_function_meaning(expected: &ResolvedFunction, actual: &ResolvedFunction) -> bool {
    expected.id == actual.id
        && expected.name == actual.name
        && expected.params == actual.params
        && expected.result_id == actual.result_id
        && expected.return_type == actual.return_type
        && expected.effects == actual.effects
        && expected.requires == actual.requires
        && expected.ensures == actual.ensures
        && expected.body == actual.body
        && expected.span == actual.span
}

fn materialize_template_expr(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    values: &BTreeMap<ValueId, ValueId>,
    path: &str,
) -> Result<ResolvedExpr, Diagnostic> {
    let kind = match &expression.kind {
        ResolvedExprKind::Int(value) => ResolvedExprKind::Int(*value),
        ResolvedExprKind::Bool(value) => ResolvedExprKind::Bool(*value),
        ResolvedExprKind::Place(place) => ResolvedExprKind::Place(Place {
            root: values
                .get(&place.root)
                .cloned()
                .ok_or_else(|| hir_error("generic template place is out of scope"))?,
            projections: place.projections.clone(),
        }),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            if instance.is_some() || !type_arguments.is_empty() {
                return Err(hir_error(
                    "generic templates cannot call generic function instances",
                ));
            }
            ResolvedExprKind::Call {
                callee: callee.clone(),
                type_arguments: Vec::new(),
                instance: None,
                args: args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            argument,
                            values,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?,
            }
        }
        ResolvedExprKind::Unary { op, value } => ResolvedExprKind::Unary {
            op: *op,
            value: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                value,
                values,
                &format!("{path}.value"),
            )?),
        },
        ResolvedExprKind::Binary { op, left, right } => ResolvedExprKind::Binary {
            op: *op,
            left: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                left,
                values,
                &format!("{path}.left"),
            )?),
            right: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                right,
                values,
                &format!("{path}.right"),
            )?),
        },
        ResolvedExprKind::Block { statements, tail } => {
            let mut block_values = values.clone();
            let mut materialized = Vec::with_capacity(statements.len());
            for (index, statement) in statements.iter().enumerate() {
                let statement_path = format!("{path}.s{index}");
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        value,
                        span,
                    } => {
                        let value = materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            value,
                            &block_values,
                            &format!("{statement_path}.value"),
                        )?;
                        let id = ValueId::local(execution, &statement_path);
                        block_values.insert(binding.id.clone(), id.clone());
                        materialized.push(ResolvedStatement::Let {
                            binding: ResolvedBinding {
                                id,
                                name: binding.name.clone(),
                                ownership: binding.ownership,
                                ty: substitute_type(&binding.ty, &template.id, arguments)?,
                                span: binding.span,
                            },
                            value,
                            span: *span,
                        });
                    }
                }
            }
            ResolvedExprKind::Block {
                statements: materialized,
                tail: Box::new(materialize_template_expr(
                    template,
                    arguments,
                    execution,
                    tail,
                    &block_values,
                    &format!("{path}.tail"),
                )?),
            }
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => ResolvedExprKind::If {
            condition: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                condition,
                values,
                &format!("{path}.condition"),
            )?),
            then_branch: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                then_branch,
                values,
                &format!("{path}.then"),
            )?),
            else_branch: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                else_branch,
                values,
                &format!("{path}.else"),
            )?),
        },
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. } => {
            return Err(hir_error(
                "generic template uses an expression outside the direct-scalar slice",
            ));
        }
    };
    Ok(ResolvedExpr {
        id: ExpressionId::new(execution, path),
        ty: substitute_type(&expression.ty, &template.id, arguments)?,
        ownership: expression.ownership,
        kind,
        span: expression.span,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFacts {
    pub copy: bool,
    pub contains_resource: bool,
    pub sized: bool,
    pub needs_drop: bool,
    pub layout_key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    Value,
    Own,
    Borrow,
    Shared,
}

impl From<ParamMode> for OwnershipMode {
    fn from(mode: ParamMode) -> Self {
        match mode {
            ParamMode::Value => Self::Value,
            ParamMode::Own => Self::Own,
            ParamMode::Borrow => Self::Borrow,
            ParamMode::Shared => Self::Shared,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub module: String,
    pub permits: Vec<String>,
    pub entrypoint: DeclarationId,
    pub declarations: DeclarationIndex,
    pub types: Vec<ResolvedTypeDeclaration>,
    pub interfaces: Vec<ResolvedInterface>,
    pub function_templates: Vec<ResolvedFunctionTemplate>,
    pub functions: Vec<ResolvedFunction>,
    pub function_instances: Vec<ResolvedFunctionInstance>,
}

impl ResolvedProgram {
    pub fn resolve_call_target(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        match instance {
            None => self
                .functions
                .iter()
                .find(|function| function.id == *callee),
            Some(instance) => self
                .function_instances
                .iter()
                .find(|candidate| candidate.id == *instance && candidate.template == *callee)
                .map(|candidate| &candidate.function),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub type_parameters: Vec<ResolvedTypeParameterDeclaration>,
    pub kind: ResolvedTypeDeclarationKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeParameterDeclaration {
    pub name: String,
    pub index: u32,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTypeDeclarationKind {
    Resource {
        drop: ResolvedResourceDrop,
    },
    Record {
        fields: Vec<ResolvedFieldDeclaration>,
    },
    Variant {
        cases: Vec<ResolvedVariantCaseDeclaration>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedVariantCaseDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub index: u32,
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedResourceDrop {
    pub id: DeclarationId,
    pub kind: ResolvedResourceDropKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedResourceDropKind {
    Trivial,
    Imported {
        import: DeclarationId,
        import_key: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterface {
    pub id: DeclarationId,
    pub name: String,
    pub permits: Vec<String>,
    pub imports: Vec<ResolvedImport>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub id: DeclarationId,
    pub name: String,
    pub interface: DeclarationId,
    pub import_key: String,
    pub parameters: Vec<ResolvedImportParameter>,
    pub result: ResolvedImportResult,
    pub effects: Vec<String>,
    pub required_authority: Vec<String>,
    pub failure: ResolvedImportFailure,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImportParameter {
    pub name: String,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub consumes_on_failure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImportResult {
    pub kind: ResolvedImportResultKind,
    pub ownership: OwnershipMode,
    pub producer: &'static str,
    pub out_slot_initialization: &'static str,
    pub ownership_transfer: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedImportResultKind {
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedImportFailure {
    Infallible,
    Status {
        domain_id: String,
        normalization: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub index: u32,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunction {
    pub id: DeclarationId,
    pub name: String,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub cleanup: CleanupInventory,
    pub cleanup_plan: CleanupPlan,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionInstance {
    pub id: FunctionInstanceId,
    pub template: DeclarationId,
    pub type_arguments: Vec<ResolvedType>,
    pub function: ResolvedFunction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionTemplate {
    pub id: DeclarationId,
    pub name: String,
    pub type_parameters: Vec<ResolvedTypeParameterDeclaration>,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParam {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinding {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExpr {
    pub id: ExpressionId,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub kind: ResolvedExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExprKind {
    Int(i64),
    Bool(bool),
    Place(Place),
    Call {
        callee: DeclarationId,
        type_arguments: Vec<ResolvedType>,
        instance: Option<FunctionInstanceId>,
        args: Vec<ResolvedExpr>,
    },
    Unary {
        op: UnaryOp,
        value: Box<ResolvedExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<ResolvedExpr>,
        right: Box<ResolvedExpr>,
    },
    Block {
        statements: Vec<ResolvedStatement>,
        tail: Box<ResolvedExpr>,
    },
    If {
        condition: Box<ResolvedExpr>,
        then_branch: Box<ResolvedExpr>,
        else_branch: Box<ResolvedExpr>,
    },
    ConstructRecord {
        record: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    ConstructVariant {
        variant: DeclarationId,
        case: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    Match {
        scrutinee: Box<ResolvedExpr>,
        arms: Vec<ResolvedMatchArm>,
    },
    Try {
        operand: Box<ResolvedExpr>,
        result: DeclarationId,
        ok_case: DeclarationId,
        ok_field: DeclarationId,
        err_case: DeclarationId,
        err_field: DeclarationId,
        residual_type: ResolvedType,
    },
    TryOption {
        operand: Box<ResolvedExpr>,
        option: DeclarationId,
        some_case: DeclarationId,
        some_field: DeclarationId,
        none_case: DeclarationId,
        residual_type: ResolvedType,
    },
    UpdateRecord {
        base: Box<ResolvedExpr>,
        record: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    Project {
        base: Box<ResolvedExpr>,
        field: DeclarationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchArm {
    pub pattern: ResolvedMatchPattern,
    pub value: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedMatchPattern {
    Variant {
        variant: DeclarationId,
        case: DeclarationId,
        fields: Vec<ResolvedMatchPatternField>,
    },
    Record {
        record: DeclarationId,
        instance: ResolvedType,
        fields: Vec<ResolvedRecordMatchPatternField>,
    },
    Wildcard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchPatternField {
    pub field: DeclarationId,
    pub binding: ResolvedBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRecordMatchPatternField {
    pub field: DeclarationId,
    pub pattern: ResolvedRecordMatchFieldPattern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedRecordMatchFieldPattern {
    Binding(ResolvedBinding),
    Wildcard,
    Record {
        record: DeclarationId,
        instance: ResolvedType,
        fields: Vec<ResolvedRecordMatchPatternField>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldInitializer {
    pub field: DeclarationId,
    pub value: ResolvedExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    Let {
        binding: ResolvedBinding,
        value: ResolvedExpr,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub root: ValueId,
    pub projections: Vec<PlaceProjection>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlaceProjection {
    Field(DeclarationId),
    VariantField {
        case: DeclarationId,
        field: DeclarationId,
    },
}

#[derive(Clone)]
struct Binding {
    id: ValueId,
    ty: ResolvedType,
    ownership: OwnershipMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Available,
    Moved,
    MaybeMoved,
}

impl Availability {
    fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Self::MaybeMoved
        }
    }
}

#[derive(Clone)]
struct ValidationBinding {
    ty: ResolvedType,
    ownership: OwnershipMode,
    availability: Availability,
    moved_places: BTreeMap<Vec<PlaceProjection>, Availability>,
    definitely_partial: BTreeSet<Vec<PlaceProjection>>,
}

/// Verify and resolve a parsed program into deterministic HIR.
///
/// Verification errors are returned unchanged. This makes the HIR boundary
/// fail closed: no backend can accidentally resolve and execute an invalid AST.
pub fn resolve(program: &Program) -> Result<ResolvedProgram, Vec<Diagnostic>> {
    let Analysis {
        diagnostics,
        resolved,
    } = analyze(program);
    resolved.ok_or(diagnostics)
}

/// The source diagnostics and optional resolved meaning from one analysis.
///
/// Warnings do not prevent resolution. Any source error fails closed before the
/// resolver runs, so invalid source cannot leak internal HIR diagnostics.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved: Option<ResolvedProgram>,
}

/// Verify source once and resolve it when only warnings remain.
pub fn analyze(program: &Program) -> Analysis {
    let diagnostics = source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Analysis {
            diagnostics,
            resolved: None,
        };
    }

    let declarations = match DeclarationIndex::from_verified(program) {
        Ok(declarations) => declarations,
        Err(diagnostic) => {
            return Analysis {
                diagnostics: vec![diagnostic],
                resolved: None,
            };
        }
    };
    match (Resolver {
        program,
        declarations,
    })
    .resolve()
    {
        Ok(resolved) => Analysis {
            diagnostics,
            resolved: Some(resolved),
        },
        Err(diagnostic) => Analysis {
            // Preserve `resolve`'s established invariant-failure behavior: an
            // internal HIR diagnostic replaces otherwise non-fatal warnings.
            diagnostics: vec![diagnostic],
            resolved: None,
        },
    }
}

/// Validate an identity-resolved program before a semantic consumer uses it.
///
/// Resolved HIR is intentionally public for agent and compiler integrations,
/// so callers can inspect or transform HIR produced by [`resolve`]. Every
/// backend calls this function and therefore fails closed when a transformation
/// breaks identities, lexical scope, or current type rules. A versioned wire
/// schema for constructing HIR outside the compiler is future work.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    validate_core(program)?;
    validate_attached_identity_references(program)?;
    crate::cleanup::validate_program(program)?;
    crate::cleanup_plan::validate_program(program)?;
    Ok(())
}

/// Validate resolved meaning without consulting attached cleanup metadata.
/// Independent cleanup-plan replayers use this boundary to avoid circularly
/// trusting the canonical cleanup-plan builder as their oracle.
pub(crate) fn validate_core(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    HirValidator::new(program)?.validate()
}

struct HirValidator<'a> {
    program: &'a ResolvedProgram,
    functions: BTreeMap<DeclarationId, &'a ResolvedFunction>,
    expression_ids: BTreeSet<ExpressionId>,
    value_ids: BTreeSet<ValueId>,
}

impl<'a> HirValidator<'a> {
    fn execution_function(&self, id: &FunctionExecutionId) -> Option<&ResolvedFunction> {
        match id {
            FunctionExecutionId::Monomorphic(declaration) => {
                self.functions.get(declaration).copied()
            }
            FunctionExecutionId::Generic(instance) => self
                .program
                .function_instances
                .iter()
                .find(|candidate| candidate.id == *instance)
                .map(|candidate| &candidate.function),
        }
    }

    fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        validate_nul_free_identities(program)?;
        let mut functions = BTreeMap::new();
        for function in &program.functions {
            if functions.insert(function.id.clone(), function).is_some() {
                return Err(hir_error(format!(
                    "duplicate resolved function identity `{}`",
                    function.id
                )));
            }
            match program.declarations.declaration(&function.id) {
                Some(declaration)
                    if declaration.kind == DeclarationKind::Function
                        && declaration.name == function.name => {}
                Some(_) => {
                    return Err(hir_error(format!(
                        "resolved function `{}` disagrees with its declaration index entry",
                        function.id
                    )));
                }
                None => {
                    return Err(hir_error(format!(
                        "resolved function `{}` is absent from the declaration index",
                        function.id
                    )));
                }
            }
        }
        Ok(Self {
            program,
            functions,
            expression_ids: BTreeSet::new(),
            value_ids: BTreeSet::new(),
        })
    }

    fn validate(mut self) -> Result<(), Diagnostic> {
        let entrypoint = self
            .functions
            .get(&self.program.entrypoint)
            .ok_or_else(|| hir_error("resolved entry point is not indexed"))?;
        if !entrypoint.params.is_empty() || entrypoint.return_type != ResolvedType::I64 {
            return Err(hir_error(
                "resolved entry point must have type `fn main() -> i64`",
            ));
        }

        let type_ids = self
            .program
            .types
            .iter()
            .map(|declaration| declaration.id.clone())
            .collect::<BTreeSet<_>>();
        if type_ids.len() != self.program.types.len() {
            return Err(hir_error("duplicate resolved type declaration identity"));
        }
        let mut interface_ids = BTreeSet::new();
        let mut import_ids = BTreeSet::new();
        let mut imports = BTreeMap::new();
        for interface in &self.program.interfaces {
            if !interface_ids.insert(interface.id.clone()) {
                return Err(hir_error(format!(
                    "duplicate resolved interface identity `{}`",
                    interface.id
                )));
            }
            match self.program.declarations.declaration(&interface.id) {
                Some(item)
                    if item.kind == DeclarationKind::Interface
                        && item.name == interface.name
                        && item.owner.is_none() => {}
                _ => {
                    return Err(hir_error(format!(
                        "resolved interface `{}` disagrees with its declaration index entry",
                        interface.id
                    )));
                }
            }
            let permits = interface.permits.iter().collect::<BTreeSet<_>>();
            if permits.len() != interface.permits.len() {
                return Err(hir_error(format!(
                    "interface `{}` has duplicate permits",
                    interface.id
                )));
            }
            for import in &interface.imports {
                if !import_ids.insert(import.id.clone())
                    || imports.insert(import.id.clone(), import).is_some()
                {
                    return Err(hir_error(format!(
                        "duplicate resolved import identity `{}`",
                        import.id
                    )));
                }
                if import.interface != interface.id
                    || self.program.declarations.import_id(&import.import_key) != Some(&import.id)
                {
                    return Err(hir_error(format!(
                        "import `{}` has an invalid owner or logical key",
                        import.id
                    )));
                }
                match self.program.declarations.declaration(&import.id) {
                    Some(item)
                        if item.kind == DeclarationKind::Import
                            && item.name == import.name
                            && item.owner.as_ref() == Some(&interface.id) => {}
                    _ => {
                        return Err(hir_error(format!(
                            "resolved import `{}` disagrees with its declaration index entry",
                            import.id
                        )));
                    }
                }
                if import.parameters.len() != 1
                    || import.parameters[0].ownership != OwnershipMode::Own
                    || !import.parameters[0].consumes_on_failure
                    || import.result.kind != ResolvedImportResultKind::Unit
                    || import.result.ownership != OwnershipMode::Value
                    || import.result.producer != "callee"
                    || import.result.out_slot_initialization != "success_only"
                    || import.result.ownership_transfer != "final_zero_status_commit"
                    || import.required_authority != import.effects
                {
                    return Err(hir_error(format!(
                        "import `{}` has an invalid ownership or result contract",
                        import.id
                    )));
                }
                self.validate_type(&import.parameters[0].ty)?;
                let parameter_is_resource = import.parameters[0]
                    .ty
                    .nominal_id()
                    .and_then(|id| self.program.declarations.declaration(id))
                    .is_some_and(|item| item.kind == DeclarationKind::Resource);
                let effects = import.effects.iter().collect::<BTreeSet<_>>();
                let authority = import.required_authority.iter().collect::<BTreeSet<_>>();
                if !parameter_is_resource
                    || effects.len() != import.effects.len()
                    || authority.len() != import.required_authority.len()
                {
                    return Err(hir_error(format!(
                        "import `{}` has a noncanonical resource or authority contract",
                        import.id
                    )));
                }
                if import
                    .effects
                    .iter()
                    .any(|effect| !permits.contains(effect))
                {
                    return Err(hir_error(format!(
                        "import `{}` exceeds interface `{}` authority",
                        import.id, interface.id
                    )));
                }
                if let ResolvedImportFailure::Status {
                    domain_id,
                    normalization,
                } = &import.failure
                {
                    if domain_id.is_empty()
                        || domain_id.len() > STATUS_DOMAIN_MAX_BYTES_V1
                        || domain_id.contains('\0')
                        || *normalization != "semaprax.status.v1"
                    {
                        return Err(hir_error(format!(
                            "import `{}` has an invalid status contract",
                            import.id
                        )));
                    }
                }
            }
        }
        let mut template_ids = BTreeSet::new();
        for template in &self.program.function_templates {
            if !template_ids.insert(template.id.clone())
                || self.functions.contains_key(&template.id)
            {
                return Err(hir_error(format!(
                    "duplicate or executable generic template `{}`",
                    template.id
                )));
            }
            match self.program.declarations.declaration(&template.id) {
                Some(declaration)
                    if declaration.kind == DeclarationKind::Function
                        && declaration.name == template.name => {}
                _ => {
                    return Err(hir_error(format!(
                        "generic template `{}` disagrees with its declaration",
                        template.id
                    )));
                }
            }
            if !(1..=2).contains(&template.type_parameters.len()) || !template.effects.is_empty() {
                return Err(hir_error(format!(
                    "generic template `{}` is outside the bounded slice",
                    template.id
                )));
            }
            if self.program.declarations.type_parameters(&template.id)
                != Some(template.type_parameters.as_slice())
            {
                return Err(hir_error(format!(
                    "generic template `{}` has non-canonical type-parameter metadata",
                    template.id
                )));
            }
            for (index, parameter) in template.type_parameters.iter().enumerate() {
                if usize::try_from(parameter.index) != Ok(index) {
                    return Err(hir_error(format!(
                        "generic template `{}` has non-canonical parameter indices",
                        template.id
                    )));
                }
            }
            let execution = FunctionExecutionId::Monomorphic(template.id.clone());
            for (index, parameter) in template.params.iter().enumerate() {
                if parameter.id != ValueId::parameter(&execution, index)
                    || parameter.ownership != OwnershipMode::Value
                {
                    return Err(hir_error(format!(
                        "generic template `{}` has invalid parameter identity or ownership",
                        template.id
                    )));
                }
                self.validate_function_template_type(template, &parameter.ty)?;
            }
            self.validate_function_template_type(template, &template.return_type)?;
            if template.result_id != ValueId::result(&execution) {
                return Err(hir_error(format!(
                    "generic template `{}` has invalid result identity",
                    template.id
                )));
            }
            self.validate_template_expressions(template, &execution)?;
            for arguments in resolved_scalar_substitutions(template.type_parameters.len()) {
                let materialized = materialize_function_template(template, &arguments)?;
                let saved_expression_ids = self.expression_ids.clone();
                let saved_value_ids = self.value_ids.clone();
                self.validate_function(
                    &materialized,
                    &FunctionExecutionId::Generic(FunctionInstanceId::derive(
                        &template.id,
                        &arguments,
                    )),
                )?;
                self.expression_ids = saved_expression_ids;
                self.value_ids = saved_value_ids;
            }
        }
        let mut resource_drop_ids = BTreeSet::new();
        let mut field_ids = BTreeSet::new();
        let mut variant_case_ids = BTreeSet::new();
        let mut case_field_ids = BTreeSet::new();
        for declaration in &self.program.types {
            let expected_kind = match &declaration.kind {
                ResolvedTypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                ResolvedTypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                ResolvedTypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            match self.program.declarations.declaration(&declaration.id) {
                Some(item)
                    if item.kind == expected_kind
                        && item.name == declaration.name
                        && item.owner.is_none() => {}
                Some(_) => {
                    return Err(hir_error(format!(
                        "resolved type `{}` disagrees with its declaration index entry",
                        declaration.id
                    )));
                }
                None => {
                    return Err(hir_error(format!(
                        "resolved type `{}` is absent from the declaration index",
                        declaration.id
                    )));
                }
            }
            let indexed_parameters = self
                .program
                .declarations
                .type_parameters(&declaration.id)
                .ok_or_else(|| {
                    hir_error(format!(
                        "type `{}` has no indexed parameter sequence",
                        declaration.id
                    ))
                })?;
            if indexed_parameters != declaration.type_parameters.as_slice()
                || declaration
                    .type_parameters
                    .iter()
                    .enumerate()
                    .any(|(index, parameter)| usize::try_from(parameter.index) != Ok(index))
                || declaration
                    .type_parameters
                    .iter()
                    .map(|parameter| &parameter.name)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != declaration.type_parameters.len()
                || (matches!(
                    declaration.kind,
                    ResolvedTypeDeclarationKind::Resource { .. }
                ) && !declaration.type_parameters.is_empty())
            {
                return Err(hir_error(format!(
                    "type `{}` has invalid generic parameter metadata",
                    declaration.id
                )));
            }
            if let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind {
                if !resource_drop_ids.insert(drop.id.clone()) {
                    return Err(hir_error(format!(
                        "duplicate resolved resource lifecycle identity `{}`",
                        drop.id
                    )));
                }
                match self.program.declarations.declaration(&drop.id) {
                    Some(item)
                        if item.kind == DeclarationKind::ResourceDrop
                            && item.name == "drop"
                            && item.owner.as_ref() == Some(&declaration.id) => {}
                    _ => {
                        return Err(hir_error(format!(
                            "resource `{}` has an invalid lifecycle declaration `{}`",
                            declaration.id, drop.id
                        )));
                    }
                }
                if let ResolvedResourceDropKind::Imported { import, import_key } = &drop.kind {
                    let resolved_import = imports.get(import).ok_or_else(|| {
                        hir_error(format!(
                            "resource `{}` lifecycle references unknown import `{import}`",
                            declaration.id
                        ))
                    })?;
                    let expected_ty = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    if resolved_import.import_key != *import_key
                        || resolved_import.parameters[0].ty != expected_ty
                        || !matches!(resolved_import.failure, ResolvedImportFailure::Infallible)
                    {
                        return Err(hir_error(format!(
                            "resource `{}` lifecycle is incompatible with import `{import}`",
                            declaration.id
                        )));
                    }
                }
            }
            if let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind {
                let indexed = self
                    .program
                    .declarations
                    .record_fields(&declaration.id)
                    .ok_or_else(|| {
                        hir_error(format!(
                            "record `{}` has no indexed field sequence",
                            declaration.id
                        ))
                    })?;
                if indexed.len() != fields.len() {
                    return Err(hir_error(format!(
                        "record `{}` field sequence disagrees with its declaration index",
                        declaration.id
                    )));
                }
                for (position, (field, indexed_field)) in fields.iter().zip(indexed).enumerate() {
                    if !field_ids.insert(field.id.clone()) {
                        return Err(hir_error(format!(
                            "duplicate resolved field identity `{}`",
                            field.id
                        )));
                    }
                    if field.id != indexed_field.id
                        || field.name != indexed_field.name
                        || usize::try_from(field.index) != Ok(position)
                        || field.index != indexed_field.index
                        || field.ty != indexed_field.ty
                    {
                        return Err(hir_error(format!(
                            "field {position} of record `{}` disagrees with its declaration index",
                            declaration.id
                        )));
                    }
                    match self.program.declarations.declaration(&field.id) {
                        Some(item)
                            if item.kind == DeclarationKind::Field
                                && item.name == field.name
                                && item.owner.as_ref() == Some(&declaration.id)
                                && self
                                    .program
                                    .declarations
                                    .field_id(&declaration.id, &field.name)
                                    == Some(&field.id) => {}
                        _ => {
                            return Err(hir_error(format!(
                                "field `{}` is not indexed under record `{}`",
                                field.id, declaration.id
                            )));
                        }
                    }
                    if declaration.type_parameters.is_empty() {
                        self.validate_type(&field.ty)?;
                        if let ResolvedType::Nominal {
                            declaration: field_declaration,
                            arguments,
                        } = &field.ty
                        {
                            if !arguments.is_empty()
                                && self
                                    .program
                                    .declarations
                                    .declaration(field_declaration)
                                    .is_some_and(|item| item.kind == DeclarationKind::Record)
                            {
                                return Err(hir_error(format!(
                                    "field `{}` nests a generic record instance outside the admitted slice",
                                    field.id
                                )));
                            }
                        }
                    } else {
                        match &field.ty {
                            ResolvedType::I64 | ResolvedType::Bool => {}
                            ResolvedType::TypeParameter { owner, index }
                                if owner == &declaration.id
                                    && declaration
                                        .type_parameters
                                        .get(usize::try_from(*index).map_err(|_| {
                                            hir_error("type parameter index does not fit usize")
                                        })?)
                                        .is_some() => {}
                            ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => {
                                return Err(hir_error(format!(
                                    "field `{}` has an invalid generic copy record template",
                                    field.id
                                )));
                            }
                        }
                    }
                }
                if declaration.type_parameters.is_empty() {
                    let record_ty = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    let cached = self.program.declarations.type_facts(&record_ty);
                    let recomputed = self.program.declarations.recompute_type_facts(&record_ty);
                    if cached.is_none() || cached != recomputed {
                        return Err(hir_error(format!(
                            "record `{}` has invalid or stale recursive type facts",
                            declaration.id
                        )));
                    }
                }
            }
            if let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind {
                if cases.is_empty() {
                    return Err(hir_error(format!(
                        "variant `{}` has no cases",
                        declaration.id
                    )));
                }
                let indexed = self
                    .program
                    .declarations
                    .variant_cases(&declaration.id)
                    .ok_or_else(|| {
                        hir_error(format!(
                            "variant `{}` has no indexed case sequence",
                            declaration.id
                        ))
                    })?;
                if indexed.len() != cases.len() {
                    return Err(hir_error(format!(
                        "variant `{}` case sequence disagrees with its declaration index",
                        declaration.id
                    )));
                }
                for (case_position, (case, indexed_case)) in cases.iter().zip(indexed).enumerate() {
                    if !variant_case_ids.insert(case.id.clone())
                        || case.id != indexed_case.id
                        || case.name != indexed_case.name
                        || usize::try_from(case.index) != Ok(case_position)
                        || case.index != indexed_case.index
                    {
                        return Err(hir_error(format!(
                            "case {case_position} of variant `{}` disagrees with its declaration index",
                            declaration.id
                        )));
                    }
                    match self.program.declarations.declaration(&case.id) {
                        Some(item)
                            if item.kind == DeclarationKind::VariantCase
                                && item.name == case.name
                                && item.owner.as_ref() == Some(&declaration.id)
                                && self
                                    .program
                                    .declarations
                                    .case_id(&declaration.id, &case.name)
                                    == Some(&case.id) => {}
                        _ => {
                            return Err(hir_error(format!(
                                "case `{}` is not indexed under variant `{}`",
                                case.id, declaration.id
                            )));
                        }
                    }
                    let indexed_fields = self
                        .program
                        .declarations
                        .case_fields(&case.id)
                        .ok_or_else(|| {
                            hir_error(format!("case `{}` has no indexed field sequence", case.id))
                        })?;
                    if indexed_fields.len() != case.fields.len() {
                        return Err(hir_error(format!(
                            "case `{}` field sequence disagrees with its declaration index",
                            case.id
                        )));
                    }
                    for (field_position, (field, indexed_field)) in
                        case.fields.iter().zip(indexed_fields).enumerate()
                    {
                        if !case_field_ids.insert(field.id.clone())
                            || field.id != indexed_field.id
                            || field.name != indexed_field.name
                            || usize::try_from(field.index) != Ok(field_position)
                            || field.index != indexed_field.index
                            || field.ty != indexed_field.ty
                            || !matches!(
                                field.ty,
                                ResolvedType::I64
                                    | ResolvedType::Bool
                                    | ResolvedType::TypeParameter { .. }
                            )
                        {
                            return Err(hir_error(format!(
                                "field {field_position} of case `{}` is invalid or disagrees with its declaration index",
                                case.id
                            )));
                        }
                        match self.program.declarations.declaration(&field.id) {
                            Some(item)
                                if item.kind == DeclarationKind::CaseField
                                    && item.name == field.name
                                    && item.owner.as_ref() == Some(&case.id)
                                    && self
                                        .program
                                        .declarations
                                        .field_id(&case.id, &field.name)
                                        == Some(&field.id) => {}
                            _ => {
                                return Err(hir_error(format!(
                                    "field `{}` is not indexed under case `{}`",
                                    field.id, case.id
                                )));
                            }
                        }
                        match &field.ty {
                            ResolvedType::I64 | ResolvedType::Bool => {}
                            ResolvedType::TypeParameter { owner, index }
                                if owner == &declaration.id
                                    && declaration
                                        .type_parameters
                                        .get(usize::try_from(*index).map_err(|_| {
                                            hir_error("type parameter index does not fit usize")
                                        })?)
                                        .is_some() => {}
                            ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => {
                                return Err(hir_error(format!(
                                    "field `{}` has an invalid generic copy payload template",
                                    field.id
                                )));
                            }
                        }
                    }
                }
                if declaration.type_parameters.is_empty() {
                    let variant_ty = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    let cached = self.program.declarations.type_facts(&variant_ty);
                    let recomputed = self.program.declarations.recompute_type_facts(&variant_ty);
                    if cached.is_none()
                        || cached != recomputed
                        || cached.as_ref().is_none_or(|facts| {
                            !facts.copy
                                || facts.contains_resource
                                || facts.needs_drop
                                || !facts.sized
                        })
                    {
                        return Err(hir_error(format!(
                            "variant `{}` has invalid or stale type facts",
                            declaration.id
                        )));
                    }
                }
            }
        }
        for declaration in self.program.declarations.declarations() {
            match declaration.kind {
                DeclarationKind::Resource | DeclarationKind::Record | DeclarationKind::Variant
                    if !type_ids.contains(&declaration.id) =>
                {
                    return Err(hir_error(format!(
                        "type `{}` has no resolved type declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Field if !field_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "field `{}` has no resolved field declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::VariantCase if !variant_case_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "variant case `{}` has no resolved case declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::CaseField if !case_field_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "case field `{}` has no resolved field declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Function
                    if !self.functions.contains_key(&declaration.id)
                        && !template_ids.contains(&declaration.id) =>
                {
                    return Err(hir_error(format!(
                        "function `{}` has no resolved function body",
                        declaration.id
                    )));
                }
                DeclarationKind::ResourceDrop if !resource_drop_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "resource lifecycle `{}` has no resolved declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Interface if !interface_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "interface `{}` has no resolved declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Import if !import_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "import `{}` has no resolved declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Resource
                | DeclarationKind::ResourceDrop
                | DeclarationKind::Record
                | DeclarationKind::Field
                | DeclarationKind::Variant
                | DeclarationKind::VariantCase
                | DeclarationKind::CaseField
                | DeclarationKind::Interface
                | DeclarationKind::Import
                | DeclarationKind::Function => {}
            }
        }

        for function in &self.program.functions {
            self.validate_function(
                function,
                &FunctionExecutionId::Monomorphic(function.id.clone()),
            )?;
        }
        let mut instance_ids = BTreeSet::new();
        let expected_instances = self.reachable_function_instances()?;
        if expected_instances.len() != self.program.function_instances.len()
            || expected_instances
                .iter()
                .zip(&self.program.function_instances)
                .any(|((id, template, arguments), actual)| {
                    id != &actual.id
                        || template != &actual.template
                        || arguments != &actual.type_arguments
                })
        {
            return Err(hir_error(
                "materialized generic function instances are not the exact reachable sequence",
            ));
        }
        for instance in &self.program.function_instances {
            if !instance_ids.insert(instance.id.clone())
                || FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
                    != instance.id
                || instance.function.id != instance.template
                || instance
                    .type_arguments
                    .iter()
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            {
                return Err(hir_error(
                    "generic function instance identity is inconsistent",
                ));
            }
            let template = self
                .program
                .function_templates
                .iter()
                .find(|template| template.id == instance.template)
                .ok_or_else(|| hir_error("generic function instance has no template"))?;
            if template.type_parameters.len() != instance.type_arguments.len()
                || template.params.len() != instance.function.params.len()
            {
                return Err(hir_error("generic function instance has invalid arity"));
            }
            for (template_param, concrete_param) in
                template.params.iter().zip(&instance.function.params)
            {
                let expected =
                    substitute_type(&template_param.ty, &template.id, &instance.type_arguments)?;
                if concrete_param.ty != expected {
                    return Err(hir_error(
                        "generic function instance parameter substitution is inconsistent",
                    ));
                }
            }
            let expected_return = substitute_type(
                &template.return_type,
                &template.id,
                &instance.type_arguments,
            )?;
            if instance.function.return_type != expected_return {
                return Err(hir_error(
                    "generic function instance result substitution is inconsistent",
                ));
            }
            let expected_function =
                materialize_function_template(template, &instance.type_arguments)?;
            if !same_function_meaning(&expected_function, &instance.function) {
                return Err(hir_error(
                    "generic function instance is not the exact template substitution",
                ));
            }
            self.validate_function(
                &instance.function,
                &FunctionExecutionId::Generic(instance.id.clone()),
            )?;
        }
        Ok(())
    }

    fn validate_function_template_type(
        &self,
        template: &ResolvedFunctionTemplate,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        match ty {
            ResolvedType::I64 | ResolvedType::Bool => Ok(()),
            ResolvedType::TypeParameter { owner, index }
                if owner == &template.id
                    && usize::try_from(*index)
                        .ok()
                        .is_some_and(|index| index < template.type_parameters.len()) =>
            {
                Ok(())
            }
            ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => {
                Err(hir_error(format!(
                    "generic template `{}` has an invalid direct-scalar signature slot",
                    template.id
                )))
            }
        }
    }

    fn validate_template_expressions(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        let mut values = BTreeMap::new();
        for parameter in &template.params {
            self.insert_value(&parameter.id)?;
            values.insert(parameter.id.clone(), parameter.ty.clone());
        }
        for (index, expression) in template.requires.iter().enumerate() {
            let mut contract_values = values.clone();
            self.validate_template_expr(
                template,
                execution,
                expression,
                &mut contract_values,
                &format!("requires.{index}"),
            )?;
        }
        self.validate_template_expr(template, execution, &template.body, &mut values, "body")?;
        self.insert_value(&template.result_id)?;
        values.insert(template.result_id.clone(), template.return_type.clone());
        for (index, expression) in template.ensures.iter().enumerate() {
            let mut contract_values = values.clone();
            self.validate_template_expr(
                template,
                execution,
                expression,
                &mut contract_values,
                &format!("ensures.{index}"),
            )?;
        }
        Ok(())
    }

    fn validate_template_expr(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if expression.id != ExpressionId::new(execution, path)
            || !self.expression_ids.insert(expression.id.clone())
            || expression.ownership != OwnershipMode::Value
        {
            return Err(hir_error(format!(
                "generic template `{}` has invalid expression identity or ownership",
                template.id
            )));
        }
        self.validate_function_template_type(template, &expression.ty)?;
        match &expression.kind {
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() || values.get(&place.root) != Some(&expression.ty)
                {
                    return Err(hir_error(
                        "generic template place is out of scope or has the wrong type",
                    ));
                }
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if instance.is_some()
                    || !type_arguments.is_empty()
                    || self
                        .program
                        .functions
                        .iter()
                        .all(|target| target.id != *callee)
                {
                    return Err(hir_error(
                        "generic template call is not a monomorphic executable target",
                    ));
                }
                for (index, argument) in args.iter().enumerate() {
                    self.validate_template_expr(
                        template,
                        execution,
                        argument,
                        values,
                        &format!("{path}.arg.{index}"),
                    )?;
                }
            }
            ResolvedExprKind::Unary { value, .. } => self.validate_template_expr(
                template,
                execution,
                value,
                values,
                &format!("{path}.value"),
            )?,
            ResolvedExprKind::Binary { left, right, .. } => {
                self.validate_template_expr(
                    template,
                    execution,
                    left,
                    values,
                    &format!("{path}.left"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    right,
                    values,
                    &format!("{path}.right"),
                )?;
            }
            ResolvedExprKind::Block { statements, tail } => {
                let mut block_values = values.clone();
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            self.validate_template_expr(
                                template,
                                execution,
                                value,
                                &mut block_values,
                                &format!("{statement_path}.value"),
                            )?;
                            if binding.id != ValueId::local(execution, &statement_path)
                                || binding.ownership != OwnershipMode::Value
                                || binding.ty != value.ty
                            {
                                return Err(hir_error("generic template binding is not canonical"));
                            }
                            self.validate_function_template_type(template, &binding.ty)?;
                            self.insert_value(&binding.id)?;
                            block_values.insert(binding.id.clone(), binding.ty.clone());
                        }
                    }
                }
                self.validate_template_expr(
                    template,
                    execution,
                    tail,
                    &mut block_values,
                    &format!("{path}.tail"),
                )?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_template_expr(
                    template,
                    execution,
                    condition,
                    &mut values.clone(),
                    &format!("{path}.condition"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    then_branch,
                    &mut values.clone(),
                    &format!("{path}.then"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    else_branch,
                    &mut values.clone(),
                    &format!("{path}.else"),
                )?;
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. } => {
                return Err(hir_error(
                    "generic template expression is outside the direct-scalar slice",
                ));
            }
        }
        Ok(())
    }

    fn reachable_function_instances(
        &self,
    ) -> Result<Vec<(FunctionInstanceId, DeclarationId, Vec<ResolvedType>)>, Diagnostic> {
        let mut seen = BTreeSet::new();
        let mut reachable = Vec::new();
        for function in &self.program.functions {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                    let Some(instance) = instance else {
                        return;
                    };
                    if seen.insert(instance.clone()) {
                        reachable.push((instance.clone(), callee.clone(), arguments.to_vec()));
                    }
                });
            }
        }
        for (instance, template, arguments) in &reachable {
            if FunctionInstanceId::derive(template, arguments) != *instance {
                return Err(hir_error(
                    "reachable generic function instance identity is inconsistent",
                ));
            }
        }
        Ok(reachable)
    }

    fn validate_function(
        &mut self,
        function: &ResolvedFunction,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        self.validate_type(&function.return_type)?;
        let permits = self
            .program
            .permits
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for effect in &function.effects {
            if !permits.contains(effect.as_str()) {
                return Err(hir_error(format!(
                    "function `{}` declares effect `{effect}` which the module does not permit",
                    function.id
                )));
            }
        }
        let declared_effects = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        let mut required_lifecycle_effects = BTreeSet::new();
        for param in &function.params {
            if param.ownership == OwnershipMode::Own {
                required_lifecycle_effects
                    .extend(resolved_lifecycle_effects(self.program, &param.ty)?);
            }
        }
        required_lifecycle_effects.extend(resolved_lifecycle_effects(
            self.program,
            &function.return_type,
        )?);
        let mut callees = Vec::new();
        visit_resolved_calls(&function.body, &mut |callee, instance, _| {
            callees.push((callee.clone(), instance.cloned()));
        });
        for (callee, instance) in callees {
            let target = self
                .program
                .resolve_call_target(&callee, instance.as_ref())
                .ok_or_else(|| hir_error(format!("function `{callee}` is not indexed")))?;
            required_lifecycle_effects.extend(resolved_lifecycle_effects(
                self.program,
                &target.return_type,
            )?);
        }
        if let Some(effect) = required_lifecycle_effects
            .iter()
            .find(|effect| !declared_effects.contains(*effect))
        {
            return Err(hir_error(format!(
                "function `{}` omits lifecycle effect `{effect}`",
                function.id
            )));
        }
        let mut scope = BTreeMap::new();
        for (index, param) in function.params.iter().enumerate() {
            reject_nul_identity("resolved value", param.id.as_str())?;
            let expected = ValueId::parameter(execution, index);
            if param.id != expected {
                return Err(hir_error(format!(
                    "parameter {} of `{}` has a non-canonical identity",
                    index, function.id
                )));
            }
            self.insert_value(&param.id)?;
            self.validate_type(&param.ty)?;
            self.validate_declared_ownership(&param.ty, param.ownership)?;
            scope.insert(
                param.id.clone(),
                ValidationBinding {
                    ty: param.ty.clone(),
                    ownership: param.ownership,
                    availability: Availability::Available,
                    moved_places: BTreeMap::new(),
                    definitely_partial: BTreeSet::new(),
                },
            );
        }
        reject_nul_identity("resolved value", function.result_id.as_str())?;
        if function.result_id != ValueId::result(execution) {
            return Err(hir_error(format!(
                "function `{}` has a non-canonical result identity",
                function.id
            )));
        }
        self.insert_value(&function.result_id)?;

        for (index, contract) in function.requires.iter().enumerate() {
            let mut contract_scope = scope.clone();
            self.validate_expr(
                execution,
                contract,
                &mut contract_scope,
                &format!("requires.{index}"),
                false,
                None,
            )?;
            self.require_type(&contract.ty, &ResolvedType::Bool, "precondition")?;
        }
        self.validate_expr(
            execution,
            &function.body,
            &mut scope,
            "body",
            true,
            Some(&declared_effects),
        )?;
        self.require_type(&function.body.ty, &function.return_type, "function body")?;
        let returned = self.expected_ownership(&function.return_type, OwnershipMode::Own)?;
        if function.body.ownership != returned {
            return Err(hir_error(format!(
                "function `{}` body has invalid return ownership",
                function.id
            )));
        }

        let mut ensures_scope = scope;
        ensures_scope.insert(
            function.result_id.clone(),
            ValidationBinding {
                ty: function.return_type.clone(),
                ownership: returned,
                availability: Availability::Available,
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        );
        for (index, contract) in function.ensures.iter().enumerate() {
            let mut contract_scope = ensures_scope.clone();
            self.validate_expr(
                execution,
                contract,
                &mut contract_scope,
                &format!("ensures.{index}"),
                false,
                None,
            )?;
            self.require_type(&contract.ty, &ResolvedType::Bool, "postcondition")?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_record_match_pattern(
        &mut self,
        function: &FunctionExecutionId,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[ResolvedRecordMatchPatternField],
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if instance != expected {
            return Err(hir_error(
                "resolved record pattern has the wrong concrete instance",
            ));
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = expected
        else {
            return Err(hir_error("resolved record pattern instance is not nominal"));
        };
        if declaration != record
            || self
                .program
                .declarations
                .declaration(record)
                .is_none_or(|item| item.kind != DeclarationKind::Record)
        {
            return Err(hir_error(
                "resolved record pattern references a foreign record",
            ));
        }
        let facts = self
            .program
            .declarations
            .type_facts(expected)
            .ok_or_else(|| hir_error("resolved record pattern has no exact type facts"))?;
        if !facts.copy || facts.contains_resource || facts.needs_drop {
            return Err(hir_error("resolved record pattern is not Copy"));
        }
        let declared_fields = self
            .program
            .declarations
            .record_fields(record)
            .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?;
        let mut seen = BTreeSet::new();
        for (field_index, field) in fields.iter().enumerate() {
            let declared = declared_fields
                .iter()
                .find(|candidate| candidate.id == field.field)
                .ok_or_else(|| {
                    hir_error(format!(
                        "resolved record pattern contains foreign field `{}`",
                        field.field
                    ))
                })?;
            if !seen.insert(field.field.clone()) {
                return Err(hir_error(
                    "resolved record pattern contains a duplicate field",
                ));
            }
            let field_ty = substitute_type(&declared.ty, record, arguments)?;
            let field_path = format!("{path}.field.{field_index}");
            match &field.pattern {
                ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    if binding.id != ValueId::local(function, &format!("{field_path}.binding"))
                        || binding.ty != field_ty
                        || binding.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error(
                            "resolved record pattern binding is not canonical",
                        ));
                    }
                    self.insert_value(&binding.id)?;
                    self.validate_type(&binding.ty)?;
                    if scope.contains_key(&binding.id) {
                        return Err(hir_error(
                            "resolved record pattern binding shadows an existing value",
                        ));
                    }
                    scope.insert(
                        binding.id.clone(),
                        ValidationBinding {
                            ty: binding.ty.clone(),
                            ownership: OwnershipMode::Value,
                            availability: Availability::Available,
                            moved_places: BTreeMap::new(),
                            definitely_partial: BTreeSet::new(),
                        },
                    );
                }
                ResolvedRecordMatchFieldPattern::Wildcard => {}
                ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => self.validate_record_match_pattern(
                    function,
                    &field_ty,
                    record,
                    instance,
                    fields,
                    scope,
                    &format!("{field_path}.record"),
                )?,
            }
        }
        if seen.len() != declared_fields.len() {
            return Err(hir_error("resolved record pattern is missing fields"));
        }
        Ok(())
    }

    fn validate_expr(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        reject_nul_identity("resolved expression", expression.id.as_str())?;
        if expression.id != ExpressionId::new(function, path) {
            return Err(hir_error(format!(
                "expression `{}` has a non-canonical identity",
                expression.id
            )));
        }
        if !self.expression_ids.insert(expression.id.clone()) {
            return Err(hir_error(format!(
                "duplicate resolved expression identity `{}`",
                expression.id
            )));
        }
        self.validate_type(&expression.ty)?;

        let (ty, ownership) = match &expression.kind {
            ResolvedExprKind::Int(_) => (ResolvedType::I64, OwnershipMode::Value),
            ResolvedExprKind::Bool(_) => (ResolvedType::Bool, OwnershipMode::Value),
            ResolvedExprKind::Place(place) => {
                let binding = scope.get(&place.root).ok_or_else(|| {
                    hir_error(format!("resolved value `{}` is out of scope", place.root))
                })?;
                match (place.projections.is_empty(), binding.availability) {
                    (true, Availability::Available) => {
                        match Self::place_availability(binding, &[]) {
                            Availability::Available => {}
                            Availability::Moved => {
                                return Err(hir_error(format!(
                                    "resolved value `{}` is partially moved",
                                    place.root
                                )));
                            }
                            Availability::MaybeMoved => {
                                return Err(hir_error(format!(
                                    "resolved value `{}` may be partially moved",
                                    place.root
                                )));
                            }
                        }
                    }
                    (true, Availability::Moved) => {
                        return Err(hir_error(format!(
                            "resolved value `{}` is used after it was moved",
                            place.root
                        )));
                    }
                    (true, Availability::MaybeMoved) => {
                        return Err(hir_error(format!(
                            "resolved value `{}` may have been moved",
                            place.root
                        )));
                    }
                    (false, _) => match Self::place_availability(binding, &place.projections) {
                        Availability::Available => {}
                        Availability::Moved => {
                            return Err(hir_error(format!(
                                "resolved place rooted at `{}` is partially moved",
                                place.root
                            )));
                        }
                        Availability::MaybeMoved => {
                            return Err(hir_error(format!(
                                "resolved place rooted at `{}` may be conditionally moved",
                                place.root
                            )));
                        }
                    },
                }
                self.resolve_place(place, binding)?
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                match instance {
                    None if !type_arguments.is_empty() => {
                        return Err(hir_error(
                            "monomorphic resolved call carries generic type arguments",
                        ));
                    }
                    Some(instance)
                        if FunctionInstanceId::derive(callee, type_arguments) != *instance =>
                    {
                        return Err(hir_error(
                            "resolved call instance disagrees with its template and arguments",
                        ));
                    }
                    Some(_) if type_arguments.is_empty() => {
                        return Err(hir_error(
                            "generic resolved call has no concrete type arguments",
                        ));
                    }
                    None | Some(_) => {}
                }
                for argument in type_arguments {
                    if !matches!(argument, ResolvedType::I64 | ResolvedType::Bool) {
                        return Err(hir_error(
                            "resolved call has a non-scalar generic type argument",
                        ));
                    }
                }
                let target = self
                    .program
                    .resolve_call_target(callee, instance.as_ref())
                    .ok_or_else(|| {
                        hir_error(format!("resolved callee `{callee}` is not indexed"))
                    })?;
                if args.len() != target.params.len() {
                    return Err(hir_error(format!(
                        "call to `{callee}` has {} arguments but expects {}",
                        args.len(),
                        target.params.len()
                    )));
                }
                let params = target.params.clone();
                let return_type = target.return_type.clone();
                let target_effects = target.effects.clone();
                match allowed_effects {
                    Some(allowed) => {
                        for effect in &target_effects {
                            if !allowed.contains(effect) {
                                return Err(hir_error(format!(
                                    "call to `{callee}` requires undeclared effect `{effect}`"
                                )));
                            }
                        }
                    }
                    None if !target_effects.is_empty() => {
                        return Err(hir_error(format!(
                            "contract calls effectful function `{callee}`"
                        )));
                    }
                    None => {}
                }
                for (index, (argument, param)) in args.iter().zip(&params).enumerate() {
                    self.validate_expr(
                        function,
                        argument,
                        scope,
                        &format!("{path}.arg.{index}"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.require_type(&argument.ty, &param.ty, "call argument")?;
                    self.validate_argument_ownership(argument.ownership, param)?;
                    if self.argument_transfers(param)? {
                        if !allow_moves {
                            return Err(hir_error(format!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        self.mark_value_sources_moved(argument, scope)?;
                    }
                }
                let ownership = self.expected_ownership(&return_type, OwnershipMode::Own)?;
                (return_type, ownership)
            }
            ResolvedExprKind::Unary { op, value } => {
                self.validate_expr(
                    function,
                    value,
                    scope,
                    &format!("{path}.value"),
                    allow_moves,
                    allowed_effects,
                )?;
                let expected = match op {
                    UnaryOp::Neg => ResolvedType::I64,
                    UnaryOp::Not => ResolvedType::Bool,
                };
                self.require_type(&value.ty, &expected, "unary operand")?;
                (expected, OwnershipMode::Value)
            }
            ResolvedExprKind::Binary { op, left, right } => {
                self.validate_expr(
                    function,
                    left,
                    scope,
                    &format!("{path}.left"),
                    allow_moves,
                    allowed_effects,
                )?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let baseline_ids = scope.keys().cloned().collect::<Vec<_>>();
                    let mut conditional_scope = scope.clone();
                    self.validate_expr(
                        function,
                        right,
                        &mut conditional_scope,
                        &format!("{path}.right"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    Self::join_conditional(scope, &conditional_scope, &baseline_ids);
                } else {
                    self.validate_expr(
                        function,
                        right,
                        scope,
                        &format!("{path}.right"),
                        allow_moves,
                        allowed_effects,
                    )?;
                }
                let output = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => {
                        self.require_type(&left.ty, &ResolvedType::I64, "binary operand")?;
                        self.require_type(&right.ty, &ResolvedType::I64, "binary operand")?;
                        ResolvedType::I64
                    }
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        self.require_type(&left.ty, &ResolvedType::I64, "comparison operand")?;
                        self.require_type(&right.ty, &ResolvedType::I64, "comparison operand")?;
                        ResolvedType::Bool
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        self.require_type(&left.ty, &ResolvedType::Bool, "boolean operand")?;
                        self.require_type(&right.ty, &ResolvedType::Bool, "boolean operand")?;
                        ResolvedType::Bool
                    }
                    BinaryOp::Eq | BinaryOp::Ne => {
                        self.require_type(&left.ty, &right.ty, "equality operands")?;
                        ResolvedType::Bool
                    }
                };
                (output, OwnershipMode::Value)
            }
            ResolvedExprKind::Block { statements, tail } => {
                let mut block_scope = scope.clone();
                for (index, statement) in statements.iter().enumerate() {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let statement_path = format!("{path}.s{index}");
                            self.validate_expr(
                                function,
                                value,
                                &mut block_scope,
                                &format!("{statement_path}.value"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if binding.id != ValueId::local(function, &statement_path) {
                                return Err(hir_error(format!(
                                    "local `{}` has a non-canonical identity",
                                    binding.id
                                )));
                            }
                            self.insert_value(&binding.id)?;
                            self.require_type(&binding.ty, &value.ty, "local binding")?;
                            if binding.ownership != value.ownership {
                                return Err(hir_error(format!(
                                    "local `{}` has inconsistent ownership",
                                    binding.id
                                )));
                            }
                            self.validate_declared_ownership(&binding.ty, binding.ownership)?;
                            if self.is_owned_resource(&binding.ty, binding.ownership)? {
                                if !allow_moves {
                                    return Err(hir_error(
                                        "contract cannot transfer ownership into a local binding",
                                    ));
                                }
                                self.mark_value_sources_moved(value, &mut block_scope)?;
                            }
                            block_scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                    availability: Availability::Available,
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            );
                        }
                    }
                }
                self.validate_expr(
                    function,
                    tail,
                    &mut block_scope,
                    &format!("{path}.tail"),
                    allow_moves,
                    allowed_effects,
                )?;
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                Self::merge_availability(scope, &block_scope, &outer_ids);
                (tail.ty.clone(), tail.ownership)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_expr(
                    function,
                    condition,
                    scope,
                    &format!("{path}.condition"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut then_scope = scope.clone();
                let mut else_scope = scope.clone();
                self.validate_expr(
                    function,
                    then_branch,
                    &mut then_scope,
                    &format!("{path}.then"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.validate_expr(
                    function,
                    else_branch,
                    &mut else_scope,
                    &format!("{path}.else"),
                    allow_moves,
                    allowed_effects,
                )?;
                Self::join_branches(scope, &then_scope, &else_scope, &outer_ids);
                self.require_type(&then_branch.ty, &else_branch.ty, "if branches")?;
                if then_branch.ownership != else_branch.ownership {
                    return Err(hir_error("if branches have inconsistent ownership"));
                }
                (then_branch.ty.clone(), then_branch.ownership)
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let declaration = self
                    .program
                    .declarations
                    .declaration(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` is not indexed")))?;
                if declaration.kind != DeclarationKind::Record {
                    return Err(hir_error(format!(
                        "constructor target `{record}` is not a record"
                    )));
                }
                let expected_fields = self
                    .program
                    .declarations
                    .record_fields(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?
                    .to_vec();
                let ResolvedType::Nominal {
                    declaration: instance_record,
                    arguments,
                } = &expression.ty
                else {
                    return Err(hir_error("record constructor result is not nominal"));
                };
                let parameters = self
                    .program
                    .declarations
                    .type_parameters(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no parameters")))?;
                if instance_record != record
                    || arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(format!(
                        "constructor for `{record}` has an invalid concrete instance"
                    )));
                }
                let mut seen = BTreeSet::new();
                for (index, initializer) in fields.iter().enumerate() {
                    let field = expected_fields
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "constructor for `{record}` contains foreign field `{}`",
                                initializer.field
                            ))
                        })?;
                    if !seen.insert(initializer.field.clone()) {
                        return Err(hir_error(format!(
                            "constructor for `{record}` repeats field `{}`",
                            initializer.field
                        )));
                    }
                    self.validate_expr(
                        function,
                        &initializer.value,
                        scope,
                        &format!("{path}.field.{index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let field_ty = substitute_type(&field.ty, record, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record field")?;
                    let expected = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected {
                        return Err(hir_error(format!(
                            "field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, scope)?;
                    }
                }
                if seen.len() != expected_fields.len() {
                    return Err(hir_error(format!(
                        "constructor for `{record}` is missing required fields"
                    )));
                }
                let ty = expression.ty.clone();
                let ownership = self.expected_ownership(&ty, OwnershipMode::Own)?;
                (ty, ownership)
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let ResolvedType::Nominal {
                    declaration: instance_variant,
                    arguments,
                } = &expression.ty
                else {
                    return Err(hir_error("variant constructor has a non-nominal result"));
                };
                if instance_variant != variant {
                    return Err(hir_error(
                        "variant constructor result disagrees with its declaration",
                    ));
                }
                let declaration = self
                    .program
                    .declarations
                    .declaration(variant)
                    .ok_or_else(|| hir_error(format!("variant `{variant}` is not indexed")))?;
                if declaration.kind != DeclarationKind::Variant {
                    return Err(hir_error(format!(
                        "constructor target `{variant}` is not a variant"
                    )));
                }
                let declared_case = self
                    .program
                    .declarations
                    .variant_cases(variant)
                    .and_then(|cases| cases.iter().find(|item| item.id == *case))
                    .ok_or_else(|| {
                        hir_error(format!(
                            "constructor for `{variant}` contains foreign case `{case}`"
                        ))
                    })?;
                let expected_fields = declared_case.fields.clone();
                let mut seen = BTreeSet::new();
                for (index, initializer) in fields.iter().enumerate() {
                    let field = expected_fields
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "constructor for `{case}` contains foreign field `{}`",
                                initializer.field
                            ))
                        })?;
                    if !seen.insert(initializer.field.clone()) {
                        return Err(hir_error(format!(
                            "constructor for `{case}` repeats field `{}`",
                            initializer.field
                        )));
                    }
                    self.validate_expr(
                        function,
                        &initializer.value,
                        scope,
                        &format!("{path}.field.{index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let field_ty = substitute_type(&field.ty, variant, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "variant payload field")?;
                    if initializer.value.ownership != OwnershipMode::Value {
                        return Err(hir_error(format!(
                            "variant payload field `{}` is not a Copy value",
                            initializer.field
                        )));
                    }
                }
                if seen.len() != expected_fields.len() {
                    return Err(hir_error(format!(
                        "constructor for `{case}` is missing required payload fields"
                    )));
                }
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                self.validate_expr(
                    function,
                    scrutinee,
                    scope,
                    &format!("{path}.scrutinee"),
                    allow_moves,
                    allowed_effects,
                )?;
                let ResolvedType::Nominal {
                    declaration: matched_type,
                    arguments,
                } = &scrutinee.ty
                else {
                    return Err(hir_error("resolved match scrutinee is not nominal"));
                };
                let matched_kind = self
                    .program
                    .declarations
                    .declaration(matched_type)
                    .map(|item| item.kind);
                if matched_kind == Some(DeclarationKind::Record) {
                    if scrutinee.ownership != OwnershipMode::Value {
                        return Err(hir_error("resolved record match scrutinee is not Copy"));
                    }
                    let [arm] = arms.as_slice() else {
                        return Err(hir_error(
                            "resolved irrefutable record match must have exactly one arm",
                        ));
                    };
                    let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                    let mut arm_scope = scope.clone();
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => self.validate_record_match_pattern(
                            function,
                            &scrutinee.ty,
                            record,
                            instance,
                            fields,
                            &mut arm_scope,
                            &format!("{path}.arm.0.record"),
                        )?,
                        ResolvedMatchPattern::Variant { .. } => {
                            return Err(hir_error(
                                "resolved variant pattern has a record scrutinee",
                            ));
                        }
                    }
                    self.validate_expr(
                        function,
                        &arm.value,
                        &mut arm_scope,
                        &format!("{path}.arm.0.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    if !matches!(arm.value.ty, ResolvedType::I64 | ResolvedType::Bool) {
                        return Err(hir_error(
                            "resolved record match arm must produce i64 or bool",
                        ));
                    }
                    for id in outer_ids {
                        if let Some(state) = arm_scope.get(&id) {
                            scope.insert(id, state.clone());
                        }
                    }
                    self.require_type(&expression.ty, &arm.value.ty, "record match expression")?;
                    if expression.ownership != arm.value.ownership {
                        return Err(hir_error(
                            "resolved record match expression has inconsistent ownership",
                        ));
                    }
                    return Ok(());
                }
                let variant = matched_type;
                if scrutinee.ownership != OwnershipMode::Value
                    || self
                        .program
                        .declarations
                        .declaration(variant)
                        .is_none_or(|item| item.kind != DeclarationKind::Variant)
                {
                    return Err(hir_error(
                        "resolved match scrutinee is not a concrete Copy variant",
                    ));
                }
                let cases = self
                    .program
                    .declarations
                    .variant_cases(variant)
                    .ok_or_else(|| hir_error(format!("variant `{variant}` has no cases")))?
                    .to_vec();
                if arms.is_empty() {
                    return Err(hir_error("resolved match has no arms"));
                }
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut arm_scopes = Vec::with_capacity(arms.len());
                let mut covered = BTreeSet::new();
                let mut wildcard_seen = false;
                let mut result = None::<(ResolvedType, OwnershipMode)>;
                for (arm_index, arm) in arms.iter().enumerate() {
                    let mut arm_scope = scope.clone();
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {
                            if wildcard_seen || covered.len() == cases.len() {
                                return Err(hir_error(
                                    "resolved match has an unreachable wildcard",
                                ));
                            }
                            wildcard_seen = true;
                        }
                        ResolvedMatchPattern::Variant {
                            variant: pattern_variant,
                            case,
                            fields,
                        } => {
                            if wildcard_seen
                                || pattern_variant != variant
                                || !covered.insert(case.clone())
                            {
                                return Err(hir_error(
                                    "resolved match has an unreachable or foreign case pattern",
                                ));
                            }
                            let declared_case =
                                cases.iter().find(|item| item.id == *case).ok_or_else(|| {
                                    hir_error(format!(
                                        "resolved match references foreign case `{case}`"
                                    ))
                                })?;
                            let mut seen_fields = BTreeSet::new();
                            for (field_index, pattern_field) in fields.iter().enumerate() {
                                let declared_field = declared_case
                                    .fields
                                    .iter()
                                    .find(|item| item.id == pattern_field.field)
                                    .ok_or_else(|| {
                                        hir_error(format!(
                                            "resolved pattern contains foreign field `{}`",
                                            pattern_field.field
                                        ))
                                    })?;
                                let binding_ty =
                                    substitute_type(&declared_field.ty, variant, arguments)?;
                                if !seen_fields.insert(pattern_field.field.clone())
                                    || pattern_field.binding.id
                                        != ValueId::local(
                                            function,
                                            &format!(
                                                "{path}.arm.{arm_index}.binding.{field_index}"
                                            ),
                                        )
                                    || pattern_field.binding.ty != binding_ty
                                    || pattern_field.binding.ownership != OwnershipMode::Value
                                {
                                    return Err(hir_error(
                                        "resolved match pattern field or binding is invalid",
                                    ));
                                }
                                self.insert_value(&pattern_field.binding.id)?;
                                self.validate_type(&pattern_field.binding.ty)?;
                                if arm_scope.contains_key(&pattern_field.binding.id) {
                                    return Err(hir_error(
                                        "resolved match pattern binding shadows an existing value",
                                    ));
                                }
                                arm_scope.insert(
                                    pattern_field.binding.id.clone(),
                                    ValidationBinding {
                                        ty: pattern_field.binding.ty.clone(),
                                        ownership: OwnershipMode::Value,
                                        availability: Availability::Available,
                                        moved_places: BTreeMap::new(),
                                        definitely_partial: BTreeSet::new(),
                                    },
                                );
                            }
                            if seen_fields.len() != declared_case.fields.len() {
                                return Err(hir_error(
                                    "resolved match pattern is missing payload fields",
                                ));
                            }
                        }
                        ResolvedMatchPattern::Record { .. } => {
                            return Err(hir_error(
                                "resolved record pattern has a variant scrutinee",
                            ));
                        }
                    }
                    self.validate_expr(
                        function,
                        &arm.value,
                        &mut arm_scope,
                        &format!("{path}.arm.{arm_index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    if let Some((expected_ty, expected_ownership)) = &result {
                        self.require_type(&arm.value.ty, expected_ty, "match arm")?;
                        if arm.value.ownership != *expected_ownership {
                            return Err(hir_error(
                                "resolved match arms have inconsistent ownership",
                            ));
                        }
                    } else {
                        result = Some((arm.value.ty.clone(), arm.value.ownership));
                    }
                    arm_scopes.push(arm_scope);
                }
                if !wildcard_seen && covered.len() != cases.len() {
                    return Err(hir_error("resolved match is not exhaustive"));
                }
                if let Some((first, rest)) = arm_scopes.split_first() {
                    let mut joined = first.clone();
                    for arm_scope in rest {
                        Self::join_conditional(&mut joined, arm_scope, &outer_ids);
                    }
                    Self::merge_availability(scope, &joined, &outer_ids);
                }
                result.ok_or_else(|| hir_error("resolved match has no result"))?
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                self.validate_expr(
                    function,
                    operand,
                    scope,
                    &format!("{path}.operand"),
                    allow_moves,
                    allowed_effects,
                )?;
                if !path.starts_with("body") {
                    return Err(hir_error(
                        "resolved `?` is outside the executable function body",
                    ));
                }
                if scope.values().any(|binding| {
                    self.program
                        .declarations
                        .type_facts(&binding.ty)
                        .is_some_and(|facts| facts.contains_resource)
                }) {
                    return Err(hir_error(
                        "resolved `?` has a live resource binding in the bounded Copy-only profile",
                    ));
                }
                if result.as_str() != crate::prelude::RESULT_ID
                    || ok_case.as_str() != crate::prelude::RESULT_OK_ID
                    || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
                    || err_case.as_str() != crate::prelude::RESULT_ERR_ID
                    || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
                {
                    return Err(hir_error(
                        "resolved `?` does not authenticate the compiler-owned Result shape",
                    ));
                }
                let ResolvedType::Nominal {
                    declaration: operand_result,
                    arguments: operand_arguments,
                } = &operand.ty
                else {
                    return Err(hir_error("resolved `?` operand is not nominal Result"));
                };
                let ResolvedType::Nominal {
                    declaration: residual_result,
                    arguments: residual_arguments,
                } = residual_type
                else {
                    return Err(hir_error("resolved `?` residual is not nominal Result"));
                };
                if operand_result != result
                    || residual_result != result
                    || operand_arguments.len() != 2
                    || residual_arguments.len() != 2
                    || operand_arguments
                        .iter()
                        .chain(residual_arguments)
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(
                        "resolved `?` has invalid concrete Result instances",
                    ));
                }
                let enclosing_return = self
                    .execution_function(function)
                    .map(|candidate| &candidate.return_type)
                    .ok_or_else(|| hir_error("resolved `?` has no enclosing function"))?;
                self.require_type(residual_type, enclosing_return, "`?` residual")?;
                self.require_type(&expression.ty, &operand_arguments[0], "`?` success value")?;
                self.require_type(
                    &operand_arguments[1],
                    &residual_arguments[1],
                    "`?` residual error",
                )?;
                if expression.ownership != OwnershipMode::Value {
                    return Err(hir_error("resolved `?` success value is not Copy"));
                }
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                self.validate_expr(
                    function,
                    operand,
                    scope,
                    &format!("{path}.operand"),
                    allow_moves,
                    allowed_effects,
                )?;
                if !path.starts_with("body") {
                    return Err(hir_error(
                        "resolved Option `?` is outside the executable function body",
                    ));
                }
                if scope.values().any(|binding| {
                    self.program
                        .declarations
                        .type_facts(&binding.ty)
                        .is_some_and(|facts| facts.contains_resource)
                }) {
                    return Err(hir_error(
                        "resolved Option `?` has a live resource binding in the bounded Copy-only profile",
                    ));
                }
                if option.as_str() != crate::prelude::OPTION_ID
                    || some_case.as_str() != crate::prelude::OPTION_SOME_ID
                    || some_field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID
                    || none_case.as_str() != crate::prelude::OPTION_NONE_ID
                {
                    return Err(hir_error(
                        "resolved Option `?` does not authenticate the compiler-owned Option shape",
                    ));
                }
                for id in [option, some_case, some_field, none_case] {
                    if self
                        .program
                        .declarations
                        .declaration(id)
                        .is_none_or(|declaration| {
                            declaration.identity_origin != IdentityOrigin::CompilerOwned
                        })
                    {
                        return Err(hir_error(format!(
                            "resolved Option `?` identity `{id}` is not compiler-owned"
                        )));
                    }
                }
                let ResolvedType::Nominal {
                    declaration: operand_option,
                    arguments: operand_arguments,
                } = &operand.ty
                else {
                    return Err(hir_error(
                        "resolved Option `?` operand is not nominal Option",
                    ));
                };
                let ResolvedType::Nominal {
                    declaration: residual_option,
                    arguments: residual_arguments,
                } = residual_type
                else {
                    return Err(hir_error(
                        "resolved Option `?` residual is not nominal Option",
                    ));
                };
                if operand_option != option
                    || residual_option != option
                    || operand_arguments.len() != 1
                    || residual_arguments.len() != 1
                    || operand_arguments
                        .iter()
                        .chain(residual_arguments)
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(
                        "resolved Option `?` has invalid concrete Option instances",
                    ));
                }
                let enclosing_return = self
                    .execution_function(function)
                    .map(|candidate| &candidate.return_type)
                    .ok_or_else(|| hir_error("resolved Option `?` has no enclosing function"))?;
                self.require_type(residual_type, enclosing_return, "Option `?` residual")?;
                self.require_type(
                    &expression.ty,
                    &operand_arguments[0],
                    "Option `?` success value",
                )?;
                if expression.ownership != OwnershipMode::Value {
                    return Err(hir_error("resolved Option `?` success value is not Copy"));
                }
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                self.validate_expr(
                    function,
                    base,
                    scope,
                    &format!("{path}.base"),
                    allow_moves,
                    allowed_effects,
                )?;
                let declaration = self
                    .program
                    .declarations
                    .declaration(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` is not indexed")))?;
                if declaration.kind != DeclarationKind::Record {
                    return Err(hir_error(format!(
                        "record update target `{record}` is not a record"
                    )));
                }
                let ty = base.ty.clone();
                let ResolvedType::Nominal {
                    declaration: instance_record,
                    arguments,
                } = &ty
                else {
                    return Err(hir_error("record update base is not nominal"));
                };
                let parameters = self
                    .program
                    .declarations
                    .type_parameters(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no parameters")))?;
                if instance_record != record
                    || arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(format!(
                        "record update for `{record}` has an invalid concrete instance"
                    )));
                }
                self.require_type(&base.ty, &ty, "record update base")?;
                let ownership = self.expected_ownership(&ty, OwnershipMode::Own)?;
                if base.ownership != ownership {
                    return Err(hir_error(format!(
                        "record update base for `{record}` has incompatible ownership"
                    )));
                }
                if ownership == OwnershipMode::Own {
                    if !allow_moves {
                        return Err(hir_error(
                            "contract cannot transfer ownership from a record update base",
                        ));
                    }
                    self.mark_value_sources_moved(base, scope)?;
                }

                let expected_fields = self
                    .program
                    .declarations
                    .record_fields(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?
                    .to_vec();
                let mut seen = BTreeSet::new();
                for (index, initializer) in fields.iter().enumerate() {
                    let field = expected_fields
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "update for `{record}` contains foreign field `{}`",
                                initializer.field
                            ))
                        })?;
                    if !seen.insert(initializer.field.clone()) {
                        return Err(hir_error(format!(
                            "update for `{record}` repeats field `{}`",
                            initializer.field
                        )));
                    }
                    self.validate_expr(
                        function,
                        &initializer.value,
                        scope,
                        &format!("{path}.field.{index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let field_ty = substitute_type(&field.ty, record, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record replacement")?;
                    let expected = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected {
                        return Err(hir_error(format!(
                            "replacement field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record replacement",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, scope)?;
                    }
                }
                (ty, ownership)
            }
            ResolvedExprKind::Project { base, field } => {
                if matches!(&base.kind, ResolvedExprKind::Place(_)) {
                    return Err(hir_error(
                        "place field projections must use a resolved place path",
                    ));
                }
                self.validate_expr(
                    function,
                    base,
                    scope,
                    &format!("{path}.base"),
                    allow_moves,
                    allowed_effects,
                )?;
                let projected = self.field_type_for_type(&base.ty, field)?;
                let ownership = self.expected_ownership(&projected, base.ownership)?;
                (projected, ownership)
            }
        };

        self.require_type(&expression.ty, &ty, "expression")?;
        if expression.ownership != ownership {
            return Err(hir_error(format!(
                "expression `{}` has inconsistent ownership",
                expression.id
            )));
        }
        Ok(())
    }

    fn resolve_place(
        &self,
        place: &Place,
        binding: &ValidationBinding,
    ) -> Result<(ResolvedType, OwnershipMode), Diagnostic> {
        let mut ty = binding.ty.clone();
        let mut ownership = binding.ownership;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => {
                    ty = self.field_type_for_type(&ty, field)?;
                    ownership = self.expected_ownership(&ty, ownership)?;
                }
                PlaceProjection::VariantField { .. } => {
                    return Err(hir_error(
                        "variant-field projections are not valid before variant HIR lands",
                    ));
                }
            }
        }
        Ok((ty, ownership))
    }

    fn field_type_for_type(
        &self,
        ty: &ResolvedType,
        field: &DeclarationId,
    ) -> Result<ResolvedType, Diagnostic> {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Err(hir_error(format!(
                "field `{field}` projects from a non-record type"
            )));
        };
        if self
            .program
            .declarations
            .declaration(declaration)
            .is_none_or(|item| item.kind != DeclarationKind::Record)
        {
            return Err(hir_error(format!(
                "field `{field}` projects from a non-record nominal type"
            )));
        }
        let parameters = self
            .program
            .declarations
            .type_parameters(declaration)
            .ok_or_else(|| hir_error(format!("record `{declaration}` has no parameters")))?;
        if arguments.len() != parameters.len()
            || arguments
                .iter()
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        {
            return Err(hir_error(format!(
                "field `{field}` projects from an invalid concrete record instance"
            )));
        }
        let template = self
            .program
            .declarations
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|candidate| candidate.id == *field))
            .ok_or_else(|| {
                hir_error(format!(
                    "field `{field}` does not belong to record `{declaration}`"
                ))
            })?;
        substitute_type(&template.ty, declaration, arguments)
    }

    fn argument_transfers(&self, param: &ResolvedParam) -> Result<bool, Diagnostic> {
        self.is_owned_resource(&param.ty, param.ownership)
    }

    fn is_owned_resource(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<bool, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| !facts.copy && ownership == OwnershipMode::Own)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn mark_value_sources_moved(
        &self,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(), Diagnostic> {
        match &expression.kind {
            ResolvedExprKind::Place(place) => {
                let Some(binding) = scope.get(&place.root) else {
                    // A block result may be backed by a local whose lexical
                    // scope ended after the expression was validated. Its
                    // transfer cannot affect any still-visible root.
                    return Ok(());
                };
                let (place_ty, place_ownership) = self.resolve_place(place, binding)?;
                let should_move = self.is_owned_resource(&place_ty, place_ownership)?
                    && Self::place_availability(binding, &place.projections)
                        == Availability::Available;
                if should_move {
                    let binding = scope.get_mut(&place.root).ok_or_else(|| {
                        hir_error(format!(
                            "resolved value `{}` disappeared during ownership validation",
                            place.root
                        ))
                    })?;
                    if place.projections.is_empty() {
                        binding.availability = Availability::Moved;
                    } else {
                        binding
                            .moved_places
                            .insert(place.projections.clone(), Availability::Moved);
                    }
                }
            }
            ResolvedExprKind::Block { tail, .. } => {
                self.mark_value_sources_moved(tail, scope)?;
            }
            ResolvedExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                let ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut then_scope = scope.clone();
                let mut else_scope = scope.clone();
                self.mark_value_sources_moved(then_branch, &mut then_scope)?;
                self.mark_value_sources_moved(else_branch, &mut else_scope)?;
                Self::join_branches(scope, &then_scope, &else_scope, &ids);
            }
            ResolvedExprKind::Match { arms, .. } => {
                let ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut arm_scopes = Vec::with_capacity(arms.len());
                for arm in arms {
                    let mut arm_scope = scope.clone();
                    self.mark_value_sources_moved(&arm.value, &mut arm_scope)?;
                    arm_scopes.push(arm_scope);
                }
                if let Some((first, rest)) = arm_scopes.split_first() {
                    let mut joined = first.clone();
                    for arm_scope in rest {
                        Self::join_conditional(&mut joined, arm_scope, &ids);
                    }
                    Self::merge_availability(scope, &joined, &ids);
                }
            }
            ResolvedExprKind::Project { base, .. } => {
                self.mark_value_sources_moved(base, scope)?;
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::Call { .. }
            | ResolvedExprKind::Unary { .. }
            | ResolvedExprKind::Binary { .. }
            | ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. } => {}
        }
        Ok(())
    }

    fn merge_availability(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        source: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(source)) = (target.get_mut(id), source.get(id)) {
                target.availability = source.availability;
                target.moved_places.clone_from(&source.moved_places);
                target
                    .definitely_partial
                    .clone_from(&source.definitely_partial);
            }
        }
    }

    fn join_conditional(
        baseline: &mut BTreeMap<ValueId, ValidationBinding>,
        conditional: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(baseline), Some(conditional)) = (baseline.get_mut(id), conditional.get(id))
            {
                let moved_places = Self::join_moved_places(baseline, conditional);
                let definitely_partial = Self::join_definitely_partial(baseline, conditional);
                baseline.availability = baseline.availability.join(conditional.availability);
                baseline.moved_places = moved_places;
                baseline.definitely_partial = definitely_partial;
            }
        }
    }

    fn join_branches(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        then_scope: &BTreeMap<ValueId, ValidationBinding>,
        else_scope: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(then_value), Some(else_value)) =
                (target.get_mut(id), then_scope.get(id), else_scope.get(id))
            {
                target.availability = then_value.availability.join(else_value.availability);
                target.moved_places = Self::join_moved_places(then_value, else_value);
                target.definitely_partial = Self::join_definitely_partial(then_value, else_value);
            }
        }
    }

    fn place_availability(
        binding: &ValidationBinding,
        requested: &[PlaceProjection],
    ) -> Availability {
        if binding.availability != Availability::Available {
            return binding.availability;
        }
        let mut maybe_moved = false;
        for (moved, state) in &binding.moved_places {
            if path_is_prefix(moved, requested) || path_is_prefix(requested, moved) {
                if *state == Availability::Moved {
                    return Availability::Moved;
                }
                maybe_moved = true;
            }
        }
        if binding
            .definitely_partial
            .iter()
            .any(|partial| path_is_prefix(requested, partial))
        {
            return Availability::Moved;
        }
        if maybe_moved {
            Availability::MaybeMoved
        } else {
            Availability::Available
        }
    }

    fn join_moved_places(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeMap<Vec<PlaceProjection>, Availability> {
        left.moved_places
            .keys()
            .chain(right.moved_places.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|path| {
                let left = left
                    .moved_places
                    .get(&path)
                    .copied()
                    .unwrap_or(Availability::Available);
                let right = right
                    .moved_places
                    .get(&path)
                    .copied()
                    .unwrap_or(Availability::Available);
                let state = left.join(right);
                (state != Availability::Available).then_some((path, state))
            })
            .collect()
    }

    fn join_definitely_partial(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeSet<Vec<PlaceProjection>> {
        let mut candidates = BTreeSet::new();
        for path in left
            .moved_places
            .keys()
            .chain(right.moved_places.keys())
            .chain(left.definitely_partial.iter())
            .chain(right.definitely_partial.iter())
        {
            for length in 0..=path.len() {
                candidates.insert(path[..length].to_vec());
            }
        }
        candidates
            .into_iter()
            .filter(|path| {
                Self::place_availability(left, path) == Availability::Moved
                    && Self::place_availability(right, path) == Availability::Moved
            })
            .collect()
    }

    fn validate_type(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        match ty {
            ResolvedType::I64 | ResolvedType::Bool => Ok(()),
            ResolvedType::TypeParameter { .. } => Err(hir_error(
                "uninstantiated type parameters are not valid in executable HIR",
            )),
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                let kind = self
                    .program
                    .declarations
                    .declaration(declaration)
                    .map(|item| item.kind)
                    .filter(|kind| {
                        matches!(
                            kind,
                            DeclarationKind::Resource
                                | DeclarationKind::Record
                                | DeclarationKind::Variant
                        )
                    })
                    .ok_or_else(|| {
                        hir_error(format!(
                            "nominal type `{declaration}` is not a resolved type declaration"
                        ))
                    })?;
                let parameters = self
                    .program
                    .declarations
                    .type_parameters(declaration)
                    .ok_or_else(|| {
                        hir_error(format!("nominal type `{declaration}` has no parameters"))
                    })?;
                if arguments.len() != parameters.len() {
                    return Err(hir_error(format!(
                        "nominal type `{declaration}` has incorrect argument arity"
                    )));
                }
                if !arguments.is_empty()
                    && (!matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                        || arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        }))
                {
                    return Err(hir_error(format!(
                        "nominal type `{declaration}` has unsupported generic arguments"
                    )));
                }
                for argument in arguments {
                    self.validate_type(argument)?;
                }
                self.program.declarations.type_facts(ty).ok_or_else(|| {
                    hir_error(format!(
                        "type `{}` has no semantic facts",
                        ty.identity_key()
                    ))
                })?;
                Ok(())
            }
        }
    }

    fn validate_declared_ownership(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        let facts = self.program.declarations.type_facts(ty).ok_or_else(|| {
            hir_error(format!(
                "type `{}` has no semantic facts",
                ty.identity_key()
            ))
        })?;
        if (facts.copy && ownership != OwnershipMode::Value)
            || (!facts.copy && ownership == OwnershipMode::Value)
        {
            return Err(hir_error(format!(
                "type `{}` has an invalid ownership mode",
                ty.identity_key()
            )));
        }
        Ok(())
    }

    fn validate_argument_ownership(
        &self,
        actual: OwnershipMode,
        param: &ResolvedParam,
    ) -> Result<(), Diagnostic> {
        let facts = self
            .program
            .declarations
            .type_facts(&param.ty)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    param.ty.identity_key()
                ))
            })?;
        let valid = if facts.copy {
            actual == OwnershipMode::Value && param.ownership == OwnershipMode::Value
        } else {
            match param.ownership {
                OwnershipMode::Own => actual == OwnershipMode::Own,
                OwnershipMode::Borrow => true,
                OwnershipMode::Shared => actual == OwnershipMode::Shared,
                OwnershipMode::Value => false,
            }
        };
        if valid {
            Ok(())
        } else {
            Err(hir_error(format!(
                "argument ownership is incompatible with parameter `{}`",
                param.id
            )))
        }
    }

    fn expected_ownership(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy
                }
            })
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(hir_error(format!(
                "{context} has inconsistent resolved types"
            )))
        }
    }

    fn insert_value(&mut self, id: &ValueId) -> Result<(), Diagnostic> {
        reject_nul_identity("resolved value", id.as_str())?;
        if self.value_ids.insert(id.clone()) {
            Ok(())
        } else {
            Err(hir_error(format!(
                "duplicate resolved value identity `{id}`"
            )))
        }
    }
}

fn validate_nul_free_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved entry point", program.entrypoint.as_str())?;

    for (key, declaration) in &program.declarations.declarations {
        reject_nul_identity("declaration index key", key.as_str())?;
        reject_nul_identity(
            declaration_identity_subject(declaration.kind),
            declaration.id.as_str(),
        )?;
        if let Some(owner) = &declaration.owner {
            reject_nul_identity("resolved declaration owner", owner.as_str())?;
        }
    }
    for id in program.declarations.types_by_name.values() {
        reject_nul_identity("resolved type lookup", id.as_str())?;
    }
    for id in program.declarations.functions_by_name.values() {
        reject_nul_identity("resolved function lookup", id.as_str())?;
    }
    for ((owner, _), field) in &program.declarations.fields_by_owner_name {
        reject_nul_identity("resolved field owner lookup", owner.as_str())?;
        reject_nul_identity("resolved field lookup", field.as_str())?;
    }
    for ((owner, _), case) in &program.declarations.cases_by_owner_name {
        reject_nul_identity("resolved variant owner lookup", owner.as_str())?;
        reject_nul_identity("resolved variant case lookup", case.as_str())?;
    }
    for (owner, fields) in &program.declarations.record_fields {
        reject_nul_identity("resolved record-field owner", owner.as_str())?;
        for field in fields {
            reject_nul_identity("resolved field", field.id.as_str())?;
            audit_resolved_type(&field.ty)?;
        }
    }
    for (owner, cases) in &program.declarations.variant_cases {
        reject_nul_identity("resolved variant-case owner", owner.as_str())?;
        for case in cases {
            reject_nul_identity("resolved variant case", case.id.as_str())?;
            for field in &case.fields {
                reject_nul_identity("resolved case field", field.id.as_str())?;
                audit_resolved_type(&field.ty)?;
            }
        }
    }
    for (case, fields) in &program.declarations.case_fields {
        reject_nul_identity("resolved case-field owner", case.as_str())?;
        for field in fields {
            reject_nul_identity("resolved case field", field.id.as_str())?;
            audit_resolved_type(&field.ty)?;
        }
    }
    for (key, import) in &program.declarations.imports_by_key {
        reject_nul_identity("resolved logical import key", key)?;
        reject_nul_identity("resolved import lookup", import.as_str())?;
    }

    for declaration in &program.types {
        let subject = match declaration.kind {
            ResolvedTypeDeclarationKind::Resource { .. } => "resolved resource",
            ResolvedTypeDeclarationKind::Record { .. } => "resolved record",
            ResolvedTypeDeclarationKind::Variant { .. } => "resolved variant",
        };
        reject_nul_identity(subject, declaration.id.as_str())?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                reject_nul_identity("resolved resource lifecycle", drop.id.as_str())?;
                if let ResolvedResourceDropKind::Imported { import, import_key } = &drop.kind {
                    reject_nul_identity("resolved lifecycle import", import.as_str())?;
                    reject_nul_identity("resolved lifecycle logical import key", import_key)?;
                }
            }
            ResolvedTypeDeclarationKind::Record { fields } => {
                for field in fields {
                    reject_nul_identity("resolved field", field.id.as_str())?;
                    audit_resolved_type(&field.ty)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    reject_nul_identity("resolved variant case", case.id.as_str())?;
                    for field in &case.fields {
                        reject_nul_identity("resolved case field", field.id.as_str())?;
                        audit_resolved_type(&field.ty)?;
                    }
                }
            }
        }
    }
    for interface in &program.interfaces {
        reject_nul_identity("resolved interface", interface.id.as_str())?;
        for import in &interface.imports {
            reject_nul_identity("resolved import", import.id.as_str())?;
            reject_nul_identity("resolved import owner", import.interface.as_str())?;
            reject_nul_identity("resolved logical import key", &import.import_key)?;
            for parameter in &import.parameters {
                audit_resolved_type(&parameter.ty)?;
            }
        }
    }
    for function in &program.functions {
        reject_nul_identity("resolved function", function.id.as_str())?;
        for parameter in &function.params {
            reject_nul_identity("resolved value", parameter.id.as_str())?;
            audit_resolved_type(&parameter.ty)?;
        }
        reject_nul_identity("resolved value", function.result_id.as_str())?;
        audit_resolved_type(&function.return_type)?;
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            audit_resolved_expression(expression)?;
        }
    }
    Ok(())
}

/// Reject target-neutral attached metadata containing identities that cannot
/// cross C-string-backed backend and trace boundaries losslessly.
///
/// This is intentionally narrower than semantic inventory/plan validation so
/// independent replayers can call it without trusting either canonical builder.
pub(crate) fn validate_attached_identity_references(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    for function in &program.functions {
        audit_cleanup_inventory(&function.cleanup)?;
        audit_cleanup_plan(&function.cleanup_plan)?;
    }
    Ok(())
}

fn audit_resolved_type(root: &ResolvedType) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::I64 | ResolvedType::Bool => {}
            ResolvedType::TypeParameter { owner, .. } => {
                reject_nul_identity("resolved type-parameter owner", owner.as_str())?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                reject_nul_identity("resolved nominal type", declaration.as_str())?;
                pending.extend(arguments);
            }
        }
    }
    Ok(())
}

fn audit_resolved_record_match_pattern(
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved record match", record.as_str())?;
    audit_resolved_type(instance)?;
    for field in fields {
        reject_nul_identity("resolved record match field", field.field.as_str())?;
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                reject_nul_identity("resolved record match binding", binding.id.as_str())?;
                audit_resolved_type(&binding.ty)?;
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
            ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => audit_resolved_record_match_pattern(record, instance, fields)?,
        }
    }
    Ok(())
}

fn audit_resolved_expression(root: &ResolvedExpr) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        reject_nul_identity("resolved expression", expression.id.as_str())?;
        audit_resolved_type(&expression.ty)?;
        match &expression.kind {
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place) => audit_hir_place(place)?,
            ResolvedExprKind::Call { callee, args, .. } => {
                reject_nul_identity("resolved call target", callee.as_str())?;
                pending.extend(args);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    reject_nul_identity("resolved value", binding.id.as_str())?;
                    audit_resolved_type(&binding.ty)?;
                    pending.push(value);
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                reject_nul_identity("resolved record constructor", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record initializer field", field.field.as_str())?;
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                reject_nul_identity("resolved variant constructor", variant.as_str())?;
                reject_nul_identity("resolved variant case", case.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved case initializer field", field.field.as_str())?;
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                for arm in arms.iter().rev() {
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            reject_nul_identity("resolved match variant", variant.as_str())?;
                            reject_nul_identity("resolved match case", case.as_str())?;
                            for field in fields {
                                reject_nul_identity("resolved match field", field.field.as_str())?;
                                reject_nul_identity(
                                    "resolved match binding",
                                    field.binding.id.as_str(),
                                )?;
                                audit_resolved_type(&field.binding.ty)?;
                            }
                        }
                        ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => audit_resolved_record_match_pattern(record, instance, fields)?,
                    }
                    pending.push(&arm.value);
                }
                pending.push(scrutinee);
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                reject_nul_identity("resolved `?` Result", result.as_str())?;
                reject_nul_identity("resolved `?` Ok case", ok_case.as_str())?;
                reject_nul_identity("resolved `?` Ok field", ok_field.as_str())?;
                reject_nul_identity("resolved `?` Err case", err_case.as_str())?;
                reject_nul_identity("resolved `?` Err field", err_field.as_str())?;
                audit_resolved_type(residual_type)?;
                pending.push(operand);
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                reject_nul_identity("resolved Option `?` Option", option.as_str())?;
                reject_nul_identity("resolved Option `?` Some case", some_case.as_str())?;
                reject_nul_identity("resolved Option `?` Some field", some_field.as_str())?;
                reject_nul_identity("resolved Option `?` None case", none_case.as_str())?;
                audit_resolved_type(residual_type)?;
                pending.push(operand);
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                reject_nul_identity("resolved record update", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record replacement field", field.field.as_str())?;
                    pending.push(&field.value);
                }
                pending.push(base);
            }
            ResolvedExprKind::Project { base, field } => {
                reject_nul_identity("resolved projected field", field.as_str())?;
                pending.push(base);
            }
        }
    }
    Ok(())
}

fn audit_hir_place(place: &Place) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved place root", place.root.as_str())?;
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(field) => {
                reject_nul_identity("resolved place field", field.as_str())?;
            }
            PlaceProjection::VariantField { case, field } => {
                reject_nul_identity("resolved place variant case", case.as_str())?;
                reject_nul_identity("resolved place variant field", field.as_str())?;
            }
        }
    }
    Ok(())
}

fn audit_field_liveness_shape(root: &crate::cleanup::FieldLivenessShape) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(shape) = pending.pop() {
        match shape {
            crate::cleanup::FieldLivenessShape::NoDrop => {}
            crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
                reject_nul_identity("cleanup lifecycle", lifecycle.as_str())?;
            }
            crate::cleanup::FieldLivenessShape::Record {
                declaration,
                fields,
            } => {
                reject_nul_identity("cleanup record", declaration.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("cleanup field", field.field.as_str())?;
                    pending.push(&field.shape);
                }
            }
        }
    }
    Ok(())
}

fn audit_inventory_place(place: &crate::cleanup::CleanupPlace) -> Result<(), Diagnostic> {
    for projection in &place.projections {
        reject_nul_identity("cleanup inventory projection", projection.as_str())?;
    }
    Ok(())
}

fn audit_cleanup_inventory(inventory: &CleanupInventory) -> Result<(), Diagnostic> {
    for slot in &inventory.slots {
        match &slot.origin {
            crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }
            | crate::cleanup::CleanupStorageOrigin::Binding { value }
            | crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value } => {
                reject_nul_identity("cleanup inventory value", value.as_str())?;
            }
            crate::cleanup::CleanupStorageOrigin::Temporary { expression } => {
                reject_nul_identity("cleanup inventory expression", expression.as_str())?;
            }
        }
        audit_resolved_type(&slot.ty)?;
        audit_field_liveness_shape(&slot.shape)?;
    }
    for flag in &inventory.flags {
        audit_inventory_place(&flag.place)?;
        reject_nul_identity("cleanup inventory lifecycle", flag.lifecycle.as_str())?;
    }
    Ok(())
}

fn audit_plan_storage(storage: &crate::cleanup_plan::StorageId) -> Result<(), Diagnostic> {
    match storage {
        crate::cleanup_plan::StorageId::Value(value) => {
            reject_nul_identity("cleanup-plan value storage", value.as_str())?;
        }
        crate::cleanup_plan::StorageId::Temporary(expression) => {
            reject_nul_identity("cleanup-plan temporary storage", expression.as_str())?;
        }
        crate::cleanup_plan::StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => {
            reject_nul_identity("cleanup-plan call-argument call", call.as_str())?;
            reject_nul_identity(
                "cleanup-plan call-argument value",
                value_expression.as_str(),
            )?;
        }
        crate::cleanup_plan::StorageId::ProvisionalResult => {}
    }
    Ok(())
}

fn audit_plan_place(place: &crate::cleanup_plan::CleanupPlace) -> Result<(), Diagnostic> {
    audit_plan_storage(&place.storage)?;
    for projection in &place.projections {
        reject_nul_identity("cleanup-plan projection", projection.as_str())?;
    }
    Ok(())
}

fn audit_status_source(source: &crate::cleanup_plan::StatusSourceId) -> Result<(), Diagnostic> {
    reject_nul_identity("cleanup-plan status expression", source.expression.as_str())
}

fn audit_result_source(
    source: &crate::cleanup_plan::CleanupResultSource,
) -> Result<(), Diagnostic> {
    match source {
        crate::cleanup_plan::CleanupResultSource::Scalar { expression } => {
            reject_nul_identity("cleanup-plan scalar result", expression.as_str())?;
        }
        crate::cleanup_plan::CleanupResultSource::Owned { storage } => {
            audit_plan_place(storage)?;
        }
    }
    Ok(())
}

fn audit_cleanup_plan(plan: &CleanupPlan) -> Result<(), Diagnostic> {
    for place in &plan.entry_state.live_owned_parameters {
        audit_plan_place(place)?;
    }
    for slot in &plan.slots {
        audit_plan_storage(&slot.storage)?;
        audit_resolved_type(&slot.ty)?;
        audit_field_liveness_shape(&slot.field_liveness_shape)?;
    }
    for source in &plan.status_sources {
        audit_status_source(&source.id)?;
        if let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } = &source.producer {
            reject_nul_identity("cleanup-plan propagated callee", callee.as_str())?;
        }
    }
    for block in &plan.blocks {
        for transition in &block.transitions {
            match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { at, destination } => {
                    reject_nul_identity("cleanup-plan initialize expression", at.as_str())?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    reject_nul_identity("cleanup-plan transfer expression", at.as_str())?;
                    audit_plan_place(source)?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments } => {
                    reject_nul_identity("cleanup-plan committed call", call.as_str())?;
                    for argument in arguments {
                        audit_plan_place(&argument.source)?;
                    }
                }
                crate::cleanup_plan::CleanupTransition::SelectFailure { source } => {
                    audit_status_source(source)?;
                }
                crate::cleanup_plan::CleanupTransition::StageCopyResult { source } => {
                    match source {
                        crate::cleanup_plan::StagedCopyResultSource::Body {
                            expression,
                            instance,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged body expression",
                                expression.as_str(),
                            )?;
                            audit_resolved_type(instance)?;
                        }
                        crate::cleanup_plan::StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            source_instance,
                            target_instance,
                            result,
                            ok_case,
                            ok_field,
                            err_case,
                            err_field,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged `?` expression",
                                expression.as_str(),
                            )?;
                            reject_nul_identity(
                                "cleanup-plan staged `?` operand",
                                operand.as_str(),
                            )?;
                            audit_resolved_type(source_instance)?;
                            audit_resolved_type(target_instance)?;
                            for (kind, declaration) in [
                                ("Result", result),
                                ("Ok case", ok_case),
                                ("Ok field", ok_field),
                                ("Err case", err_case),
                                ("Err field", err_field),
                            ] {
                                reject_nul_identity(
                                    &format!("cleanup-plan staged `?` {kind}"),
                                    declaration.as_str(),
                                )?;
                            }
                        }
                        crate::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            source_instance,
                            target_instance,
                            option,
                            some_case,
                            some_field,
                            none_case,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged Option `?` expression",
                                expression.as_str(),
                            )?;
                            reject_nul_identity(
                                "cleanup-plan staged Option `?` operand",
                                operand.as_str(),
                            )?;
                            audit_resolved_type(source_instance)?;
                            audit_resolved_type(target_instance)?;
                            for (kind, declaration) in [
                                ("Option", option),
                                ("Some case", some_case),
                                ("Some field", some_field),
                                ("None case", none_case),
                            ] {
                                reject_nul_identity(
                                    &format!("cleanup-plan staged Option `?` {kind}"),
                                    declaration.as_str(),
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }
    for edge in &plan.edges {
        match &edge.condition {
            crate::cleanup_plan::EdgeCondition::Always => {}
            crate::cleanup_plan::EdgeCondition::BooleanResult(expression, _) => {
                reject_nul_identity("cleanup-plan boolean expression", expression.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::VariantCase {
                scrutinee, case, ..
            } => {
                reject_nul_identity("cleanup-plan match scrutinee", scrutinee.as_str())?;
                reject_nul_identity("cleanup-plan variant case", case.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::StatusZero(source)
            | crate::cleanup_plan::EdgeCondition::StatusNonzero(source) => {
                audit_status_source(source)?;
            }
        }
    }
    for region in &plan.regions {
        for storage in &region.slots {
            audit_plan_storage(storage)?;
        }
    }
    for exit in &plan.exits {
        for finalizer in &exit.finalize_in_order {
            audit_plan_place(&finalizer.source)?;
            reject_nul_identity(
                "cleanup-plan finalizer lifecycle",
                finalizer.lifecycle_id.as_str(),
            )?;
        }
        match &exit.continuation {
            crate::cleanup_plan::ExitContinuation::Continue(_)
            | crate::cleanup_plan::ExitContinuation::ReturnUnit => {}
            crate::cleanup_plan::ExitContinuation::CommitResult { source } => {
                audit_result_source(source)?;
            }
            crate::cleanup_plan::ExitContinuation::ReturnFailure { source } => {
                audit_status_source(source)?;
            }
        }
    }
    Ok(())
}

fn declaration_identity_subject(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Resource => "resolved resource declaration",
        DeclarationKind::ResourceDrop => "resolved resource lifecycle declaration",
        DeclarationKind::Record => "resolved record declaration",
        DeclarationKind::Field => "resolved field declaration",
        DeclarationKind::Variant => "resolved variant declaration",
        DeclarationKind::VariantCase => "resolved variant case declaration",
        DeclarationKind::CaseField => "resolved case field declaration",
        DeclarationKind::Interface => "resolved interface declaration",
        DeclarationKind::Import => "resolved import declaration",
        DeclarationKind::Function => "resolved function declaration",
    }
}

fn reject_nul_identity(subject: &str, value: &str) -> Result<(), Diagnostic> {
    if value.contains('\0') {
        Err(hir_error(format!("{subject} identity contains NUL")))
    } else {
        Ok(())
    }
}

fn path_is_prefix<T: PartialEq>(prefix: &[T], path: &[T]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

fn resolved_lifecycle_effects(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<BTreeSet<String>, Diagnostic> {
    fn collect(
        program: &ResolvedProgram,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        effects: &mut BTreeSet<String>,
    ) -> Result<(), Diagnostic> {
        let Some(id) = ty.nominal_id() else {
            return Ok(());
        };
        if !visiting.insert(id.clone()) {
            return Ok(());
        }
        let declaration = program
            .types
            .iter()
            .find(|item| item.id == *id)
            .ok_or_else(|| hir_error(format!("type `{id}` has no lifecycle declaration")))?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                    let resolved = program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|item| item.id == *import)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "resource `{id}` references missing import `{import}`"
                            ))
                        })?;
                    effects.extend(resolved.effects.iter().cloned());
                }
            }
            ResolvedTypeDeclarationKind::Record { fields } => {
                for field in fields {
                    collect(program, &field.ty, visiting, effects)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        collect(program, &field.ty, visiting, effects)?;
                    }
                }
            }
        }
        visiting.remove(id);
        Ok(())
    }

    let mut effects = BTreeSet::new();
    collect(program, ty, &mut BTreeSet::new(), &mut effects)?;
    Ok(effects)
}

fn visit_resolved_calls(
    expression: &ResolvedExpr,
    visit: &mut impl FnMut(&DeclarationId, Option<&FunctionInstanceId>, &[ResolvedType]),
) {
    match &expression.kind {
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } => {
            visit(callee, instance.as_ref(), type_arguments);
            for arg in args {
                visit_resolved_calls(arg, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. } => visit_resolved_calls(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_resolved_calls(left, visit);
            visit_resolved_calls(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { value, .. } => visit_resolved_calls(value, visit),
                }
            }
            visit_resolved_calls(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_resolved_calls(condition, visit);
            visit_resolved_calls(then_branch, visit);
            visit_resolved_calls(else_branch, visit);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            visit_resolved_calls(scrutinee, visit);
            for arm in arms {
                visit_resolved_calls(&arm.value, visit);
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_resolved_calls(base, visit);
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
    }
}

fn hir_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

struct Resolver<'a> {
    program: &'a Program,
    declarations: DeclarationIndex,
}

impl Resolver<'_> {
    fn resolve(self) -> Result<ResolvedProgram, Diagnostic> {
        let entrypoint = self
            .program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .map(|function| DeclarationId::new(function.stable_id.clone()))
            .ok_or_else(|| {
                self.error(
                    "SPX-H005",
                    "verified program has no resolved entry point",
                    Span::default(),
                )
            })?;
        self.validate_record_layouts()?;
        let types = self
            .program
            .types
            .iter()
            .chain(crate::prelude::declarations())
            .map(|declaration| {
                let id = DeclarationId::new(declaration.stable_id.clone());
                let kind = match &declaration.kind {
                    TypeDeclarationKind::Resource { lifecycles } => {
                        let lifecycle = lifecycles.first().ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("resource `{id}` has no resolved lifecycle"),
                                declaration.span,
                            )
                        })?;
                        let lifecycle_id = DeclarationId::new(
                            lifecycle.stable_id.clone().ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("resource `{id}` lifecycle has no identity"),
                                    lifecycle.span,
                                )
                            })?,
                        );
                        let drop_kind = match &lifecycle.kind {
                            ResourceLifecycleKind::Trivial => ResolvedResourceDropKind::Trivial,
                            ResourceLifecycleKind::Imported { import_key } => {
                                let import = self
                                    .declarations
                                    .import_id(import_key)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            format!(
                                                "resource `{id}` lifecycle references unknown import key `{import_key}`"
                                            ),
                                            lifecycle.span,
                                        )
                                    })?;
                                ResolvedResourceDropKind::Imported {
                                    import,
                                    import_key: import_key.clone(),
                                }
                            }
                        };
                        ResolvedTypeDeclarationKind::Resource {
                            drop: ResolvedResourceDrop {
                                id: lifecycle_id,
                                kind: drop_kind,
                            },
                        }
                    }
                    TypeDeclarationKind::Record { .. } => {
                        let fields = self
                            .declarations
                            .record_fields(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("record `{id}` has no resolved fields"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        ResolvedTypeDeclarationKind::Record { fields }
                    }
                    TypeDeclarationKind::Variant { .. } => {
                        let cases = self
                            .declarations
                            .variant_cases(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("variant `{id}` has no resolved cases"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        ResolvedTypeDeclarationKind::Variant { cases }
                    }
                };
                Ok(ResolvedTypeDeclaration {
                    type_parameters: self
                        .declarations
                        .type_parameters(&id)
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("type `{id}` has no parameter metadata"),
                                declaration.span,
                            )
                        })?
                        .to_vec(),
                    id,
                    name: declaration.name.clone(),
                    kind,
                    span: declaration.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let interfaces = self
            .program
            .interfaces
            .iter()
            .map(|interface| {
                let interface_id = DeclarationId::new(interface.stable_id.clone());
                let imports = interface
                    .imports
                    .iter()
                    .map(|import| {
                        let parameters = import
                            .params
                            .iter()
                            .map(|param| {
                                Ok(ResolvedImportParameter {
                                    name: param.name.clone(),
                                    ty: self.resolve_type(&param.ty, param.span)?,
                                    ownership: param.mode.into(),
                                    consumes_on_failure: param.name == import.consumes,
                                })
                            })
                            .collect::<Result<Vec<_>, Diagnostic>>()?;
                        let failure = match &import.failure {
                            ImportFailure::Infallible => ResolvedImportFailure::Infallible,
                            ImportFailure::Status { domain_id } => ResolvedImportFailure::Status {
                                domain_id: domain_id.clone(),
                                normalization: "semaprax.status.v1",
                            },
                        };
                        Ok(ResolvedImport {
                            id: DeclarationId::new(import.stable_id.clone()),
                            name: import.name.clone(),
                            interface: interface_id.clone(),
                            import_key: import.stable_id.clone(),
                            parameters,
                            result: ResolvedImportResult {
                                kind: ResolvedImportResultKind::Unit,
                                ownership: OwnershipMode::Value,
                                producer: "callee",
                                out_slot_initialization: "success_only",
                                ownership_transfer: "final_zero_status_commit",
                            },
                            effects: import.effects.clone(),
                            required_authority: import.effects.clone(),
                            failure,
                            span: import.span,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                Ok(ResolvedInterface {
                    id: interface_id,
                    name: interface.name.clone(),
                    permits: interface.permits.clone(),
                    imports,
                    span: interface.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let functions = self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
            .map(|function| self.resolve_function(function))
            .collect::<Result<_, _>>()?;
        let function_templates = self
            .program
            .functions
            .iter()
            .filter(|function| !function.type_parameters.is_empty())
            .map(|function| self.resolve_function_template(function))
            .collect::<Result<_, _>>()?;
        let function_instances = self.discover_function_instances()?;
        let mut resolved = ResolvedProgram {
            module: self.program.module.clone(),
            permits: self.program.permits.clone(),
            entrypoint,
            declarations: self.declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        };
        let inventories = resolved
            .functions
            .iter()
            .map(|function| crate::cleanup::build_inventory(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, inventory) in resolved.functions.iter_mut().zip(inventories) {
            function.cleanup = inventory;
        }
        let instance_inventories = resolved
            .function_instances
            .iter()
            .map(|instance| crate::cleanup::build_inventory(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, inventory) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_inventories)
        {
            instance.function.cleanup = inventory;
        }
        let cleanup_plans = resolved
            .functions
            .iter()
            .map(|function| crate::cleanup_plan::build_plan(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, cleanup_plan) in resolved.functions.iter_mut().zip(cleanup_plans) {
            function.cleanup_plan = cleanup_plan;
        }
        let instance_cleanup_plans = resolved
            .function_instances
            .iter()
            .map(|instance| crate::cleanup_plan::build_plan(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, cleanup_plan) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_cleanup_plans)
        {
            instance.function.cleanup_plan = cleanup_plan;
        }
        validate(&resolved)?;
        Ok(resolved)
    }

    fn validate_record_layouts(&self) -> Result<(), Diagnostic> {
        for declaration in &self.program.types {
            if !matches!(&declaration.kind, TypeDeclarationKind::Record { .. }) {
                continue;
            }
            if !declaration.type_parameters.is_empty() {
                continue;
            }
            let ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(declaration.stable_id.clone()),
                arguments: Vec::new(),
            };
            if self.declarations.type_facts(&ty).is_none() {
                return Err(self.error(
                    "SPX-T217",
                    format!(
                        "record `{}` has an illegal by-value recursive layout",
                        declaration.name
                    ),
                    declaration.span,
                ));
            }
        }
        Ok(())
    }

    fn resolve_function(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let template_id = DeclarationId::new(function.stable_id.clone());
        let function_scope = FunctionExecutionId::Monomorphic(template_id.clone());
        self.resolve_function_in_scope(function, &function_scope, template_id)
    }

    fn resolve_function_template(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunctionTemplate, Diagnostic> {
        let function_id = DeclarationId::new(function.stable_id.clone());
        let function_scope = FunctionExecutionId::Monomorphic(function_id.clone());
        let type_parameters = function
            .type_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                Ok(ResolvedTypeParameterDeclaration {
                    name: parameter.name.clone(),
                    index: u32::try_from(index).map_err(|_| {
                        self.error(
                            "SPX-H006",
                            format!("function `{}` has too many type parameters", function.name),
                            parameter.span,
                        )
                    })?,
                    span: parameter.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_function_type(function, &param.ty, param.span)?;
                let id = ValueId::parameter(&function_scope, index);
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership: OwnershipMode::Value,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership: OwnershipMode::Value,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type =
            self.resolve_function_type(function, &function.return_type, function.span)?;
        let result_id = ValueId::result(&function_scope);
        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_scope,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(&function_scope, &function.body, &bindings, "body")?;
        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: OwnershipMode::Value,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_scope,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(ResolvedFunctionTemplate {
            id: function_id,
            name: function.name.clone(),
            type_parameters,
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            span: function.span,
        })
    }

    fn discover_function_instances(&self) -> Result<Vec<ResolvedFunctionInstance>, Diagnostic> {
        let mut calls = Vec::new();
        for function in self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
        {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                expression.visit_call_instances(&mut |name, arguments, span| {
                    calls.push((name.to_owned(), arguments.to_vec(), span));
                });
            }
        }

        let mut seen = BTreeSet::new();
        let mut instances = Vec::new();
        for (name, source_arguments, span) in calls {
            let Some(template) = self
                .program
                .functions
                .iter()
                .find(|function| function.name == name && !function.type_parameters.is_empty())
            else {
                continue;
            };
            let type_arguments = source_arguments
                .iter()
                .map(|argument| self.resolve_type(argument, span))
                .collect::<Result<Vec<_>, _>>()?;
            let template_id = DeclarationId::new(template.stable_id.clone());
            let id = FunctionInstanceId::derive(&template_id, &type_arguments);
            if !seen.insert(id.clone()) {
                continue;
            }
            let specialized =
                specialize_source_function(template, &source_arguments).ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("generic function `{}` specialization failed", template.name),
                        span,
                    )
                })?;
            let execution = FunctionExecutionId::Generic(id.clone());
            let function =
                self.resolve_function_in_scope(&specialized, &execution, template_id.clone())?;
            instances.push(ResolvedFunctionInstance {
                id,
                template: template_id,
                type_arguments,
                function,
            });
        }
        Ok(instances)
    }

    fn resolve_function_in_scope(
        &self,
        function: &crate::ast::Function,
        function_scope: &FunctionExecutionId,
        function_id: DeclarationId,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_type(&param.ty, param.span)?;
                let id = ValueId::parameter(function_scope, index);
                let ownership = param.mode.into();
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type = self.resolve_type(&function.return_type, function.span)?;
        let result_id = ValueId::result(function_scope);

        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    function_scope,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(function_scope, &function.body, &bindings, "body")?;

        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: self.expression_ownership(
                    &return_type,
                    OwnershipMode::Own,
                    function.span,
                )?,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    function_scope,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;

        Ok(ResolvedFunction {
            id: function_id,
            name: function.name.clone(),
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            cleanup: CleanupInventory::unresolved(),
            cleanup_plan: CleanupPlan::unresolved(),
            span: function.span,
        })
    }

    fn resolve_type(&self, ty: &Type, span: Span) -> Result<ResolvedType, Diagnostic> {
        match ty {
            Type::I64 => Ok(ResolvedType::I64),
            Type::Bool => Ok(ResolvedType::Bool),
            Type::Named { name, arguments } => {
                let declaration = self.declarations.type_id(name).cloned().ok_or_else(|| {
                    self.error("SPX-H001", format!("unresolved type `{name}`"), span)
                })?;
                let arguments = arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, span))
                    .collect::<Result<Vec<_>, _>>()?;
                let parameters =
                    self.declarations
                        .type_parameters(&declaration)
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("type `{declaration}` has no parameter metadata"),
                                span,
                            )
                        })?;
                if arguments.len() != parameters.len()
                    || (!arguments.is_empty()
                        && arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        }))
                {
                    return Err(self.error(
                        "SPX-H006",
                        format!("type `{declaration}` has invalid concrete arguments"),
                        span,
                    ));
                }
                Ok(ResolvedType::Nominal {
                    declaration,
                    arguments,
                })
            }
        }
    }

    fn resolve_function_type(
        &self,
        function: &crate::ast::Function,
        ty: &Type,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        if let Type::Named { name, arguments } = ty {
            if arguments.is_empty() {
                if let Some(index) = function
                    .type_parameters
                    .iter()
                    .position(|parameter| parameter.name == *name)
                {
                    return Ok(ResolvedType::TypeParameter {
                        owner: DeclarationId::new(function.stable_id.clone()),
                        index: u32::try_from(index).map_err(|_| {
                            self.error(
                                "SPX-H006",
                                format!(
                                    "function `{}` type parameter index does not fit u32",
                                    function.name
                                ),
                                span,
                            )
                        })?,
                    });
                }
            }
        }
        self.resolve_type(ty, span)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_record_match_pattern(
        &self,
        function: &FunctionExecutionId,
        expected: &ResolvedType,
        type_name: &str,
        fields: &[crate::ast::RecordMatchPatternField],
        bindings: &mut BTreeMap<String, Binding>,
        path: &str,
        span: Span,
    ) -> Result<ResolvedMatchPattern, Diagnostic> {
        let ResolvedType::Nominal {
            declaration: record,
            arguments,
        } = expected
        else {
            return Err(self.error(
                "SPX-H001",
                "record pattern has a non-record concrete instance",
                span,
            ));
        };
        let named_record = self.declarations.type_id(type_name);
        if named_record != Some(record)
            || self
                .declarations
                .declaration(record)
                .is_none_or(|item| item.kind != DeclarationKind::Record)
        {
            return Err(self.error(
                "SPX-H001",
                format!("record pattern `{type_name}` does not match `{record}`"),
                span,
            ));
        }
        let templates = self
            .declarations
            .record_fields(record)
            .ok_or_else(|| self.error("SPX-H006", "record pattern has no fields", span))?;
        let mut resolved_fields = Vec::with_capacity(fields.len());
        for (field_index, field) in fields.iter().enumerate() {
            let field_id = self
                .declarations
                .field_id(record, &field.name)
                .cloned()
                .ok_or_else(|| {
                    self.error(
                        "SPX-H001",
                        format!("unresolved record pattern field `{record}.{}`", field.name),
                        field.span,
                    )
                })?;
            let template = templates
                .iter()
                .find(|candidate| candidate.id == field_id)
                .ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("record pattern field `{field_id}` has no template"),
                        field.span,
                    )
                })?;
            let field_ty = substitute_type(&template.ty, record, arguments)?;
            let field_path = format!("{path}.field.{field_index}");
            let pattern = match &field.pattern {
                crate::ast::RecordMatchFieldPattern::Binding { name, span } => {
                    let binding = ResolvedBinding {
                        id: ValueId::local(function, &format!("{field_path}.binding")),
                        name: name.clone(),
                        ownership: OwnershipMode::Value,
                        ty: field_ty.clone(),
                        span: *span,
                    };
                    bindings.insert(
                        name.clone(),
                        Binding {
                            id: binding.id.clone(),
                            ty: field_ty,
                            ownership: OwnershipMode::Value,
                        },
                    );
                    ResolvedRecordMatchFieldPattern::Binding(binding)
                }
                crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {
                    ResolvedRecordMatchFieldPattern::Wildcard
                }
                crate::ast::RecordMatchFieldPattern::Record {
                    type_name,
                    fields,
                    span,
                    ..
                } => {
                    let ResolvedMatchPattern::Record {
                        record,
                        instance,
                        fields,
                    } = self.resolve_record_match_pattern(
                        function,
                        &field_ty,
                        type_name,
                        fields,
                        bindings,
                        &format!("{field_path}.record"),
                        *span,
                    )?
                    else {
                        unreachable!("record resolver returns a record pattern");
                    };
                    ResolvedRecordMatchFieldPattern::Record {
                        record,
                        instance,
                        fields,
                    }
                }
            };
            resolved_fields.push(ResolvedRecordMatchPatternField {
                field: field_id,
                pattern,
            });
        }
        Ok(ResolvedMatchPattern::Record {
            record: record.clone(),
            instance: expected.clone(),
            fields: resolved_fields,
        })
    }

    fn resolve_expr(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let id = ExpressionId::new(function, path);
        let (kind, ty, ownership) = match &expr.kind {
            ExprKind::Int(value) => (
                ResolvedExprKind::Int(*value),
                ResolvedType::I64,
                OwnershipMode::Value,
            ),
            ExprKind::Bool(value) => (
                ResolvedExprKind::Bool(*value),
                ResolvedType::Bool,
                OwnershipMode::Value,
            ),
            ExprKind::Var(name) => {
                let binding = bindings.get(name).ok_or_else(|| {
                    self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
                })?;
                (
                    ResolvedExprKind::Place(Place {
                        root: binding.id.clone(),
                        projections: Vec::new(),
                    }),
                    binding.ty.clone(),
                    binding.ownership,
                )
            }
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                let template = self
                    .declarations
                    .function_id(name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("unresolved function `{name}`"),
                            expr.span,
                        )
                    })?;
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|function| function.stable_id == template.as_str())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("function identity `{template}` has no declaration"),
                            expr.span,
                        )
                    })?;
                let resolved_arguments = type_arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, expr.span))
                    .collect::<Result<Vec<_>, _>>()?;
                let (callee, instance, return_source_type) = if target.type_parameters.is_empty() {
                    if !resolved_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("monomorphic function `{template}` has type arguments"),
                            expr.span,
                        ));
                    }
                    (template.clone(), None, target.return_type.clone())
                } else {
                    if resolved_arguments.len() != target.type_parameters.len()
                        || resolved_arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        })
                    {
                        return Err(self.error(
                            "SPX-H006",
                            format!("generic function `{template}` has invalid type arguments"),
                            expr.span,
                        ));
                    }
                    let instance = FunctionInstanceId::derive(&template, &resolved_arguments);
                    let return_type = substitute_source_function_type(
                        target,
                        type_arguments,
                        &target.return_type,
                    )
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("generic function `{template}` return substitution failed"),
                            expr.span,
                        )
                    })?;
                    (template.clone(), Some(instance), return_type)
                };
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        self.resolve_expr(
                            function,
                            argument,
                            bindings,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?;
                let ty = self.resolve_type(&return_source_type, target.span)?;
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, target.span)?;
                (
                    ResolvedExprKind::Call {
                        callee,
                        type_arguments: resolved_arguments,
                        instance,
                        args,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Unary { op, value } => {
                let value =
                    self.resolve_expr(function, value, bindings, &format!("{path}.value"))?;
                let ty = match op {
                    UnaryOp::Neg => ResolvedType::I64,
                    UnaryOp::Not => ResolvedType::Bool,
                };
                (
                    ResolvedExprKind::Unary {
                        op: *op,
                        value: Box::new(value),
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.resolve_expr(function, left, bindings, &format!("{path}.left"))?;
                let right =
                    self.resolve_expr(function, right, bindings, &format!("{path}.right"))?;
                let ty = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => ResolvedType::I64,
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => ResolvedType::Bool,
                };
                (
                    ResolvedExprKind::Binary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Block { statements, tail } => {
                let mut scope = bindings.clone();
                let mut resolved_statements = Vec::with_capacity(statements.len());
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        Statement::Let {
                            name,
                            name_span,
                            value,
                            span,
                        } => {
                            let value = self.resolve_expr(
                                function,
                                value,
                                &scope,
                                &format!("{statement_path}.value"),
                            )?;
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &statement_path),
                                name: name.clone(),
                                ownership: value.ownership,
                                ty: value.ty.clone(),
                                span: *name_span,
                            };
                            scope.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                },
                            );
                            resolved_statements.push(ResolvedStatement::Let {
                                binding,
                                value,
                                span: *span,
                            });
                        }
                    }
                }
                let tail = self.resolve_expr(function, tail, &scope, &format!("{path}.tail"))?;
                let ty = tail.ty.clone();
                let ownership = tail.ownership;
                (
                    ResolvedExprKind::Block {
                        statements: resolved_statements,
                        tail: Box::new(tail),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition =
                    self.resolve_expr(function, condition, bindings, &format!("{path}.condition"))?;
                let then_branch =
                    self.resolve_expr(function, then_branch, bindings, &format!("{path}.then"))?;
                let else_branch =
                    self.resolve_expr(function, else_branch, bindings, &format!("{path}.else"))?;
                let ty = then_branch.ty.clone();
                let ownership = then_branch.ownership;
                (
                    ResolvedExprKind::If {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                fields,
                ..
            } => {
                let record = self
                    .declarations
                    .type_id(type_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved record `{type_name}`"),
                            expr.span,
                        )
                    })?;
                if self
                    .declarations
                    .declaration(&record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a record"),
                        expr.span,
                    ));
                }
                let arguments = type_arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, expr.span))
                    .collect::<Result<Vec<_>, _>>()?;
                let parameters = self.declarations.type_parameters(&record).ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("record `{record}` has no parameter metadata"),
                        expr.span,
                    )
                })?;
                if arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(self.error(
                        "SPX-H006",
                        format!("record `{record}` has invalid concrete arguments"),
                        expr.span,
                    ));
                }
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&record, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved field `{}.{}`", type_name, initializer.name),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = ResolvedType::Nominal {
                    declaration: record.clone(),
                    arguments,
                };
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::ConstructRecord {
                        record,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                case_name,
                fields,
                ..
            } => {
                let variant = self
                    .declarations
                    .type_id(type_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved variant `{type_name}`"),
                            expr.span,
                        )
                    })?;
                if self
                    .declarations
                    .declaration(&variant)
                    .is_none_or(|item| item.kind != DeclarationKind::Variant)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a variant"),
                        expr.span,
                    ));
                }
                let case = self
                    .declarations
                    .case_id(&variant, case_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved case `{type_name}::{case_name}`"),
                            expr.span,
                        )
                    })?;
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&case, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved payload field `{type_name}::{case_name}.{}`",
                                    initializer.name
                                ),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = ResolvedType::Nominal {
                    declaration: variant.clone(),
                    arguments: type_arguments
                        .iter()
                        .map(|argument| self.resolve_type(argument, expr.span))
                        .collect::<Result<Vec<_>, _>>()?,
                };
                (
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields: resolved_fields,
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee =
                    self.resolve_expr(function, scrutinee, bindings, &format!("{path}.scrutinee"))?;
                let ResolvedType::Nominal {
                    declaration: matched_type,
                    arguments,
                } = &scrutinee.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve match on a non-record/non-variant value",
                        expr.span,
                    ));
                };
                let matched_kind = self
                    .declarations
                    .declaration(matched_type)
                    .map(|item| item.kind);
                if !matches!(
                    matched_kind,
                    Some(DeclarationKind::Record | DeclarationKind::Variant)
                ) {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve match on a non-record/non-variant value",
                        expr.span,
                    ));
                }
                let instance_arguments = arguments.clone();
                let matched_type = matched_type.clone();
                let mut resolved_arms = Vec::with_capacity(arms.len());
                for (arm_index, arm) in arms.iter().enumerate() {
                    let mut arm_bindings = bindings.clone();
                    let pattern = match &arm.pattern {
                        MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                        MatchPattern::Variant {
                            case_name, fields, ..
                        } => {
                            if matched_kind != Some(DeclarationKind::Variant) {
                                return Err(self.error(
                                    "SPX-H001",
                                    "variant pattern has a record scrutinee",
                                    arm.span,
                                ));
                            }
                            let case = self
                                .declarations
                                .case_id(&matched_type, case_name)
                                .cloned()
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H001",
                                        format!("unresolved case `{matched_type}::{case_name}`"),
                                        arm.span,
                                    )
                                })?;
                            let mut resolved_fields = Vec::with_capacity(fields.len());
                            for (field_index, field) in fields.iter().enumerate() {
                                let field_id = self
                                    .declarations
                                    .field_id(&case, &field.name)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!(
                                                "unresolved pattern field `{case}.{}`",
                                                field.name
                                            ),
                                            field.span,
                                        )
                                    })?;
                                let field_template = self
                                    .declarations
                                    .case_fields(&case)
                                    .and_then(|items| items.iter().find(|item| item.id == field_id))
                                    .map(|item| item.ty.clone())
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!("pattern field `{field_id}` has no type"),
                                            field.span,
                                        )
                                    })?;
                                let field_ty = substitute_type(
                                    &field_template,
                                    &matched_type,
                                    &instance_arguments,
                                )?;
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding.{field_index}"),
                                    ),
                                    name: field.binding.clone(),
                                    ownership: OwnershipMode::Value,
                                    ty: field_ty.clone(),
                                    span: field.binding_span,
                                };
                                arm_bindings.insert(
                                    field.binding.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: field_ty,
                                        ownership: OwnershipMode::Value,
                                    },
                                );
                                resolved_fields.push(ResolvedMatchPatternField {
                                    field: field_id,
                                    binding,
                                });
                            }
                            ResolvedMatchPattern::Variant {
                                variant: matched_type.clone(),
                                case,
                                fields: resolved_fields,
                            }
                        }
                        MatchPattern::Record {
                            type_name,
                            fields,
                            span,
                            ..
                        } => {
                            if matched_kind != Some(DeclarationKind::Record) {
                                return Err(self.error(
                                    "SPX-H001",
                                    "record pattern has a variant scrutinee",
                                    arm.span,
                                ));
                            }
                            self.resolve_record_match_pattern(
                                function,
                                &scrutinee.ty,
                                type_name,
                                fields,
                                &mut arm_bindings,
                                &format!("{path}.arm.{arm_index}.record"),
                                *span,
                            )?
                        }
                    };
                    let value = self.resolve_expr(
                        function,
                        &arm.value,
                        &arm_bindings,
                        &format!("{path}.arm.{arm_index}.value"),
                    )?;
                    resolved_arms.push(ResolvedMatchArm {
                        pattern,
                        value,
                        span: arm.span,
                    });
                }
                let first = resolved_arms.first().ok_or_else(|| {
                    self.error("SPX-H006", "resolved match has no arms", expr.span)
                })?;
                let ty = first.value.ty.clone();
                let ownership = first.value.ownership;
                (
                    ResolvedExprKind::Match {
                        scrutinee: Box::new(scrutinee),
                        arms: resolved_arms,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Try { operand } => {
                let operand =
                    self.resolve_expr(function, operand, bindings, &format!("{path}.operand"))?;
                let operand_type = operand.ty.clone();
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &operand_type
                else {
                    return Err(self.error(
                        "SPX-H006",
                        "resolved `?` operand is not the ordinary Result",
                        expr.span,
                    ));
                };
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|candidate| {
                        matches!(
                            function,
                            FunctionExecutionId::Monomorphic(declaration)
                                if candidate.stable_id == declaration.as_str()
                        )
                    })
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("resolved `?` has unknown enclosing function `{function}`"),
                            expr.span,
                        )
                    })?;
                let residual_type = self.resolve_type(&target.return_type, target.span)?;
                match (declaration.as_str(), arguments.as_slice()) {
                    (crate::prelude::RESULT_ID, [ok_type, _]) => (
                        ResolvedExprKind::Try {
                            operand: Box::new(operand),
                            result: DeclarationId::new(crate::prelude::RESULT_ID),
                            ok_case: DeclarationId::new(crate::prelude::RESULT_OK_ID),
                            ok_field: DeclarationId::new(crate::prelude::RESULT_OK_VALUE_ID),
                            err_case: DeclarationId::new(crate::prelude::RESULT_ERR_ID),
                            err_field: DeclarationId::new(crate::prelude::RESULT_ERR_ERROR_ID),
                            residual_type,
                        },
                        ok_type.clone(),
                        OwnershipMode::Value,
                    ),
                    (crate::prelude::OPTION_ID, [some_type]) => (
                        ResolvedExprKind::TryOption {
                            operand: Box::new(operand),
                            option: DeclarationId::new(crate::prelude::OPTION_ID),
                            some_case: DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                            some_field: DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID),
                            none_case: DeclarationId::new(crate::prelude::OPTION_NONE_ID),
                            residual_type,
                        },
                        some_type.clone(),
                        OwnershipMode::Value,
                    ),
                    _ => {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved `?` operand is not an ordinary Result or Option",
                            expr.span,
                        ));
                    }
                }
            }
            ExprKind::UpdateRecord { base, fields } => {
                let base = self.resolve_expr(function, base, bindings, &format!("{path}.base"))?;
                let ResolvedType::Nominal {
                    declaration: record,
                    arguments: _,
                } = &base.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve a record update on a non-record value",
                        expr.span,
                    ));
                };
                if self
                    .declarations
                    .declaration(record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve a record update on a non-record value",
                        expr.span,
                    ));
                }
                let record = record.clone();
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&record, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved replacement field `{}.{}`",
                                    record, initializer.name
                                ),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = base.ty.clone();
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::UpdateRecord {
                        base: Box::new(base),
                        record,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Project { base, field, .. } => {
                let base = self.resolve_expr(function, base, bindings, &format!("{path}.base"))?;
                let ResolvedType::Nominal {
                    declaration: record,
                    arguments,
                } = &base.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve field `{field}` on a non-record value"),
                        expr.span,
                    ));
                };
                if self
                    .declarations
                    .declaration(record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve field `{field}` on a non-record value"),
                        expr.span,
                    ));
                }
                let instance_arguments = arguments.clone();
                let field_id = self
                    .declarations
                    .field_id(record, field)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved field `{field}` on record `{record}`"),
                            expr.span,
                        )
                    })?;
                let field_ty = self
                    .declarations
                    .record_fields(record)
                    .and_then(|fields| fields.iter().find(|item| item.id == field_id))
                    .map(|field| field.ty.clone())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("field `{field_id}` has no resolved type"),
                            expr.span,
                        )
                    })?;
                let field_ty = substitute_type(&field_ty, record, &instance_arguments)?;
                let ownership = self.expression_ownership(&field_ty, base.ownership, expr.span)?;
                let kind = match &base.kind {
                    ResolvedExprKind::Place(place) => {
                        let mut place = place.clone();
                        place
                            .projections
                            .push(PlaceProjection::Field(field_id.clone()));
                        ResolvedExprKind::Place(place)
                    }
                    _ => ResolvedExprKind::Project {
                        base: Box::new(base),
                        field: field_id,
                    },
                };
                (kind, field_ty, ownership)
            }
        };
        Ok(ResolvedExpr {
            id,
            ty,
            ownership,
            kind,
            span: expr.span,
        })
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic::error(code, message, span).at_path(&self.program.path)
    }

    fn expression_ownership(
        &self,
        ty: &ResolvedType,
        non_copy_mode: OwnershipMode,
        span: Span,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy_mode
                }
            })
            .ok_or_else(|| {
                self.error(
                    "SPX-H004",
                    format!(
                        "semantic facts are unavailable for type `{}`",
                        ty.identity_key()
                    ),
                    span,
                )
            })
    }
}

#[cfg(test)]
mod record_tests {
    use std::path::Path;

    use super::{validate, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
    use crate::{hir, parse};

    fn record_program() -> hir::ResolvedProgram {
        let source = r#"
module test.hostile_record_hir;
@id("node.type")
record Node {
    @id("node.value")
    value: i64,
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        hir::resolve(&parse(source, Path::new("hostile-record-hir.spx")).unwrap()).unwrap()
    }

    #[cfg(test)]
    mod identity_nul_tests {
        use std::path::Path;

        use super::super::{
            validate, DeclarationId, ExpressionId, ResolvedResourceDropKind,
            ResolvedTypeDeclarationKind, ValueId,
        };
        use crate::{codegen, hir, parse, wasm};

        fn identity_program() -> hir::ResolvedProgram {
            let source = r#"
module test.hostile_identity_nul;
@id("token.type")
resource Token {
    @id("token.drop")
    drop import "host.dispose";
}
@id("pair.type")
record Pair { @id("pair.value") value: i64, }
@id("host.interface")
interface Host permits {} {
    @id("host.dispose")
    import fn dispose(token: own Token) -> unit
        effects {}
        failure infallible
        consumes token always;
}
@id("helper.function")
fn helper(value: i64) -> i64 { value }
@id("pair.make")
fn make_pair(value: i64) -> Pair { Pair { value: value } }
@id("pair.read")
fn read_pair(pair: Pair) -> i64 { pair.value }
@id("pair.read-temporary")
fn read_temporary() -> i64 { Pair { value: 1 }.value }
@id("token.discard")
fn discard(token: own Token) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { helper(1) }
"#;
            hir::resolve(&parse(source, Path::new("hostile-identity-nul.spx")).unwrap()).unwrap()
        }

        fn assert_nul_rejected(program: &hir::ResolvedProgram, kind: &str) {
            let diagnostic = validate(program).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-H006", "wrong code for {kind}");
            assert!(
                diagnostic.message.contains("contains NUL"),
                "wrong diagnostic for {kind}: {}",
                diagnostic.message
            );
        }

        fn function_index(program: &hir::ResolvedProgram, name: &str) -> usize {
            program
                .functions
                .iter()
                .position(|function| function.name == name)
                .unwrap()
        }

        fn tail(expression: &super::super::ResolvedExpr) -> &super::super::ResolvedExpr {
            match &expression.kind {
                super::super::ResolvedExprKind::Block { tail, .. } => tail,
                _ => expression,
            }
        }

        fn tail_mut(
            expression: &mut super::super::ResolvedExpr,
        ) -> &mut super::super::ResolvedExpr {
            if matches!(
                &expression.kind,
                super::super::ResolvedExprKind::Block { .. }
            ) {
                let super::super::ResolvedExprKind::Block { tail, .. } = &mut expression.kind
                else {
                    unreachable!()
                };
                tail
            } else {
                expression
            }
        }

        #[test]
        fn validator_rejects_nul_in_every_persistent_hir_identity_carrier() {
            let original = identity_program();

            let mut program = original.clone();
            program.entrypoint = DeclarationId::new("app.main\0forged");
            assert_nul_rejected(&program, "entry point");

            let mut program = original.clone();
            program.types[0].id = DeclarationId::new("token.type\0forged");
            assert_nul_rejected(&program, "resource");

            let mut program = original.clone();
            let ResolvedTypeDeclarationKind::Resource { drop } = &mut program.types[0].kind else {
                panic!("Token must be a resource")
            };
            drop.id = DeclarationId::new("token.drop\0forged");
            assert_nul_rejected(&program, "resource lifecycle");

            let mut program = original.clone();
            let record = program
                .types
                .iter_mut()
                .find(|declaration| declaration.name == "Pair")
                .unwrap();
            record.id = DeclarationId::new("pair.type\0forged");
            assert_nul_rejected(&program, "record");

            let mut program = original.clone();
            let record = program
                .types
                .iter_mut()
                .find(|declaration| declaration.name == "Pair")
                .unwrap();
            let ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
                panic!("Pair must be a record")
            };
            fields[0].id = DeclarationId::new("pair.value\0forged");
            assert_nul_rejected(&program, "field");

            let mut program = original.clone();
            program.interfaces[0].id = DeclarationId::new("host.interface\0forged");
            assert_nul_rejected(&program, "interface");

            let mut program = original.clone();
            program.interfaces[0].imports[0].id = DeclarationId::new("host.dispose\0forged");
            assert_nul_rejected(&program, "import");

            let mut program = original.clone();
            program.interfaces[0].imports[0].import_key = "host.dispose\0forged".to_owned();
            assert_nul_rejected(&program, "logical import key");

            let mut program = original;
            program.functions[0].id = DeclarationId::new("helper.function\0forged");
            assert_nul_rejected(&program, "function");
        }

        #[test]
        fn validator_rejects_nul_in_derived_expression_and_value_identities() {
            let original = identity_program();
            let helper_index = original
                .functions
                .iter()
                .position(|function| function.name == "helper")
                .unwrap();

            let mut program = original.clone();
            program.functions[helper_index].body.id = ExpressionId("expression\0forged".to_owned());
            assert_nul_rejected(&program, "expression");

            let mut program = original.clone();
            program.functions[helper_index].params[0].id = ValueId("value\0forged".to_owned());
            assert_nul_rejected(&program, "parameter value");

            let mut program = original;
            program.functions[helper_index].result_id = ValueId("result\0forged".to_owned());
            assert_nul_rejected(&program, "result value");
        }

        #[test]
        fn validator_normalizes_nul_across_core_hir_reference_carriers() {
            let original = identity_program();

            let mut program = original.clone();
            let helper = function_index(&program, "helper");
            program.functions[helper].params[0].ty = super::super::ResolvedType::Nominal {
                declaration: DeclarationId::new("type\0forged"),
                arguments: vec![super::super::ResolvedType::TypeParameter {
                    owner: DeclarationId::new("owner.safe"),
                    index: 0,
                }],
            };
            assert_nul_rejected(&program, "nominal type declaration");

            let mut program = original.clone();
            let helper = function_index(&program, "helper");
            program.functions[helper].params[0].ty = super::super::ResolvedType::TypeParameter {
                owner: DeclarationId::new("owner\0forged"),
                index: 0,
            };
            assert_nul_rejected(&program, "type-parameter owner");

            let mut program = original.clone();
            let main = function_index(&program, "main");
            let super::super::ResolvedExprKind::Call { callee, .. } =
                &mut tail_mut(&mut program.functions[main].body).kind
            else {
                panic!("main must call helper")
            };
            *callee = DeclarationId::new("callee\0forged");
            assert_nul_rejected(&program, "call target");

            let mut program = original.clone();
            let helper = function_index(&program, "helper");
            let super::super::ResolvedExprKind::Place(place) =
                &mut tail_mut(&mut program.functions[helper].body).kind
            else {
                panic!("helper must return a place")
            };
            place.root = ValueId("place\0forged".to_owned());
            assert_nul_rejected(&program, "place root");

            let mut program = original.clone();
            let reader = function_index(&program, "read_pair");
            let super::super::ResolvedExprKind::Place(place) =
                &mut tail_mut(&mut program.functions[reader].body).kind
            else {
                panic!("record parameter projection must remain a place")
            };
            place.projections[0] =
                super::super::PlaceProjection::Field(DeclarationId::new("field\0forged"));
            assert_nul_rejected(&program, "place projection");

            let mut program = original.clone();
            let maker = function_index(&program, "make_pair");
            let super::super::ResolvedExprKind::ConstructRecord { record, .. } =
                &mut tail_mut(&mut program.functions[maker].body).kind
            else {
                panic!("make_pair must construct a record")
            };
            *record = DeclarationId::new("record\0forged");
            assert_nul_rejected(&program, "record constructor");

            let mut program = original.clone();
            let maker = function_index(&program, "make_pair");
            let super::super::ResolvedExprKind::ConstructRecord { fields, .. } =
                &mut tail_mut(&mut program.functions[maker].body).kind
            else {
                panic!("make_pair must construct a record")
            };
            fields[0].field = DeclarationId::new("initializer\0forged");
            assert_nul_rejected(&program, "record initializer field");

            let mut program = original;
            let reader = function_index(&program, "read_temporary");
            let super::super::ResolvedExprKind::Project { field, .. } =
                &mut tail_mut(&mut program.functions[reader].body).kind
            else {
                panic!("temporary record projection must remain explicit")
            };
            *field = DeclarationId::new("projected\0forged");
            assert_nul_rejected(&program, "projected field");
        }

        #[test]
        fn validator_normalizes_nul_across_cleanup_inventory_and_plan_references() {
            let original = identity_program();
            let discard = function_index(&original, "discard");

            let mut program = original.clone();
            let crate::cleanup::CleanupStorageOrigin::Parameter { value, .. } =
                &mut program.functions[discard].cleanup.slots[0].origin
            else {
                panic!("discard must own parameter storage")
            };
            *value = ValueId("inventory\0forged".to_owned());
            assert_nul_rejected(&program, "inventory value");

            let mut program = original.clone();
            program.functions[discard].cleanup.flags[0]
                .place
                .projections
                .push(DeclarationId::new("inventory.projection\0forged"));
            assert_nul_rejected(&program, "inventory projection");

            let mut program = original.clone();
            program.functions[discard].cleanup_plan.slots[0].storage =
                crate::cleanup_plan::StorageId::CallArgument {
                    call: ExpressionId("plan.call\0forged".to_owned()),
                    parameter_index: 0,
                    value_expression: ExpressionId("plan.value".to_owned()),
                };
            assert_nul_rejected(&program, "plan call-argument storage");

            let mut program = original.clone();
            program.functions[discard]
                .cleanup_plan
                .entry_state
                .live_owned_parameters[0]
                .projections
                .push(DeclarationId::new("plan.projection\0forged"));
            assert_nul_rejected(&program, "plan place projection");

            let mut program = original.clone();
            let finalizer = program.functions[discard]
                .cleanup_plan
                .exits
                .iter_mut()
                .find_map(|exit| exit.finalize_in_order.first_mut())
                .expect("discard must finalize its parameter");
            finalizer.lifecycle_id = DeclarationId::new("plan.lifecycle\0forged");
            assert_nul_rejected(&program, "plan finalizer lifecycle");

            let mut program = original;
            let main = function_index(&program, "main");
            let source = program.functions[main]
                .cleanup_plan
                .status_sources
                .iter_mut()
                .find(|source| {
                    matches!(
                        &source.producer,
                        crate::cleanup_plan::StatusProducer::PropagatedCall { .. }
                    )
                })
                .expect("main call must have a propagated status source");
            let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } =
                &mut source.producer
            else {
                unreachable!()
            };
            *callee = DeclarationId::new("plan.callee\0forged");
            assert_nul_rejected(&program, "plan propagated callee");
        }

        #[test]
        fn native_and_wasm_reject_nul_before_backend_feature_gates() {
            let mut program = identity_program();
            let main = function_index(&program, "main");
            let super::super::ResolvedExprKind::Call { callee, .. } =
                &mut tail_mut(&mut program.functions[main].body).kind
            else {
                panic!("main must call helper")
            };
            *callee = DeclarationId::new("helper.function\0forged");

            let native = codegen::emit_hir_c(&program).unwrap_err();
            assert_eq!(native.code, "SPX-H006");
            assert!(native.message.contains("contains NUL"));

            let wasm = wasm::emit_resolved_module(&program).unwrap_err();
            assert_eq!(wasm.code, "SPX-H006");
            assert!(wasm.message.contains("contains NUL"));

            let mut cleanup_program = identity_program();
            let discard = function_index(&cleanup_program, "discard");
            let finalizer = cleanup_program.functions[discard]
                .cleanup_plan
                .exits
                .iter_mut()
                .find_map(|exit| exit.finalize_in_order.first_mut())
                .expect("discard must finalize its parameter");
            finalizer.lifecycle_id = DeclarationId::new("cleanup.lifecycle\0forged");

            let native = codegen::emit_hir_c(&cleanup_program).unwrap_err();
            assert_eq!(native.code, "SPX-H006");
            assert!(native.message.contains("contains NUL"));

            let wasm = wasm::emit_resolved_module(&cleanup_program).unwrap_err();
            assert_eq!(wasm.code, "SPX-H006");
            assert!(wasm.message.contains("contains NUL"));
        }

        #[test]
        fn valid_identity_program_keeps_its_existing_validation_result() {
            let program = identity_program();
            validate(&program).unwrap();
            let helper = function_index(&program, "helper");
            assert!(matches!(
                &tail(&program.functions[helper].body).kind,
                super::super::ResolvedExprKind::Place(_)
            ));
            let ResolvedTypeDeclarationKind::Resource { drop } = &program.types[0].kind else {
                panic!("Token must be a resource")
            };
            assert!(matches!(
                drop.kind,
                ResolvedResourceDropKind::Imported { .. }
            ));
        }
    }

    #[test]
    fn validator_rejects_a_forged_by_value_recursive_record_index() {
        let mut program = record_program();
        let recursive = ResolvedType::Nominal {
            declaration: DeclarationId::new("node.type"),
            arguments: Vec::new(),
        };
        let ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
            panic!("Node must be a record");
        };
        fields[0].ty = recursive.clone();
        program
            .declarations
            .record_fields
            .get_mut(&DeclarationId::new("node.type"))
            .unwrap()[0]
            .ty = recursive;

        assert_eq!(validate(&program).unwrap_err().code, "SPX-H006");
    }

    #[test]
    fn validator_rejects_a_field_owned_by_the_wrong_record() {
        let mut program = record_program();
        program
            .declarations
            .declarations
            .get_mut(&DeclarationId::new("node.value"))
            .unwrap()
            .owner = Some(DeclarationId::new("forged.owner"));

        assert_eq!(validate(&program).unwrap_err().code, "SPX-H006");
    }
}
