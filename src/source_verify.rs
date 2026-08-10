use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ast::{
    BinaryOp, Expr, ExprKind, FieldDeclaration, Function, ImportDeclaration, ImportFailure,
    InterfaceDeclaration, MatchPattern, ParamMode, Program, ResourceLifecycleKind, Span, Statement,
    Type, TypeDeclaration, TypeDeclarationKind, UnaryOp, VariantCaseDeclaration,
};
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug)]
struct Binding {
    ty: Type,
    mode: ParamMode,
    availability: Availability,
    moved_places: HashMap<Vec<String>, Availability>,
    definitely_partial: HashSet<Vec<String>>,
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
}

impl CheckedValue {
    fn value(ty: Type) -> Self {
        Self {
            ty,
            mode: ParamMode::Value,
        }
    }

    fn returned(ty: Type, contains_resource: bool) -> Self {
        let mode = if contains_resource {
            ParamMode::Own
        } else {
            ParamMode::Value
        };
        Self { ty, mode }
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
        match template {
            Type::I64 => Some(Type::I64),
            Type::Bool => Some(Type::Bool),
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
                        return arguments.get(index).cloned();
                    }
                }
                Some(Type::Named {
                    name: name.clone(),
                    arguments: nested
                        .iter()
                        .map(|argument| {
                            Self::substitute_variant_type(declaration, arguments, argument)
                        })
                        .collect::<Option<Vec<_>>>()?,
                })
            }
        }
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
        let Type::Named { name, .. } = ty else {
            return false;
        };
        let Some(declaration) = self.declaration(name) else {
            return false;
        };
        match &declaration.kind {
            TypeDeclarationKind::Resource { .. } => true,
            TypeDeclarationKind::Record { fields } => {
                if !visiting.insert(name.clone()) {
                    return true;
                }
                let contains = fields
                    .iter()
                    .any(|field| self.contains_resource_inner(&field.ty, visiting));
                visiting.remove(name);
                contains
            }
            TypeDeclarationKind::Variant { cases } => {
                if !visiting.insert(name.clone()) {
                    return true;
                }
                let contains = cases.iter().any(|case| {
                    case.fields
                        .iter()
                        .any(|field| self.contains_resource_inner(&field.ty, visiting))
                });
                visiting.remove(name);
                contains
            }
        }
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
        let Type::Named { name, .. } = ty else { return };
        let Some(declaration) = self.declaration(name) else {
            return;
        };
        if !visiting.insert(name.clone()) {
            return;
        }
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
                for field in fields {
                    self.lifecycle_effects_inner(&field.ty, imports, visiting, effects);
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        self.lifecycle_effects_inner(&field.ty, imports, visiting, effects);
                    }
                }
            }
        }
        visiting.remove(name);
    }
}

pub(crate) fn verify(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
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
        if !matches!(declaration.kind, TypeDeclarationKind::Variant { .. })
            && !declaration.type_parameters.is_empty()
        {
            diagnostics.push(error(
                program,
                "SPX-T223",
                "only variant declarations may declare generic parameters in this slice",
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
                    .with_help("the v1 import @id is also its target-neutral logical import key"),
                );
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
    for interface in &program.interfaces {
        let permits = interface
            .permits
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for import in &interface.imports {
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
            let valid_shape = import.params.len() == 1
                && import.params[0].mode == ParamMode::Own
                && types.is_opaque_resource(&import.params[0].ty)
                && import.consumes == import.params[0].name;
            if !valid_shape {
                diagnostics.push(error(
                    program,
                    "SPX-I404",
                    format!(
                        "import `{}.{}` must take one owned resource parameter and consume it always",
                        interface.name, import.name
                    ),
                    import.span,
                ));
            }
            if let ImportFailure::Status { domain_id } = &import.failure {
                if domain_id.is_empty()
                    || domain_id.len() > STATUS_DOMAIN_MAX_BYTES_V1
                    || domain_id.contains('\0')
                {
                    diagnostics.push(error(
                        program,
                        "SPX-I403",
                        format!(
                            "import `{}.{}` has an invalid failure domain; status v1 requires 1..={STATUS_DOMAIN_MAX_BYTES_V1} UTF-8 bytes and forbids NUL",
                            interface.name, import.name,
                        ),
                        import.span,
                    ));
                }
            }
            let mut effects = HashSet::new();
            for effect in &import.effects {
                if !effects.insert(effect.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-I403",
                        format!(
                            "import `{}.{}` declares duplicate effect `{effect}`",
                            interface.name, import.name
                        ),
                        import.span,
                    ));
                }
                if !permits.contains(effect.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-I404",
                        format!(
                            "import `{}.{}` requires effect `{effect}` outside interface `{}` permits",
                            interface.name, import.name, interface.name
                        ),
                        import.span,
                    ));
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
                        import.params.len() == 1
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
            for field in fields {
                check_declared_type(
                    program,
                    &field.ty,
                    field.span,
                    &types,
                    &HashSet::new(),
                    &mut diagnostics,
                );
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
    }

    for function in &program.functions {
        check_declared_type(
            program,
            &function.return_type,
            function.span,
            &types,
            &HashSet::new(),
            &mut diagnostics,
        );
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
            check_declared_type(
                program,
                &param.ty,
                param.span,
                &types,
                &HashSet::new(),
                &mut diagnostics,
            );
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

        if let Some(actual) = check_expr(
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
            if actual.ty != function.return_type {
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
                    .with_help("return an owned resource or declare a future lifetime-bound view"),
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
                required_lifecycle_effects.extend(types.lifecycle_effects(&param.ty, &import_keys));
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
    }

    if let Some(main) = functions.get("main") {
        if !main.params.is_empty() || main.return_type != Type::I64 {
            diagnostics.push(error(
                program,
                "SPX-T104",
                "entry function must have signature `fn main() -> i64`",
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
    diagnostics
}

fn record_layout_is_recursive(
    name: &str,
    types: &TypeTable<'_>,
    visiting: &mut HashSet<String>,
    checked: &mut HashSet<String>,
) -> bool {
    if checked.contains(name) {
        return false;
    }
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let recursive = types
        .declaration(name)
        .and_then(|declaration| match &declaration.kind {
            TypeDeclarationKind::Record { fields } => Some(fields),
            TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => None,
        })
        .is_some_and(|fields| {
            fields.iter().any(|field| {
                let Type::Named {
                    name: field_type, ..
                } = &field.ty
                else {
                    return false;
                };
                matches!(
                    types.declaration(field_type).map(|item| &item.kind),
                    Some(TypeDeclarationKind::Record { .. })
                ) && record_layout_is_recursive(field_type, types, visiting, checked)
            })
        });
    visiting.remove(name);
    if !recursive {
        checked.insert(name.to_owned());
    }
    recursive
}

fn check_declared_type(
    program: &Program,
    ty: &Type,
    span: Span,
    types: &TypeTable<'_>,
    parameters: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Type::Named { name, arguments } = ty {
        if parameters.contains(name.as_str()) {
            if !arguments.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T220",
                    format!("type parameter `{name}` cannot take type arguments"),
                    span,
                ));
            }
            return;
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
            return;
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
            && (!matches!(declaration.kind, TypeDeclarationKind::Variant { .. })
                || arguments
                    .iter()
                    .any(|argument| !matches!(argument, Type::I64 | Type::Bool)))
        {
            diagnostics.push(error(
                program,
                "SPX-T223",
                format!(
                    "generic copy variant `{name}` accepts only direct `i64` or `bool` arguments"
                ),
                span,
            ));
        }
        for argument in arguments {
            check_declared_type(program, argument, span, types, parameters, diagnostics);
        }
    }
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
                CheckedValue {
                    ty: binding.ty.clone(),
                    mode: binding.mode,
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
        ExprKind::Call { name, args } => {
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
                let Some(param) = target.and_then(|target| target.params.get(index)) else {
                    continue;
                };
                if actual.as_ref().is_some_and(|actual| actual.ty != param.ty) {
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
            target.map(|target| {
                CheckedValue::returned(
                    target.return_type.clone(),
                    types.contains_resource(&target.return_type),
                )
            })
        }
        ExprKind::Unary { op, value } => {
            let actual = check_expr(
                program,
                current,
                value,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            let expected = match op {
                UnaryOp::Neg => Type::I64,
                UnaryOp::Not => Type::Bool,
            };
            if actual.ty != expected {
                diagnostics.push(error(
                    program,
                    "SPX-T206",
                    format!("unary operator expects {expected}, received {}", actual.ty),
                    expr.span,
                ));
            }
            Some(CheckedValue::value(expected))
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
            let (expected, output) = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    (Type::I64, Type::I64)
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    (Type::I64, Type::Bool)
                }
                BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
                BinaryOp::Eq | BinaryOp::Ne => {
                    if left_ty.is_some()
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
            if left_ty.as_ref().is_some_and(|value| value.ty != expected)
                || right_ty.as_ref().is_some_and(|value| value.ty != expected)
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
            type_name, fields, ..
        } => {
            let declaration = types.declaration(type_name);
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
                    if actual.ty != declared.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "field `{}.{}` expects {}, received {}",
                                type_name, field.name, declared.ty, actual.ty
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
                let ty = Type::Named {
                    name: type_name.clone(),
                    arguments: Vec::new(),
                };
                CheckedValue::returned(ty.clone(), types.contains_resource(&ty))
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
            let variant_instance = scrutinee_value.as_ref().and_then(|value| match &value.ty {
                Type::Named { name, arguments } if types.variant_cases(&value.ty).is_some() => {
                    Some((name.clone(), arguments.clone()))
                }
                Type::I64 | Type::Bool | Type::Named { .. } => None,
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
            let operand = check_expr(
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
            let operand = operand?;
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
            if let Some((ok, error_ty)) = ordinary_result_arguments(&operand.ty) {
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
            if let Some(some) = ordinary_option_argument(&operand.ty) {
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
                    operand.ty
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
                    if actual.ty != declared.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "field `{}.{}` expects {}, received {}",
                                base_value.ty, field.name, declared.ty, actual.ty
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
                                "contract expression cannot transfer an owned record replacement",
                                field.value.span,
                            ));
                        }
                    } else if types.contains_resource(&declared.ty)
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
            let mode = if types.contains_resource(&declared.ty) {
                base_value.mode
            } else {
                ParamMode::Value
            };
            Some(CheckedValue {
                ty: declared.ty.clone(),
                mode,
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
            if check_expr(
                program,
                current,
                condition,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )
            .is_some_and(|value| value.ty != Type::Bool)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T210",
                    "`if` condition must be bool",
                    condition.span,
                ));
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
                    if then_value.ty != else_value.ty {
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
    match &expr.kind {
        ExprKind::Var(name) => {
            if let Some(binding) = variables.get_mut(name) {
                if types.contains_resource(&binding.ty)
                    && binding.mode == ParamMode::Own
                    && binding.availability == Availability::Available
                {
                    binding.availability = Availability::Moved;
                }
            }
        }
        ExprKind::Block { tail, .. } => mark_value_sources_moved(tail, variables, types),
        ExprKind::Project { base, .. } => {
            if let Some(place) = source_place(expr, variables, types) {
                if let Some(binding) = variables.get_mut(&place.root) {
                    if binding.mode == ParamMode::Own {
                        binding
                            .moved_places
                            .insert(place.projections, Availability::Moved);
                    }
                }
            } else {
                mark_value_sources_moved(base, variables, types);
            }
        }
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            let names = variables.keys().cloned().collect::<Vec<_>>();
            let mut then_variables = variables.clone();
            let mut else_variables = variables.clone();
            mark_value_sources_moved(then_branch, &mut then_variables, types);
            mark_value_sources_moved(else_branch, &mut else_variables, types);
            for name in names {
                if let Some(binding) = variables.get_mut(&name) {
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
        ExprKind::UpdateRecord { .. } | ExprKind::ConstructRecord { .. } => {}
        _ => {}
    }
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
    match &expr.kind {
        ExprKind::Var(name) => {
            let binding = variables.get(name)?;
            Some(SourcePlace {
                root: name.clone(),
                root_span: expr.span,
                projections: Vec::new(),
                ty: binding.ty.clone(),
                mode: binding.mode,
            })
        }
        ExprKind::Project { base, field, .. } => {
            let mut place = source_place(base, variables, types)?;
            let declared = types
                .record_fields(&place.ty)?
                .iter()
                .find(|candidate| candidate.name == *field)?;
            place.ty = declared.ty.clone();
            if !types.contains_resource(&place.ty) {
                place.mode = ParamMode::Value;
            }
            place.projections.push(field.clone());
            Some(place)
        }
        _ => None,
    }
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
    if check_expr(
        program,
        function,
        contract,
        &mut contract_variables,
        functions,
        types,
        result_type,
        false,
        diagnostics,
    )
    .is_some_and(|value| value.ty != Type::Bool)
    {
        diagnostics.push(error(
            program,
            "SPX-C101",
            format!("{kind} on `{}` must be bool", function.name),
            contract.span,
        ));
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
