//! Checked resource-free value dependencies and hygienic source relocation.
//! Type declarations stay in their authenticated provider modules.

use super::{intent, invalid, limit, Result, MAX_DEPENDENCIES};
use crate::ast::{
    ExprKind, Function, MatchPattern, ModuleUse, ModuleUseKind, ParamMode, Program,
    RecordMatchFieldPattern, RecordMatchPatternField, Span, Statement, Type,
};
use crate::hir::{self, DeclarationKind, OwnershipMode, ResolvedType};
use crate::project::ProjectRevision;
use crate::workspace_graph::WorkspaceGraphProjectionModule;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const MAX_SYNTAX: usize = 4096;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;

struct Dependency {
    alias: String,
    provider: Option<String>,
}

pub(super) struct TypeMovePlan {
    dependencies: BTreeMap<String, Dependency>,
    aliases: BTreeMap<String, String>,
    locals: BTreeSet<String>,
    builtins: BTreeMap<(usize, usize), crate::byte_ops::ByteOp>,
    extended: bool,
}

pub(super) fn plan(
    revision: &ProjectRevision,
    source: &Program,
    function: &Function,
) -> Result<TypeMovePlan> {
    let (module, checked) = authenticate(revision, source, function)?;
    signature(module, function, checked)?;
    let mut result = TypeMovePlan {
        dependencies: BTreeMap::new(),
        aliases: BTreeMap::new(),
        locals: function.params.iter().map(|p| p.name.clone()).collect(),
        builtins: BTreeMap::new(),
        extended: false,
    };
    result.locals.insert(function.name.clone());
    result.locals.insert("result".to_owned());
    let mut types = BTreeSet::new();
    let mut type_nodes = 0;
    let mut nodes = Nodes::new(checked);
    while let Some(node) = nodes.next()? {
        result.extended |= node
            .ownership()
            .is_some_and(|mode| mode != OwnershipMode::Value);
        if let Node::Expression(expression) = node {
            use hir::ResolvedExprKind as E;
            let operation = match &expression.kind {
                E::Call { callee, .. } => Some(callee),
                E::BorrowPlace { operation, .. } | E::ByteRange { operation, .. } => {
                    Some(operation)
                }
                _ => None,
            };
            if let Some(op) = operation.and_then(|id| crate::byte_ops::by_id(id.as_str())) {
                charge(&mut type_nodes, 1)?;
                if result
                    .builtins
                    .insert((expression.span.start, expression.span.end), op)
                    .is_some()
                {
                    return Err(invalid("movement builtin source occurrence is ambiguous"));
                }
                result.extended = true;
            }
        }
        if let Some(ty) = node.ty() {
            checked_type(module, ty)?;
            result.extended |= matches!(
                ty,
                ResolvedType::String
                    | ResolvedType::Bytes
                    | ResolvedType::ArrayU8(_)
                    | ResolvedType::Str
                    | ResolvedType::SliceU8
            );
            if let ResolvedType::Nominal {
                declaration,
                arguments,
            } = ty
            {
                if !types.contains(ty) {
                    charge(&mut type_nodes, 1 + arguments.len())?;
                    types.insert(ty.clone());
                    let args = arguments
                        .iter()
                        .map(|t| match t {
                            ResolvedType::I64 => "i64",
                            ResolvedType::Bool => "bool",
                            _ => unreachable!("checked direct nominal argument"),
                        })
                        .collect::<Vec<_>>();
                    let id = declaration.as_str();
                    let prelude =
                        matches!(id, crate::prelude::OPTION_ID | crate::prelude::RESULT_ID);
                    let shape = intent::nominal_type_dependency_fingerprint(revision, id)?
                        .ok_or_else(|| {
                            invalid("movement type has no authenticated declaration shape")
                        })?;
                    if !prelude && (!arguments.is_empty() || shape["generic"] == true) {
                        return Err(invalid(
                            "movement does not open generic source type imports",
                        ));
                    }
                    let provider = if prelude {
                        None
                    } else {
                        Some(
                            shape["module"]
                                .as_str()
                                .ok_or_else(|| {
                                    invalid("movement source type has no provider module")
                                })?
                                .to_owned(),
                        )
                    };
                    let visible = prelude
                        || source.types.iter().any(|ty| ty.stable_id == id)
                        || source.module_uses.iter().any(|binding| {
                            binding.kind == ModuleUseKind::Type && binding.persistent_id == id
                        });
                    // Inferred results and nested projected values need not
                    // have a source spelling. Their owner/provider is still
                    // authenticated by retained HIR and the complete shape.
                    let name = if visible {
                        let Type::Named { name, .. } =
                            intent::nominal_type_plan(revision, source, id, &json!(args))?
                        else {
                            return Err(invalid(
                                "movement nominal plan did not produce a named type",
                            ));
                        };
                        if result
                            .aliases
                            .insert(name.clone(), id.to_owned())
                            .is_some_and(|old| old != id)
                        {
                            return Err(invalid("movement source type alias is ambiguous"));
                        }
                        name
                    } else {
                        shape["name"]
                            .as_str()
                            .ok_or_else(|| {
                                invalid("movement inferred nominal owner has no source name")
                            })?
                            .to_owned()
                    };
                    result
                        .dependencies
                        .entry(id.to_owned())
                        .or_insert(Dependency {
                            alias: name,
                            provider,
                        });
                    if result.dependencies.len() > MAX_DEPENDENCIES {
                        return Err(limit(
                            "movement exceeds sixty-four nominal type dependencies",
                        ));
                    }
                }
            }
        }
        node.check_ownership()?;
    }
    let mut copy = function.clone();
    let mut names = BTreeSet::new();
    rewrite(&mut copy, &mut names, &mut |name| {
        if !result.aliases.contains_key(name) {
            return Err(invalid(
                "movement source type spelling lacks a checked nominal dependency",
            ));
        }
        Ok(())
    })?;
    result.locals.extend(names);
    Ok(result)
}

pub(super) fn validate_signature(
    revision: &ProjectRevision,
    source: &Program,
    function: &Function,
) -> Result<()> {
    let (module, checked) = authenticate(revision, source, function)?;
    signature(module, function, checked)
}

impl TypeMovePlan {
    pub(super) fn extended(&self) -> bool {
        self.extended
    }

    pub(super) fn builtin_at(&self, span: Span) -> Option<crate::byte_ops::ByteOp> {
        self.builtins.get(&(span.start, span.end)).copied()
    }

    pub(super) fn builtin_names(&self) -> BTreeSet<&'static str> {
        self.builtins.values().map(|op| op.name()).collect()
    }

    pub(super) fn local_names(&self) -> &BTreeSet<String> {
        &self.locals
    }
    pub(super) fn dependency_count(&self) -> usize {
        self.dependencies.len()
    }

    pub(super) fn relocate(
        &self,
        destination: &mut Program,
        function: &mut Function,
        occupied: &mut BTreeSet<String>,
    ) -> Result<()> {
        let mut replacements = BTreeMap::new();
        for (id, dependency) in &self.dependencies {
            let mut existing = destination
                .types
                .iter()
                .filter(|ty| ty.stable_id == *id)
                .map(|ty| ty.name.clone())
                .collect::<Vec<_>>();
            existing.extend(
                destination
                    .module_uses
                    .iter()
                    .filter(|binding| {
                        binding.kind == ModuleUseKind::Type && binding.persistent_id == *id
                    })
                    .map(|binding| binding.alias.clone()),
            );
            if dependency.provider.is_none() {
                existing.push(dependency.alias.clone());
            }
            let name = match existing.as_slice() {
                [name] => {
                    if self.locals.contains(name) {
                        return Err(invalid(
                            "movement existing type binding conflicts with a moved local name",
                        ));
                    }
                    if dependency.provider.is_none() && super::namespace(destination).contains(name)
                    {
                        return Err(invalid(
                            "movement compiler prelude type binding is shadowed",
                        ));
                    }
                    name.clone()
                }
                [] => {
                    let provider = dependency
                        .provider
                        .as_ref()
                        .ok_or_else(|| invalid("movement prelude type binding is absent"))?;
                    if *provider == destination.module {
                        return Err(invalid(
                            "movement destination provider lacks its selected declaration",
                        ));
                    }
                    let alias = super::choose_alias(&dependency.alias, occupied)?;
                    destination.module_uses.push(ModuleUse {
                        kind: ModuleUseKind::Type,
                        persistent_id: id.clone(),
                        target_module: provider.clone(),
                        alias: alias.clone(),
                        span: Span::default(),
                    });
                    alias
                }
                _ => {
                    return Err(invalid(
                        "movement destination has multiple bindings for one nominal type",
                    ))
                }
            };
            occupied.insert(name.clone());
            replacements.insert(id.clone(), name);
        }
        rewrite(function, &mut BTreeSet::new(), &mut |name| {
            let id = self
                .aliases
                .get(name)
                .ok_or_else(|| invalid("movement type alias was not authenticated"))?;
            *name = replacements
                .get(id)
                .ok_or_else(|| invalid("movement destination type binding is absent"))?
                .clone();
            Ok(())
        })
    }
}

fn authenticate<'a>(
    revision: &'a ProjectRevision,
    source: &Program,
    function: &Function,
) -> Result<(
    &'a WorkspaceGraphProjectionModule,
    &'a hir::ResolvedFunction,
)> {
    let mut found = None;
    for module in revision.semantic.image_modules() {
        for checked in module
            .functions()
            .iter()
            .filter(|f| f.id.as_str() == function.stable_id)
        {
            if module.path() != source.path
                || module.module() != source.module
                || checked.name != function.name
                || checked.span != function.span
                || found.replace((module, checked)).is_some()
            {
                return Err(invalid(
                    "movement checked function source identity is ambiguous or changed",
                ));
            }
        }
    }
    let original = revision
        .sources()
        .iter()
        .find(|s| s.path() == source.path)
        .ok_or_else(|| invalid("movement canonical source is absent"))?;
    let parsed = crate::parse(original.source(), original.path()).map_err(|error| vec![error])?;
    if &parsed != source || !parsed.functions.iter().any(|f| f == function) {
        return Err(invalid(
            "movement input differs from authenticated canonical source",
        ));
    }
    found.ok_or_else(|| invalid("movement checked function is absent"))
}

fn signature(
    module: &WorkspaceGraphProjectionModule,
    source: &Function,
    checked: &hir::ResolvedFunction,
) -> Result<()> {
    if !source.type_parameters.is_empty() || source.params.len() != checked.params.len() {
        return Err(invalid(
            "movement requires a monomorphic checked function signature",
        ));
    }
    checked_type(module, &checked.return_type)?;
    if matches!(
        checked.return_type,
        ResolvedType::Str | ResolvedType::SliceU8
    ) {
        return Err(invalid(
            "movement does not relocate borrowed return surfaces",
        ));
    }
    for (parameter, resolved) in source.params.iter().zip(&checked.params) {
        let expected_ownership = match (&parameter.mode, &parameter.ty) {
            (ParamMode::Value, Type::String) => OwnershipMode::Own,
            (ParamMode::Value, _) => OwnershipMode::Value,
            (ParamMode::Own, Type::Bytes | Type::Named { .. }) => OwnershipMode::Own,
            _ => {
                return Err(invalid(
                    "movement signature does not admit borrowed or shared parameters",
                ))
            }
        };
        if resolved.ownership != expected_ownership
            || parameter.name != resolved.name
            || parameter.span != resolved.span
        {
            return Err(invalid(
                "movement signature requires exact checked value or owning parameters",
            ));
        }
        checked_type(module, &resolved.ty)?;
    }
    Ok(())
}

fn checked_type(module: &WorkspaceGraphProjectionModule, ty: &ResolvedType) -> Result<()> {
    match ty {
        ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::Bytes
        | ResolvedType::ArrayU8(_)
        | ResolvedType::Str
        | ResolvedType::SliceU8 => Ok(()),
        ResolvedType::Nominal { arguments, .. } => {
            if arguments.len() > intent::MAX_AGGREGATE_TYPE_ARGUMENTS {
                return Err(limit(
                    "movement nominal type argument inventory exceeds its bound",
                ));
            }
            if arguments
                .iter()
                .any(|t| !matches!(t, ResolvedType::I64 | ResolvedType::Bool))
            {
                return Err(invalid(
                    "movement nominal arguments require direct i64 or bool",
                ));
            }
            let (kind, facts) = module.value_type_facts(ty).ok_or_else(|| {
                invalid("movement nominal value has no retained checked type facts")
            })?;
            if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                || !facts.sized
                || facts.contains_resource
                || facts.copy == facts.needs_drop
            {
                return Err(invalid("movement nominal values require sized resource-free Copy or owning records or variants"));
            }
            Ok(())
        }
        _ => Err(invalid(
            "movement requires admitted resource-free values or internal byte/string views",
        )),
    }
}

fn charge(nodes: &mut usize, count: usize) -> Result<()> {
    *nodes = nodes
        .checked_add(count)
        .ok_or_else(|| limit("movement type/pattern inventory overflow"))?;
    if *nodes > MAX_SYNTAX {
        return Err(limit("movement type/pattern inventory exceeds 4096 nodes"));
    }
    Ok(())
}

fn rewrite(
    function: &mut Function,
    locals: &mut BTreeSet<String>,
    rename: &mut impl FnMut(&mut String) -> Result<()>,
) -> Result<()> {
    let mut syntax = 0;
    for parameter in &mut function.params {
        rewrite_type(&mut parameter.ty, &mut syntax, rename)?;
    }
    rewrite_type(&mut function.return_type, &mut syntax, rename)?;
    intent::walk_function(function, &mut 0, &mut |expression| {
        match &mut expression.kind {
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                ..
            }
            | ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                ..
            } => {
                charge(&mut syntax, 1 + type_arguments.len())?;
                direct_arguments(type_arguments)?;
                rename(type_name)?;
            }
            ExprKind::Block { statements, .. } => {
                for statement in statements {
                    if let Statement::Let { name, declared, .. } = statement {
                        locals.insert(name.clone());
                        if let Some(ty) = declared {
                            rewrite_type(ty, &mut syntax, rename)?;
                        }
                    }
                }
            }
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    rewrite_pattern(&mut arm.pattern, locals, &mut syntax, 0, rename)?;
                }
            }
            _ => {}
        }
        Ok(())
    })
}

fn direct_arguments(arguments: &[Type]) -> Result<()> {
    if arguments
        .iter()
        .any(|t| !matches!(t, Type::I64 | Type::Bool))
    {
        return Err(invalid(
            "movement explicit type arguments require direct i64 or bool",
        ));
    }
    Ok(())
}

fn rewrite_type(
    ty: &mut Type,
    syntax: &mut usize,
    rename: &mut impl FnMut(&mut String) -> Result<()>,
) -> Result<()> {
    match ty {
        Type::I64
        | Type::I32
        | Type::U8
        | Type::Usize
        | Type::Bool
        | Type::String
        | Type::Bytes
        | Type::ArrayU8(_)
        | Type::Str
        | Type::SliceU8 => Ok(()),
        Type::Named { name, arguments } => {
            charge(syntax, 1 + arguments.len())?;
            direct_arguments(arguments)?;
            rename(name)
        }
        _ => Err(invalid(
            "movement source annotation requires a scalar or checked nominal value",
        )),
    }
}

fn rewrite_pattern(
    pattern: &mut MatchPattern,
    locals: &mut BTreeSet<String>,
    syntax: &mut usize,
    depth: usize,
    rename: &mut impl FnMut(&mut String) -> Result<()>,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit("movement pattern depth exceeds its bound"));
    }
    charge(syntax, 1)?;
    match pattern {
        MatchPattern::Variant {
            type_name, fields, ..
        } => {
            rename(type_name)?;
            charge(syntax, fields.len())?;
            locals.extend(fields.iter().map(|field| field.binding.clone()));
        }
        MatchPattern::Record {
            type_name, fields, ..
        } => {
            rename(type_name)?;
            rewrite_record_fields(fields, locals, syntax, depth + 1, rename)?;
        }
        MatchPattern::Binding { name, .. } => {
            locals.insert(name.clone());
        }
        MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                rewrite_pattern(alternative, locals, syntax, depth + 1, rename)?;
            }
        }
        MatchPattern::Wildcard { .. } | MatchPattern::Literal { .. } => {}
    }
    Ok(())
}

fn rewrite_record_fields(
    fields: &mut [RecordMatchPatternField],
    locals: &mut BTreeSet<String>,
    syntax: &mut usize,
    depth: usize,
    rename: &mut impl FnMut(&mut String) -> Result<()>,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit(
            "movement nested record pattern depth exceeds its bound",
        ));
    }
    charge(syntax, fields.len())?;
    for field in fields {
        match &mut field.pattern {
            RecordMatchFieldPattern::Binding { name, .. } => {
                locals.insert(name.clone());
            }
            RecordMatchFieldPattern::Record {
                type_name, fields, ..
            } => {
                rename(type_name)?;
                rewrite_record_fields(fields, locals, syntax, depth + 1, rename)?;
            }
            RecordMatchFieldPattern::Wildcard { .. } => {}
        }
    }
    Ok(())
}

/// Compare retained checked identities in structural order, excluding source
/// positions and revision-scoped expression/value IDs changed by formatting.
pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    target: &str,
) -> Result<()> {
    let (old_module, old) = locate_checked(before, target)?;
    let (new_module, new) = locate_checked(after, target)?;
    let mut left = Nodes::new(old);
    let mut right = Nodes::new(new);
    loop {
        match (left.next()?, right.next()?) {
            (None, None) => return Ok(()),
            (Some(a), Some(b)) => {
                if !a.same_identity(b) {
                    return Err(invalid(
                        "movement changed a checked value type, ownership, or declaration identity",
                    ));
                }
                if let Some(ty) = a.ty() {
                    checked_type(old_module, ty)?;
                }
                if let Some(ty) = b.ty() {
                    checked_type(new_module, ty)?;
                }
                a.check_ownership()?;
                b.check_ownership()?;
            }
            _ => return Err(invalid("movement changed the checked value inventory")),
        }
    }
}

fn locate_checked<'a>(
    revision: &'a ProjectRevision,
    target: &str,
) -> Result<(
    &'a WorkspaceGraphProjectionModule,
    &'a hir::ResolvedFunction,
)> {
    let mut found = None;
    for module in revision.semantic.image_modules() {
        for function in module
            .functions()
            .iter()
            .filter(|f| f.id.as_str() == target)
        {
            if found.replace((module, function)).is_some() {
                return Err(invalid("movement rebuilt function identity is ambiguous"));
            }
        }
    }
    found.ok_or_else(|| invalid("movement rebuilt function identity is absent"))
}

#[derive(Clone, Copy)]
enum Node<'a> {
    Function(&'a hir::ResolvedFunction),
    Param(&'a hir::ResolvedParam),
    Type(&'a ResolvedType),
    Expression(&'a hir::ResolvedExpr),
    Statement(&'a hir::ResolvedStatement),
    Binding(&'a hir::ResolvedBinding),
    Field(&'a hir::ResolvedFieldInitializer),
    Arm(&'a hir::ResolvedMatchArm),
    Pattern(&'a hir::ResolvedMatchPattern),
    RecordField(&'a hir::ResolvedRecordMatchPatternField),
}

impl<'a> Node<'a> {
    fn ty(self) -> Option<&'a ResolvedType> {
        match self {
            Self::Type(ty) => Some(ty),
            Self::Param(p) => Some(&p.ty),
            Self::Expression(e) => Some(&e.ty),
            Self::Binding(b) => Some(&b.ty),
            Self::Pattern(hir::ResolvedMatchPattern::Record { instance, .. }) => Some(instance),
            Self::RecordField(hir::ResolvedRecordMatchPatternField {
                pattern: hir::ResolvedRecordMatchFieldPattern::Record { instance, .. },
                ..
            }) => Some(instance),
            _ => None,
        }
    }
    fn ownership(self) -> Option<OwnershipMode> {
        match self {
            Self::Param(p) => Some(p.ownership),
            Self::Expression(e) => Some(e.ownership),
            Self::Binding(b) => Some(b.ownership),
            _ => None,
        }
    }
    fn check_ownership(self) -> Result<()> {
        if self
            .ownership()
            .is_some_and(|mode| matches!(mode, OwnershipMode::Borrow | OwnershipMode::Shared))
            && !matches!(self.ty(), Some(ResolvedType::Str | ResolvedType::SliceU8))
        {
            return Err(invalid(
                "movement permits internal borrowed/shared views only over string or byte slices",
            ));
        }
        Ok(())
    }
    fn same_identity(self, other: Self) -> bool {
        if std::mem::discriminant(&self) != std::mem::discriminant(&other)
            || self.ty() != other.ty()
            || self.ownership() != other.ownership()
        {
            return false;
        }
        use hir::{
            ResolvedExprKind as E, ResolvedMatchPattern as P, ResolvedRecordMatchFieldPattern as F,
        };
        match (self, other) {
            (Self::Function(a), Self::Function(b)) => {
                a.id == b.id
                    && a.name == b.name
                    && a.effects == b.effects
                    && a.params.len() == b.params.len()
                    && a.requires.len() == b.requires.len()
                    && a.ensures.len() == b.ensures.len()
            }
            (Self::Param(a), Self::Param(b)) => a.name == b.name,
            (Self::Binding(a), Self::Binding(b)) => a.name == b.name,
            (Self::Field(a), Self::Field(b)) => a.field == b.field,
            (Self::Expression(a), Self::Expression(b)) => {
                if std::mem::discriminant(&a.kind) != std::mem::discriminant(&b.kind) {
                    return false;
                }
                match (&a.kind, &b.kind) {
                    (
                        E::ConstructRecord { record: a, .. },
                        E::ConstructRecord { record: b, .. },
                    )
                    | (E::UpdateRecord { record: a, .. }, E::UpdateRecord { record: b, .. }) => {
                        a == b
                    }
                    (
                        E::ConstructVariant {
                            variant: a,
                            case: ac,
                            ..
                        },
                        E::ConstructVariant {
                            variant: b,
                            case: bc,
                            ..
                        },
                    ) => a == b && ac == bc,
                    (E::Project { field: a, .. }, E::Project { field: b, .. }) => a == b,
                    (E::Match { mode: a, .. }, E::Match { mode: b, .. }) => a == b,
                    (E::Place(a), E::Place(b)) => a == b,
                    (
                        E::BorrowPlace {
                            operation: a,
                            place: ap,
                        },
                        E::BorrowPlace {
                            operation: b,
                            place: bp,
                        },
                    ) => a == b && ap == bp,
                    (E::ByteRange { operation: a, .. }, E::ByteRange { operation: b, .. }) => {
                        a == b
                    }
                    (
                        E::Call {
                            callee: a,
                            type_arguments: aa,
                            ..
                        },
                        E::Call {
                            callee: b,
                            type_arguments: ba,
                            ..
                        },
                    ) => a == b && aa == ba,
                    _ => true,
                }
            }
            (Self::Pattern(a), Self::Pattern(b)) => match (a, b) {
                (
                    P::Variant {
                        variant: a,
                        case: ac,
                        fields: af,
                    },
                    P::Variant {
                        variant: b,
                        case: bc,
                        fields: bf,
                    },
                ) => {
                    a == b
                        && ac == bc
                        && af.iter().map(|f| &f.field).eq(bf.iter().map(|f| &f.field))
                }
                (P::Record { record: a, .. }, P::Record { record: b, .. }) => a == b,
                _ => std::mem::discriminant(a) == std::mem::discriminant(b),
            },
            (Self::RecordField(a), Self::RecordField(b)) => {
                a.field == b.field
                    && match (&a.pattern, &b.pattern) {
                        (F::Record { record: a, .. }, F::Record { record: b, .. }) => a == b,
                        (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
                    }
            }
            (Self::Statement(a), Self::Statement(b)) => {
                std::mem::discriminant(a) == std::mem::discriminant(b)
            }
            _ => true,
        }
    }
    fn child(self, index: usize) -> Option<Self> {
        use hir::{
            ResolvedExprKind as E, ResolvedMatchPattern as P, ResolvedRecordMatchFieldPattern as F,
            ResolvedStatement as S,
        };
        match self {
            Self::Function(f) => {
                if let Some(p) = f.params.get(index) {
                    return Some(Self::Param(p));
                }
                let index = index.checked_sub(f.params.len())?;
                if index == 0 {
                    return Some(Self::Type(&f.return_type));
                }
                let index = index - 1;
                if let Some(e) = f.requires.get(index) {
                    return Some(Self::Expression(e));
                }
                let index = index.checked_sub(f.requires.len())?;
                if index == 0 {
                    return Some(Self::Expression(&f.body));
                }
                f.ensures.get(index - 1).map(Self::Expression)
            }
            Self::Type(_) | Self::Param(_) | Self::Binding(_) => None,
            Self::Field(f) => (index == 0).then_some(Self::Expression(&f.value)),
            Self::Statement(s) => match s {
                S::Let { binding, value, .. } | S::Assign { binding, value, .. } => {
                    [Self::Binding(binding), Self::Expression(value)]
                        .get(index)
                        .copied()
                }
                S::Unsafe { .. } | S::While { .. } => s.child(index).map(Self::Expression),
            },
            Self::Arm(a) => match index {
                0 => Some(Self::Pattern(&a.pattern)),
                1 => Some(Self::Expression(a.guard.as_deref().unwrap_or(&a.value))),
                2 if a.guard.is_some() => Some(Self::Expression(&a.value)),
                _ => None,
            },
            Self::Pattern(p) => match p {
                P::Variant { fields, .. } => fields.get(index).map(|f| Self::Binding(&f.binding)),
                P::Record { fields, .. } => fields.get(index).map(Self::RecordField),
                P::Binding(b) => (index == 0).then_some(Self::Binding(b)),
                P::Or(a) => a.get(index).map(Self::Pattern),
                P::Wildcard | P::Literal(_) => None,
            },
            Self::RecordField(f) => match &f.pattern {
                F::Binding(b) => (index == 0).then_some(Self::Binding(b)),
                F::Record { fields, .. } => fields.get(index).map(Self::RecordField),
                F::Wildcard => None,
            },
            Self::Expression(e) => match &e.kind {
                E::Call { args, .. } => args.get(index).map(Self::Expression),
                E::NativeRustImportCall(c) => c.args.get(index).map(Self::Expression),
                E::HostCommandCall(c) => c.args.get(index).map(Self::Expression),
                E::Unary { value, .. }
                | E::Project { base: value, .. }
                | E::Upcast { source: value } => (index == 0).then_some(Self::Expression(value)),
                E::Try {
                    operand,
                    residual_type,
                    ..
                }
                | E::TryOption {
                    operand,
                    residual_type,
                    ..
                } => [Self::Expression(operand), Self::Type(residual_type)]
                    .get(index)
                    .copied(),
                E::Binary { left, right, .. } => [left.as_ref(), right.as_ref()]
                    .get(index)
                    .copied()
                    .map(Self::Expression),
                E::ByteRange {
                    source, start, end, ..
                } => [source.as_ref(), start.as_ref(), end.as_ref()]
                    .get(index)
                    .copied()
                    .map(Self::Expression),
                E::Block { statements, tail } => {
                    if index < statements.len() {
                        statements.get(index).map(Self::Statement)
                    } else {
                        (index == statements.len()).then_some(Self::Expression(tail))
                    }
                }
                E::If {
                    condition,
                    then_branch,
                    else_branch,
                } => [
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref(),
                ]
                .get(index)
                .copied()
                .map(Self::Expression),
                E::ConstructRecord { fields, .. } | E::ConstructVariant { fields, .. } => {
                    fields.get(index).map(Self::Field)
                }
                E::Match {
                    scrutinee, arms, ..
                } => {
                    if index == 0 {
                        Some(Self::Expression(scrutinee))
                    } else {
                        arms.get(index - 1).map(Self::Arm)
                    }
                }
                E::UpdateRecord { base, fields, .. } => {
                    if index == 0 {
                        Some(Self::Expression(base))
                    } else {
                        fields.get(index - 1).map(Self::Field)
                    }
                }
                E::Int(_)
                | E::Int32(_)
                | E::Char(_)
                | E::Uint8(_)
                | E::Usize(_)
                | E::Float32(_)
                | E::Float64(_)
                | E::Bool(_)
                | E::String(_)
                | E::ArrayU8(_)
                | E::RepeatArrayU8 { .. }
                | E::BorrowPlace { .. }
                | E::Place(_) => None,
            },
        }
    }
}

struct Nodes<'a> {
    stack: Vec<(Node<'a>, usize)>,
    visits: usize,
    first: bool,
}
impl<'a> Nodes<'a> {
    fn new(function: &'a hir::ResolvedFunction) -> Self {
        Self {
            stack: vec![(Node::Function(function), 0)],
            visits: 0,
            first: true,
        }
    }
    fn next(&mut self) -> Result<Option<Node<'a>>> {
        let node = if self.first {
            self.first = false;
            Some(self.stack[0].0)
        } else {
            loop {
                let Some((parent, index)) = self.stack.last_mut() else {
                    return Ok(None);
                };
                if let Some(child) = parent.child(*index) {
                    *index += 1;
                    if self.stack.len() > MAX_DEPTH {
                        return Err(limit("movement checked value depth exceeds its bound"));
                    }
                    self.stack.push((child, 0));
                    break Some(child);
                }
                self.stack.pop();
            }
        };
        if self.visits >= MAX_VISITS {
            return Err(limit("movement checked value traversal exceeds its bound"));
        }
        self.visits += 1;
        Ok(node)
    }
}
