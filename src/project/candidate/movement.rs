//! Relocate one checked Copy declaration and reconstruct its lexical bindings.
//! No HIR is mutated or deserialized; full Project admission remains mandatory.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ast::{ExprKind, MatchMode, ModuleUse, ModuleUseKind, Program, Span, Statement};
use crate::diagnostic::Diagnostic;
use crate::project::{ProjectRevision, MAX_TOTAL_SOURCE_BYTES};

use super::{intent, parse_revision};

#[path = "movement_types.rs"]
mod types;

const MAX_DEPENDENCIES: usize = 64;
const MAX_ALIASES: usize = 65_536;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) struct DeclarationMove {
    pub(super) id: String,
    pub(super) source_path: String,
    pub(super) source_module: String,
    pub(super) destination_path: String,
    pub(super) destination_module: String,
}

struct Plan {
    source: usize,
    function: usize,
    calls: BTreeMap<String, String>,
    dependencies: BTreeMap<String, String>,
    local_names: BTreeSet<String>,
    types: types::TypeMovePlan,
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(intent::IntentSummary, DeclarationMove)> {
    request_shape(request)?;
    let target = text(request, "target")?;
    let plan = plan(revision, programs, target)?;
    let (destination, anchor) = locate(programs, text(request, "destination")?)?;
    if !programs[destination].functions[anchor].explicit_id
        || !programs[destination].functions[anchor]
            .type_parameters
            .is_empty()
        || !destination_admitted(programs, &plan, destination)
    {
        return Err(invalid(
            "movement requires a distinct admitted destination module anchor",
        ));
    }
    let metadata = DeclarationMove {
        id: target.to_owned(),
        source_path: programs[plan.source].path.clone(),
        source_module: programs[plan.source].module.clone(),
        destination_path: programs[destination].path.clone(),
        destination_module: programs[destination].module.clone(),
    };
    let original_bindings = programs
        .iter()
        .map(intent::call_bindings)
        .collect::<Result<Vec<_>>>()?;
    let mut function = programs[plan.source].functions.remove(plan.function);
    let mut remaining_source_calls = BTreeSet::new();
    let mut source_uses_target = false;
    let mut migrated_calls = 0;
    let mut nodes = 0;
    // Every surviving caller is interpreted through its original module's
    // admitted binding map. Destination aliases become a local function name;
    // other callers retain their aliases and receive the new import origin.
    for (owner, program) in programs.iter_mut().enumerate() {
        intent::walk_program(program, &mut nodes, &mut |expression| {
            if let ExprKind::Call { name, .. } = &mut expression.kind {
                if owner == plan.source {
                    remaining_source_calls.insert(name.clone());
                }
                if original_bindings[owner]
                    .get(name)
                    .is_some_and(|id| id == target)
                {
                    migrated_calls += 1;
                    source_uses_target |= owner == plan.source;
                    if owner == destination {
                        *name = function.name.clone();
                    }
                }
            }
            Ok(())
        })?;
    }
    for (owner, program) in programs.iter_mut().enumerate() {
        program.module_uses.retain(|binding| {
            !(binding.kind == ModuleUseKind::Function
                && ((owner == destination && binding.persistent_id == target)
                    || (owner == plan.source
                        && plan.calls.contains_key(&binding.alias)
                        && !remaining_source_calls.contains(&binding.alias))))
        });
        for binding in &mut program.module_uses {
            if binding.kind == ModuleUseKind::Function && binding.persistent_id == target {
                binding.target_module = metadata.destination_module.clone();
            }
        }
    }
    if source_uses_target {
        programs[plan.source].module_uses.push(ModuleUse {
            kind: ModuleUseKind::Function,
            persistent_id: target.to_owned(),
            target_module: metadata.destination_module.clone(),
            alias: function.name.clone(),
            span: Span::default(),
        });
    }
    let mut occupied = namespace(&programs[destination]);
    occupied.insert(function.name.clone());
    occupied.extend(plan.local_names.iter().cloned());
    let mut bindings = intent::call_bindings(&programs[destination])?;
    let mut names = BTreeMap::from([(target.to_owned(), function.name.clone())]);
    for (dependency, module) in &plan.dependencies {
        if dependency == target {
            continue;
        }
        if let Some((alias, _)) = bindings
            .iter()
            .find(|(alias, id)| *id == dependency && !plan.local_names.contains(*alias))
        {
            names.insert(dependency.clone(), alias.clone());
            continue;
        }
        if bindings.values().any(|id| id == dependency) {
            return Err(invalid(
                "movement dependency binding conflicts with a moved local name",
            ));
        }
        let preferred = plan
            .calls
            .iter()
            .find(|(_, id)| *id == dependency)
            .map(|(name, _)| name.as_str())
            .ok_or_else(|| invalid("movement dependency lacks its original call binding"))?;
        let alias = choose_alias(preferred, &mut occupied)?;
        programs[destination].module_uses.push(ModuleUse {
            kind: ModuleUseKind::Function,
            persistent_id: dependency.clone(),
            target_module: module.clone(),
            alias: alias.clone(),
            span: Span::default(),
        });
        bindings.insert(alias.clone(), dependency.clone());
        names.insert(dependency.clone(), alias);
    }
    intent::walk_function(&mut function, &mut nodes, &mut |expression| {
        if let ExprKind::Call { name, .. } = &mut expression.kind {
            let id = plan
                .calls
                .get(name)
                .ok_or_else(|| invalid("moved call lost its stable binding"))?;
            *name = names
                .get(id)
                .ok_or_else(|| invalid("moved call lacks a destination binding"))?
                .clone();
            migrated_calls += 1;
        }
        Ok(())
    })?;
    plan.types
        .relocate(&mut programs[destination], &mut function, &mut occupied)?;
    programs[destination].functions.push(function);
    Ok((
        intent::IntentSummary {
            target_id: target.to_owned(),
            kind: "move_declaration".to_owned(),
            migrated_calls,
        },
        metadata,
    ))
}

/// Independent reconstruction checks every canonical source, including import
/// removal/addition and caller aliases. Metadata never authorizes extra edits.
pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    let mut expected = parse_revision(before)?;
    let _ = apply(before, &mut expected, request)?;
    if before.manifest().to_canonical_toml() != after.manifest().to_canonical_toml()
        || expected.len() != after.sources().len()
    {
        return Err(invalid(
            "movement changed the fixed Project source or manifest inventory",
        ));
    }
    let mut total = 0usize;
    for (program, source) in expected.iter().zip(after.sources()) {
        let (canonical, overflow) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(program)
            });
        total = total.saturating_add(canonical.len());
        if overflow || total > MAX_TOTAL_SOURCE_BYTES {
            return Err(limit("movement source replay exceeds its bound"));
        }
        if program.path != source.path() || canonical != source.source() {
            return Err(invalid(
                "movement candidate differs from exact reconstructed sources",
            ));
        }
    }
    if call_inventory(before)? != call_inventory(after)? {
        return Err(invalid(
            "movement changed admitted caller, phase, or callee identity counts",
        ));
    }
    types::validate(before, after, text(request, "target")?)?;
    Ok(())
}

type Calls = BTreeMap<(String, &'static str, String), usize>;

fn call_inventory(revision: &ProjectRevision) -> Result<Calls> {
    let mut calls = Calls::new();
    let mut total = 0usize;
    let mut append = |id: &str,
                      requires: &[crate::hir::ResolvedExpr],
                      body: &crate::hir::ResolvedExpr,
                      ensures: &[crate::hir::ResolvedExpr]| {
        for (phase, expressions) in [
            ("requires", requires),
            ("body", std::slice::from_ref(body)),
            ("ensures", ensures),
        ] {
            for expression in expressions {
                crate::hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                    total = total.saturating_add(1);
                    if total <= 65_536 {
                        *calls
                            .entry((id.to_owned(), phase, callee.as_str().to_owned()))
                            .or_default() += 1;
                    }
                });
            }
        }
    };
    for module in revision.semantic.image_modules() {
        for function in module.functions() {
            append(
                function.id.as_str(),
                &function.requires,
                &function.body,
                &function.ensures,
            );
        }
        for function in module.function_templates() {
            append(
                function.id.as_str(),
                &function.requires,
                &function.body,
                &function.ensures,
            );
        }
    }
    if total > 65_536 {
        return Err(limit("movement admitted call inventory exceeds its bound"));
    }
    Ok(calls)
}

/// Structural discovery only: destination anchors do not assert cycle freedom
/// or full admission for the final reconstructed Project.
pub(super) fn destinations(revision: &ProjectRevision, target: &str) -> Result<Vec<String>> {
    let programs = parse_revision(revision)?;
    let Ok(plan) = plan(revision, &programs, target) else {
        return Ok(Vec::new());
    };
    let mut anchors = Vec::new();
    for (index, program) in programs.iter().enumerate() {
        if destination_admitted(&programs, &plan, index) {
            anchors.extend(
                program
                    .functions
                    .iter()
                    .filter(|f| f.explicit_id && f.type_parameters.is_empty())
                    .map(|f| f.stable_id.clone()),
            );
        }
    }
    anchors.sort();
    Ok(anchors)
}

fn plan(revision: &ProjectRevision, programs: &[Program], target: &str) -> Result<Plan> {
    let (source, index) = locate(programs, target)?;
    let function = &programs[source].functions[index];
    if !function.explicit_id
        || function.name == "main"
        || !function.type_parameters.is_empty()
        || revision
            .manifest()
            .web_exports()
            .iter()
            .any(|id| id == target)
    {
        return Err(invalid(
            "movement requires an explicit non-exported monomorphic Copy function",
        ));
    }
    let types = types::plan(revision, &programs[source], function)?;
    let bindings = intent::call_bindings(&programs[source])?;
    let mut calls = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    let mut local_names = function
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<BTreeSet<_>>();
    local_names.extend(types.local_names().iter().cloned());
    let mut inspected = function.clone();
    let mut nodes = 0;
    intent::walk_function(&mut inspected, &mut nodes, &mut |expression| {
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::Bool(_)
            | ExprKind::Var(_)
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::ConstructRecord { .. }
            | ExprKind::ConstructVariant { .. }
            | ExprKind::Project { .. }
            | ExprKind::UpdateRecord { .. }
            | ExprKind::Match {
                mode: MatchMode::Value,
                ..
            } => {}
            ExprKind::Block { statements, .. } => {
                for statement in statements {
                    match statement {
                        Statement::Let { name, .. } => {
                            local_names.insert(name.clone());
                        }
                        Statement::Assign { field: None, .. } | Statement::While { .. } => {}
                        Statement::Assign { field: Some(_), .. } | Statement::Unsafe { .. } => {
                            return Err(invalid(
                                "movement does not relocate field mutation or audited boundaries",
                            ))
                        }
                    }
                }
            }
            ExprKind::Call {
                name,
                type_arguments,
                ..
            } => {
                if !type_arguments.is_empty() {
                    return Err(invalid("movement does not relocate generic calls"));
                }
                let id = bindings.get(name).ok_or_else(|| {
                    invalid("movement requires explicit source function call bindings")
                })?;
                let (provider, function) = locate(programs, id)?;
                if !programs[provider].functions[function].explicit_id
                    || !programs[provider].functions[function]
                        .type_parameters
                        .is_empty()
                {
                    return Err(invalid(
                        "movement dependency requires an explicit monomorphic Copy signature",
                    ));
                }
                if !dependencies.contains_key(id) {
                    types::validate_signature(
                        revision,
                        &programs[provider],
                        &programs[provider].functions[function],
                    )?;
                }
                calls.insert(name.clone(), id.clone());
                dependencies.insert(id.clone(), programs[provider].module.clone());
                if dependencies.len().saturating_add(types.dependency_count()) > MAX_DEPENDENCIES {
                    return Err(limit(
                        "movement exceeds sixty-four combined callable and nominal type dependencies",
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    "movement expression requires unsupported type, ownership, or audit relocation",
                ))
            }
        }
        Ok(())
    })?;
    if dependencies.len().saturating_add(types.dependency_count()) > MAX_DEPENDENCIES {
        return Err(limit(
            "movement exceeds sixty-four combined callable and nominal type dependencies",
        ));
    }
    Ok(Plan {
        source,
        function: index,
        calls,
        dependencies,
        local_names,
        types,
    })
}

fn destination_admitted(programs: &[Program], plan: &Plan, destination: usize) -> bool {
    if plan.source == destination {
        return false;
    }
    let target = &programs[plan.source].functions[plan.function];
    let program = &programs[destination];
    if target
        .effects
        .iter()
        .any(|effect| !program.permits.contains(effect))
    {
        return false;
    }
    // An existing import of this exact function is replaced by the declaration.
    !program.functions.iter().any(|f| f.name == target.name)
        && !program.types.iter().any(|d| d.name == target.name)
        && !program.interfaces.iter().any(|d| d.name == target.name)
        && !program.protocols.iter().any(|d| d.name == target.name)
        && !program.module_uses.iter().any(|binding| {
            binding.alias == target.name
                && (binding.kind != ModuleUseKind::Function
                    || binding.persistent_id != target.stable_id)
        })
}

fn namespace(program: &Program) -> BTreeSet<String> {
    program
        .functions
        .iter()
        .map(|f| f.name.clone())
        .chain(program.types.iter().map(|d| d.name.clone()))
        .chain(program.interfaces.iter().map(|d| d.name.clone()))
        .chain(program.protocols.iter().map(|d| d.name.clone()))
        .chain(program.module_uses.iter().map(|d| d.alias.clone()))
        .collect()
}

fn choose_alias(preferred: &str, occupied: &mut BTreeSet<String>) -> Result<String> {
    if occupied.insert(preferred.to_owned()) {
        return Ok(preferred.to_owned());
    }
    for index in 0..MAX_ALIASES {
        let alias = format!("_spx_move_{index}");
        if occupied.insert(alias.clone()) {
            return Ok(alias);
        }
    }
    Err(limit(
        "movement cannot allocate a bounded destination call alias",
    ))
}

fn locate(programs: &[Program], id: &str) -> Result<(usize, usize)> {
    if id.is_empty() || id.len() > intent::MAX_ID_BYTES || id.contains('\0') {
        return Err(invalid("movement selector must be a bounded stable ID"));
    }
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        for (index, function) in program.functions.iter().enumerate() {
            if function.stable_id == id && found.replace((owner, index)).is_some() {
                return Err(invalid("movement function selector is ambiguous"));
            }
        }
    }
    found.ok_or_else(|| invalid("movement function selector is absent"))
}

fn request_shape(request: &Value) -> Result<()> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("movement intention must be an object"))?;
    if object.len() != 3
        || ["kind", "target", "destination"]
            .iter()
            .any(|key| !object.contains_key(*key))
        || text(request, "kind")? != "move_declaration"
    {
        return Err(invalid(
            "movement intention has missing, unknown, or invalid fields",
        ));
    }
    Ok(())
}

fn text<'a>(request: &'a Value, key: &str) -> Result<&'a str> {
    request
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("movement field must be text"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}
