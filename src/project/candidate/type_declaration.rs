//! Explicit resource-free data declarations through canonical source replay.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ast::{
    FieldDeclaration, Program, Span, Type, TypeDeclaration, TypeDeclarationKind,
    VariantCaseDeclaration,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationIndex, ResolvedType, ResolvedTypeDeclarationKind};
use crate::project::ProjectRevision;

use super::{declaration, intent::IntentSummary, parse_revision, MAX_TOTAL_SOURCE_BYTES};

pub(super) const MAX_TYPE_FIELDS: usize = 64;
pub(super) const MAX_TYPE_CASES: usize = 64;
pub(super) const MAX_TYPE_IDENTITIES: usize = 4096;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) struct TypeAddition {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) kind: &'static str,
    pub(super) path: String,
    pub(super) module: String,
    pub(super) expected_identities: Vec<Value>,
}

struct Inventory<'a> {
    ids: BTreeSet<String>,
    expected: Vec<Value>,
    path: &'a str,
    module: &'a str,
}

impl Inventory<'_> {
    fn add(&mut self, id: &str, kind: &str, owner: Option<&str>) -> Result<()> {
        declaration::stable_id(id)?;
        if self.expected.len() >= MAX_TYPE_IDENTITIES {
            return Err(capacity(
                "type declaration exceeds 4096 combined identities",
            ));
        }
        if !self.ids.insert(id.to_owned()) {
            return Err(grammar(
                "type declaration identity is already bound or repeated",
            ));
        }
        self.expected.push(json!({"id":id,"kind":kind,"identity_origin":"explicit","owner":owner,"path":self.path,"module":self.module}));
        Ok(())
    }

    fn fields(
        &mut self,
        revision: &ProjectRevision,
        program: &Program,
        value: &Value,
        owner: &str,
        kind: &str,
    ) -> Result<Vec<FieldDeclaration>> {
        let requested = array(value, MAX_TYPE_FIELDS)?;
        let mut names = BTreeSet::new();
        let mut fields = Vec::new();
        for field in requested {
            object(field, &["id", "name", "type"])?;
            let id = text(field, "id")?;
            let name = declaration::identifier(text(field, "name")?)?;
            if !names.insert(name) {
                return Err(grammar("type fields must have unique local names"));
            }
            let ty = field_type(revision, program, &field["type"])?;
            self.add(id, kind, Some(owner))?;
            fields.push(FieldDeclaration {
                stable_id: id.to_owned(),
                explicit_id: true,
                name: name.to_owned(),
                name_span: Span::default(),
                ty,
                span: Span::default(),
            });
        }
        Ok(fields)
    }
}

fn field_type(revision: &ProjectRevision, program: &Program, value: &Value) -> Result<Type> {
    if let Some(name) = value.as_str() {
        return match name {
            "i64" => Ok(Type::I64),
            "bool" => Ok(Type::Bool),
            "i32" => Ok(Type::I32),
            "u8" => Ok(Type::U8),
            "usize" => Ok(Type::Usize),
            "string" => Ok(Type::String),
            "Bytes" => Ok(Type::Bytes),
            _ => Err(grammar(
                "new type field is outside the resource-free constructor vocabulary",
            )),
        };
    }
    object(value, &["kind", "target", "type_arguments"])?;
    if text(value, "kind")? != "nominal" {
        return Err(grammar("new type field object requires nominal kind"));
    }
    // This existing owner authenticates the complete explicit declaration,
    // exact generic arity, and one current local/imported source binding.
    // A newly introduced identity cannot resolve in the retained predecessor.
    super::intent::nominal_type_plan(
        revision,
        program,
        text(value, "target")?,
        &value["type_arguments"],
    )
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(IntentSummary, TypeAddition)> {
    object(request, &["kind", "target", "declaration"])?;
    if text(request, "kind")? != "add_declaration" {
        return Err(grammar("type creation requires add_declaration"));
    }
    let target = text(request, "target")?;
    let (owner, _) = declaration::anchor(programs, target)?;
    let value = &request["declaration"];
    let kind = match text(value, "kind")? {
        "record" => {
            object(value, &["kind", "id", "name", "fields"])?;
            "record"
        }
        "variant" => {
            object(value, &["kind", "id", "name", "cases"])?;
            "variant"
        }
        _ => {
            return Err(grammar(
                "new type declaration requires record or variant kind",
            ))
        }
    };
    let id = declaration::stable_id(text(value, "id")?)?;
    let name = declaration::identifier(text(value, "name")?)?;
    let program = &programs[owner];
    if matches!(name, "main" | "Option" | "Result")
        || program.functions.iter().any(|entry| entry.name == name)
        || program.types.iter().any(|entry| entry.name == name)
        || program.interfaces.iter().any(|entry| entry.name == name)
        || program.protocols.iter().any(|entry| entry.name == name)
        || program.module_uses.iter().any(|entry| entry.alias == name)
    {
        return Err(grammar(
            "new type name conflicts with a module or compiler binding",
        ));
    }
    let mut ids = super::interface::identities(programs)?;
    for program in programs.iter() {
        for imported in &program.module_uses {
            ids.insert(imported.persistent_id.clone());
        }
    }
    ids.extend(crate::prelude::all_ids().iter().map(|id| (*id).to_owned()));
    // Retained graph identities also cover compiler/interface declarations
    // that must not become available merely because source has no local name.
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| grammar("retained type identity graph is invalid"))?;
    for fact in graph["declarations"]
        .as_array()
        .ok_or_else(|| grammar("retained type identity graph lacks declarations"))?
    {
        let existing = fact["id"]
            .as_str()
            .ok_or_else(|| grammar("retained type identity is invalid"))?;
        ids.insert(existing.to_owned());
    }
    let mut inventory = Inventory {
        ids,
        expected: Vec::new(),
        path: &program.path,
        module: &program.module,
    };
    inventory.add(id, kind, None)?;
    let declaration_kind = if kind == "record" {
        TypeDeclarationKind::Record {
            fields: inventory.fields(revision, program, &value["fields"], id, "field")?,
        }
    } else {
        let requested = array(&value["cases"], MAX_TYPE_CASES)?;
        if requested.is_empty() {
            return Err(grammar("new variant must declare at least one case"));
        }
        let mut names = BTreeSet::new();
        let mut cases = Vec::new();
        for case in requested {
            object(case, &["id", "name", "fields"])?;
            let case_id = text(case, "id")?;
            let case_name = declaration::identifier(text(case, "name")?)?;
            if !names.insert(case_name) {
                return Err(grammar("variant cases must have unique local names"));
            }
            inventory.add(case_id, "variant_case", Some(id))?;
            let fields =
                inventory.fields(revision, program, &case["fields"], case_id, "case_field")?;
            cases.push(VariantCaseDeclaration {
                stable_id: case_id.to_owned(),
                explicit_id: true,
                name: case_name.to_owned(),
                name_span: Span::default(),
                fields,
                span: Span::default(),
            });
        }
        TypeDeclarationKind::Variant { cases }
    };
    inventory
        .expected
        .sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let addition = TypeAddition {
        id: id.to_owned(),
        name: name.to_owned(),
        kind,
        path: program.path.clone(),
        module: program.module.clone(),
        expected_identities: inventory.expected,
    };
    programs[owner].types.push(TypeDeclaration {
        stable_id: id.to_owned(),
        explicit_id: true,
        name: name.to_owned(),
        name_span: Span::default(),
        type_parameters: Vec::new(),
        kind: declaration_kind,
        extends: None,
        span: Span::default(),
    });
    Ok((
        IntentSummary {
            target_id: target.to_owned(),
            kind: "add_declaration".to_owned(),
            migrated_calls: 0,
        },
        addition,
    ))
}

pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    let mut expected = parse_revision(before)?;
    let (_, addition) = apply(before, &mut expected, request)?;
    if expected.len() != after.sources().len() {
        return Err(grammar("type declaration replay changed source inventory"));
    }
    for (program, source) in expected.iter().zip(after.sources()) {
        let (canonical, overflow) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(program)
            });
        if overflow {
            return Err(capacity("type declaration replay exceeds source bounds"));
        }
        if program.path != source.path() || canonical != source.source() {
            return Err(grammar(
                "type declaration differs from exact independent source reconstruction",
            ));
        }
    }
    validate_checked_data(after, &addition)
}

fn validate_checked_data(revision: &ProjectRevision, addition: &TypeAddition) -> Result<()> {
    // Use all retained declarations as borrowed lookup inputs, but rebuild
    // only this selected nominal closure. Unused declarations need the same
    // actual TypeFacts proof as ones that happen to occur in a function body.
    let mut declarations = BTreeMap::new();
    let mut selected = None;
    for module in revision.semantic.image_modules() {
        for declaration in module.types() {
            if declarations
                .insert(declaration.id.as_str(), declaration)
                .is_some()
            {
                return Err(grammar("new data type checked identity is ambiguous"));
            }
            if declaration.id.as_str() == addition.id
                && (module.path() != addition.path
                    || module.module() != addition.module
                    || declaration.name != addition.name
                    || selected.replace(declaration).is_some())
            {
                return Err(grammar("new data type checked source binding changed"));
            }
        }
    }
    let declaration =
        selected.ok_or_else(|| grammar("new data type has no retained checked declaration"))?;
    if !declaration.type_parameters.is_empty() {
        return Err(grammar("new data type must remain monomorphic"));
    }
    let fields = match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } if addition.kind == "record" => {
            fields.iter().collect::<Vec<_>>()
        }
        ResolvedTypeDeclarationKind::Variant { cases } if addition.kind == "variant" => {
            cases.iter().flat_map(|case| case.fields.iter()).collect()
        }
        _ => return Err(grammar("new data type checked declaration kind changed")),
    };
    if fields.len() > MAX_TYPE_IDENTITIES {
        return Err(capacity(
            "new data type checked field inventory exceeds its bound",
        ));
    }
    if fields
        .iter()
        .any(|field| matches!(field.ty, ResolvedType::Str | ResolvedType::SliceU8))
    {
        return Err(grammar("new data type cannot store borrowed views"));
    }
    // The shared owner rejects classes/resources throughout the selected
    // dependency closure and computes layout, sizedness and drop facts through
    // the ordinary DeclarationIndex algorithm under its existing work/byte
    // bounds. Source and independent HIR validation additionally prohibit
    // borrowed aggregate fields, including inside referenced declarations.
    let facts = DeclarationIndex::record_evolution_type_facts(&declaration.id, &declarations)
        .map_err(|diagnostic| vec![diagnostic])?
        .ok_or_else(|| grammar("new data type lacks bounded checked resource-free TypeFacts"))?;
    if !facts.sized || facts.contains_resource {
        return Err(grammar(
            "new data type requires checked sized resource-free fields",
        ));
    }
    Ok(())
}

fn array(value: &Value, max: usize) -> Result<&[Value]> {
    let values = value
        .as_array()
        .ok_or_else(|| grammar("type declaration field must be an array"))?;
    if values.len() > max {
        return Err(capacity("type declaration list exceeds sixty-four items"));
    }
    Ok(values)
}
fn object(value: &Value, keys: &[&str]) -> Result<()> {
    let value = value
        .as_object()
        .ok_or_else(|| grammar("type declaration must be an object"))?;
    if value.len() != keys.len() || keys.iter().any(|key| !value.contains_key(*key)) {
        return Err(grammar("type declaration has missing or unknown fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| grammar("type declaration field must be text"))
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}
