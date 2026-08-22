use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ast::{
    BinaryOp, Expr, ExprKind, FieldDeclaration, Function, ImportDeclaration, ImportFailure,
    ImportResult, InterfaceDeclaration, MatchPattern, Param, ParamMode, Program,
    RecordMatchFieldPattern, RecordMatchPatternField, ResourceLifecycleKind, Span, Statement, Type,
    TypeDeclaration, TypeDeclarationKind, UnaryOp, VariantCaseDeclaration,
};
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;

#[cfg(test)]
thread_local! {
    static SOURCE_VERIFY_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_capacity_high_water() {
    SOURCE_VERIFY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(crate) fn capacity_high_water() -> usize {
    SOURCE_VERIFY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_capacity_high_water(bytes: usize) {
    SOURCE_VERIFY_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn binding_owned_capacity(binding: &Binding) -> usize {
    let moved = binding
        .moved_places
        .iter()
        .fold(0usize, |bytes, (place, _)| {
            bytes
                + std::mem::size_of::<(Vec<String>, Availability)>()
                + place.capacity() * std::mem::size_of::<String>()
                + place.iter().map(String::capacity).sum::<usize>()
        });
    let partial = binding
        .definitely_partial
        .iter()
        .fold(0usize, |bytes, place| {
            bytes
                + std::mem::size_of::<Vec<String>>()
                + place.capacity() * std::mem::size_of::<String>()
                + place.iter().map(String::capacity).sum::<usize>()
        });
    binding
        .moved_places
        .capacity()
        .saturating_mul(std::mem::size_of::<(Vec<String>, Availability)>())
        .saturating_add(
            binding
                .definitely_partial
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<String>>()),
        )
        .saturating_add(ast_type_owned_capacity(&binding.ty))
        .saturating_add(moved)
        .saturating_add(partial)
}

#[cfg(test)]
fn ast_type_owned_capacity(ty: &Type) -> usize {
    match ty {
        Type::I64 | Type::Char | Type::U8 | Type::F32 | Type::F64 | Type::Bool => 0,
        Type::Named { name, arguments } => name
            .capacity()
            .saturating_add(arguments.capacity() * std::mem::size_of::<Type>())
            .saturating_add(arguments.iter().map(ast_type_owned_capacity).sum::<usize>()),
    }
}

#[cfg(test)]
fn scope_owned_capacity(scope: &VerifierScope) -> usize {
    scope
        .bindings
        .capacity()
        .saturating_mul(std::mem::size_of::<(String, Binding)>())
        .saturating_add(
            scope
                .bindings
                .iter()
                .fold(0usize, |bytes, (name, binding)| {
                    bytes + name.capacity() + binding_owned_capacity(binding)
                }),
        )
}

#[derive(Clone, Debug)]
struct Binding {
    ty: Type,
    mode: ParamMode,
    availability: Availability,
    moved_places: HashMap<Vec<String>, Availability>,
    definitely_partial: HashSet<Vec<String>>,
    native_unit_discard: bool,
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
            Availability::MaybeMoved
        }
    }
}

#[derive(Clone, Debug)]
struct CheckedValue {
    ty: Type,
    mode: ParamMode,
    native_unit: bool,
}

fn reject_native_unit_value(
    program: &Program,
    expression: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.native_unit && !matches!(expression.kind, ExprKind::Var(_)) {
        diagnostics.push(error(
            program,
            "SPX-B107",
            "Native Rust Interop declaration set is unsupported: scalar value signature required",
            expression.span,
        ));
    }
}

impl CheckedValue {
    fn value(ty: Type) -> Self {
        Self {
            ty,
            mode: ParamMode::Value,
            native_unit: false,
        }
    }

    fn returned(ty: Type, contains_resource: bool) -> Self {
        let mode = if contains_resource {
            ParamMode::Own
        } else {
            ParamMode::Value
        };
        Self {
            ty,
            mode,
            native_unit: false,
        }
    }
}

struct TypeTable<'a> {
    declarations: HashMap<&'a str, &'a TypeDeclaration>,
}

impl<'a> TypeTable<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            declarations: program
                .types
                .iter()
                .chain(crate::prelude::declarations())
                .map(|declaration| (declaration.name.as_str(), declaration))
                .collect(),
        }
    }

    fn declaration(&self, name: &str) -> Option<&'a TypeDeclaration> {
        self.declarations.get(name).copied()
    }

    fn record_fields(&self, ty: &Type) -> Option<&'a [FieldDeclaration]> {
        let Type::Named { name, .. } = ty else {
            return None;
        };
        match &self.declaration(name)?.kind {
            TypeDeclarationKind::Record { fields } => Some(fields),
            TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => None,
        }
    }

    fn record_field_type(&self, instance: &Type, field: &FieldDeclaration) -> Option<Type> {
        let Type::Named { name, arguments } = instance else {
            return None;
        };
        let declaration = self.declaration(name)?;
        Self::substitute_variant_type(declaration, arguments, &field.ty)
    }

    fn variant_cases(&self, ty: &Type) -> Option<&'a [VariantCaseDeclaration]> {
        let Type::Named { name, .. } = ty else {
            return None;
        };
        match &self.declaration(name)?.kind {
            TypeDeclarationKind::Variant { cases } => Some(cases),
            TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Record { .. } => None,
        }
    }

    fn substitute_variant_type(
        declaration: &TypeDeclaration,
        arguments: &[Type],
        template: &Type,
    ) -> Option<Type> {
        enum Frame<'a> {
            Enter(&'a Type),
            Finish(&'a str, usize),
        }
        let mut frames = vec![Frame::Enter(template)];
        let mut resolved = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(template) => match template {
                    Type::I64 => resolved.push(Type::I64),
                    Type::Char => resolved.push(Type::Char),
                    Type::U8 => resolved.push(Type::U8),
                    Type::F32 => resolved.push(Type::F32),
                    Type::F64 => resolved.push(Type::F64),
                    Type::Bool => resolved.push(Type::Bool),
                    Type::Named {
                        name,
                        arguments: nested,
                    } => {
                        if nested.is_empty() {
                            if let Some(index) = declaration
                                .type_parameters
                                .iter()
                                .position(|parameter| parameter.name == *name)
                            {
                                resolved.push(arguments.get(index)?.clone());
                                continue;
                            }
                        }
                        frames.push(Frame::Finish(name, nested.len()));
                        frames.extend(nested.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::Finish(name, count) => {
                    let split = resolved.len().checked_sub(count)?;
                    let nested = resolved.drain(split..).collect();
                    resolved.push(Type::Named {
                        name: name.to_owned(),
                        arguments: nested,
                    });
                }
            }
        }
        (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
    }

    fn contains_resource(&self, ty: &Type) -> bool {
        self.contains_resource_inner(ty, &mut HashSet::new())
    }

    fn is_opaque_resource(&self, ty: &Type) -> bool {
        let Type::Named { name, .. } = ty else {
            return false;
        };
        self.declaration(name).is_some_and(|declaration| {
            matches!(declaration.kind, TypeDeclarationKind::Resource { .. })
        })
    }

    fn contains_resource_inner(&self, ty: &Type, visiting: &mut HashSet<String>) -> bool {
        enum Frame {
            Enter(Type),
            Exit(String),
        }
        let mut frames = vec![Frame::Enter(ty.clone())];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Exit(instance) => {
                    visiting.remove(&instance);
                }
                Frame::Enter(ty) => {
                    let Type::Named { name, arguments } = &ty else {
                        continue;
                    };
                    let Some(declaration) = self.declaration(name) else {
                        continue;
                    };
                    if matches!(declaration.kind, TypeDeclarationKind::Resource { .. }) {
                        return true;
                    }
                    let instance = ty.to_string();
                    if !visiting.insert(instance.clone()) {
                        return true;
                    }
                    frames.push(Frame::Exit(instance));
                    let fields: Box<dyn DoubleEndedIterator<Item = &FieldDeclaration>> =
                        match &declaration.kind {
                            TypeDeclarationKind::Record { fields } => Box::new(fields.iter()),
                            TypeDeclarationKind::Variant { cases } => {
                                Box::new(cases.iter().flat_map(|case| &case.fields))
                            }
                            TypeDeclarationKind::Resource { .. } => unreachable!(),
                        };
                    for field in fields.rev() {
                        let Some(field_ty) =
                            Self::substitute_variant_type(declaration, arguments, &field.ty)
                        else {
                            return true;
                        };
                        frames.push(Frame::Enter(field_ty));
                    }
                }
            }
        }
        false
    }

    fn lifecycle_effects(
        &self,
        ty: &Type,
        imports: &HashMap<&str, (&InterfaceDeclaration, &ImportDeclaration)>,
    ) -> HashSet<String> {
        let mut effects = HashSet::new();
        self.lifecycle_effects_inner(ty, imports, &mut HashSet::new(), &mut effects);
        effects
    }

    fn lifecycle_effects_inner(
        &self,
        ty: &Type,
        imports: &HashMap<&str, (&InterfaceDeclaration, &ImportDeclaration)>,
        visiting: &mut HashSet<String>,
        effects: &mut HashSet<String>,
    ) {
        enum Frame {
            Enter(Type),
            Exit(String),
        }
        let mut frames = vec![Frame::Enter(ty.clone())];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Exit(instance) => {
                    visiting.remove(&instance);
                }
                Frame::Enter(ty) => {
                    let Type::Named { name, arguments } = &ty else {
                        continue;
                    };
                    let Some(declaration) = self.declaration(name) else {
                        continue;
                    };
                    let instance = ty.to_string();
                    if !visiting.insert(instance.clone()) {
                        continue;
                    }
                    frames.push(Frame::Exit(instance));
                    match &declaration.kind {
                        TypeDeclarationKind::Resource { lifecycles } => {
                            if let Some(crate::ast::ResourceLifecycleDeclaration {
                                kind: ResourceLifecycleKind::Imported { import_key },
                                ..
                            }) = lifecycles.first()
                            {
                                if let Some((_, import)) = imports.get(import_key.as_str()) {
                                    effects.extend(import.effects.iter().cloned());
                                }
                            }
                        }
                        TypeDeclarationKind::Record { fields } => {
                            for field in fields.iter().rev() {
                                if let Some(field_ty) =
                                    Self::substitute_variant_type(declaration, arguments, &field.ty)
                                {
                                    frames.push(Frame::Enter(field_ty));
                                }
                            }
                        }
                        TypeDeclarationKind::Variant { cases } => {
                            for field in cases.iter().flat_map(|case| &case.fields).rev() {
                                if let Some(field_ty) =
                                    Self::substitute_variant_type(declaration, arguments, &field.ty)
                                {
                                    frames.push(Frame::Enter(field_ty));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn verify(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if !program.module_uses.is_empty() {
        diagnostics.push(
            Diagnostic::io(
                "SPX-G172",
                "source module imports require Workspace Semantic Graph resolution",
            )
            .at_path(&program.path),
        );
        return diagnostics;
    }
    let mut functions = HashMap::new();
    let mut ids = crate::prelude::all_ids()
        .into_iter()
        .collect::<HashSet<_>>();
    let mut type_names = HashSet::new();

    for declaration in &program.types {
        if crate::prelude::is_reserved_type_name(&declaration.name) {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "type name `{}` is reserved by compiler prelude `{}`",
                    declaration.name,
                    crate::prelude::SCHEMA_V1
                ),
                declaration.name_span,
            ));
        }
        if matches!(declaration.kind, TypeDeclarationKind::Resource { .. })
            && !declaration.type_parameters.is_empty()
        {
            diagnostics.push(error(
                program,
                "SPX-T223",
                "only record and variant declarations may declare generic parameters in this slice",
                declaration.type_parameters[0].span,
            ));
        }
        let mut parameter_names = HashSet::new();
        for parameter in &declaration.type_parameters {
            if !source_identifier(&parameter.name)
                || !parameter_names.insert(parameter.name.as_str())
            {
                diagnostics.push(error(
                    program,
                    "SPX-T220",
                    format!(
                        "invalid or duplicate type parameter `{}` on `{}`",
                        parameter.name, declaration.name
                    ),
                    parameter.span,
                ));
            }
        }
        if !source_identifier(&declaration.name) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } => "record",
                TypeDeclarationKind::Variant { .. } => "variant",
            };
            diagnostics.push(error(
                program,
                "SPX-S106",
                format!("`{}` is not a valid {kind} identifier", declaration.name),
                declaration.name_span,
            ));
        }
        if !type_names.insert(declaration.name.as_str()) {
            let duplicate = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } => "type",
                TypeDeclarationKind::Variant { .. } => "type",
            };
            diagnostics.push(error(
                program,
                "SPX-S107",
                format!("duplicate {duplicate} `{}`", declaration.name),
                declaration.span,
            ));
        }
        if declaration.stable_id.contains('\0') {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } => "record",
                TypeDeclarationKind::Variant { .. } => "variant",
            };
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-S102",
                format!("{kind} `{}`", declaration.name),
                declaration.span,
            ));
        } else if !ids.insert(declaration.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", declaration.stable_id),
                declaration.span,
            ));
        }
        if !declaration.explicit_id {
            let (subject, help) = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => ("resource", "your.namespace.resource"),
                TypeDeclarationKind::Record { .. } => ("record", "your.namespace.record"),
                TypeDeclarationKind::Variant { .. } => ("variant", "your.namespace.variant"),
            };
            diagnostics.push(
                Diagnostic::warning(
                    "SPX-S108",
                    format!(
                        "{subject} `{}` has an automatic identity that changes when renamed",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(&program.path)
                .with_help(format!("add @id(\"{help}\") before the declaration")),
            );
        }
        if let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind {
            if lifecycles.is_empty() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O112",
                        format!(
                            "resource `{}` must declare exactly one destruction strategy",
                            declaration.name
                        ),
                        declaration.name_span,
                    )
                    .with_help(
                        "declare an explicitly identified `drop trivial` or `drop import` strategy",
                    ),
                );
            } else if lifecycles.len() > 1 {
                diagnostics.push(error(
                    program,
                    "SPX-O113",
                    format!(
                        "resource `{}` declares more than one destruction strategy",
                        declaration.name
                    ),
                    lifecycles[1].span,
                ));
            }
            for lifecycle in lifecycles {
                if let ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind {
                    if import_key.contains('\0') {
                        diagnostics.push(error(
                            program,
                            "SPX-O113",
                            format!(
                                "resource lifecycle `{}.drop` has an invalid logical import key; persistent identities forbid NUL",
                                declaration.name
                            ),
                            lifecycle.span,
                        ));
                    }
                }
                match lifecycle.stable_id.as_deref() {
                    Some(id) if !id.is_empty() => {
                        if id.contains('\0') {
                            diagnostics.push(invalid_stable_id(
                                program,
                                "SPX-O113",
                                format!("resource lifecycle `{}.drop`", declaration.name),
                                lifecycle.span,
                            ));
                        } else if !ids.insert(id) {
                            diagnostics.push(error(
                                program,
                                "SPX-S102",
                                format!("duplicate stable id `{id}`"),
                                lifecycle.span,
                            ));
                        }
                    }
                    _ => diagnostics.push(
                        error(
                            program,
                            "SPX-O113",
                            format!(
                                "resource lifecycle `{}.drop` requires an explicit @id",
                                declaration.name
                            ),
                            lifecycle.span,
                        )
                        .with_help("add @id(\"your.namespace.resource.drop\") before `drop`"),
                    ),
                }
            }
        }
        if let TypeDeclarationKind::Record { fields } = &declaration.kind {
            let mut field_names = HashSet::new();
            let mut field_ids = HashSet::new();
            for field in fields {
                if !source_identifier(&field.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S110",
                        format!("`{}` is not a valid field identifier", field.name),
                        field.name_span,
                    ));
                }
                if !field_names.insert(field.name.as_str())
                    || !field_ids.insert(field.stable_id.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-S111",
                        format!(
                            "duplicate field `{}` in record `{}`",
                            field.name, declaration.name
                        ),
                        field.span,
                    ));
                }
                if field.stable_id.contains('\0') {
                    diagnostics.push(invalid_stable_id(
                        program,
                        "SPX-S102",
                        format!("field `{}.{}`", declaration.name, field.name),
                        field.span,
                    ));
                } else if !ids.insert(field.stable_id.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-S102",
                        format!("duplicate stable id `{}`", field.stable_id),
                        field.span,
                    ));
                }
                if !field.explicit_id {
                    diagnostics.push(
                        Diagnostic::warning(
                            "SPX-S112",
                            format!(
                                "field `{}.{}` has an automatic identity that changes when renamed",
                                declaration.name, field.name
                            ),
                            field.name_span,
                        )
                        .at_path(&program.path)
                        .with_help("add @id(\"your.namespace.record.field\") before the field"),
                    );
                }
            }
        }
        if let TypeDeclarationKind::Variant { cases } = &declaration.kind {
            if cases.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!(
                        "variant `{}` must declare at least one case",
                        declaration.name
                    ),
                    declaration.span,
                ));
            }
            let mut case_names = HashSet::new();
            let mut case_ids = HashSet::new();
            for case in cases {
                if !source_identifier(&case.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S110",
                        format!("`{}` is not a valid variant case identifier", case.name),
                        case.name_span,
                    ));
                }
                if !case_names.insert(case.name.as_str())
                    || !case_ids.insert(case.stable_id.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-S111",
                        format!(
                            "duplicate case `{}` in variant `{}`",
                            case.name, declaration.name
                        ),
                        case.span,
                    ));
                }
                if case.stable_id.contains('\0') {
                    diagnostics.push(invalid_stable_id(
                        program,
                        "SPX-S102",
                        format!("case `{}::{}`", declaration.name, case.name),
                        case.span,
                    ));
                } else if !ids.insert(case.stable_id.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-S102",
                        format!("duplicate stable id `{}`", case.stable_id),
                        case.span,
                    ));
                }
                if !case.explicit_id {
                    diagnostics.push(
                        Diagnostic::warning(
                            "SPX-S112",
                            format!(
                                "case `{}::{}` has an automatic identity that changes when renamed",
                                declaration.name, case.name
                            ),
                            case.name_span,
                        )
                        .at_path(&program.path)
                        .with_help("add @id(\"your.namespace.variant.case\") before the case"),
                    );
                }
                let mut field_names = HashSet::new();
                let mut field_ids = HashSet::new();
                for field in &case.fields {
                    if !source_identifier(&field.name) {
                        diagnostics.push(error(
                            program,
                            "SPX-S110",
                            format!("`{}` is not a valid case field identifier", field.name),
                            field.name_span,
                        ));
                    }
                    if !field_names.insert(field.name.as_str())
                        || !field_ids.insert(field.stable_id.as_str())
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-S111",
                            format!(
                                "duplicate field `{}` in case `{}::{}`",
                                field.name, declaration.name, case.name
                            ),
                            field.span,
                        ));
                    }
                    if field.stable_id.contains('\0') {
                        diagnostics.push(invalid_stable_id(
                            program,
                            "SPX-S102",
                            format!(
                                "case field `{}::{}.{}`",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    } else if !ids.insert(field.stable_id.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-S102",
                            format!("duplicate stable id `{}`", field.stable_id),
                            field.span,
                        ));
                    }
                    if !field.explicit_id {
                        diagnostics.push(
                            Diagnostic::warning(
                                "SPX-S112",
                                format!(
                                    "case field `{}::{}.{}` has an automatic identity that changes when renamed",
                                    declaration.name, case.name, field.name
                                ),
                                field.name_span,
                            )
                            .at_path(&program.path)
                            .with_help(
                                "add @id(\"your.namespace.variant.case.field\") before the field",
                            ),
                        );
                    }
                }
            }
        }
    }

    let mut interface_names = HashSet::new();
    let mut import_keys = HashMap::new();
    for interface in &program.interfaces {
        if !source_identifier(&interface.name) {
            diagnostics.push(error(
                program,
                "SPX-I403",
                format!("`{}` is not a valid interface identifier", interface.name),
                interface.name_span,
            ));
        }
        if !interface_names.insert(interface.name.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-I403",
                format!("duplicate interface `{}`", interface.name),
                interface.span,
            ));
        }
        if !interface.explicit_id || interface.stable_id.is_empty() {
            diagnostics.push(
                error(
                    program,
                    "SPX-I403",
                    format!("interface `{}` requires an explicit @id", interface.name),
                    interface.name_span,
                )
                .with_help("add @id(\"your.namespace.interface\") before the interface"),
            );
        }
        if interface.stable_id.contains('\0') {
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-I403",
                format!("interface `{}`", interface.name),
                interface.span,
            ));
        } else if !ids.insert(interface.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", interface.stable_id),
                interface.span,
            ));
        }
        let mut permitted = HashSet::new();
        for effect in &interface.permits {
            if !permitted.insert(effect.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!(
                        "interface `{}` declares duplicate permit `{effect}`",
                        interface.name
                    ),
                    interface.span,
                ));
            }
        }
        let mut import_names = HashSet::new();
        for import in &interface.imports {
            if !source_identifier(&import.name) {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!("`{}` is not a valid import identifier", import.name),
                    import.name_span,
                ));
            }
            if !import_names.insert(import.name.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!("duplicate import `{}.{}`", interface.name, import.name),
                    import.span,
                ));
            }
            if !import.explicit_id || import.stable_id.is_empty() {
                if import.native_rust {
                    diagnostics.push(error(
                        program,
                        "SPX-B107",
                        "Native Rust Interop declaration set is unsupported: explicit persistent ID required",
                        import.name_span,
                    ));
                } else {
                    diagnostics.push(
                        error(
                            program,
                            "SPX-I403",
                            format!(
                                "import `{}.{}` requires an explicit @id",
                                interface.name, import.name
                            ),
                            import.name_span,
                        )
                        .with_help(
                            "the v1 import @id is also its target-neutral logical import key",
                        ),
                    );
                }
            }
            let import_identity_is_valid = !import.stable_id.contains('\0');
            if !import_identity_is_valid {
                diagnostics.push(invalid_stable_id(
                    program,
                    "SPX-I403",
                    format!("import `{}.{}`", interface.name, import.name),
                    import.span,
                ));
            } else if !ids.insert(import.stable_id.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-S102",
                    format!("duplicate stable id `{}`", import.stable_id),
                    import.span,
                ));
            }
            if import_identity_is_valid
                && import_keys
                    .insert(import.stable_id.as_str(), (interface, import))
                    .is_some()
            {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!("duplicate logical import key `{}`", import.stable_id),
                    import.span,
                ));
            }
        }
    }

    let types = TypeTable::new(program);
    let mut native_rust_names = HashSet::new();
    for interface in &program.interfaces {
        let permits = interface
            .permits
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for import in &interface.imports {
            if import.native_rust && !native_rust_names.insert(import.name.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: symbol collision",
                    import.span,
                ));
            }
            if import.native_rust
                && program
                    .functions
                    .iter()
                    .any(|function| function.name == import.name)
            {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: symbol collision",
                    import.span,
                ));
            }
            for param in &import.params {
                check_declared_type(
                    program,
                    &param.ty,
                    param.span,
                    &types,
                    &HashSet::new(),
                    &mut diagnostics,
                );
            }
            let valid_shape = if import.native_rust {
                import.params.len() <= 8
                    && import.consumes.is_empty()
                    && import.params.iter().all(|parameter| {
                        parameter.mode == ParamMode::Value
                            && matches!(parameter.ty, Type::I64 | Type::Bool)
                    })
            } else {
                import.result == crate::ast::ImportResult::Unit
                    && import.params.len() == 1
                    && import.params[0].mode == ParamMode::Own
                    && types.is_opaque_resource(&import.params[0].ty)
                    && import.consumes == import.params[0].name
            };
            if !valid_shape {
                diagnostics.push(error(
                    program,
                    if import.native_rust { "SPX-B107" } else { "SPX-I404" },
                    if import.native_rust {
                        "Native Rust Interop declaration set is unsupported: scalar value signature required".to_owned()
                    } else {
                        format!(
                            "import `{}.{}` must take one owned resource parameter and consume it always",
                            interface.name, import.name
                        )
                    },
                    import.span,
                ));
            }
            if let ImportFailure::Status { domain_id } = &import.failure {
                if (import.native_rust && !native_rust_status_domain(domain_id))
                    || (!import.native_rust
                        && (domain_id.is_empty()
                            || domain_id.len() > STATUS_DOMAIN_MAX_BYTES_V1
                            || domain_id.contains('\0')))
                {
                    diagnostics.push(error(
                        program,
                        if import.native_rust { "SPX-B107" } else { "SPX-I403" },
                        if import.native_rust {
                            "Native Rust Interop declaration set is unsupported: status domain is invalid".to_owned()
                        } else {
                            format!(
                                "import `{}.{}` has an invalid failure domain",
                                interface.name, import.name
                            )
                        },
                        import.span,
                    ));
                }
            }
            let mut effects = HashSet::new();
            if import.native_rust
                && import
                    .effects
                    .windows(2)
                    .any(|pair| pair[0].as_str() >= pair[1].as_str())
            {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                    import.span,
                ));
            }
            for effect in &import.effects {
                if !effects.insert(effect.as_str()) {
                    diagnostics.push(if import.native_rust {
                        error(
                            program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                            import.span,
                        )
                    } else {
                        error(
                            program,
                            "SPX-I403",
                            format!(
                                "import `{}.{}` declares duplicate effect `{effect}`",
                                interface.name, import.name
                            ),
                            import.span,
                        )
                    });
                }
                if !permits.contains(effect.as_str()) {
                    diagnostics.push(if import.native_rust {
                        error(
                            program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                            import.span,
                        )
                    } else {
                        error(
                            program,
                            "SPX-I404",
                            format!(
                                "import `{}.{}` requires effect `{effect}` outside interface `{}` permits",
                                interface.name, import.name, interface.name
                            ),
                            import.span,
                        )
                    });
                }
            }
        }
    }
    for declaration in &program.types {
        let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
            continue;
        };
        if let [lifecycle] = lifecycles.as_slice() {
            if let ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind {
                if import_key.contains('\0') {
                    continue;
                }
                let compatible = import_keys
                    .get(import_key.as_str())
                    .is_some_and(|(_, import)| {
                        !import.native_rust
                            && import.params.len() == 1
                            && import.params[0].mode == ParamMode::Own
                            && import.params[0].ty
                                == (Type::Named {
                                    name: declaration.name.clone(),
                                    arguments: Vec::new(),
                                })
                            && import.consumes == import.params[0].name
                            && matches!(import.failure, ImportFailure::Infallible)
                    });
                if !compatible {
                    let message = if import_keys.contains_key(import_key.as_str()) {
                        format!(
                            "logical import `{import_key}` is incompatible with automatic finalization of `{}`",
                            declaration.name
                        )
                    } else {
                        format!(
                            "resource `{}` references unknown logical import `{import_key}`",
                            declaration.name
                        )
                    };
                    diagnostics.push(error(program, "SPX-O113", message, lifecycle.span));
                }
            }
        }
    }
    for declaration in &program.types {
        if let TypeDeclarationKind::Record { fields } = &declaration.kind {
            let parameters = declaration
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>();
            for field in fields {
                check_declared_type(
                    program,
                    &field.ty,
                    field.span,
                    &types,
                    &parameters,
                    &mut diagnostics,
                );
                if !parameters.is_empty() {
                    let is_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty() && parameters.contains(name.as_str())
                    );
                    let is_unknown_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty() && types.declaration(name).is_none()
                    );
                    if !matches!(field.ty, Type::I64 | Type::Bool)
                        && !is_parameter
                        && !is_unknown_parameter
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T223",
                            format!(
                                "generic record field `{}.{}` must have direct `i64`, `bool`, or an in-scope record type parameter",
                                declaration.name, field.name
                            ),
                            field.span,
                        ));
                    }
                } else if matches!(
                    &field.ty,
                    Type::Named { name, arguments }
                        if !arguments.is_empty()
                            && matches!(
                                types.declaration(name).map(|item| &item.kind),
                                Some(TypeDeclarationKind::Record { .. })
                            )
                ) {
                    diagnostics.push(error(
                        program,
                        "SPX-T223",
                        format!(
                            "record field `{}.{}` cannot nest a generic record instance in this slice",
                            declaration.name, field.name
                        ),
                        field.span,
                    ));
                }
            }
        }
        if let TypeDeclarationKind::Variant { cases } = &declaration.kind {
            let parameters = declaration
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>();
            for case in cases {
                for field in &case.fields {
                    check_declared_type(
                        program,
                        &field.ty,
                        field.span,
                        &types,
                        &parameters,
                        &mut diagnostics,
                    );
                    let is_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty() && parameters.contains(name.as_str())
                    );
                    let is_unknown_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty()
                                && !parameters.is_empty()
                                && types.declaration(name).is_none()
                    );
                    if !matches!(field.ty, Type::I64 | Type::Bool)
                        && !is_parameter
                        && !is_unknown_parameter
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "case field `{}::{}.{}` must have direct `i64`, `bool`, or an in-scope variant type parameter in Copy Variants v1",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    }
                }
            }
        }
    }
    let mut checked_layouts = HashSet::new();
    for declaration in &program.types {
        if matches!(declaration.kind, TypeDeclarationKind::Record { .. })
            && record_layout_is_recursive(
                declaration.name.as_str(),
                &types,
                &mut HashSet::new(),
                &mut checked_layouts,
            )
        {
            diagnostics.push(error(
                program,
                "SPX-T217",
                format!(
                    "record `{}` has an illegal recursive by-value layout",
                    declaration.name
                ),
                declaration.span,
            ));
            break;
        }
    }

    for function in &program.functions {
        if !source_identifier(&function.name) {
            diagnostics.push(error(
                program,
                "SPX-S104",
                format!("`{}` is not a valid function identifier", function.name),
                function.name_span,
            ));
        }
        if functions.insert(function.name.as_str(), function).is_some() {
            diagnostics.push(error(
                program,
                "SPX-S101",
                format!("duplicate function `{}`", function.name),
                function.name_span,
            ));
        }
        if function.stable_id.contains('\0') {
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-S102",
                format!("function `{}`", function.name),
                function.span,
            ));
        } else if function
            .stable_id
            .starts_with("semaprax.function-execution.v1:")
        {
            diagnostics.push(error(
                program,
                "SPX-T225",
                format!(
                    "function `{}` uses the reserved generic execution identity domain",
                    function.name
                ),
                function.span,
            ));
        } else if !ids.insert(function.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", function.stable_id),
                function.span,
            ));
        }
        if !function.explicit_id {
            diagnostics.push(
                Diagnostic::warning(
                    "SPX-S103",
                    format!(
                        "function `{}` has an automatic identity that changes when renamed",
                        function.name
                    ),
                    function.name_span,
                )
                .at_path(&program.path)
                .with_help("add @id(\"your.namespace.symbol\") before the declaration"),
            );
        }
        if !function.type_parameters.is_empty() {
            if !(1..=2).contains(&function.type_parameters.len()) {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` requires one or two type parameters",
                        function.name
                    ),
                    function.span,
                ));
            }
            let mut parameter_names = HashSet::new();
            for parameter in &function.type_parameters {
                if !source_identifier(&parameter.name)
                    || !parameter_names.insert(parameter.name.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "invalid or duplicate type parameter `{}` on function `{}`",
                            parameter.name, function.name
                        ),
                        parameter.span,
                    ));
                }
            }
            if !function.effects.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T226",
                    format!(
                        "generic function `{}` must be effect-free in this slice",
                        function.name
                    ),
                    function.span,
                ));
            }
            for param in &function.params {
                if param.mode != ParamMode::Value
                    || !generic_function_signature_slot(&param.ty, &parameter_names)
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T224",
                        format!(
                            "generic function `{}.{}` must use a direct `i64`, `bool`, or an in-scope function type parameter by value",
                            function.name, param.name
                        ),
                        param.span,
                    ));
                }
            }
            if !generic_function_signature_slot(&function.return_type, &parameter_names) {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "generic function `{}` must return direct `i64`, `bool`, or an in-scope function type parameter",
                        function.name
                    ),
                    function.span,
                ));
            }
            if function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| !generic_function_expression_is_direct_scalar(expression))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T226",
                    format!(
                        "generic function `{}` uses an expression outside the direct-scalar slice",
                        function.name
                    ),
                    function.span,
                ));
            }
        }
    }

    let call_graph = program
        .functions
        .iter()
        .map(|function| {
            let mut callees = Vec::new();
            for contract in &function.requires {
                contract.visit_calls(&mut |callee, _| callees.push(callee.to_owned()));
            }
            function
                .body
                .visit_calls(&mut |callee, _| callees.push(callee.to_owned()));
            for contract in &function.ensures {
                contract.visit_calls(&mut |callee, _| callees.push(callee.to_owned()));
            }
            (function.name.clone(), callees)
        })
        .collect::<HashMap<_, _>>();
    let generic_functions = program
        .functions
        .iter()
        .filter(|function| !function.type_parameters.is_empty())
        .map(|function| function.name.as_str())
        .collect::<HashSet<_>>();
    for function in program
        .functions
        .iter()
        .filter(|function| !function.type_parameters.is_empty())
    {
        let participates_in_cycle = call_graph.get(&function.name).is_some_and(|callees| {
            callees.iter().any(|callee| {
                function_reaches(&call_graph, callee, &function.name, &mut HashSet::new())
            })
        });
        if participates_in_cycle {
            diagnostics.push(error(
                program,
                "SPX-T226",
                format!(
                    "generic function `{}` participates in a recursive call cycle",
                    function.name
                ),
                function.span,
            ));
        }
        let direct_generic_call = call_graph.get(&function.name).is_some_and(|callees| {
            callees
                .iter()
                .any(|callee| generic_functions.contains(callee.as_str()))
        });
        let reaches_other_generic = call_graph.get(&function.name).is_some_and(|callees| {
            callees.iter().any(|callee| {
                function_reaches_any(&call_graph, callee, &generic_functions, &mut HashSet::new())
            })
        });
        if reaches_other_generic && !direct_generic_call && !participates_in_cycle {
            diagnostics.push(error(
                program,
                "SPX-T226",
                format!(
                    "generic function `{}` transitively reaches another generic function",
                    function.name
                ),
                function.span,
            ));
        }
    }

    for template in &program.functions {
        let type_parameters = template
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        check_declared_type(
            program,
            &template.return_type,
            template.span,
            &types,
            &type_parameters,
            &mut diagnostics,
        );
        for param in &template.params {
            check_declared_type(
                program,
                &param.ty,
                param.span,
                &types,
                &type_parameters,
                &mut diagnostics,
            );
        }
        let generic_parameter_list_is_valid = (1..=2).contains(&template.type_parameters.len())
            && template
                .type_parameters
                .iter()
                .all(|parameter| source_identifier(&parameter.name))
            && template
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>()
                .len()
                == template.type_parameters.len();
        let specializations = if template.type_parameters.is_empty() {
            vec![template.clone()]
        } else if generic_parameter_list_is_valid {
            // These clones exist only to validate every admitted direct-scalar
            // substitution. Executable HIR instances are discovered separately
            // from reachable explicit calls and never originate here.
            scalar_function_substitutions(template.type_parameters.len())
                .iter()
                .filter_map(|arguments| validation_specialize_function(template, arguments))
                .collect()
        } else {
            Vec::new()
        };
        let mut specialized_diagnostics = HashSet::new();
        for function in &specializations {
            let specialized_diagnostic_start = diagnostics.len();
            let mut variables = HashMap::new();
            for param in &function.params {
                if !source_identifier(&param.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S105",
                        format!("`{}` is not a valid parameter identifier", param.name),
                        param.span,
                    ));
                }
                check_ownership_mode(program, function, param, &types, &mut diagnostics);
                if variables
                    .insert(
                        param.name.clone(),
                        Binding {
                            ty: param.ty.clone(),
                            mode: param.mode,
                            availability: Availability::Available,
                            moved_places: HashMap::new(),
                            definitely_partial: HashSet::new(),
                            native_unit_discard: false,
                        },
                    )
                    .is_some()
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T102",
                        format!("duplicate parameter `{}`", param.name),
                        param.span,
                    ));
                }
            }

            let entry_variables = variables.clone();
            for contract in &function.requires {
                require_bool(
                    program,
                    function,
                    contract,
                    &entry_variables,
                    &functions,
                    &types,
                    None,
                    &mut diagnostics,
                    "precondition",
                );
            }

            if let Some(actual) = check_expr_iterative(
                program,
                function,
                &function.body,
                &mut variables,
                &functions,
                &types,
                None,
                true,
                &mut diagnostics,
            ) {
                if actual.native_unit {
                    reject_native_unit_value(program, &function.body, &actual, &mut diagnostics);
                }
                if !actual.native_unit && actual.ty != function.return_type {
                    diagnostics.push(error(
                        program,
                        "SPX-T103",
                        format!(
                            "function `{}` returns {}, but its signature declares {}",
                            function.name, actual.ty, function.return_type
                        ),
                        function.body.span,
                    ));
                }
                if types.contains_resource(&function.return_type) && actual.mode != ParamMode::Own {
                    diagnostics.push(
                        error(
                            program,
                            "SPX-O104",
                            format!(
                                "function `{}` cannot return a {} resource as owned",
                                function.name,
                                actual.mode.text()
                            ),
                            function.body.span,
                        )
                        .with_help(
                            "return an owned resource or declare a future lifetime-bound view",
                        ),
                    );
                }
            }

            for contract in &function.ensures {
                require_bool(
                    program,
                    function,
                    contract,
                    &variables,
                    &functions,
                    &types,
                    Some(&function.return_type),
                    &mut diagnostics,
                    "postcondition",
                );
            }

            let declared: HashSet<_> = function.effects.iter().map(String::as_str).collect();
            let mut required_lifecycle_effects = BTreeSet::new();
            for param in &function.params {
                if param.mode == ParamMode::Own {
                    required_lifecycle_effects
                        .extend(types.lifecycle_effects(&param.ty, &import_keys));
                }
            }
            required_lifecycle_effects
                .extend(types.lifecycle_effects(&function.return_type, &import_keys));
            function.body.visit_calls(&mut |callee, _| {
                if let Some(target) = functions.get(callee) {
                    required_lifecycle_effects
                        .extend(types.lifecycle_effects(&target.return_type, &import_keys));
                }
            });
            for effect in required_lifecycle_effects {
                if !declared.contains(effect.as_str()) {
                    diagnostics.push(
                    error(
                        program,
                        "SPX-E103",
                        format!(
                            "function `{}` can own a resource; automatic finalization requires effect `{effect}`",
                            function.name
                        ),
                        function.span,
                    )
                    .with_help(format!(
                        "add `{effect}` to the function's `uses` set and module permits"
                    )),
                );
                }
            }
            for effect in &function.effects {
                if !program.permits.iter().any(|permit| permit == effect) {
                    diagnostics.push(error(
                        program,
                        "SPX-E101",
                        format!(
                            "function `{}` uses `{effect}` but module `{}` does not permit it",
                            function.name, program.module
                        ),
                        function.span,
                    ));
                }
            }
            function.body.visit_calls(&mut |callee, span| {
                if let Some(target) = functions.get(callee) {
                    for effect in &target.effects {
                        if !declared.contains(effect.as_str()) {
                            diagnostics.push(error(
                                program,
                                "SPX-E102",
                                format!(
                                    "call to `{callee}` requires effect `{effect}`; add it to `{}`",
                                    function.name
                                ),
                                span,
                            ));
                        }
                    }
                }
            });
            if !template.type_parameters.is_empty() {
                let added = diagnostics
                    .drain(specialized_diagnostic_start..)
                    .collect::<Vec<_>>();
                for diagnostic in added {
                    if specialized_diagnostics.insert(diagnostic.json()) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }
    }

    if let Some(main) = functions.get("main") {
        if !main.type_parameters.is_empty()
            || !main.params.is_empty()
            || main.return_type != Type::I64
        {
            diagnostics.push(error(
                program,
                "SPX-T104",
                "entry function must be monomorphic with signature `fn main() -> i64`",
                main.span,
            ));
        }
    } else {
        diagnostics.push(
            Diagnostic::error(
                "SPX-T105",
                "executable module must define `fn main() -> i64`",
                program
                    .functions
                    .first()
                    .map_or(Span::default(), |function| function.span),
            )
            .at_path(&program.path),
        );
    }
    let mut native_interop_failures = HashSet::new();
    diagnostics.retain(|diagnostic| {
        diagnostic.code != "SPX-B107" || native_interop_failures.insert(diagnostic.message.clone())
    });
    if let Some(native_failure) = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-B107")
        .cloned()
    {
        return vec![native_failure];
    }
    diagnostics
}

fn native_rust_status_domain(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=128).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && (bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

fn record_layout_is_recursive(
    name: &str,
    types: &TypeTable<'_>,
    visiting: &mut HashSet<String>,
    checked: &mut HashSet<String>,
) -> bool {
    enum Frame<'a> {
        Enter(&'a str),
        Fields {
            name: &'a str,
            fields: &'a [FieldDeclaration],
            parameters: HashSet<&'a str>,
            index: usize,
        },
    }

    let mut frames = vec![Frame::Enter(name)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(name) => {
                if checked.contains(name) {
                    results.push(false);
                    continue;
                }
                if !visiting.insert(name.to_owned()) {
                    results.push(true);
                    continue;
                }
                let Some(declaration) = types.declaration(name) else {
                    visiting.remove(name);
                    checked.insert(name.to_owned());
                    results.push(false);
                    continue;
                };
                let TypeDeclarationKind::Record { fields } = &declaration.kind else {
                    visiting.remove(name);
                    checked.insert(name.to_owned());
                    results.push(false);
                    continue;
                };
                let parameters = declaration
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<HashSet<_>>();
                frames.push(Frame::Fields {
                    name,
                    fields,
                    parameters,
                    index: 0,
                });
            }
            Frame::Fields {
                name,
                fields,
                parameters,
                mut index,
            } => {
                if results.pop().unwrap_or(false) {
                    visiting.remove(name);
                    results.push(true);
                    continue;
                }
                let mut child = None;
                while let Some(field) = fields.get(index) {
                    index += 1;
                    let Type::Named {
                        name: field_type,
                        arguments,
                    } = &field.ty
                    else {
                        continue;
                    };
                    if arguments.is_empty() && parameters.contains(field_type.as_str()) {
                        continue;
                    }
                    if matches!(
                        types.declaration(field_type).map(|item| &item.kind),
                        Some(TypeDeclarationKind::Record { .. })
                    ) {
                        child = Some(field_type.as_str());
                        break;
                    }
                }
                if let Some(child) = child {
                    frames.push(Frame::Fields {
                        name,
                        fields,
                        parameters,
                        index,
                    });
                    frames.push(Frame::Enter(child));
                } else {
                    visiting.remove(name);
                    checked.insert(name.to_owned());
                    results.push(false);
                }
            }
        }
    }
    results.pop().unwrap_or(false)
}

fn check_declared_type(
    program: &Program,
    ty: &Type,
    span: Span,
    types: &TypeTable<'_>,
    parameters: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        let Type::Named { name, arguments } = ty else {
            continue;
        };
        if parameters.contains(name.as_str()) {
            if !arguments.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T220",
                    format!("type parameter `{name}` cannot take type arguments"),
                    span,
                ));
            }
            continue;
        }
        let Some(declaration) = types.declaration(name) else {
            let (code, message) = if parameters.is_empty() {
                (
                    "SPX-T001",
                    format!("unknown type `{name}`; declare it with `resource {name};`"),
                )
            } else {
                (
                    "SPX-T220",
                    format!("`{name}` is not an in-scope type parameter"),
                )
            };
            diagnostics.push(error(program, code, message, span));
            continue;
        };
        if arguments.len() != declaration.type_parameters.len() {
            diagnostics.push(error(
                program,
                "SPX-T221",
                format!(
                    "type `{name}` expects {} type arguments, received {}",
                    declaration.type_parameters.len(),
                    arguments.len()
                ),
                span,
            ));
        }
        if !arguments.is_empty()
            && (!matches!(
                declaration.kind,
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Variant { .. }
            ) || arguments
                .iter()
                .any(|argument| !matches!(argument, Type::I64 | Type::Bool)))
        {
            diagnostics.push(error(
                program,
                "SPX-T223",
                format!("generic copy type `{name}` accepts only direct `i64` or `bool` arguments"),
                span,
            ));
        }
        pending.extend(arguments.iter().rev());
    }
}

fn direct_function_type_argument(ty: &Type) -> bool {
    matches!(ty, Type::I64 | Type::Bool)
}

fn generic_function_signature_slot(ty: &Type, parameters: &HashSet<&str>) -> bool {
    match ty {
        Type::I64 | Type::Bool => true,
        Type::Char | Type::U8 | Type::F32 | Type::F64 => false,
        Type::Named { name, arguments } => {
            arguments.is_empty() && parameters.contains(name.as_str())
        }
    }
}

fn substitute_function_type(
    function: &Function,
    arguments: &[Type],
    template: &Type,
) -> Option<Type> {
    enum Frame<'a> {
        Enter(&'a Type),
        Finish(&'a str, usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                Type::I64 => resolved.push(Type::I64),
                Type::Char => resolved.push(Type::Char),
                Type::U8 => resolved.push(Type::U8),
                Type::F32 => resolved.push(Type::F32),
                Type::F64 => resolved.push(Type::F64),
                Type::Bool => resolved.push(Type::Bool),
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
                            resolved.push(arguments.get(index)?.clone());
                            continue;
                        }
                    }
                    frames.push(Frame::Finish(name, nested.len()));
                    frames.extend(nested.iter().rev().map(Frame::Enter));
                }
            },
            Frame::Finish(name, count) => {
                let split = resolved.len().checked_sub(count)?;
                let arguments = resolved.drain(split..).collect();
                resolved.push(Type::Named {
                    name: name.to_owned(),
                    arguments,
                });
            }
        }
    }
    (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
}

fn scalar_function_substitutions(parameter_count: usize) -> Vec<Vec<Type>> {
    let count = 1_usize << parameter_count;
    (0..count)
        .map(|bits| {
            (0..parameter_count)
                .map(|index| {
                    if bits & (1 << index) == 0 {
                        Type::I64
                    } else {
                        Type::Bool
                    }
                })
                .collect()
        })
        .collect()
}

fn generic_function_expression_is_direct_scalar(expression: &Expr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::Var(_) => {}
            ExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
            ExprKind::Unary { value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ExprKind::Block { statements, tail } => {
                pending.push(tail);
                pending.extend(statements.iter().rev().map(|statement| match statement {
                    crate::ast::Statement::Let { value, .. } => value,
                }));
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ExprKind::ConstructRecord { .. }
            | ExprKind::ConstructVariant { .. }
            | ExprKind::Match { .. }
            | ExprKind::Try { .. }
            | ExprKind::UpdateRecord { .. }
            | ExprKind::Project { .. } => return false,
        }
    }
    true
}

fn function_reaches(
    graph: &HashMap<String, Vec<String>>,
    current: &str,
    target: &str,
    visited: &mut HashSet<String>,
) -> bool {
    let mut pending = vec![current];
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if !visited.insert(current.to_owned()) {
            continue;
        }
        if let Some(callees) = graph.get(current) {
            pending.extend(callees.iter().rev().map(String::as_str));
        }
    }
    false
}

fn function_reaches_any(
    graph: &HashMap<String, Vec<String>>,
    current: &str,
    targets: &HashSet<&str>,
    visited: &mut HashSet<String>,
) -> bool {
    let mut pending = vec![current];
    while let Some(current) = pending.pop() {
        if targets.contains(current) {
            return true;
        }
        if !visited.insert(current.to_owned()) {
            continue;
        }
        if let Some(callees) = graph.get(current) {
            pending.extend(callees.iter().rev().map(String::as_str));
        }
    }
    false
}

fn validation_specialize_function(function: &Function, arguments: &[Type]) -> Option<Function> {
    let mut specialized = function.clone();
    for param in &mut specialized.params {
        param.ty = substitute_function_type(function, arguments, &param.ty)?;
    }
    specialized.return_type = substitute_function_type(function, arguments, &function.return_type)?;
    Some(specialized)
}

fn validation_specialize_signature(
    function: &Function,
    arguments: &[Type],
) -> Option<(Vec<Param>, Type)> {
    let mut params = Vec::with_capacity(function.params.len());
    for parameter in &function.params {
        let mut specialized = parameter.clone();
        specialized.ty = substitute_function_type(function, arguments, &parameter.ty)?;
        params.push(specialized);
    }
    let return_type = substitute_function_type(function, arguments, &function.return_type)?;
    Some((params, return_type))
}

fn check_ownership_mode(
    program: &Program,
    function: &Function,
    param: &crate::ast::Param,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match (types.contains_resource(&param.ty), param.mode) {
        (true, ParamMode::Value) => diagnostics.push(
            error(
                program,
                "SPX-O001",
                format!(
                    "resource parameter `{}.{}` needs `own`, `borrow`, or `shared`",
                    function.name, param.name
                ),
                param.span,
            )
            .with_help(format!(
                "use `{}: own {}` to transfer ownership",
                param.name, param.ty
            )),
        ),
        (false, mode) if mode != ParamMode::Value => diagnostics.push(error(
            program,
            "SPX-O002",
            format!(
                "ownership mode `{}` is only valid for resource types; `{}` is a value type",
                mode.text(),
                param.ty
            ),
            param.span,
        )),
        _ => {}
    }
}

fn ordinary_result_arguments(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if name != "Result" || arguments.len() != 2 {
        return None;
    }
    Some((&arguments[0], &arguments[1]))
}

fn ordinary_option_argument(ty: &Type) -> Option<&Type> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if name != "Option" || arguments.len() != 1 {
        return None;
    }
    Some(&arguments[0])
}

#[allow(clippy::too_many_arguments)]
fn check_record_pattern(
    program: &Program,
    pattern_type: &str,
    fields: &[RecordMatchPatternField],
    expected: &Type,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    enum Frame<'a, 't> {
        Enter {
            pattern_type: &'a str,
            fields: &'a [RecordMatchPatternField],
            expected: Type,
            span: Span,
        },
        Fields {
            pattern_type: &'a str,
            fields: &'a [RecordMatchPatternField],
            expected: Type,
            declared_fields: &'t [FieldDeclaration],
            index: usize,
            supplied: HashSet<&'a str>,
            span: Span,
        },
    }

    let mut frames = vec![Frame::Enter {
        pattern_type,
        fields,
        expected: expected.clone(),
        span,
    }];
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter {
                pattern_type,
                fields,
                expected,
                span,
            } => {
                let compatible = matches!(
                    &expected,
                    Type::Named { name, .. } if name == pattern_type
                );
                let declared_fields = types.record_fields(&expected);
                if !compatible || declared_fields.is_none() || types.contains_resource(&expected) {
                    diagnostics.push(error(
                        program,
                        "SPX-M103",
                        format!(
                            "record pattern `{pattern_type}` is incompatible with `{expected}`"
                        ),
                        span,
                    ));
                    continue;
                }
                frames.push(Frame::Fields {
                    pattern_type,
                    fields,
                    expected,
                    declared_fields: declared_fields.expect("checked above"),
                    index: 0,
                    supplied: HashSet::new(),
                    span,
                });
            }
            Frame::Fields {
                pattern_type,
                fields,
                expected,
                declared_fields,
                index,
                mut supplied,
                span,
            } => {
                let Some(field) = fields.get(index) else {
                    for declared in declared_fields {
                        if !supplied.contains(declared.name.as_str()) {
                            diagnostics.push(error(
                                program,
                                "SPX-M104",
                                format!(
                                    "record pattern `{pattern_type}` is missing field `{}`",
                                    declared.name
                                ),
                                span,
                            ));
                        }
                    }
                    continue;
                };
                let declared = declared_fields
                    .iter()
                    .find(|candidate| candidate.name == field.name);
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-M104",
                        format!(
                            "unknown or duplicate record pattern field `{}.{}`",
                            pattern_type, field.name
                        ),
                        field.span,
                    ));
                    frames.push(Frame::Fields {
                        pattern_type,
                        fields,
                        expected,
                        declared_fields,
                        index: index + 1,
                        supplied,
                        span,
                    });
                    continue;
                }
                let declared = declared.expect("checked above");
                let field_ty = types
                    .record_field_type(&expected, declared)
                    .unwrap_or_else(|| declared.ty.clone());
                frames.push(Frame::Fields {
                    pattern_type,
                    fields,
                    expected,
                    declared_fields,
                    index: index + 1,
                    supplied,
                    span,
                });
                match &field.pattern {
                    RecordMatchFieldPattern::Binding { name, span } => {
                        if !source_identifier(name) || variables.contains_key(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-M104",
                                format!("invalid or duplicate record pattern binding `{name}`"),
                                *span,
                            ));
                        } else {
                            variables.insert(
                                name.clone(),
                                Binding {
                                    ty: field_ty,
                                    mode: ParamMode::Value,
                                    availability: Availability::Available,
                                    moved_places: HashMap::new(),
                                    definitely_partial: HashSet::new(),
                                    native_unit_discard: false,
                                },
                            );
                        }
                    }
                    RecordMatchFieldPattern::Wildcard { .. } => {}
                    RecordMatchFieldPattern::Record {
                        type_name,
                        fields,
                        span,
                        ..
                    } => frames.push(Frame::Enter {
                        pattern_type: type_name,
                        fields,
                        expected: field_ty,
                        span: *span,
                    }),
                }
            }
        }
    }
}

struct VerifierScope {
    bindings: HashMap<String, Binding>,
}

enum VerifierFrame<'a> {
    Enter {
        expression: &'a Expr,
        scope: usize,
    },
    ResumeUnary {
        expression: &'a Expr,
        operand: &'a Expr,
        op: UnaryOp,
    },
    ResumeBinaryLeft {
        expression: &'a Expr,
        op: BinaryOp,
        right: &'a Expr,
        scope: usize,
    },
    ResumeBinaryRight {
        expression: &'a Expr,
        op: BinaryOp,
        left: &'a Expr,
        left_value: Option<CheckedValue>,
        scope: usize,
        evaluated_scope: usize,
        baseline_names: Vec<String>,
    },
    ResumeIfCondition {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
    },
    ResumeIfThen {
        expression: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
        then_scope: usize,
        baseline_names: Vec<String>,
    },
    ResumeIfElse {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
        else_scope: usize,
        baseline_names: Vec<String>,
        then_value: Option<CheckedValue>,
        then_bindings: HashMap<String, Binding>,
    },
    ResumeBlockStatement {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
    },
    ResumeBlockTail {
        parent_scope: usize,
        block_scope: usize,
        outer_names: Vec<String>,
    },
    ResumeCallArgument {
        expression: &'a Expr,
        name: &'a str,
        args: &'a [Expr],
        scope: usize,
        index: usize,
        target: VerifierCallTarget<'a>,
    },
    ResumeTry {
        expression: &'a Expr,
        operand: &'a Expr,
        scope: usize,
    },
    ResumeProject {
        expression: &'a Expr,
        base: &'a Expr,
        field: &'a str,
    },
    ResumeRecordField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: Option<&'a [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareRecordField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: Option<&'a [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeVariantField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        case_name: &'a str,
        fields: &'a [crate::ast::FieldInitializer],
        declaration: Option<&'a TypeDeclaration>,
        case: Option<&'a VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareVariantField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        case_name: &'a str,
        fields: &'a [crate::ast::FieldInitializer],
        declaration: Option<&'a TypeDeclaration>,
        case: Option<&'a VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeUpdateBase {
        expression: &'a Expr,
        base: &'a Expr,
        fields: &'a [crate::ast::FieldInitializer],
        scope: usize,
    },
    ResumeUpdateField {
        expression: &'a Expr,
        base_type: Type,
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: &'a [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareUpdateField {
        expression: &'a Expr,
        base_type: Type,
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: &'a [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeMatchScrutinee {
        expression: &'a Expr,
        scrutinee: &'a Expr,
        arms: &'a [crate::ast::MatchArm],
        scope: usize,
    },
    ResumeRecordMatchArm {
        arm: &'a crate::ast::MatchArm,
        parent_scope: usize,
        arm_scope: usize,
        outer_names: Vec<String>,
    },
    PrepareVariantMatchArm(VariantMatchState<'a>),
    ResumeVariantMatchArm {
        state: VariantMatchState<'a>,
        arm_scope: usize,
    },
}

#[allow(dead_code)]
struct VariantMatchState<'a> {
    expression: &'a Expr,
    arms: &'a [crate::ast::MatchArm],
    parent_scope: usize,
    index: usize,
    outer_names: Vec<String>,
    baseline: HashMap<String, Binding>,
    arm_states: Vec<HashMap<String, Binding>>,
    covered: HashSet<String>,
    wildcard_seen: bool,
    result: Option<CheckedValue>,
    variant_name: Option<String>,
    variant_arguments: Vec<Type>,
    declared_cases: Option<&'a [VariantCaseDeclaration]>,
}

enum VerifierCallTarget<'a> {
    Native(&'a ImportDeclaration),
    Ordinary(Option<VerifierFunctionSignature<'a>>),
}

enum VerifierFunctionSignature<'a> {
    Borrowed(&'a Function),
    Specialized {
        params: Vec<Param>,
        return_type: Type,
    },
}

#[cfg(test)]
fn verifier_signature_owned_capacity(signature: &VerifierFunctionSignature<'_>) -> usize {
    match signature {
        VerifierFunctionSignature::Borrowed(_) => 0,
        VerifierFunctionSignature::Specialized {
            params,
            return_type,
        } => params
            .capacity()
            .saturating_mul(std::mem::size_of::<Param>())
            .saturating_add(
                params
                    .iter()
                    .map(|param| {
                        param
                            .name
                            .capacity()
                            .saturating_add(ast_type_owned_capacity(&param.ty))
                    })
                    .sum::<usize>(),
            )
            .saturating_add(ast_type_owned_capacity(return_type)),
    }
}

#[cfg(test)]
fn variant_match_state_owned_capacity(state: &VariantMatchState<'_>) -> usize {
    state
        .outer_names
        .capacity()
        .saturating_mul(std::mem::size_of::<String>())
        .saturating_add(
            state
                .outer_names
                .iter()
                .map(String::capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .baseline
                .capacity()
                .saturating_mul(std::mem::size_of::<(String, Binding)>()),
        )
        .saturating_add(
            state
                .baseline
                .iter()
                .map(|(name, binding)| name.capacity() + binding_owned_capacity(binding))
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .arm_states
                .capacity()
                .saturating_mul(std::mem::size_of::<HashMap<String, Binding>>()),
        )
        .saturating_add(
            state
                .arm_states
                .iter()
                .map(|bindings| {
                    bindings
                        .capacity()
                        .saturating_mul(std::mem::size_of::<(String, Binding)>())
                        .saturating_add(
                            bindings
                                .iter()
                                .map(|(name, binding)| {
                                    name.capacity() + binding_owned_capacity(binding)
                                })
                                .sum::<usize>(),
                        )
                })
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .covered
                .capacity()
                .saturating_mul(std::mem::size_of::<String>()),
        )
        .saturating_add(state.covered.iter().map(String::capacity).sum::<usize>())
        .saturating_add(state.variant_name.as_ref().map_or(0, String::capacity))
        .saturating_add(
            state
                .variant_arguments
                .capacity()
                .saturating_mul(std::mem::size_of::<Type>()),
        )
        .saturating_add(
            state
                .variant_arguments
                .iter()
                .map(ast_type_owned_capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            state
                .result
                .as_ref()
                .map_or(0, |value| ast_type_owned_capacity(&value.ty)),
        )
}

#[cfg(test)]
fn diagnostics_owned_capacity(diagnostics: &Vec<Diagnostic>) -> usize {
    diagnostics.capacity() * std::mem::size_of::<Diagnostic>()
        + diagnostics
            .iter()
            .map(|diagnostic| {
                diagnostic.message.capacity()
                    + diagnostic.path.as_ref().map_or(0, String::capacity)
                    + diagnostic.help.as_ref().map_or(0, String::capacity)
            })
            .sum::<usize>()
}

#[cfg(test)]
fn verifier_frame_owned_capacity(frame: &VerifierFrame<'_>) -> usize {
    let strings = |values: &Vec<String>| {
        values
            .capacity()
            .saturating_mul(std::mem::size_of::<String>())
            .saturating_add(values.iter().map(String::capacity).sum::<usize>())
    };
    match frame {
        VerifierFrame::ResumeBinaryRight { baseline_names, .. }
        | VerifierFrame::ResumeIfThen { baseline_names, .. } => strings(baseline_names),
        VerifierFrame::ResumeIfElse {
            baseline_names,
            then_bindings,
            ..
        } => strings(baseline_names).saturating_add(
            then_bindings
                .capacity()
                .saturating_mul(std::mem::size_of::<(String, Binding)>())
                .saturating_add(
                    then_bindings
                        .iter()
                        .map(|(name, binding)| name.capacity() + binding_owned_capacity(binding))
                        .sum::<usize>(),
                ),
        ),
        VerifierFrame::ResumeBlockStatement { outer_names, .. }
        | VerifierFrame::ResumeBlockTail { outer_names, .. }
        | VerifierFrame::ResumeRecordMatchArm { outer_names, .. } => strings(outer_names),
        VerifierFrame::ResumeRecordField { supplied, .. }
        | VerifierFrame::PrepareRecordField { supplied, .. }
        | VerifierFrame::ResumeVariantField { supplied, .. }
        | VerifierFrame::PrepareVariantField { supplied, .. } => supplied
            .capacity()
            .saturating_mul(std::mem::size_of::<&str>()),
        VerifierFrame::ResumeUpdateField {
            base_type,
            supplied,
            ..
        }
        | VerifierFrame::PrepareUpdateField {
            base_type,
            supplied,
            ..
        } => ast_type_owned_capacity(base_type).saturating_add(
            supplied
                .capacity()
                .saturating_mul(std::mem::size_of::<&str>()),
        ),
        VerifierFrame::ResumeCallArgument { target, .. } => match target {
            VerifierCallTarget::Native(_) => 0,
            VerifierCallTarget::Ordinary(Some(signature)) => {
                verifier_signature_owned_capacity(signature)
            }
            VerifierCallTarget::Ordinary(None) => 0,
        },
        VerifierFrame::PrepareVariantMatchArm(state)
        | VerifierFrame::ResumeVariantMatchArm { state, .. } => {
            variant_match_state_owned_capacity(state)
        }
        _ => 0,
    }
}

impl VerifierFunctionSignature<'_> {
    fn params(&self) -> &[Param] {
        match self {
            Self::Borrowed(function) => &function.params,
            Self::Specialized { params, .. } => params,
        }
    }

    fn return_type(&self) -> &Type {
        match self {
            Self::Borrowed(function) => &function.return_type,
            Self::Specialized { return_type, .. } => return_type,
        }
    }
}

struct IterativeVerifier<'a, 'p> {
    program: &'p Program,
    current: &'p Function,
    functions: &'p HashMap<&'p str, &'p Function>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    scopes: Vec<VerifierScope>,
    frames: Vec<VerifierFrame<'p>>,
    values: Vec<Option<CheckedValue>>,
}

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        program: &'p Program,
        current: &'p Function,
        variables: HashMap<String, Binding>,
        functions: &'p HashMap<&'p str, &'p Function>,
        types: &'p TypeTable<'p>,
        result_type: Option<&'p Type>,
        allow_moves: bool,
        diagnostics: &'a mut Vec<Diagnostic>,
    ) -> Self {
        const { assert!(std::mem::size_of::<VerifierFrame<'static>>() == 320) };
        const { assert!(std::mem::size_of::<VariantMatchState<'static>>() == 312) };
        Self {
            program,
            current,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
            scopes: vec![VerifierScope {
                bindings: variables,
            }],
            frames: Vec::new(),
            values: Vec::new(),
        }
    }

    #[allow(clippy::collapsible_else_if)]
    fn run(&mut self, expression: &'p Expr) -> Result<Option<CheckedValue>, Diagnostic> {
        self.frames.push(VerifierFrame::Enter {
            expression,
            scope: 0,
        });
        while let Some(frame) = self.frames.pop() {
            #[cfg(test)]
            note_capacity_high_water(
                self.frames.capacity() * std::mem::size_of::<VerifierFrame<'_>>()
                    + self.scopes.capacity() * std::mem::size_of::<VerifierScope>()
                    + self.values.capacity() * std::mem::size_of::<Option<CheckedValue>>()
                    + self
                        .values
                        .iter()
                        .flatten()
                        .map(|value| ast_type_owned_capacity(&value.ty))
                        .sum::<usize>()
                    + self.scopes.iter().map(scope_owned_capacity).sum::<usize>()
                    + self
                        .frames
                        .iter()
                        .map(verifier_frame_owned_capacity)
                        .sum::<usize>()
                    + verifier_frame_owned_capacity(&frame)
                    + diagnostics_owned_capacity(self.diagnostics),
            );
            match frame {
                VerifierFrame::Enter { expression, scope } => match &expression.kind {
                    ExprKind::Int(_) => self.values.push(Some(CheckedValue::value(Type::I64))),
                    ExprKind::Char(_) => self.values.push(Some(CheckedValue::value(Type::Char))),
                    ExprKind::Uint8(_) => self.values.push(Some(CheckedValue::value(Type::U8))),
                    ExprKind::Float32(_) => self.values.push(Some(CheckedValue::value(Type::F32))),
                    ExprKind::Float64(_) => self.values.push(Some(CheckedValue::value(Type::F64))),
                    ExprKind::Bool(_) => self.values.push(Some(CheckedValue::value(Type::Bool))),
                    ExprKind::Var(name) if name == "result" => {
                        let value = self.result_type.map(|ty| {
                            CheckedValue::returned(ty.clone(), self.types.contains_resource(ty))
                        });
                        if value.is_none() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T201",
                                "`result` is only available in postconditions",
                                expression.span,
                            ));
                        }
                        self.values.push(value);
                    }
                    ExprKind::Var(name) => {
                        let value = self.scopes[scope].bindings.get(name).map(|binding| {
                            match binding.availability {
                                Availability::Moved => self.diagnostics.push(
                                    error(
                                        self.program,
                                        "SPX-O101",
                                        format!("use of resource `{name}` after ownership was moved"),
                                        expression.span,
                                    )
                                    .with_help(
                                        "borrow the resource if the callee does not need ownership",
                                    ),
                                ),
                                Availability::MaybeMoved => self.diagnostics.push(
                                    error(
                                        self.program,
                                        "SPX-O107",
                                        format!("resource `{name}` may have been moved on another control-flow path"),
                                        expression.span,
                                    )
                                    .with_help("move the resource on every path or keep it borrowed"),
                                ),
                                Availability::Available => match overlapping_place_state(binding, &[]) {
                                    Availability::Moved => self.diagnostics.push(
                                        error(self.program, "SPX-O109", format!("use of partially moved place `{name}`"), expression.span)
                                            .with_help("use an available sibling field or avoid moving this place earlier"),
                                    ),
                                    Availability::MaybeMoved => self.diagnostics.push(
                                        error(self.program, "SPX-O110", format!("place `{name}` may have been moved on another control-flow path"), expression.span)
                                            .with_help("move the field on every path or keep it borrowed"),
                                    ),
                                    Availability::Available => {}
                                },
                            }
                            if binding.native_unit_discard {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-B107",
                                    "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                    expression.span,
                                ));
                            }
                            CheckedValue { ty: binding.ty.clone(), mode: binding.mode, native_unit: binding.native_unit_discard }
                        });
                        if value.is_none() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T202",
                                format!("unknown value `{name}` in `{}`", self.current.name),
                                expression.span,
                            ));
                        }
                        self.values.push(value);
                    }
                    ExprKind::Unary { op, value } => {
                        self.frames.push(VerifierFrame::ResumeUnary {
                            expression,
                            operand: value,
                            op: *op,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: value,
                            scope,
                        });
                    }
                    ExprKind::Binary { op, left, right } => {
                        self.frames.push(VerifierFrame::ResumeBinaryLeft {
                            expression,
                            op: *op,
                            right,
                            scope,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: left,
                            scope,
                        });
                    }
                    ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } => {
                        let native = self
                            .program
                            .interfaces
                            .iter()
                            .flat_map(|interface| &interface.imports)
                            .find(|import| import.native_rust && import.name == *name);
                        let target = if let Some(import) = native {
                            if !type_arguments.is_empty() || args.len() != import.params.len() {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-B107",
                                    "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                    expression.span,
                                ));
                            }
                            for effect in &import.effects {
                                if !self.current.effects.contains(effect) {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-B107",
                                        "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                                        expression.span,
                                    ));
                                }
                            }
                            VerifierCallTarget::Native(import)
                        } else {
                            let target = self.functions.get(name.as_str()).copied();
                            if target.is_none() {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T203",
                                    format!("unknown function `{name}`"),
                                    expression.span,
                                ));
                            }
                            if target.is_some_and(|target| args.len() != target.params.len()) {
                                let target = target.expect("checked above");
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T204",
                                    format!(
                                        "`{name}` expects {} arguments, received {}",
                                        target.params.len(),
                                        args.len()
                                    ),
                                    expression.span,
                                ));
                            }
                            let specialized = target.and_then(|target| {
                                if target.type_parameters.is_empty() {
                                    if !type_arguments.is_empty() {
                                        self.diagnostics.push(error(
                                            self.program,
                                            "SPX-T225",
                                            format!("monomorphic function `{name}` does not accept type arguments"),
                                            expression.span,
                                        ));
                                        return None;
                                    }
                                    return Some(VerifierFunctionSignature::Borrowed(target));
                                }
                                if !self.current.type_parameters.is_empty() {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-T226",
                                        format!("generic function `{}` cannot call generic function `{name}` in this slice", self.current.name),
                                        expression.span,
                                    ));
                                }
                                if type_arguments.len() != target.type_parameters.len() {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-T225",
                                        format!("generic function `{name}` expects {} explicit type arguments, received {}", target.type_parameters.len(), type_arguments.len()),
                                        expression.span,
                                    ));
                                    return None;
                                }
                                if type_arguments.iter().any(|argument| !direct_function_type_argument(argument)) {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-T225",
                                        format!("generic function `{name}` accepts only direct `i64` or `bool` type arguments"),
                                        expression.span,
                                    ));
                                    return None;
                                }
                                validation_specialize_signature(target, type_arguments).map(
                                    |(params, return_type)| {
                                        VerifierFunctionSignature::Specialized {
                                            params,
                                            return_type,
                                        }
                                    },
                                )
                            });
                            VerifierCallTarget::Ordinary(specialized)
                        };
                        if let Some(argument) = args.first() {
                            self.frames.push(VerifierFrame::ResumeCallArgument {
                                expression,
                                name,
                                args,
                                scope,
                                index: 0,
                                target,
                            });
                            self.frames.push(VerifierFrame::Enter {
                                expression: argument,
                                scope,
                            });
                        } else {
                            self.values.push(Some(match target {
                                VerifierCallTarget::Native(import) => {
                                    let mut value = CheckedValue::value(match import.result {
                                        ImportResult::Unit => Type::Named {
                                            name: "\0native-rust-unit".to_owned(),
                                            arguments: Vec::new(),
                                        },
                                        ImportResult::I64 => Type::I64,
                                        ImportResult::Bool => Type::Bool,
                                    });
                                    value.native_unit = import.result == ImportResult::Unit;
                                    value
                                }
                                VerifierCallTarget::Ordinary(Some(target)) => {
                                    CheckedValue::returned(
                                        target.return_type().clone(),
                                        self.types.contains_resource(target.return_type()),
                                    )
                                }
                                VerifierCallTarget::Ordinary(None) => {
                                    self.values.push(None);
                                    continue;
                                }
                            }));
                        }
                    }
                    ExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        self.frames.push(VerifierFrame::ResumeIfCondition {
                            expression,
                            then_branch,
                            else_branch,
                            scope,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: condition,
                            scope,
                        });
                    }
                    ExprKind::Block { statements, tail } => {
                        let outer_names = self.scopes[scope]
                            .bindings
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>();
                        let block_scope = self.scopes.len();
                        self.scopes.push(VerifierScope {
                            bindings: self.scopes[scope].bindings.clone(),
                        });
                        if let Some(Statement::Let {
                            name,
                            name_span,
                            value,
                            ..
                        }) = statements.first()
                        {
                            if !source_identifier(name) {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-S109",
                                    format!("`{name}` is reserved and cannot name a local binding"),
                                    *name_span,
                                ));
                            }
                            self.frames.push(VerifierFrame::ResumeBlockStatement {
                                expression,
                                statements,
                                tail,
                                parent_scope: scope,
                                block_scope,
                                index: 0,
                                outer_names,
                            });
                            self.frames.push(VerifierFrame::Enter {
                                expression: value,
                                scope: block_scope,
                            });
                        } else {
                            self.frames.push(VerifierFrame::ResumeBlockTail {
                                parent_scope: scope,
                                block_scope,
                                outer_names,
                            });
                            self.frames.push(VerifierFrame::Enter {
                                expression: tail,
                                scope: block_scope,
                            });
                        }
                    }
                    ExprKind::Try { operand } => {
                        self.frames.push(VerifierFrame::ResumeTry {
                            expression,
                            operand,
                            scope,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: operand,
                            scope,
                        });
                    }
                    ExprKind::Project { base, field, .. } => {
                        if let Some(place) =
                            source_place(expression, &self.scopes[scope].bindings, self.types)
                        {
                            check_source_place_availability(
                                self.program,
                                &place,
                                &self.scopes[scope].bindings,
                                expression.span,
                                self.diagnostics,
                            );
                            self.values.push(Some(CheckedValue {
                                ty: place.ty,
                                mode: place.mode,
                                native_unit: false,
                            }));
                        } else {
                            self.frames.push(VerifierFrame::ResumeProject {
                                expression,
                                base,
                                field,
                            });
                            self.frames.push(VerifierFrame::Enter {
                                expression: base,
                                scope,
                            });
                        }
                    }
                    ExprKind::ConstructRecord {
                        type_name,
                        type_arguments,
                        fields,
                        ..
                    } => {
                        let declaration = self.types.declaration(type_name);
                        let instance = Type::Named {
                            name: type_name.clone(),
                            arguments: type_arguments.clone(),
                        };
                        check_declared_type(
                            self.program,
                            &instance,
                            expression.span,
                            self.types,
                            &HashSet::new(),
                            self.diagnostics,
                        );
                        let declared_fields =
                            declaration.and_then(|declaration| match &declaration.kind {
                                TypeDeclarationKind::Record { fields } => Some(fields.as_slice()),
                                TypeDeclarationKind::Resource { .. }
                                | TypeDeclarationKind::Variant { .. } => None,
                            });
                        if declared_fields.is_none() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T215",
                                format!("`{type_name}` is not a declared record type"),
                                expression.span,
                            ));
                        }
                        if !fields.is_empty() {
                            self.frames.push(VerifierFrame::PrepareRecordField {
                                expression,
                                type_name,
                                type_arguments,
                                fields,
                                declared_fields,
                                scope,
                                index: 0,
                                supplied: HashSet::new(),
                            });
                        } else {
                            if let Some(declared_fields) = declared_fields {
                                for field in declared_fields {
                                    self.diagnostics.push(error(self.program, "SPX-T213", format!("record `{type_name}` construction is missing field `{}`", field.name), expression.span));
                                }
                                self.values.push(Some(CheckedValue::returned(
                                    instance.clone(),
                                    self.types.contains_resource(&instance),
                                )));
                            } else {
                                self.values.push(None);
                            }
                        }
                    }
                    ExprKind::ConstructVariant {
                        type_name,
                        type_arguments,
                        case_name,
                        fields,
                        ..
                    } => {
                        let declaration = self.types.declaration(type_name);
                        let instance = Type::Named {
                            name: type_name.clone(),
                            arguments: type_arguments.clone(),
                        };
                        check_declared_type(
                            self.program,
                            &instance,
                            expression.span,
                            self.types,
                            &HashSet::new(),
                            self.diagnostics,
                        );
                        let cases = declaration.and_then(|declaration| match &declaration.kind {
                            TypeDeclarationKind::Variant { cases } => Some(cases.as_slice()),
                            TypeDeclarationKind::Resource { .. }
                            | TypeDeclarationKind::Record { .. } => None,
                        });
                        let case = cases
                            .and_then(|cases| cases.iter().find(|case| case.name == *case_name));
                        if cases.is_none() || case.is_none() {
                            self.diagnostics.push(error(self.program, "SPX-T215", format!("`{type_name}::{case_name}` is not a declared variant constructor"), expression.span));
                        }
                        if !fields.is_empty() {
                            self.frames.push(VerifierFrame::PrepareVariantField {
                                expression,
                                type_name,
                                type_arguments,
                                case_name,
                                fields,
                                declaration,
                                case,
                                scope,
                                index: 0,
                                supplied: HashSet::new(),
                            });
                        } else {
                            if let Some(case) = case {
                                for field in &case.fields {
                                    self.diagnostics.push(error(self.program, "SPX-T213", format!("variant construction `{type_name}::{case_name}` is missing payload field `{}`", field.name), expression.span));
                                }
                                self.values.push(Some(CheckedValue::value(instance)));
                            } else {
                                self.values.push(None);
                            }
                        }
                    }
                    ExprKind::UpdateRecord { base, fields } => {
                        self.frames.push(VerifierFrame::ResumeUpdateBase {
                            expression,
                            base,
                            fields,
                            scope,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: base,
                            scope,
                        });
                    }
                    ExprKind::Match { scrutinee, arms } => {
                        self.frames.push(VerifierFrame::ResumeMatchScrutinee {
                            expression,
                            scrutinee,
                            arms,
                            scope,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: scrutinee,
                            scope,
                        });
                    }
                },
                VerifierFrame::ResumeUnary {
                    expression,
                    operand,
                    op,
                } => {
                    let Some(actual) = self.values.pop().flatten() else {
                        self.values.push(None);
                        continue;
                    };
                    let numeric = matches!(op, UnaryOp::Neg)
                        .then(|| actual.ty.clone())
                        .filter(|ty| matches!(ty, Type::I64 | Type::F32 | Type::F64));
                    let expected = match (&op, &numeric) {
                        (UnaryOp::Neg, Some(ty)) => ty.clone(),
                        (UnaryOp::Neg, None) => Type::I64,
                        (UnaryOp::Not, _) => Type::Bool,
                    };
                    if !actual.native_unit && actual.ty != expected {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T206",
                            format!("unary operator expects {expected}, received {}", actual.ty),
                            expression.span,
                        ));
                    }
                    reject_native_unit_value(self.program, operand, &actual, self.diagnostics);
                    self.values.push(Some(CheckedValue::value(expected)));
                }
                VerifierFrame::ResumeBinaryLeft {
                    expression,
                    op,
                    right,
                    scope,
                } => {
                    let left_value = self.values.pop().unwrap_or(None);
                    let baseline_names = self.scopes[scope]
                        .bindings
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>();
                    let evaluated_scope = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        let index = self.scopes.len();
                        self.scopes.push(VerifierScope {
                            bindings: self.scopes[scope].bindings.clone(),
                        });
                        index
                    } else {
                        scope
                    };
                    let left = match &expression.kind {
                        ExprKind::Binary { left, .. } => left.as_ref(),
                        _ => unreachable!(),
                    };
                    self.frames.push(VerifierFrame::ResumeBinaryRight {
                        expression,
                        op,
                        left,
                        left_value,
                        scope,
                        evaluated_scope,
                        baseline_names,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: right,
                        scope: evaluated_scope,
                    });
                }
                VerifierFrame::ResumeBinaryRight {
                    expression,
                    op,
                    left,
                    left_value,
                    scope,
                    evaluated_scope,
                    baseline_names,
                } => {
                    let right_value = self.values.pop().unwrap_or(None);
                    if evaluated_scope != scope {
                        if evaluated_scope + 1 != self.scopes.len() {
                            return Err(Diagnostic::io(
                                "SPX-H006",
                                "lazy verifier scope is not the active child",
                            ));
                        }
                        let evaluated = self
                            .scopes
                            .pop()
                            .expect("active lazy scope index checked above")
                            .bindings;
                        join_conditional(
                            &mut self.scopes[scope].bindings,
                            &evaluated,
                            &baseline_names,
                        );
                    }
                    if let Some(value) = &left_value {
                        reject_native_unit_value(self.program, left, value, self.diagnostics);
                    }
                    let right = match &expression.kind {
                        ExprKind::Binary { right, .. } => right.as_ref(),
                        _ => unreachable!(),
                    };
                    if let Some(value) = &right_value {
                        reject_native_unit_value(self.program, right, value, self.diagnostics);
                    }
                    let native_unit = left_value.as_ref().is_some_and(|value| value.native_unit)
                        || right_value.as_ref().is_some_and(|value| value.native_unit);
                    let left_ordered =
                        left_value
                            .as_ref()
                            .map(|value| value.ty.clone())
                            .filter(|ty| {
                                matches!(
                                    ty,
                                    Type::I64 | Type::Char | Type::U8 | Type::F32 | Type::F64
                                )
                            });
                    let left_narrow = left_value
                        .as_ref()
                        .map(|value| value.ty.clone())
                        .filter(|ty| matches!(ty, Type::U8));
                    let left_numeric = left_value
                        .as_ref()
                        .map(|value| value.ty.clone())
                        .filter(|ty| matches!(ty, Type::F32 | Type::F64));
                    if !native_unit
                        && matches!(op, BinaryOp::Rem)
                        && (left_numeric.is_some() || left_narrow.is_some())
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T208",
                            format!("operator `{}` expects i64 operands", op.text()),
                            expression.span,
                        ));
                    }
                    let (expected, output) = match op {
                        BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Rem => {
                            let expected = left_numeric.or(left_narrow).unwrap_or(Type::I64);
                            (expected.clone(), expected)
                        }
                        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                            let expected = left_ordered.unwrap_or(Type::I64);
                            (expected, Type::Bool)
                        }
                        BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
                        BinaryOp::Eq | BinaryOp::Ne => {
                            if !native_unit
                                && left_value.is_some()
                                && right_value.is_some()
                                && left_value.as_ref().map(|value| &value.ty)
                                    != right_value.as_ref().map(|value| &value.ty)
                            {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T207",
                                    "equality operands must have the same type",
                                    expression.span,
                                ));
                            }
                            self.values.push(Some(CheckedValue::value(Type::Bool)));
                            continue;
                        }
                    };
                    if !native_unit
                        && (left_value
                            .as_ref()
                            .is_some_and(|value| value.ty != expected)
                            || right_value
                                .as_ref()
                                .is_some_and(|value| value.ty != expected))
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T208",
                            format!("operator `{}` expects {expected} operands", op.text()),
                            expression.span,
                        ));
                    }
                    self.values.push(Some(CheckedValue::value(output)));
                }
                VerifierFrame::ResumeIfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                } => {
                    let condition_value = self.values.pop().unwrap_or(None);
                    let condition = match &expression.kind {
                        ExprKind::If { condition, .. } => condition.as_ref(),
                        _ => unreachable!(),
                    };
                    if let Some(value) = condition_value {
                        if value.native_unit {
                            reject_native_unit_value(
                                self.program,
                                condition,
                                &value,
                                self.diagnostics,
                            );
                        } else if value.ty != Type::Bool {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T210",
                                "`if` condition must be bool",
                                condition.span,
                            ));
                        }
                    }
                    let baseline_names = self.scopes[scope]
                        .bindings
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>();
                    let then_scope = self.scopes.len();
                    self.scopes.push(VerifierScope {
                        bindings: self.scopes[scope].bindings.clone(),
                    });
                    self.frames.push(VerifierFrame::ResumeIfThen {
                        expression,
                        else_branch,
                        scope,
                        then_scope,
                        baseline_names,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: then_branch,
                        scope: then_scope,
                    });
                }
                VerifierFrame::ResumeIfThen {
                    expression,
                    else_branch,
                    scope,
                    then_scope,
                    baseline_names,
                } => {
                    if then_scope + 1 != self.scopes.len() {
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            "then verifier scope is not the active child",
                        ));
                    }
                    let then_value = self.values.pop().unwrap_or(None);
                    let then_bindings = self
                        .scopes
                        .pop()
                        .expect("active then scope index checked above")
                        .bindings;
                    let else_scope = self.scopes.len();
                    self.scopes.push(VerifierScope {
                        bindings: self.scopes[scope].bindings.clone(),
                    });
                    let then_branch = match &expression.kind {
                        ExprKind::If { then_branch, .. } => then_branch.as_ref(),
                        _ => unreachable!(),
                    };
                    self.frames.push(VerifierFrame::ResumeIfElse {
                        expression,
                        then_branch,
                        else_branch,
                        scope,
                        else_scope,
                        baseline_names,
                        then_value,
                        then_bindings,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: else_branch,
                        scope: else_scope,
                    });
                }
                VerifierFrame::ResumeIfElse {
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                    else_scope,
                    baseline_names,
                    then_value,
                    then_bindings,
                } => {
                    if else_scope + 1 != self.scopes.len() {
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            "else verifier scope is not the active child",
                        ));
                    }
                    let else_value = self.values.pop().unwrap_or(None);
                    let else_bindings = self
                        .scopes
                        .pop()
                        .expect("active else scope index checked above")
                        .bindings;
                    for name in &baseline_names {
                        if let Some(binding) = self.scopes[scope].bindings.get_mut(name) {
                            let then_state = then_bindings
                                .get(name)
                                .map_or(Availability::Available, |value| value.availability);
                            let else_state = else_bindings
                                .get(name)
                                .map_or(Availability::Available, |value| value.availability);
                            binding.availability = then_state.join(else_state);
                            if let (Some(then_binding), Some(else_binding)) =
                                (then_bindings.get(name), else_bindings.get(name))
                            {
                                binding.moved_places =
                                    join_moved_places(then_binding, else_binding);
                                binding.definitely_partial =
                                    join_definitely_partial(then_binding, else_binding);
                            }
                        }
                    }
                    let output = match (then_value, else_value) {
                        (Some(then_value), Some(else_value)) => {
                            if then_value.native_unit || else_value.native_unit {
                                reject_native_unit_value(
                                    self.program,
                                    then_branch,
                                    &then_value,
                                    self.diagnostics,
                                );
                                reject_native_unit_value(
                                    self.program,
                                    else_branch,
                                    &else_value,
                                    self.diagnostics,
                                );
                            } else if then_value.ty != else_value.ty {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T211",
                                    format!(
                                        "`if` branches return different types: {} and {}",
                                        then_value.ty, else_value.ty
                                    ),
                                    expression.span,
                                ));
                            }
                            if self.types.contains_resource(&then_value.ty)
                                && then_value.mode != else_value.mode
                            {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-O106",
                                    "`if` branches must produce the same resource ownership mode",
                                    expression.span,
                                ));
                            }
                            Some(then_value)
                        }
                        _ => None,
                    };
                    self.values.push(output);
                }
                VerifierFrame::ResumeBlockStatement {
                    expression,
                    statements,
                    tail,
                    parent_scope,
                    block_scope,
                    index,
                    outer_names,
                } => {
                    let actual = self.values.pop().unwrap_or(None);
                    let Statement::Let {
                        name,
                        name_span,
                        value,
                        ..
                    } = &statements[index];
                    if self.scopes[block_scope].bindings.contains_key(name) {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T209",
                            format!("local binding `{name}` shadows an existing value"),
                            *name_span,
                        ));
                    } else if let Some(actual) = actual {
                        if self.types.contains_resource(&actual.ty) && actual.mode == ParamMode::Own
                        {
                            if self.allow_moves {
                                mark_value_sources_moved(
                                    value,
                                    &mut self.scopes[block_scope].bindings,
                                    self.types,
                                );
                            } else {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-O105",
                                    "contract expression cannot transfer an owned resource into a local binding",
                                    value.span,
                                ));
                            }
                        }
                        self.scopes[block_scope].bindings.insert(
                            name.clone(),
                            Binding {
                                ty: actual.ty,
                                mode: actual.mode,
                                availability: Availability::Available,
                                moved_places: HashMap::new(),
                                definitely_partial: HashSet::new(),
                                native_unit_discard: actual.native_unit,
                            },
                        );
                    }
                    let next = index + 1;
                    if let Some(Statement::Let {
                        name,
                        name_span,
                        value,
                        ..
                    }) = statements.get(next)
                    {
                        if !source_identifier(name) {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-S109",
                                format!("`{name}` is reserved and cannot name a local binding"),
                                *name_span,
                            ));
                        }
                        self.frames.push(VerifierFrame::ResumeBlockStatement {
                            expression,
                            statements,
                            tail,
                            parent_scope,
                            block_scope,
                            index: next,
                            outer_names,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: value,
                            scope: block_scope,
                        });
                    } else {
                        self.frames.push(VerifierFrame::ResumeBlockTail {
                            parent_scope,
                            block_scope,
                            outer_names,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: tail,
                            scope: block_scope,
                        });
                    }
                }
                VerifierFrame::ResumeBlockTail {
                    parent_scope,
                    block_scope,
                    outer_names,
                } => {
                    if block_scope + 1 != self.scopes.len() {
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            "block verifier scope is not the active child",
                        ));
                    }
                    let actual = self.values.pop().unwrap_or(None);
                    let block_bindings = self
                        .scopes
                        .pop()
                        .expect("active block scope index checked above")
                        .bindings;
                    merge_moved(
                        &mut self.scopes[parent_scope].bindings,
                        &block_bindings,
                        &outer_names,
                    );
                    self.values.push(actual);
                }
                VerifierFrame::ResumeCallArgument {
                    expression,
                    name,
                    args,
                    scope,
                    index,
                    target,
                } => {
                    let actual = self.values.pop().unwrap_or(None);
                    let argument = &args[index];
                    match &target {
                        VerifierCallTarget::Native(import) => {
                            if let (Some(actual), Some(parameter)) =
                                (actual.as_ref(), import.params.get(index))
                            {
                                reject_native_unit_value(
                                    self.program,
                                    argument,
                                    actual,
                                    self.diagnostics,
                                );
                                if !actual.native_unit
                                    && (actual.ty != parameter.ty
                                        || actual.mode != ParamMode::Value)
                                {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-B107",
                                        "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                        argument.span,
                                    ));
                                }
                            }
                        }
                        VerifierCallTarget::Ordinary(specialized) => {
                            if let Some(param) = specialized
                                .as_ref()
                                .and_then(|target| target.params().get(index))
                            {
                                if let Some(actual) = &actual {
                                    reject_native_unit_value(
                                        self.program,
                                        argument,
                                        actual,
                                        self.diagnostics,
                                    );
                                }
                                if actual.as_ref().is_some_and(|actual| {
                                    !actual.native_unit && actual.ty != param.ty
                                }) {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-T205",
                                        format!(
                                            "argument `{}` to `{name}` expects {}, received {}",
                                            param.name,
                                            param.ty,
                                            actual.as_ref().expect("type checked above").ty
                                        ),
                                        argument.span,
                                    ));
                                }
                                check_argument_ownership(
                                    self.program,
                                    self.current,
                                    name,
                                    argument,
                                    param,
                                    actual.as_ref(),
                                    &mut self.scopes[scope].bindings,
                                    self.types,
                                    self.allow_moves,
                                    self.diagnostics,
                                );
                            }
                        }
                    }
                    let next = index + 1;
                    if let Some(argument) = args.get(next) {
                        self.frames.push(VerifierFrame::ResumeCallArgument {
                            expression,
                            name,
                            args,
                            scope,
                            index: next,
                            target,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: argument,
                            scope,
                        });
                    } else {
                        let output = match target {
                            VerifierCallTarget::Native(import) => {
                                let mut value = CheckedValue::value(match import.result {
                                    ImportResult::Unit => Type::Named {
                                        name: "\0native-rust-unit".to_owned(),
                                        arguments: Vec::new(),
                                    },
                                    ImportResult::I64 => Type::I64,
                                    ImportResult::Bool => Type::Bool,
                                });
                                value.native_unit = import.result == ImportResult::Unit;
                                Some(value)
                            }
                            VerifierCallTarget::Ordinary(Some(target)) => {
                                Some(CheckedValue::returned(
                                    target.return_type().clone(),
                                    self.types.contains_resource(target.return_type()),
                                ))
                            }
                            VerifierCallTarget::Ordinary(None) => None,
                        };
                        self.values.push(output);
                    }
                }
                VerifierFrame::ResumeTry {
                    expression,
                    operand,
                    scope,
                } => {
                    let Some(operand_value) = self.values.pop().flatten() else {
                        self.values.push(None);
                        continue;
                    };
                    reject_native_unit_value(
                        self.program,
                        operand,
                        &operand_value,
                        self.diagnostics,
                    );
                    if !self.allow_moves {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T218",
                            "`?` is only valid in an executable function body",
                            expression.span,
                        ));
                    }
                    if self.scopes[scope]
                        .bindings
                        .values()
                        .any(|binding| self.types.contains_resource(&binding.ty))
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T218",
                            "`?` with a live resource binding is not supported yet",
                            expression.span,
                        ));
                    }
                    if let Some((ok, error_ty)) = ordinary_result_arguments(&operand_value.ty) {
                        let Some((_, residual_error_ty)) =
                            ordinary_result_arguments(&self.current.return_type)
                        else {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T218",
                                format!(
                                    "function `{}` must return the ordinary compiler-owned Result to propagate a Result with `?`",
                                    self.current.name
                                ),
                                expression.span,
                            ));
                            self.values.push(Some(CheckedValue::value(ok.clone())));
                            continue;
                        };
                        if error_ty != residual_error_ty {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T219",
                                format!("`?` cannot propagate error type {error_ty} into Result error type {residual_error_ty}"),
                                expression.span,
                            ));
                        }
                        self.values.push(Some(CheckedValue::value(ok.clone())));
                        continue;
                    }
                    if let Some(some) = ordinary_option_argument(&operand_value.ty) {
                        let outer = ordinary_option_argument(&self.current.return_type);
                        if outer.is_none() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T218",
                                format!("function `{}` must return the ordinary compiler-owned Option to propagate an Option with `?`", self.current.name),
                                expression.span,
                            ));
                        } else if !matches!(some, Type::I64 | Type::Bool)
                            || outer.is_some_and(|value| !matches!(value, Type::I64 | Type::Bool))
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T218",
                                "Option `?` accepts only direct `i64` or `bool` source and enclosing payloads",
                                expression.span,
                            ));
                        }
                        self.values.push(Some(CheckedValue::value(some.clone())));
                        continue;
                    }
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T218",
                        format!("`?` operand must be an ordinary compiler-owned Result or Option, received {}", operand_value.ty),
                        expression.span,
                    ));
                    self.values.push(None);
                }
                VerifierFrame::ResumeProject {
                    expression,
                    base,
                    field,
                } => {
                    let Some(base_value) = self.values.pop().flatten() else {
                        self.values.push(None);
                        continue;
                    };
                    reject_native_unit_value(self.program, base, &base_value, self.diagnostics);
                    let Some(fields) = self.types.record_fields(&base_value.ty) else {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T214",
                            format!("cannot project field `{field}` from `{}`", base_value.ty),
                            expression.span,
                        ));
                        self.values.push(None);
                        continue;
                    };
                    let Some(declared) = fields.iter().find(|candidate| candidate.name == field)
                    else {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T214",
                            format!("record `{}` has no field `{field}`", base_value.ty),
                            expression.span,
                        ));
                        self.values.push(None);
                        continue;
                    };
                    let projected = self
                        .types
                        .record_field_type(&base_value.ty, declared)
                        .unwrap_or_else(|| declared.ty.clone());
                    let mode = if self.types.contains_resource(&projected) {
                        base_value.mode
                    } else {
                        ParamMode::Value
                    };
                    self.values.push(Some(CheckedValue {
                        ty: projected,
                        mode,
                        native_unit: false,
                    }));
                }
                VerifierFrame::PrepareRecordField {
                    expression,
                    type_name,
                    type_arguments,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    mut supplied,
                } => {
                    let field = &fields[index];
                    let declared = declared_fields.and_then(|declared| {
                        declared
                            .iter()
                            .find(|candidate| candidate.name == field.name)
                    });
                    if !supplied.insert(field.name.as_str()) || declared.is_none() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T212",
                            format!(
                                "unknown or duplicate field `{}` in `{type_name}` construction",
                                field.name
                            ),
                            field.span,
                        ));
                    }
                    self.frames.push(VerifierFrame::ResumeRecordField {
                        expression,
                        type_name,
                        type_arguments,
                        fields,
                        declared_fields,
                        scope,
                        index,
                        supplied,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: &field.value,
                        scope,
                    });
                }
                VerifierFrame::ResumeRecordField {
                    expression,
                    type_name,
                    type_arguments,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                } => {
                    let actual = self.values.pop().unwrap_or(None);
                    let field = &fields[index];
                    let declared = declared_fields.and_then(|declared| {
                        declared
                            .iter()
                            .find(|candidate| candidate.name == field.name)
                    });
                    if let (Some(declared), Some(actual)) = (declared, actual) {
                        reject_native_unit_value(
                            self.program,
                            &field.value,
                            &actual,
                            self.diagnostics,
                        );
                        let expected = self
                            .types
                            .declaration(type_name)
                            .and_then(|declaration| {
                                TypeTable::substitute_variant_type(
                                    declaration,
                                    type_arguments,
                                    &declared.ty,
                                )
                            })
                            .unwrap_or_else(|| declared.ty.clone());
                        if actual.ty != expected {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T215",
                                format!(
                                    "field `{}.{}` expects {}, received {}",
                                    type_name, field.name, expected, actual.ty
                                ),
                                field.value.span,
                            ));
                        }
                        if self.types.contains_resource(&declared.ty)
                            && actual.mode == ParamMode::Own
                        {
                            if self.allow_moves {
                                mark_value_sources_moved(
                                    &field.value,
                                    &mut self.scopes[scope].bindings,
                                    self.types,
                                );
                            } else {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-O105",
                                    "contract expression cannot transfer an owned record field",
                                    field.value.span,
                                ));
                            }
                        } else if self.types.contains_resource(&declared.ty)
                            && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-O108",
                                "cannot move an owned field through a borrowed or shared record",
                                field.value.span,
                            ));
                        }
                    }
                    let next = index + 1;
                    if fields.get(next).is_some() {
                        self.frames.push(VerifierFrame::PrepareRecordField {
                            expression,
                            type_name,
                            type_arguments,
                            fields,
                            declared_fields,
                            scope,
                            index: next,
                            supplied,
                        });
                    } else {
                        if let Some(declared_fields) = declared_fields {
                            for field in declared_fields {
                                if !supplied.contains(field.name.as_str()) {
                                    self.diagnostics.push(error(self.program, "SPX-T213", format!("record `{type_name}` construction is missing field `{}`", field.name), expression.span));
                                }
                            }
                            let instance = Type::Named {
                                name: type_name.to_owned(),
                                arguments: type_arguments.to_vec(),
                            };
                            self.values.push(Some(CheckedValue::returned(
                                instance.clone(),
                                self.types.contains_resource(&instance),
                            )));
                        } else {
                            self.values.push(None);
                        }
                    }
                }
                VerifierFrame::PrepareVariantField {
                    expression,
                    type_name,
                    type_arguments,
                    case_name,
                    fields,
                    declaration,
                    case,
                    scope,
                    index,
                    mut supplied,
                } => {
                    let field = &fields[index];
                    let declared = case.and_then(|case| {
                        case.fields
                            .iter()
                            .find(|candidate| candidate.name == field.name)
                    });
                    if !supplied.insert(field.name.as_str()) || declared.is_none() {
                        self.diagnostics.push(error(self.program, "SPX-T212", format!("unknown or duplicate payload field `{}` in `{type_name}::{case_name}` construction", field.name), field.span));
                    }
                    self.frames.push(VerifierFrame::ResumeVariantField {
                        expression,
                        type_name,
                        type_arguments,
                        case_name,
                        fields,
                        declaration,
                        case,
                        scope,
                        index,
                        supplied,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: &field.value,
                        scope,
                    });
                }
                VerifierFrame::ResumeVariantField {
                    expression,
                    type_name,
                    type_arguments,
                    case_name,
                    fields,
                    declaration,
                    case,
                    scope,
                    index,
                    supplied,
                } => {
                    let actual = self.values.pop().unwrap_or(None);
                    let field = &fields[index];
                    let declared = case.and_then(|case| {
                        case.fields
                            .iter()
                            .find(|candidate| candidate.name == field.name)
                    });
                    if let (Some(declaration), Some(declared), Some(actual)) =
                        (declaration, declared, actual)
                    {
                        reject_native_unit_value(
                            self.program,
                            &field.value,
                            &actual,
                            self.diagnostics,
                        );
                        let expected = TypeTable::substitute_variant_type(
                            declaration,
                            type_arguments,
                            &declared.ty,
                        )
                        .unwrap_or_else(|| declared.ty.clone());
                        if actual.ty != expected {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T215",
                                format!(
                                    "payload `{}::{}.{}` expects {}, received {}",
                                    type_name, case_name, field.name, expected, actual.ty
                                ),
                                field.value.span,
                            ));
                        }
                    }
                    let next = index + 1;
                    if fields.get(next).is_some() {
                        self.frames.push(VerifierFrame::PrepareVariantField {
                            expression,
                            type_name,
                            type_arguments,
                            case_name,
                            fields,
                            declaration,
                            case,
                            scope,
                            index: next,
                            supplied,
                        });
                    } else {
                        if let Some(case) = case {
                            for field in &case.fields {
                                if !supplied.contains(field.name.as_str()) {
                                    self.diagnostics.push(error(self.program, "SPX-T213", format!("variant construction `{type_name}::{case_name}` is missing payload field `{}`", field.name), expression.span));
                                }
                            }
                            self.values.push(Some(CheckedValue::value(Type::Named {
                                name: type_name.to_owned(),
                                arguments: type_arguments.to_vec(),
                            })));
                        } else {
                            self.values.push(None);
                        }
                    }
                }
                VerifierFrame::ResumeUpdateBase {
                    expression,
                    base,
                    fields,
                    scope,
                } => {
                    let Some(base_value) = self.values.pop().flatten() else {
                        self.values.push(None);
                        continue;
                    };
                    reject_native_unit_value(self.program, base, &base_value, self.diagnostics);
                    let Some(declared_fields) = self.types.record_fields(&base_value.ty) else {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T215",
                            format!(
                                "record update requires a record base, received {}",
                                base_value.ty
                            ),
                            base.span,
                        ));
                        self.values.push(None);
                        continue;
                    };
                    if self.types.contains_resource(&base_value.ty) {
                        match base_value.mode {
                            ParamMode::Own if self.allow_moves => mark_value_sources_moved(
                                base,
                                &mut self.scopes[scope].bindings,
                                self.types,
                            ),
                            ParamMode::Own => self.diagnostics.push(error(
                                self.program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned record update base",
                                base.span,
                            )),
                            ParamMode::Borrow | ParamMode::Shared => self.diagnostics.push(error(
                                self.program,
                                "SPX-O108",
                                "cannot update an owned record through a borrowed or shared base",
                                base.span,
                            )),
                            ParamMode::Value => {}
                        }
                    }
                    if !fields.is_empty() {
                        self.frames.push(VerifierFrame::PrepareUpdateField {
                            expression,
                            base_type: base_value.ty,
                            fields,
                            declared_fields,
                            scope,
                            index: 0,
                            supplied: HashSet::new(),
                        });
                    } else {
                        self.values.push(Some(CheckedValue::returned(
                            base_value.ty.clone(),
                            self.types.contains_resource(&base_value.ty),
                        )));
                    }
                }
                VerifierFrame::PrepareUpdateField {
                    expression,
                    base_type,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    mut supplied,
                } => {
                    let field = &fields[index];
                    let declared = declared_fields
                        .iter()
                        .find(|candidate| candidate.name == field.name);
                    if !supplied.insert(field.name.as_str()) || declared.is_none() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T212",
                            format!(
                                "unknown or duplicate field `{}` in `{}` update",
                                field.name, base_type
                            ),
                            field.span,
                        ));
                    }
                    self.frames.push(VerifierFrame::ResumeUpdateField {
                        expression,
                        base_type,
                        fields,
                        declared_fields,
                        scope,
                        index,
                        supplied,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: &field.value,
                        scope,
                    });
                }
                VerifierFrame::ResumeUpdateField {
                    expression,
                    base_type,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                } => {
                    let actual = self.values.pop().unwrap_or(None);
                    let field = &fields[index];
                    let declared = declared_fields
                        .iter()
                        .find(|candidate| candidate.name == field.name);
                    if let (Some(declared), Some(actual)) = (declared, actual) {
                        reject_native_unit_value(
                            self.program,
                            &field.value,
                            &actual,
                            self.diagnostics,
                        );
                        let expected = self
                            .types
                            .record_field_type(&base_type, declared)
                            .unwrap_or_else(|| declared.ty.clone());
                        if actual.ty != expected {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T215",
                                format!(
                                    "field `{}.{}` expects {}, received {}",
                                    base_type, field.name, expected, actual.ty
                                ),
                                field.value.span,
                            ));
                        }
                        if self.types.contains_resource(&expected) && actual.mode == ParamMode::Own
                        {
                            if self.allow_moves {
                                mark_value_sources_moved(
                                    &field.value,
                                    &mut self.scopes[scope].bindings,
                                    self.types,
                                );
                            } else {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-O105",
                                    "contract expression cannot transfer an owned record replacement",
                                    field.value.span,
                                ));
                            }
                        } else if self.types.contains_resource(&expected)
                            && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-O108",
                                "cannot move an owned replacement through a borrowed or shared value",
                                field.value.span,
                            ));
                        }
                    }
                    let next = index + 1;
                    if fields.get(next).is_some() {
                        self.frames.push(VerifierFrame::PrepareUpdateField {
                            expression,
                            base_type,
                            fields,
                            declared_fields,
                            scope,
                            index: next,
                            supplied,
                        });
                    } else {
                        self.values.push(Some(CheckedValue::returned(
                            base_type.clone(),
                            self.types.contains_resource(&base_type),
                        )));
                    }
                }
                VerifierFrame::ResumeMatchScrutinee {
                    expression,
                    scrutinee,
                    arms,
                    scope,
                } => {
                    let scrutinee_value = self.values.pop().unwrap_or(None);
                    if let Some(value) = &scrutinee_value {
                        reject_native_unit_value(self.program, scrutinee, value, self.diagnostics);
                    }
                    if scrutinee_value
                        .as_ref()
                        .is_some_and(|value| self.types.record_fields(&value.ty).is_some())
                    {
                        let scrutinee_value = scrutinee_value.expect("record checked above");
                        if self.types.contains_resource(&scrutinee_value.ty)
                            || scrutinee_value.mode != ParamMode::Value
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-O111",
                                "plain record match requires a Copy scrutinee",
                                scrutinee.span,
                            ));
                        }
                        let Some((first, rest)) = arms.split_first() else {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-M101",
                                format!(
                                    "non-exhaustive match; missing record pattern for `{}`",
                                    scrutinee_value.ty
                                ),
                                expression.span,
                            ));
                            self.values.push(None);
                            continue;
                        };
                        for arm in rest {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-M102",
                                "unreachable arm after an irrefutable record pattern",
                                arm.pattern.span(),
                            ));
                        }
                        let outer_names = self.scopes[scope]
                            .bindings
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>();
                        let arm_scope = self.scopes.len();
                        self.scopes.push(VerifierScope {
                            bindings: self.scopes[scope].bindings.clone(),
                        });
                        match &first.pattern {
                            MatchPattern::Wildcard { .. } => {}
                            MatchPattern::Record {
                                type_name,
                                fields,
                                span,
                                ..
                            } => check_record_pattern(
                                self.program,
                                type_name,
                                fields,
                                &scrutinee_value.ty,
                                &mut self.scopes[arm_scope].bindings,
                                self.types,
                                self.diagnostics,
                                *span,
                            ),
                            MatchPattern::Variant { .. } => self.diagnostics.push(error(
                                self.program,
                                "SPX-M103",
                                "variant pattern is incompatible with a record scrutinee",
                                first.pattern.span(),
                            )),
                        }
                        self.frames.push(VerifierFrame::ResumeRecordMatchArm {
                            arm: first,
                            parent_scope: scope,
                            arm_scope,
                            outer_names,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: &first.value,
                            scope: arm_scope,
                        });
                        continue;
                    }
                    let variant_instance =
                        scrutinee_value.as_ref().and_then(|value| match &value.ty {
                            Type::Named { name, arguments }
                                if self.types.variant_cases(&value.ty).is_some() =>
                            {
                                Some((name.clone(), arguments.clone()))
                            }
                            Type::I64
                            | Type::Char
                            | Type::U8
                            | Type::F32
                            | Type::F64
                            | Type::Bool
                            | Type::Named { .. } => None,
                        });
                    let variant_name = variant_instance.as_ref().map(|(name, _)| name.clone());
                    let declared_cases = scrutinee_value
                        .as_ref()
                        .and_then(|value| self.types.variant_cases(&value.ty));
                    if declared_cases.is_none() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M103",
                            format!(
                                "match scrutinee must be a Copy variant, received {}",
                                scrutinee_value.as_ref().map_or_else(
                                    || "an invalid value".to_owned(),
                                    |value| value.ty.to_string()
                                )
                            ),
                            scrutinee.span,
                        ));
                    }
                    let outer_names = self.scopes[scope]
                        .bindings
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>();
                    let state = VariantMatchState {
                        expression,
                        arms,
                        parent_scope: scope,
                        index: 0,
                        outer_names,
                        baseline: self.scopes[scope].bindings.clone(),
                        arm_states: Vec::new(),
                        covered: HashSet::new(),
                        wildcard_seen: false,
                        result: None,
                        variant_name,
                        variant_arguments: variant_instance
                            .map(|(_, arguments)| arguments)
                            .unwrap_or_default(),
                        declared_cases,
                    };
                    self.frames
                        .push(VerifierFrame::PrepareVariantMatchArm(state));
                }
                VerifierFrame::ResumeRecordMatchArm {
                    arm,
                    parent_scope,
                    arm_scope,
                    outer_names,
                } => {
                    if arm_scope + 1 != self.scopes.len() {
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            "record match arm scope is not the active child",
                        ));
                    }
                    let result = self.values.pop().unwrap_or(None);
                    if let Some(value) = &result {
                        reject_native_unit_value(self.program, &arm.value, value, self.diagnostics);
                    }
                    let arm_bindings = self
                        .scopes
                        .pop()
                        .expect("record arm scope is active")
                        .bindings;
                    merge_moved(
                        &mut self.scopes[parent_scope].bindings,
                        &arm_bindings,
                        &outer_names,
                    );
                    if result.as_ref().is_some_and(|value| {
                        !matches!(value.ty, Type::I64 | Type::Bool)
                            || value.mode != ParamMode::Value
                    }) {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T216",
                            "record match arm must return a Copy i64 or bool value",
                            arm.value.span,
                        ));
                        self.values.push(None);
                    } else {
                        self.values.push(result);
                    }
                }
                VerifierFrame::PrepareVariantMatchArm(mut state) => {
                    if state.index >= state.arms.len() {
                        if !state.wildcard_seen {
                            if let (Some(variant_name), Some(cases)) =
                                (&state.variant_name, state.declared_cases)
                            {
                                if let Some(missing) = cases
                                    .iter()
                                    .find(|case| !state.covered.contains(&case.name))
                                {
                                    let witness = if missing.fields.is_empty() {
                                        format!("{variant_name}::{} {{}}", missing.name)
                                    } else {
                                        format!("{variant_name}::{} {{ .. }}", missing.name)
                                    };
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-M101",
                                        format!("non-exhaustive match; missing case `{witness}`"),
                                        state.expression.span,
                                    ));
                                }
                            }
                        }
                        if let Some((first, rest)) = state.arm_states.split_first() {
                            let mut joined = first.clone();
                            for branch in rest {
                                for name in &state.outer_names {
                                    if let (Some(joined_binding), Some(branch_binding)) =
                                        (joined.get_mut(name), branch.get(name))
                                    {
                                        joined_binding.availability = joined_binding
                                            .availability
                                            .join(branch_binding.availability);
                                        joined_binding.moved_places =
                                            join_moved_places(joined_binding, branch_binding);
                                        joined_binding.definitely_partial =
                                            join_definitely_partial(joined_binding, branch_binding);
                                    }
                                }
                            }
                            merge_moved(
                                &mut self.scopes[state.parent_scope].bindings,
                                &joined,
                                &state.outer_names,
                            );
                        }
                        self.values.push(state.result);
                        continue;
                    }
                    let arm = &state.arms[state.index];
                    let arm_scope = self.scopes.len();
                    self.scopes.push(VerifierScope {
                        bindings: state.baseline.clone(),
                    });
                    match &arm.pattern {
                        MatchPattern::Wildcard { span } => {
                            if state.wildcard_seen
                                || state
                                    .declared_cases
                                    .is_some_and(|cases| state.covered.len() == cases.len())
                            {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-M102",
                                    "unreachable wildcard match arm",
                                    *span,
                                ));
                            }
                            state.wildcard_seen = true;
                        }
                        MatchPattern::Variant {
                            type_name,
                            case_name,
                            fields,
                            span,
                            ..
                        } => {
                            let compatible = state.variant_name.as_deref() == Some(type_name);
                            let declared_case = compatible
                                .then_some(state.declared_cases)
                                .flatten()
                                .and_then(|cases| {
                                    cases.iter().find(|case| case.name == *case_name)
                                });
                            if declared_case.is_none() {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-M103",
                                    format!("pattern `{type_name}::{case_name}` is incompatible with the match scrutinee"),
                                    *span,
                                ));
                            } else if state.wildcard_seen
                                || !state.covered.insert(case_name.clone())
                            {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-M102",
                                    format!(
                                        "unreachable duplicate case `{type_name}::{case_name}`"
                                    ),
                                    *span,
                                ));
                            }
                            let mut supplied = HashSet::new();
                            let mut bindings = HashSet::new();
                            for field in fields {
                                let declared_field = declared_case.and_then(|case| {
                                    case.fields
                                        .iter()
                                        .find(|candidate| candidate.name == field.name)
                                });
                                if !supplied.insert(field.name.as_str()) || declared_field.is_none()
                                {
                                    self.diagnostics.push(error(self.program, "SPX-M104", format!("unknown or duplicate pattern field `{}` in `{type_name}::{case_name}`", field.name), field.span));
                                }
                                if !source_identifier(&field.binding)
                                    || !bindings.insert(field.binding.as_str())
                                    || self.scopes[arm_scope].bindings.contains_key(&field.binding)
                                {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-M104",
                                        format!(
                                            "invalid or duplicate pattern binding `{}`",
                                            field.binding
                                        ),
                                        field.binding_span,
                                    ));
                                    continue;
                                }
                                if let Some(declared_field) = declared_field {
                                    let binding_ty = state
                                        .variant_name
                                        .as_ref()
                                        .and_then(|name| {
                                            self.types.declaration(name).and_then(|declaration| {
                                                TypeTable::substitute_variant_type(
                                                    declaration,
                                                    &state.variant_arguments,
                                                    &declared_field.ty,
                                                )
                                            })
                                        })
                                        .unwrap_or_else(|| declared_field.ty.clone());
                                    self.scopes[arm_scope].bindings.insert(
                                        field.binding.clone(),
                                        Binding {
                                            ty: binding_ty,
                                            mode: ParamMode::Value,
                                            availability: Availability::Available,
                                            moved_places: HashMap::new(),
                                            definitely_partial: HashSet::new(),
                                            native_unit_discard: false,
                                        },
                                    );
                                }
                            }
                            if let Some(declared_case) = declared_case {
                                for field in &declared_case.fields {
                                    if !supplied.contains(field.name.as_str()) {
                                        self.diagnostics.push(error(self.program, "SPX-M104", format!("pattern `{type_name}::{case_name}` is missing payload field `{}`", field.name), *span));
                                    }
                                }
                            }
                        }
                        MatchPattern::Record { span, .. } => self.diagnostics.push(error(
                            self.program,
                            "SPX-M103",
                            "record pattern is incompatible with a variant scrutinee",
                            *span,
                        )),
                    }
                    self.frames
                        .push(VerifierFrame::ResumeVariantMatchArm { state, arm_scope });
                    self.frames.push(VerifierFrame::Enter {
                        expression: &arm.value,
                        scope: arm_scope,
                    });
                }
                VerifierFrame::ResumeVariantMatchArm {
                    mut state,
                    arm_scope,
                } => {
                    if arm_scope + 1 != self.scopes.len() {
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            "variant match arm scope is not the active child",
                        ));
                    }
                    let arm = &state.arms[state.index];
                    let arm_value = self.values.pop().unwrap_or(None);
                    if let Some(value) = &arm_value {
                        reject_native_unit_value(self.program, &arm.value, value, self.diagnostics);
                    }
                    if let Some(arm_value) = arm_value {
                        if let Some(expected) = &state.result {
                            if expected.ty != arm_value.ty || expected.mode != arm_value.mode {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T216",
                                    format!(
                                        "match arms return incompatible values: {} and {}",
                                        expected.ty, arm_value.ty
                                    ),
                                    arm.value.span,
                                ));
                            }
                        } else {
                            state.result = Some(arm_value);
                        }
                    }
                    state.arm_states.push(
                        self.scopes
                            .pop()
                            .expect("variant arm scope is active")
                            .bindings,
                    );
                    state.index += 1;
                    self.frames
                        .push(VerifierFrame::PrepareVariantMatchArm(state));
                }
            }
        }
        if self.values.len() != 1 {
            return Err(Diagnostic::io(
                "SPX-H006",
                "iterative verifier value stack did not settle",
            ));
        }
        Ok(self.values.pop().expect("value count checked above"))
    }
}

#[allow(clippy::too_many_arguments)]
fn check_expr_iterative(
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    let initial = std::mem::take(variables);
    let mut verifier = IterativeVerifier::new(
        program,
        current,
        initial,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
    let result = verifier.run(expr);
    *variables = verifier
        .scopes
        .first_mut()
        .map(|scope| std::mem::take(&mut scope.bindings))
        .unwrap_or_default();
    drop(verifier);
    match result {
        Ok(value) => value,
        Err(diagnostic) => {
            diagnostics.push(diagnostic.at_path(&program.path));
            None
        }
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn check_expr(
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    match &expr.kind {
        ExprKind::Int(_) => Some(CheckedValue::value(Type::I64)),
        ExprKind::Char(_) => Some(CheckedValue::value(Type::Char)),
        ExprKind::Uint8(_) => Some(CheckedValue::value(Type::U8)),
        ExprKind::Float32(_) => Some(CheckedValue::value(Type::F32)),
        ExprKind::Float64(_) => Some(CheckedValue::value(Type::F64)),
        ExprKind::Bool(_) => Some(CheckedValue::value(Type::Bool)),
        ExprKind::Var(name) if name == "result" => result_type
            .map(|ty| CheckedValue::returned(ty.clone(), types.contains_resource(ty)))
            .or_else(|| {
                diagnostics.push(error(
                    program,
                    "SPX-T201",
                    "`result` is only available in postconditions",
                    expr.span,
                ));
                None
            }),
        ExprKind::Var(name) => variables
            .get(name.as_str())
            .map(|binding| {
                match binding.availability {
                    Availability::Moved => diagnostics.push(
                        error(
                            program,
                            "SPX-O101",
                            format!("use of resource `{name}` after ownership was moved"),
                            expr.span,
                        )
                        .with_help("borrow the resource if the callee does not need ownership"),
                    ),
                    Availability::MaybeMoved => diagnostics.push(
                        error(
                            program,
                            "SPX-O107",
                            format!(
                                "resource `{name}` may have been moved on another control-flow path"
                            ),
                            expr.span,
                        )
                        .with_help("move the resource on every path or keep it borrowed"),
                    ),
                    Availability::Available => match overlapping_place_state(binding, &[]) {
                        Availability::Moved => diagnostics.push(
                            error(
                                program,
                                "SPX-O109",
                                format!("use of partially moved place `{name}`"),
                                expr.span,
                            )
                            .with_help(
                                "use an available sibling field or avoid moving this place earlier",
                            ),
                        ),
                        Availability::MaybeMoved => diagnostics.push(
                            error(
                                program,
                                "SPX-O110",
                                format!(
                                    "place `{name}` may have been moved on another control-flow path"
                                ),
                                expr.span,
                            )
                            .with_help("move the field on every path or keep it borrowed"),
                        ),
                        Availability::Available => {}
                    },
                }
                if binding.native_unit_discard {
                    diagnostics.push(error(
                        program,
                        "SPX-B107",
                        "Native Rust Interop declaration set is unsupported: scalar value signature required",
                        expr.span,
                    ));
                }
                CheckedValue {
                    ty: binding.ty.clone(),
                    mode: binding.mode,
                    native_unit: binding.native_unit_discard,
                }
            })
            .or_else(|| {
                diagnostics.push(error(
                    program,
                    "SPX-T202",
                    format!("unknown value `{name}` in `{}`", current.name),
                    expr.span,
                ));
                None
            }),
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            let native_import = program
                .interfaces
                .iter()
                .flat_map(|interface| &interface.imports)
                .find(|import| import.native_rust && import.name == *name);
            if let Some(import) = native_import {
                if !type_arguments.is_empty() || args.len() != import.params.len() {
                    diagnostics.push(error(
                        program,
                        "SPX-B107",
                        "Native Rust Interop declaration set is unsupported: scalar value signature required",
                        expr.span,
                    ));
                }
                for effect in &import.effects {
                    if !current.effects.contains(effect) {
                        diagnostics.push(error(
                            program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                            expr.span,
                        ));
                    }
                }
                for (index, argument) in args.iter().enumerate() {
                    let actual = check_expr(
                        program,
                        current,
                        argument,
                        variables,
                        functions,
                        types,
                        result_type,
                        allow_moves,
                        diagnostics,
                    );
                    if let (Some(actual), Some(parameter)) = (actual, import.params.get(index)) {
                        reject_native_unit_value(program, argument, &actual, diagnostics);
                        if !actual.native_unit
                            && (actual.ty != parameter.ty || actual.mode != ParamMode::Value)
                        {
                            diagnostics.push(error(
                                program,
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                argument.span,
                            ));
                        }
                    }
                }
                let native_unit = import.result == ImportResult::Unit;
                let mut checked = CheckedValue::value(match import.result {
                    ImportResult::Unit => Type::Named {
                        name: "\0native-rust-unit".to_owned(),
                        arguments: Vec::new(),
                    },
                    ImportResult::I64 => Type::I64,
                    ImportResult::Bool => Type::Bool,
                });
                checked.native_unit = native_unit;
                return Some(checked);
            }
            let target = functions.get(name.as_str()).copied();
            if target.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T203",
                    format!("unknown function `{name}`"),
                    expr.span,
                ));
            }
            if target.is_some_and(|target| args.len() != target.params.len()) {
                let target = target.expect("checked above");
                diagnostics.push(error(
                    program,
                    "SPX-T204",
                    format!(
                        "`{name}` expects {} arguments, received {}",
                        target.params.len(),
                        args.len()
                    ),
                    expr.span,
                ));
            }
            let specialized_target = target.and_then(|target| {
                if target.type_parameters.is_empty() {
                    if !type_arguments.is_empty() {
                        diagnostics.push(error(
                            program,
                            "SPX-T225",
                            format!("monomorphic function `{name}` does not accept type arguments"),
                            expr.span,
                        ));
                        return None;
                    }
                    return Some(target.clone());
                }
                if !current.type_parameters.is_empty() {
                    diagnostics.push(error(
                        program,
                        "SPX-T226",
                        format!(
                            "generic function `{}` cannot call generic function `{name}` in this slice",
                            current.name
                        ),
                        expr.span,
                    ));
                }
                if type_arguments.len() != target.type_parameters.len() {
                    diagnostics.push(error(
                        program,
                        "SPX-T225",
                        format!(
                            "generic function `{name}` expects {} explicit type arguments, received {}",
                            target.type_parameters.len(),
                            type_arguments.len()
                        ),
                        expr.span,
                    ));
                    return None;
                }
                if type_arguments.iter().any(|argument| !direct_function_type_argument(argument)) {
                    diagnostics.push(error(
                        program,
                        "SPX-T225",
                        format!(
                            "generic function `{name}` accepts only direct `i64` or `bool` type arguments"
                        ),
                        expr.span,
                    ));
                    return None;
                }
                    validation_specialize_function(target, type_arguments)
            });
            for (index, arg) in args.iter().enumerate() {
                let actual = check_expr(
                    program,
                    current,
                    arg,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                let Some(param) = specialized_target
                    .as_ref()
                    .and_then(|target| target.params.get(index))
                else {
                    continue;
                };
                if let Some(actual) = &actual {
                    reject_native_unit_value(program, arg, actual, diagnostics);
                }
                if actual
                    .as_ref()
                    .is_some_and(|actual| !actual.native_unit && actual.ty != param.ty)
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T205",
                        format!(
                            "argument `{}` to `{name}` expects {}, received {}",
                            param.name,
                            param.ty,
                            actual.as_ref().expect("type checked above").ty
                        ),
                        arg.span,
                    ));
                }
                check_argument_ownership(
                    program,
                    current,
                    name,
                    arg,
                    param,
                    actual.as_ref(),
                    variables,
                    types,
                    allow_moves,
                    diagnostics,
                );
            }
            specialized_target.map(|target| {
                CheckedValue::returned(
                    target.return_type.clone(),
                    types.contains_resource(&target.return_type),
                )
            })
        }
        ExprKind::Unary { op, value } => {
            // Peel maximal unary chains iteratively. The language admits an
            // exact semantic depth of 512, which must not consume one verifier
            // call frame per node on an ordinary caller stack.
            let mut unary = vec![(*op, value.as_ref(), expr.span)];
            let mut leaf = value.as_ref();
            while let ExprKind::Unary { op, value } = &leaf.kind {
                unary.push((*op, value.as_ref(), leaf.span));
                leaf = value;
            }
            let mut actual = check_expr(
                program,
                current,
                leaf,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            for (op, operand, span) in unary.into_iter().rev() {
                let numeric = matches!(op, UnaryOp::Neg)
                    .then(|| actual.ty.clone())
                    .filter(|ty| matches!(ty, Type::I64 | Type::F32 | Type::F64));
                let expected = match (&op, &numeric) {
                    (UnaryOp::Neg, Some(ty)) => ty.clone(),
                    (UnaryOp::Neg, None) => Type::I64,
                    (UnaryOp::Not, _) => Type::Bool,
                };
                if !actual.native_unit && actual.ty != expected {
                    diagnostics.push(error(
                        program,
                        "SPX-T206",
                        format!("unary operator expects {expected}, received {}", actual.ty),
                        span,
                    ));
                }
                reject_native_unit_value(program, operand, &actual, diagnostics);
                actual = CheckedValue::value(expected);
            }
            Some(actual)
        }
        ExprKind::Binary { op, left, right } => {
            let left_ty = check_expr(
                program,
                current,
                left,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let right_ty = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                let names = variables.keys().cloned().collect::<Vec<_>>();
                let mut right_variables = variables.clone();
                let value = check_expr(
                    program,
                    current,
                    right,
                    &mut right_variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                join_conditional(variables, &right_variables, &names);
                value
            } else {
                check_expr(
                    program,
                    current,
                    right,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                )
            };
            if let Some(value) = &left_ty {
                reject_native_unit_value(program, left, value, diagnostics);
            }
            if let Some(value) = &right_ty {
                reject_native_unit_value(program, right, value, diagnostics);
            }
            let native_unit_operand = left_ty.as_ref().is_some_and(|value| value.native_unit)
                || right_ty.as_ref().is_some_and(|value| value.native_unit);
            let left_ordered = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| {
                    matches!(
                        ty,
                        Type::I64 | Type::Char | Type::U8 | Type::F32 | Type::F64
                    )
                });
            let left_narrow = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| matches!(ty, Type::U8));
            let left_numeric = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| matches!(ty, Type::F32 | Type::F64));
            if !native_unit_operand
                && matches!(op, BinaryOp::Rem)
                && (left_numeric.is_some() || left_narrow.is_some())
            {
                diagnostics.push(error(
                    program,
                    "SPX-T208",
                    format!("operator `{}` expects i64 operands", op.text()),
                    expr.span,
                ));
            }
            let (expected, output) = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    let expected = left_numeric.or(left_narrow).unwrap_or(Type::I64);
                    (expected.clone(), expected)
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    let expected = left_ordered.unwrap_or(Type::I64);
                    (expected, Type::Bool)
                }
                BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
                BinaryOp::Eq | BinaryOp::Ne => {
                    if !native_unit_operand
                        && left_ty.is_some()
                        && right_ty.is_some()
                        && left_ty.as_ref().map(|value| &value.ty)
                            != right_ty.as_ref().map(|value| &value.ty)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T207",
                            "equality operands must have the same type",
                            expr.span,
                        ));
                    }
                    return Some(CheckedValue::value(Type::Bool));
                }
            };
            if !native_unit_operand
                && (left_ty.as_ref().is_some_and(|value| value.ty != expected)
                    || right_ty.as_ref().is_some_and(|value| value.ty != expected))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T208",
                    format!("operator `{}` expects {expected} operands", op.text()),
                    expr.span,
                ));
            }
            Some(CheckedValue::value(output))
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            let declaration = types.declaration(type_name);
            let instance = Type::Named {
                name: type_name.clone(),
                arguments: type_arguments.clone(),
            };
            check_declared_type(
                program,
                &instance,
                expr.span,
                types,
                &HashSet::new(),
                diagnostics,
            );
            let declared_fields = declaration.and_then(|declaration| match &declaration.kind {
                TypeDeclarationKind::Record { fields } => Some(fields.as_slice()),
                TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => None,
            });
            if declared_fields.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!("`{type_name}` is not a declared record type"),
                    expr.span,
                ));
            }

            let mut supplied = HashSet::new();
            for field in fields {
                let declared = declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                });
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T212",
                        format!(
                            "unknown or duplicate field `{}` in `{type_name}` construction",
                            field.name
                        ),
                        field.span,
                    ));
                }
                let actual = check_expr(
                    program,
                    current,
                    &field.value,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let (Some(declared), Some(actual)) = (declared, actual) {
                    reject_native_unit_value(program, &field.value, &actual, diagnostics);
                    let expected = declaration
                        .and_then(|declaration| {
                            TypeTable::substitute_variant_type(
                                declaration,
                                type_arguments,
                                &declared.ty,
                            )
                        })
                        .unwrap_or_else(|| declared.ty.clone());
                    if actual.ty != expected {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "field `{}.{}` expects {}, received {}",
                                type_name, field.name, expected, actual.ty
                            ),
                            field.value.span,
                        ));
                    }
                    if types.contains_resource(&declared.ty) && actual.mode == ParamMode::Own {
                        if allow_moves {
                            mark_value_sources_moved(&field.value, variables, types);
                        } else {
                            diagnostics.push(error(
                                program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned record field",
                                field.value.span,
                            ));
                        }
                    } else if types.contains_resource(&declared.ty)
                        && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O108",
                            "cannot move an owned field through a borrowed or shared record",
                            field.value.span,
                        ));
                    }
                }
            }
            if let Some(declared_fields) = declared_fields {
                for field in declared_fields {
                    if !supplied.contains(field.name.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-T213",
                            format!(
                                "record `{type_name}` construction is missing field `{}`",
                                field.name
                            ),
                            expr.span,
                        ));
                    }
                }
            }

            declared_fields.map(|_| {
                CheckedValue::returned(instance.clone(), types.contains_resource(&instance))
            })
        }
        ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            case_name,
            fields,
            ..
        } => {
            let declaration = types.declaration(type_name);
            let instance = Type::Named {
                name: type_name.clone(),
                arguments: type_arguments.clone(),
            };
            check_declared_type(
                program,
                &instance,
                expr.span,
                types,
                &HashSet::new(),
                diagnostics,
            );
            let cases = declaration.and_then(|declaration| match &declaration.kind {
                TypeDeclarationKind::Variant { cases } => Some(cases.as_slice()),
                TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Record { .. } => None,
            });
            let case = cases.and_then(|cases| cases.iter().find(|case| case.name == *case_name));
            if cases.is_none() || case.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!("`{type_name}::{case_name}` is not a declared variant constructor"),
                    expr.span,
                ));
            }
            let mut supplied = HashSet::new();
            for field in fields {
                let declared = case.and_then(|case| {
                    case.fields
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                });
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T212",
                        format!(
                            "unknown or duplicate payload field `{}` in `{type_name}::{case_name}` construction",
                            field.name
                        ),
                        field.span,
                    ));
                }
                let actual = check_expr(
                    program,
                    current,
                    &field.value,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let (Some(declaration), Some(declared), Some(actual)) =
                    (declaration, declared, actual)
                {
                    reject_native_unit_value(program, &field.value, &actual, diagnostics);
                    let expected = TypeTable::substitute_variant_type(
                        declaration,
                        type_arguments,
                        &declared.ty,
                    )
                        .unwrap_or_else(|| declared.ty.clone());
                    if actual.ty != expected {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "payload `{}::{}.{}` expects {}, received {}",
                                type_name, case_name, field.name, expected, actual.ty
                            ),
                            field.value.span,
                        ));
                    }
                }
            }
            if let Some(case) = case {
                for field in &case.fields {
                    if !supplied.contains(field.name.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-T213",
                            format!(
                                "variant construction `{type_name}::{case_name}` is missing payload field `{}`",
                                field.name
                            ),
                            expr.span,
                        ));
                    }
                }
            }
            case.map(|_| CheckedValue::value(instance))
        }
        ExprKind::Match { scrutinee, arms } => {
            let scrutinee_value = check_expr(
                program,
                current,
                scrutinee,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            if let Some(value) = &scrutinee_value {
                reject_native_unit_value(program, scrutinee, value, diagnostics);
            }
            if scrutinee_value
                .as_ref()
                .is_some_and(|value| types.record_fields(&value.ty).is_some())
            {
                let Some(scrutinee_value) = scrutinee_value else {
                    unreachable!("record instance was checked above");
                };
                if types.contains_resource(&scrutinee_value.ty)
                    || scrutinee_value.mode != ParamMode::Value
                {
                    diagnostics.push(error(
                        program,
                        "SPX-O111",
                        "plain record match requires a Copy scrutinee",
                        scrutinee.span,
                    ));
                }
                let Some((first, rest)) = arms.split_first() else {
                    diagnostics.push(error(
                        program,
                        "SPX-M101",
                        format!(
                            "non-exhaustive match; missing record pattern for `{}`",
                            scrutinee_value.ty
                        ),
                        expr.span,
                    ));
                    return None;
                };
                for arm in rest {
                    diagnostics.push(error(
                        program,
                        "SPX-M102",
                        "unreachable arm after an irrefutable record pattern",
                        arm.pattern.span(),
                    ));
                }
                let outer_names = variables.keys().cloned().collect::<Vec<_>>();
                let mut arm_variables = variables.clone();
                match &first.pattern {
                    MatchPattern::Wildcard { .. } => {}
                    MatchPattern::Record {
                        type_name,
                        fields,
                        span,
                        ..
                    } => check_record_pattern(
                        program,
                        type_name,
                        fields,
                        &scrutinee_value.ty,
                        &mut arm_variables,
                        types,
                        diagnostics,
                        *span,
                    ),
                    MatchPattern::Variant { .. } => diagnostics.push(error(
                        program,
                        "SPX-M103",
                        "variant pattern is incompatible with a record scrutinee",
                        first.pattern.span(),
                    )),
                }
                let result = check_expr(
                    program,
                    current,
                    &first.value,
                    &mut arm_variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let Some(value) = &result {
                    reject_native_unit_value(program, &first.value, value, diagnostics);
                }
                merge_moved(variables, &arm_variables, &outer_names);
                if result.as_ref().is_some_and(|value| {
                    !matches!(value.ty, Type::I64 | Type::Bool)
                        || value.mode != ParamMode::Value
                }) {
                    diagnostics.push(error(
                        program,
                        "SPX-T216",
                        "record match arm must return a Copy i64 or bool value",
                        first.value.span,
                    ));
                    return None;
                }
                return result;
            }
            let variant_instance = scrutinee_value.as_ref().and_then(|value| match &value.ty {
                Type::Named { name, arguments } if types.variant_cases(&value.ty).is_some() => {
                    Some((name.clone(), arguments.clone()))
                }
                Type::I64
                | Type::Char
                | Type::U8
                | Type::F32
                | Type::F64
                | Type::Bool
                | Type::Named { .. } => None,
            });
            let variant_name = variant_instance.as_ref().map(|(name, _)| name.clone());
            let declared_cases = scrutinee_value
                .as_ref()
                .and_then(|value| types.variant_cases(&value.ty));
            if declared_cases.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-M103",
                    format!(
                        "match scrutinee must be a Copy variant, received {}",
                        scrutinee_value
                            .as_ref()
                            .map_or_else(|| "an invalid value".to_owned(), |value| value.ty.to_string())
                    ),
                    scrutinee.span,
                ));
            }

            let outer_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut arm_states = Vec::new();
            let mut covered = HashSet::new();
            let mut wildcard_seen = false;
            let mut result = None::<CheckedValue>;
            for arm in arms {
                let mut arm_variables = variables.clone();
                match &arm.pattern {
                    MatchPattern::Wildcard { span } => {
                        if wildcard_seen
                            || declared_cases
                                .is_some_and(|cases| covered.len() == cases.len())
                        {
                            diagnostics.push(error(
                                program,
                                "SPX-M102",
                                "unreachable wildcard match arm",
                                *span,
                            ));
                        }
                        wildcard_seen = true;
                    }
                    MatchPattern::Variant {
                        type_name,
                        case_name,
                        fields,
                        span,
                        ..
                    } => {
                        let compatible = variant_name.as_deref() == Some(type_name.as_str());
                        let declared_case = compatible.then_some(declared_cases)
                            .flatten()
                            .and_then(|cases| cases.iter().find(|case| case.name == *case_name));
                        if declared_case.is_none() {
                            diagnostics.push(error(
                                program,
                                "SPX-M103",
                                format!(
                                    "pattern `{type_name}::{case_name}` is incompatible with the match scrutinee"
                                ),
                                *span,
                            ));
                        } else if wildcard_seen || !covered.insert(case_name.as_str()) {
                            diagnostics.push(error(
                                program,
                                "SPX-M102",
                                format!("unreachable duplicate case `{type_name}::{case_name}`"),
                                *span,
                            ));
                        }
                        let mut supplied = HashSet::new();
                        let mut bindings = HashSet::new();
                        for field in fields {
                            let declared_field = declared_case.and_then(|case| {
                                case.fields
                                    .iter()
                                    .find(|candidate| candidate.name == field.name)
                            });
                            if !supplied.insert(field.name.as_str()) || declared_field.is_none() {
                                diagnostics.push(error(
                                    program,
                                    "SPX-M104",
                                    format!(
                                        "unknown or duplicate pattern field `{}` in `{type_name}::{case_name}`",
                                        field.name
                                    ),
                                    field.span,
                                ));
                            }
                            if !source_identifier(&field.binding)
                                || !bindings.insert(field.binding.as_str())
                                || arm_variables.contains_key(&field.binding)
                            {
                                diagnostics.push(error(
                                    program,
                                    "SPX-M104",
                                    format!("invalid or duplicate pattern binding `{}`", field.binding),
                                    field.binding_span,
                                ));
                                continue;
                            }
                            if let Some(declared_field) = declared_field {
                                let binding_ty = variant_instance
                                    .as_ref()
                                    .and_then(|(name, arguments)| {
                                        types.declaration(name).and_then(|declaration| {
                                            TypeTable::substitute_variant_type(
                                                declaration,
                                                arguments,
                                                &declared_field.ty,
                                            )
                                        })
                                    })
                                    .unwrap_or_else(|| declared_field.ty.clone());
                                arm_variables.insert(
                                    field.binding.clone(),
                                    Binding {
                                        ty: binding_ty,
                                        mode: ParamMode::Value,
                                        availability: Availability::Available,
                                        moved_places: HashMap::new(),
                                        definitely_partial: HashSet::new(),
                                        native_unit_discard: false,
                                    },
                                );
                            }
                        }
                        if let Some(declared_case) = declared_case {
                            for field in &declared_case.fields {
                                if !supplied.contains(field.name.as_str()) {
                                    diagnostics.push(error(
                                        program,
                                        "SPX-M104",
                                        format!(
                                            "pattern `{type_name}::{case_name}` is missing payload field `{}`",
                                            field.name
                                        ),
                                        *span,
                                    ));
                                }
                            }
                        }
                    }
                    MatchPattern::Record { span, .. } => diagnostics.push(error(
                        program,
                        "SPX-M103",
                        "record pattern is incompatible with a variant scrutinee",
                        *span,
                    )),
                }
                let arm_value = check_expr(
                    program,
                    current,
                    &arm.value,
                    &mut arm_variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let Some(value) = &arm_value {
                    reject_native_unit_value(program, &arm.value, value, diagnostics);
                }
                if let Some(arm_value) = arm_value {
                    if let Some(expected) = &result {
                        if expected.ty != arm_value.ty || expected.mode != arm_value.mode {
                            diagnostics.push(error(
                                program,
                                "SPX-T216",
                                format!(
                                    "match arms return incompatible values: {} and {}",
                                    expected.ty, arm_value.ty
                                ),
                                arm.value.span,
                            ));
                        }
                    } else {
                        result = Some(arm_value);
                    }
                }
                arm_states.push(arm_variables);
            }
            if !wildcard_seen {
                if let (Some(variant_name), Some(cases)) = (&variant_name, declared_cases) {
                    if let Some(missing) = cases
                        .iter()
                        .find(|case| !covered.contains(case.name.as_str()))
                    {
                        let witness = if missing.fields.is_empty() {
                            format!("{variant_name}::{} {{}}", missing.name)
                        } else {
                            format!("{variant_name}::{} {{ .. }}", missing.name)
                        };
                        diagnostics.push(error(
                            program,
                            "SPX-M101",
                            format!("non-exhaustive match; missing case `{witness}`"),
                            expr.span,
                        ));
                    }
                }
            }
            if let Some((first, rest)) = arm_states.split_first() {
                let mut joined = first.clone();
                for state in rest {
                    for name in &outer_names {
                        if let (Some(joined_binding), Some(state_binding)) =
                            (joined.get_mut(name), state.get(name))
                        {
                            joined_binding.availability =
                                joined_binding.availability.join(state_binding.availability);
                            joined_binding.moved_places =
                                join_moved_places(joined_binding, state_binding);
                            joined_binding.definitely_partial =
                                join_definitely_partial(joined_binding, state_binding);
                        }
                    }
                }
                merge_moved(variables, &joined, &outer_names);
            }
            result
        }
        ExprKind::Try { operand } => {
            let operand_value = check_expr(
                program,
                current,
                operand,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let operand_value = operand_value?;
            reject_native_unit_value(program, operand, &operand_value, diagnostics);
            if !allow_moves {
                diagnostics.push(error(
                    program,
                    "SPX-T218",
                    "`?` is only valid in an executable function body",
                    expr.span,
                ));
            }
            if variables
                .values()
                .any(|binding| types.contains_resource(&binding.ty))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T218",
                    "`?` with a live resource binding is not supported yet",
                    expr.span,
                ));
            }
            if let Some((ok, error_ty)) = ordinary_result_arguments(&operand_value.ty) {
                let Some((_, residual_error_ty)) =
                    ordinary_result_arguments(&current.return_type)
                else {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        format!(
                            "function `{}` must return the ordinary compiler-owned Result to propagate a Result with `?`",
                            current.name
                        ),
                        expr.span,
                    ));
                    return Some(CheckedValue::value(ok.clone()));
                };
                if error_ty != residual_error_ty {
                    diagnostics.push(error(
                        program,
                        "SPX-T219",
                        format!(
                            "`?` cannot propagate error type {error_ty} into Result error type {residual_error_ty}"
                        ),
                        expr.span,
                    ));
                }
                return Some(CheckedValue::value(ok.clone()));
            }
            if let Some(some) = ordinary_option_argument(&operand_value.ty) {
                let outer = ordinary_option_argument(&current.return_type);
                if outer.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        format!(
                            "function `{}` must return the ordinary compiler-owned Option to propagate an Option with `?`",
                            current.name
                        ),
                        expr.span,
                    ));
                } else if !matches!(some, Type::I64 | Type::Bool)
                    || outer.is_some_and(|value| !matches!(value, Type::I64 | Type::Bool))
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        "Option `?` accepts only direct `i64` or `bool` source and enclosing payloads",
                        expr.span,
                    ));
                }
                return Some(CheckedValue::value(some.clone()));
            }
            diagnostics.push(error(
                program,
                "SPX-T218",
                format!(
                    "`?` operand must be an ordinary compiler-owned Result or Option, received {}",
                    operand_value.ty
                ),
                expr.span,
            ));
            None
        }
        ExprKind::UpdateRecord { base, fields } => {
            let base_value = check_expr(
                program,
                current,
                base,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            reject_native_unit_value(program, base, &base_value, diagnostics);
            let declared_fields = types.record_fields(&base_value.ty);
            if declared_fields.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!(
                        "record update requires a record base, received {}",
                        base_value.ty
                    ),
                    base.span,
                ));
                return None;
            }

            if types.contains_resource(&base_value.ty) {
                match base_value.mode {
                    ParamMode::Own if allow_moves => {
                        mark_value_sources_moved(base, variables, types);
                    }
                    ParamMode::Own => diagnostics.push(error(
                        program,
                        "SPX-O105",
                        "contract expression cannot transfer an owned record update base",
                        base.span,
                    )),
                    ParamMode::Borrow | ParamMode::Shared => diagnostics.push(error(
                        program,
                        "SPX-O108",
                        "cannot update an owned record through a borrowed or shared base",
                        base.span,
                    )),
                    ParamMode::Value => {}
                }
            }

            let mut supplied = HashSet::new();
            for field in fields {
                let declared = declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                });
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T212",
                        format!(
                            "unknown or duplicate field `{}` in `{}` update",
                            field.name, base_value.ty
                        ),
                        field.span,
                    ));
                }
                let actual = check_expr(
                    program,
                    current,
                    &field.value,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let (Some(declared), Some(actual)) = (declared, actual) {
                    reject_native_unit_value(program, &field.value, &actual, diagnostics);
                    let expected = types
                        .record_field_type(&base_value.ty, declared)
                        .unwrap_or_else(|| declared.ty.clone());
                    if actual.ty != expected {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "field `{}.{}` expects {}, received {}",
                                base_value.ty, field.name, expected, actual.ty
                            ),
                            field.value.span,
                        ));
                    }
                    if types.contains_resource(&expected) && actual.mode == ParamMode::Own {
                        if allow_moves {
                            mark_value_sources_moved(&field.value, variables, types);
                        } else {
                            diagnostics.push(error(
                                program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned record replacement",
                                field.value.span,
                            ));
                        }
                    } else if types.contains_resource(&expected)
                        && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O108",
                            "cannot move an owned replacement through a borrowed or shared value",
                            field.value.span,
                        ));
                    }
                }
            }

            Some(CheckedValue::returned(
                base_value.ty.clone(),
                types.contains_resource(&base_value.ty),
            ))
        }
        ExprKind::Project { base, field, .. } => {
            if let Some(place) = source_place(expr, variables, types) {
                check_source_place_availability(
                    program,
                    &place,
                    variables,
                    expr.span,
                    diagnostics,
                );
                return Some(CheckedValue {
                    ty: place.ty,
                    mode: place.mode,
                    native_unit: false,
                });
            }
            let base_value = check_expr(
                program,
                current,
                base,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            reject_native_unit_value(program, base, &base_value, diagnostics);
            let Some(fields) = types.record_fields(&base_value.ty) else {
                diagnostics.push(error(
                    program,
                    "SPX-T214",
                    format!("cannot project field `{field}` from `{}`", base_value.ty),
                    expr.span,
                ));
                return None;
            };
            let Some(declared) = fields.iter().find(|candidate| candidate.name == *field) else {
                diagnostics.push(error(
                    program,
                    "SPX-T214",
                    format!("record `{}` has no field `{field}`", base_value.ty),
                    expr.span,
                ));
                return None;
            };
            let projected = types
                .record_field_type(&base_value.ty, declared)
                .unwrap_or_else(|| declared.ty.clone());
            let mode = if types.contains_resource(&projected) {
                base_value.mode
            } else {
                ParamMode::Value
            };
            Some(CheckedValue {
                ty: projected,
                mode,
                native_unit: false,
            })
        }
        ExprKind::Block { statements, tail } => {
            let outer_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut scope = variables.clone();
            for statement in statements {
                match statement {
                    Statement::Let {
                        name,
                        name_span,
                        value,
                        ..
                    } => {
                        if !source_identifier(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-S109",
                                format!("`{name}` is reserved and cannot name a local binding"),
                                *name_span,
                            ));
                        }
                        let actual = check_expr(
                            program,
                            current,
                            value,
                            &mut scope,
                            functions,
                            types,
                            result_type,
                            allow_moves,
                            diagnostics,
                        );
                        if scope.contains_key(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-T209",
                                format!("local binding `{name}` shadows an existing value"),
                                *name_span,
                            ));
                            continue;
                        }
                        if let Some(actual) = actual {
                            if types.contains_resource(&actual.ty) && actual.mode == ParamMode::Own
                            {
                                if allow_moves {
                                    mark_value_sources_moved(value, &mut scope, types);
                                } else {
                                    diagnostics.push(error(
                                        program,
                                        "SPX-O105",
                                        "contract expression cannot transfer an owned resource into a local binding",
                                        value.span,
                                    ));
                                }
                            }
                            scope.insert(
                                name.clone(),
                                Binding {
                                    ty: actual.ty,
                                    mode: actual.mode,
                                    availability: Availability::Available,
                                    moved_places: HashMap::new(),
                                    definitely_partial: HashSet::new(),
                                    native_unit_discard: actual.native_unit,
                                },
                            );
                        }
                    }
                }
            }
            let actual = check_expr(
                program,
                current,
                tail,
                &mut scope,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            merge_moved(variables, &scope, &outer_names);
            actual
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if let Some(value) = check_expr(
                program,
                current,
                condition,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            ) {
                if value.native_unit {
                    reject_native_unit_value(program, condition, &value, diagnostics);
                } else if value.ty != Type::Bool {
                    diagnostics.push(error(
                        program,
                        "SPX-T210",
                        "`if` condition must be bool",
                        condition.span,
                    ));
                }
            }
            let original_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut then_variables = variables.clone();
            let mut else_variables = variables.clone();
            let then_value = check_expr(
                program,
                current,
                then_branch,
                &mut then_variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let else_value = check_expr(
                program,
                current,
                else_branch,
                &mut else_variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            for name in &original_names {
                if let Some(binding) = variables.get_mut(name) {
                    let then_state = then_variables
                        .get(name)
                        .map_or(Availability::Available, |value| value.availability);
                    let else_state = else_variables
                        .get(name)
                        .map_or(Availability::Available, |value| value.availability);
                    binding.availability = then_state.join(else_state);
                    if let (Some(then_binding), Some(else_binding)) =
                        (then_variables.get(name), else_variables.get(name))
                    {
                        binding.moved_places = join_moved_places(then_binding, else_binding);
                        binding.definitely_partial =
                            join_definitely_partial(then_binding, else_binding);
                    }
                }
            }
            match (then_value, else_value) {
                (Some(then_value), Some(else_value)) => {
                    if then_value.native_unit || else_value.native_unit {
                        reject_native_unit_value(
                            program,
                            then_branch,
                            &then_value,
                            diagnostics,
                        );
                        reject_native_unit_value(
                            program,
                            else_branch,
                            &else_value,
                            diagnostics,
                        );
                    } else if then_value.ty != else_value.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T211",
                            format!(
                                "`if` branches return different types: {} and {}",
                                then_value.ty, else_value.ty
                            ),
                            expr.span,
                        ));
                    }
                    if types.contains_resource(&then_value.ty) && then_value.mode != else_value.mode
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O106",
                            "`if` branches must produce the same resource ownership mode",
                            expr.span,
                        ));
                    }
                    Some(then_value)
                }
                _ => None,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_argument_ownership(
    program: &Program,
    current: &Function,
    callee: &str,
    arg: &Expr,
    param: &crate::ast::Param,
    actual: Option<&CheckedValue>,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(actual) = actual else {
        return;
    };
    if !types.contains_resource(&actual.ty) {
        return;
    }
    match param.mode {
        ParamMode::Own => {
            if actual.mode != ParamMode::Own {
                if matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    && source_place(arg, variables, types)
                        .is_some_and(|place| !place.projections.is_empty())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-O108",
                        "cannot move an owned field through a borrowed or shared record",
                        arg.span,
                    ));
                    return;
                }
                diagnostics.push(
                    error(
                        program,
                        "SPX-O102",
                        format!(
                            "argument to `{}.{}` is {}, so `{current_name}` cannot transfer it to `{callee}`",
                            current.name,
                            param.name,
                            actual.mode.text(),
                            current_name = current.name
                        ),
                        arg.span,
                    )
                    .with_help(format!(
                        "provide an owned `{}` value at this ownership boundary",
                        actual.ty
                    )),
                );
            } else if allow_moves {
                mark_value_sources_moved(arg, variables, types);
            } else {
                diagnostics.push(error(
                    program,
                    "SPX-O105",
                    format!("contract expression cannot transfer a resource into `{callee}`"),
                    arg.span,
                ));
            }
        }
        ParamMode::Shared if actual.mode != ParamMode::Shared => diagnostics.push(
            error(
                program,
                "SPX-O103",
                format!("`{callee}` requires shared resource ownership"),
                arg.span,
            )
            .with_help("create or receive an explicitly shared resource before this call"),
        ),
        ParamMode::Borrow | ParamMode::Shared | ParamMode::Value => {}
    }
}

fn mark_value_sources_moved(
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
) {
    enum Frame<'a> {
        Enter(&'a Expr, usize),
        AfterThen {
            else_branch: &'a Expr,
            parent: usize,
            then_scope: usize,
            names: Vec<String>,
        },
        AfterElse {
            parent: usize,
            else_scope: usize,
            names: Vec<String>,
            then_variables: HashMap<String, Binding>,
        },
    }
    let root = std::mem::take(variables);
    let mut scopes = vec![root];
    let mut frames = vec![Frame::Enter(expr, 0)];
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(expr, scope) => match &expr.kind {
                ExprKind::Var(name) => {
                    if let Some(binding) = scopes[scope].get_mut(name) {
                        if types.contains_resource(&binding.ty)
                            && binding.mode == ParamMode::Own
                            && binding.availability == Availability::Available
                        {
                            binding.availability = Availability::Moved;
                        }
                    }
                }
                ExprKind::Block { tail, .. } => frames.push(Frame::Enter(tail, scope)),
                ExprKind::Project { base, .. } => {
                    if let Some(place) = source_place(expr, &scopes[scope], types) {
                        if let Some(binding) = scopes[scope].get_mut(&place.root) {
                            if binding.mode == ParamMode::Own {
                                binding
                                    .moved_places
                                    .insert(place.projections, Availability::Moved);
                            }
                        }
                    } else {
                        frames.push(Frame::Enter(base, scope));
                    }
                }
                ExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    let names = scopes[scope].keys().cloned().collect::<Vec<_>>();
                    let then_scope = scopes.len();
                    scopes.push(scopes[scope].clone());
                    frames.push(Frame::AfterThen {
                        else_branch,
                        parent: scope,
                        then_scope,
                        names,
                    });
                    frames.push(Frame::Enter(then_branch, then_scope));
                }
                ExprKind::UpdateRecord { .. } | ExprKind::ConstructRecord { .. } => {}
                _ => {}
            },
            Frame::AfterThen {
                else_branch,
                parent,
                then_scope,
                names,
            } => {
                debug_assert_eq!(then_scope + 1, scopes.len());
                let then_variables = scopes.pop().expect("then move scope is active");
                let else_scope = scopes.len();
                scopes.push(scopes[parent].clone());
                frames.push(Frame::AfterElse {
                    parent,
                    else_scope,
                    names,
                    then_variables,
                });
                frames.push(Frame::Enter(else_branch, else_scope));
            }
            Frame::AfterElse {
                parent,
                else_scope,
                names,
                then_variables,
            } => {
                debug_assert_eq!(else_scope + 1, scopes.len());
                let else_variables = scopes.pop().expect("else move scope is active");
                for name in names {
                    if let Some(binding) = scopes[parent].get_mut(&name) {
                        let then_state = then_variables
                            .get(&name)
                            .map_or(Availability::Available, |value| value.availability);
                        let else_state = else_variables
                            .get(&name)
                            .map_or(Availability::Available, |value| value.availability);
                        binding.availability = then_state.join(else_state);
                        if let (Some(then_binding), Some(else_binding)) =
                            (then_variables.get(&name), else_variables.get(&name))
                        {
                            binding.moved_places = join_moved_places(then_binding, else_binding);
                            binding.definitely_partial =
                                join_definitely_partial(then_binding, else_binding);
                        }
                    }
                }
            }
        }
    }
    *variables = scopes.pop().expect("root move scope is retained");
}

fn merge_moved(
    target: &mut HashMap<String, Binding>,
    source: &HashMap<String, Binding>,
    names: &[String],
) {
    for name in names {
        if let (Some(target), Some(source)) = (target.get_mut(name), source.get(name)) {
            target.availability = source.availability;
            target.moved_places.clone_from(&source.moved_places);
            target
                .definitely_partial
                .clone_from(&source.definitely_partial);
        }
    }
}

fn join_conditional(
    baseline: &mut HashMap<String, Binding>,
    conditional: &HashMap<String, Binding>,
    names: &[String],
) {
    for name in names {
        if let (Some(baseline), Some(conditional)) = (baseline.get_mut(name), conditional.get(name))
        {
            let moved_places = join_moved_places(baseline, conditional);
            let definitely_partial = join_definitely_partial(baseline, conditional);
            baseline.availability = baseline.availability.join(conditional.availability);
            baseline.moved_places = moved_places;
            baseline.definitely_partial = definitely_partial;
        }
    }
}

#[derive(Clone)]
struct SourcePlace {
    root: String,
    root_span: Span,
    projections: Vec<String>,
    ty: Type,
    mode: ParamMode,
}

fn source_place(
    expr: &Expr,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> Option<SourcePlace> {
    let mut current = expr;
    let mut projected = Vec::new();
    while let ExprKind::Project { base, field, .. } = &current.kind {
        projected.push(field.as_str());
        current = base;
    }
    let ExprKind::Var(name) = &current.kind else {
        return None;
    };
    let binding = variables.get(name)?;
    let mut place = SourcePlace {
        root: name.clone(),
        root_span: current.span,
        projections: Vec::with_capacity(projected.len()),
        ty: binding.ty.clone(),
        mode: binding.mode,
    };
    for field in projected.into_iter().rev() {
        let declared = types
            .record_fields(&place.ty)?
            .iter()
            .find(|candidate| candidate.name == field)?;
        place.ty = types.record_field_type(&place.ty, declared)?;
        if !types.contains_resource(&place.ty) {
            place.mode = ParamMode::Value;
        }
        place.projections.push(field.to_owned());
    }
    Some(place)
}

fn check_source_place_availability(
    program: &Program,
    place: &SourcePlace,
    variables: &HashMap<String, Binding>,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(binding) = variables.get(&place.root) else {
        return;
    };
    match binding.availability {
        Availability::Moved => {
            diagnostics.push(
                error(
                    program,
                    "SPX-O101",
                    format!("use of resource `{}` after ownership was moved", place.root),
                    place.root_span,
                )
                .with_help("borrow the resource if the callee does not need ownership"),
            );
            return;
        }
        Availability::MaybeMoved => {
            diagnostics.push(
                error(
                    program,
                    "SPX-O107",
                    format!(
                        "resource `{}` may have been moved on another control-flow path",
                        place.root
                    ),
                    place.root_span,
                )
                .with_help("move the resource on every path or keep it borrowed"),
            );
            return;
        }
        Availability::Available => {}
    }
    let state = overlapping_place_state(binding, &place.projections);
    let display = format!("{}.{}", place.root, place.projections.join("."));
    match state {
        Availability::Available => {}
        Availability::Moved => diagnostics.push(
            error(
                program,
                "SPX-O109",
                format!("use of partially moved place `{display}`"),
                span,
            )
            .with_help("use an available sibling field or avoid moving this place earlier"),
        ),
        Availability::MaybeMoved => diagnostics.push(
            error(
                program,
                "SPX-O110",
                format!("place `{display}` may have been moved on another control-flow path"),
                span,
            )
            .with_help("move the field on every path or keep it borrowed"),
        ),
    }
}

fn overlapping_place_state(binding: &Binding, requested: &[String]) -> Availability {
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

fn path_is_prefix<T: PartialEq>(prefix: &[T], path: &[T]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

fn join_moved_places(left: &Binding, right: &Binding) -> HashMap<Vec<String>, Availability> {
    left.moved_places
        .keys()
        .chain(right.moved_places.keys())
        .cloned()
        .collect::<HashSet<_>>()
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

fn join_definitely_partial(left: &Binding, right: &Binding) -> HashSet<Vec<String>> {
    let mut candidates = HashSet::new();
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
            overlapping_place_state(left, path) == Availability::Moved
                && overlapping_place_state(right, path) == Availability::Moved
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn require_bool(
    program: &Program,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    contract.visit_calls(&mut |callee, span| {
        if let Some(target) = functions.get(callee) {
            if !target.effects.is_empty() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-C102",
                        format!(
                            "{kind} on `{}` calls effectful function `{callee}` with effects {{{}}}",
                            function.name,
                            target.effects.join(", ")
                        ),
                        span,
                    )
                    .with_help("contracts must be deterministic and effect-free"),
                );
            }
        }
    });
    let mut contract_variables = variables.clone();
    if let Some(value) = check_expr_iterative(
        program,
        function,
        contract,
        &mut contract_variables,
        functions,
        types,
        result_type,
        false,
        diagnostics,
    ) {
        if value.native_unit {
            reject_native_unit_value(program, contract, &value, diagnostics);
        } else if value.ty != Type::Bool {
            diagnostics.push(error(
                program,
                "SPX-C101",
                format!("{kind} on `{}` must be bool", function.name),
                contract.span,
            ));
        }
    }
}

fn error(
    program: &Program,
    code: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(code, message, span).at_path(&program.path)
}

fn invalid_stable_id(
    program: &Program,
    code: &'static str,
    subject: impl Into<String>,
    span: Span,
) -> Diagnostic {
    error(
        program,
        code,
        format!(
            "{} has an invalid stable id; persistent identities forbid NUL",
            subject.into()
        ),
        span,
    )
}

fn source_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let plain = matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    plain
        && !matches!(
            value,
            "module"
                | "permit"
                | "resource"
                | "fn"
                | "own"
                | "borrow"
                | "shared"
                | "uses"
                | "requires"
                | "ensures"
                | "let"
                | "if"
                | "else"
                | "true"
                | "false"
                | "result"
        )
}

#[cfg(test)]
mod iterative_verifier_tests {
    use super::*;
    use std::path::Path;

    #[allow(clippy::type_complexity)]
    fn diagnostics_key(
        diagnostics: &[Diagnostic],
    ) -> Vec<(
        &'static str,
        crate::diagnostic::Severity,
        &str,
        Option<&str>,
        Option<Span>,
        Option<&str>,
    )> {
        diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.code,
                    diagnostic.severity,
                    diagnostic.message.as_str(),
                    diagnostic.path.as_deref(),
                    diagnostic.span,
                    diagnostic.help.as_deref(),
                )
            })
            .collect()
    }

    fn compare_scalar_body(source: &str) {
        let program = crate::parse(source, Path::new("iterative-verifier.spx")).unwrap();
        let current = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        let expression = match &current.body.kind {
            ExprKind::Block { statements, tail } if statements.is_empty() => tail.as_ref(),
            _ => &current.body,
        };
        let functions = program
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function))
            .collect::<HashMap<_, _>>();
        let types = TypeTable::new(&program);
        let mut oracle_scope = HashMap::new();
        for parameter in &current.params {
            oracle_scope.insert(
                parameter.name.clone(),
                Binding {
                    ty: parameter.ty.clone(),
                    mode: parameter.mode,
                    availability: Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                },
            );
        }
        let iterative_scope = oracle_scope.clone();
        let mut oracle_diagnostics = Vec::new();
        let oracle = check_expr(
            &program,
            current,
            expression,
            &mut oracle_scope,
            &functions,
            &types,
            None,
            true,
            &mut oracle_diagnostics,
        );
        let mut iterative_diagnostics = Vec::new();
        let mut iterative = IterativeVerifier::new(
            &program,
            current,
            iterative_scope,
            &functions,
            &types,
            None,
            true,
            &mut iterative_diagnostics,
        );
        let actual = iterative.run(expression).unwrap();
        assert_eq!(
            oracle
                .as_ref()
                .map(|value| (&value.ty, value.mode, value.native_unit)),
            actual
                .as_ref()
                .map(|value| (&value.ty, value.mode, value.native_unit))
        );
        assert_eq!(oracle_scope.len(), iterative.scopes[0].bindings.len());
        for (name, expected) in oracle_scope {
            let actual = &iterative.scopes[0].bindings[&name];
            assert_eq!(expected.ty, actual.ty);
            assert_eq!(expected.mode, actual.mode);
            assert_eq!(expected.availability, actual.availability);
            assert_eq!(expected.moved_places, actual.moved_places);
            assert_eq!(expected.definitely_partial, actual.definitely_partial);
            assert_eq!(expected.native_unit_discard, actual.native_unit_discard);
        }
        drop(iterative);
        assert_eq!(
            diagnostics_key(&oracle_diagnostics),
            diagnostics_key(&iterative_diagnostics)
        );
    }

    #[test]
    fn scalar_frame_machine_matches_recursive_oracle() {
        compare_scalar_body("module t; fn main() -> i64 { -(1 + true) }");
        compare_scalar_body("module t; fn main() -> bool { missing_left == missing_right }");
        compare_scalar_body("module t; fn main(flag: bool) -> bool { flag && missing }");
        compare_scalar_body("module t; fn main(flag: bool) -> i64 { if flag { 1 } else { true } }");
        compare_scalar_body(
            "module t; fn main(flag: bool) -> i64 { if missing_condition { missing_then } else { missing_else } }",
        );
        compare_scalar_body(
            "module t; fn main(flag: bool) -> i64 { let value = 1 + true; let value = missing; if flag { value } else { missing_tail } }",
        );
        compare_scalar_body(
            "module t; fn zero() -> i64 { 0 } fn main() -> i64 { zero(missing_a, missing_b) }",
        );
        compare_scalar_body(
            "module t; fn one(value: i64) -> i64 { value } fn main() -> i64 { one(true) + one(missing) }",
        );
        compare_scalar_body(
            "module t; fn identity<T>(value: T) -> T { value } fn main() -> i64 { identity<i64>(1) + identity<bool>(true) }",
        );
        compare_scalar_body(
            "module t; @id(\"t.host\") interface Host permits {  } { @id(\"t.host.ping\") import rust fn ping(value: i64) -> unit effects {  } failure infallible; } fn main() -> i64 { let acknowledged = ping(1); let copied = acknowledged; 1 }",
        );
        compare_scalar_body(
            "module t; @id(\"t.buffer\") resource Buffer { @id(\"t.buffer.drop\") drop trivial; } fn inspect(value: borrow Buffer) -> i64 { 1 } fn consume(value: own Buffer) -> i64 { 1 } fn main(buffer: own Buffer) -> i64 { let first = consume(buffer) + missing; inspect(buffer) }",
        );
        compare_scalar_body(
            "module t; @id(\"t.buffer\") resource Buffer { @id(\"t.buffer.drop\") drop trivial; } fn inspect(value: borrow Buffer) -> i64 { 1 } fn consume(value: own Buffer) -> bool { true } fn main(buffer: own Buffer, left: bool, right: bool) -> i64 { let selected = left && (right && consume(buffer)); inspect(buffer) }",
        );
        compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, @id(\"t.pair.y\") y: i64, } fn main() -> Pair { Pair { missing: missing_rhs, x: true, x: missing_duplicate } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, @id(\"t.pair.y\") y: i64, } fn main() -> Pair { let pair = Pair { x: 1, y: 2 }; pair with { missing: missing_rhs, x: true, x: missing_duplicate } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.choice\") variant Choice { @id(\"t.choice.none\") None, @id(\"t.choice.value\") Value { @id(\"t.choice.value.v\") value: i64, }, } fn main(choice: Choice) -> i64 { match choice { Choice::Value { value: item } => item, Choice::None {} => 0, } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.choice\") variant Choice { @id(\"t.choice.none\") None, @id(\"t.choice.value\") Value { @id(\"t.choice.value.v\") value: i64, }, } fn main(choice: Choice) -> i64 { match choice { Choice::Value { missing: binding } => missing_arm, _ => true, } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.choice\") variant Choice { @id(\"t.choice.value\") Value { @id(\"t.choice.value.v\") value: i64, }, } fn main() -> Choice { Choice::Value { missing: missing_rhs, value: true, value: missing_duplicate } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, } fn main(pair: Pair) -> i64 { match pair { Pair { x } => x, _ => missing_unreachable, } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.inner\") record Inner { @id(\"t.inner.value\") value: i64, @id(\"t.inner.flag\") flag: bool, } @id(\"t.outer\") record Outer { @id(\"t.outer.inner\") inner: Inner, @id(\"t.outer.other\") other: i64, } fn main(input: Outer) -> i64 { match input { Outer { inner: Inner { value: item, missing: skipped, value: duplicate }, other: item } => missing_arm, } }",
        );
        compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, } fn main(pair: Pair) -> i64 { pair.x }",
        );
        compare_scalar_body("module t; fn main() -> i64 { missing.field }");
        compare_scalar_body("module t; fn main() -> i64 { missing? }");
    }
}
