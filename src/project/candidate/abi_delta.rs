//! Candidate-bound ABI-shaped facts. This is descriptive retained-HIR evidence,
//! never a compatibility decision, runtime witness, or deployment authority.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{wire, ProjectCandidate};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationIndex, OwnershipMode, ResolvedParam, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind, TypeFacts,
};
use crate::project::ProjectRevision;
use crate::workspace_graph::WorkspaceGraphProjectionModule;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ABI_DELTA_SCHEMA: &str = "semaprax.project-candidate-abi-delta.v1";
pub const PROJECT_CANDIDATE_ABI_DELTA_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-abi-delta-verification.v1";
pub const MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES: usize = 4 * 1024 * 1024;
const MAX_FACT_BYTES: usize = 32 * 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const FACT_DOMAIN: &[u8] = b"semaprax.candidate-abi-delta.facts.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-abi-delta.report.v1\0";

#[derive(Default)]
struct Budget {
    bytes: usize,
    items: usize,
    visits: usize,
}

impl Budget {
    fn visit(&mut self, depth: usize) -> Result<()> {
        self.visits = self.visits.checked_add(1).ok_or_else(capacity)?;
        if self.visits > MAX_VISITS || depth > MAX_DEPTH {
            return Err(capacity());
        }
        Ok(())
    }

    fn fact(&mut self, value: &Value) -> Result<()> {
        struct Count {
            bytes: usize,
            limit: usize,
        }
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if bytes.len() > self.limit.saturating_sub(self.bytes) {
                    return Err(io::Error::other("ABI fact limit"));
                }
                self.bytes += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut count = Count {
            bytes: 0,
            limit: MAX_FACT_BYTES.saturating_sub(self.bytes),
        };
        serde_json::to_writer(&mut count, value).map_err(|_| capacity())?;
        self.bytes = self.bytes.checked_add(count.bytes).ok_or_else(capacity)?;
        self.items = self.items.checked_add(1).ok_or_else(capacity)?;
        if self.bytes > MAX_FACT_BYTES || self.items > MAX_ITEMS {
            return Err(capacity());
        }
        Ok(())
    }
}

#[derive(Default)]
struct Inventory {
    functions: BTreeMap<String, Value>,
    nominals: BTreeMap<String, Value>,
    targets: BTreeMap<String, Value>,
}

impl ProjectCandidate {
    /// Compare exact retained ABI-shaped facts for manifest exports and their
    /// transitively reachable record/variant types. No compatibility is inferred.
    pub fn abi_delta(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let mut budget = Budget::default();
        let before = inventory(&self.base, &self.base_targets, &mut budget)?;
        let after = inventory(&self.revision, &self.targets, &mut budget)?;
        let functions = compare(&before.functions, &after.functions, &mut budget)?;
        let nominals = compare(&before.nominals, &after.nominals, &mut budget)?;
        let targets = compare(&before.targets, &after.targets, &mut budget)?;
        let facts = json!({"functions":functions,"public_nominals":nominals,"targets":targets});
        let facts_bytes = wire::render(facts.clone(), MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES)
            .map_err(|_| capacity())?;
        let value = json!({
            "schema":PROJECT_CANDIDATE_ABI_DELTA_SCHEMA,
            "candidate_digest":expected_candidate,
            "base_project_revision":self.base.project_revision(),
            "project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),
            "workspace_revision":self.revision.workspace_revision(),
            "base_project_graph_digest":self.base.semantic_graph_digest(),
            "project_graph_digest":self.revision.semantic_graph_digest(),
            "facts_digest":wire::digest(FACT_DOMAIN,facts_bytes.as_bytes()),
            "facts":facts,
            "inventory":{
                "base_functions":before.functions.len(),"candidate_functions":after.functions.len(),
                "base_public_nominals":before.nominals.len(),"candidate_public_nominals":after.nominals.len(),
                "base_targets":before.targets.len(),"candidate_targets":after.targets.len(),
            },
            "selection_basis":"exact_manifest_web_exports_and_command_with_transitive_resolved_record_variant_signature_types",
            "target_fact_basis":"already_retained_candidate_native_c11_and_structurally_validated_core_wasm_projection_facts",
            "compatibility":"not_assessed","runtime":"not_assessed","deployment":"not_assessed",
            "external_consumers":"not_assessed",
            "source_authority":false,"filesystem_authority":false,"execution_authority":false,
            "publication_authority":false,"deployment_authority":false,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES,
                "max_fact_work_bytes":MAX_FACT_BYTES,"max_items":MAX_ITEMS,
                "max_visits":MAX_VISITS,"max_depth":MAX_DEPTH},
            "nonclaims":["not_an_ABI_compatibility_assessment","not_runtime_or_external_consumer_evidence",
                "not_a_linker_loader_package_or_deployment_contract","no_source_filesystem_execution_publication_or_deployment_authority",
                "target_facts_are_retained_structural_projections_not_execution"]
        });
        wire::render(value, MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES).map_err(|_| capacity())
    }

    /// Replays the complete candidate and accepts only byte-exact recomputation.
    pub fn verify_abi_delta(&self, expected_candidate: &str, bytes: &[u8]) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES {
            return Err(capacity());
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        if replay.abi_delta(expected_candidate)?.as_bytes() != bytes {
            return Err(verification());
        }
        wire::render(json!({
            "schema":PROJECT_CANDIDATE_ABI_DELTA_VERIFICATION_SCHEMA,
            "result":"exact_recomputation","candidate_digest":expected_candidate,
            "base_project_revision":self.base.project_revision(),
            "project_revision":self.revision.project_revision(),
            "delta_digest":wire::digest(REPORT_DOMAIN,bytes),
            "compatibility":"not_assessed","runtime":"not_assessed","deployment":"not_assessed",
            "external_consumers":"not_assessed","source_authority":false,"execution_authority":false,
        }), MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES).map_err(|_| capacity())
    }
}

fn inventory(
    revision: &ProjectRevision,
    target_facts: &Value,
    budget: &mut Budget,
) -> Result<Inventory> {
    let modules = revision.semantic.image_modules();
    let declarations = modules
        .iter()
        .flat_map(|module| {
            module
                .types()
                .iter()
                .map(move |ty| (ty.id.as_str(), (module, ty)))
        })
        .collect::<BTreeMap<_, _>>();
    if declarations.len()
        != modules
            .iter()
            .map(|module| module.types().len())
            .sum::<usize>()
    {
        return Err(invalid());
    }
    let all_declarations = declarations
        .iter()
        .map(|(id, (_, ty))| (*id, *ty))
        .collect::<BTreeMap<_, _>>();
    let mut selected = revision
        .manifest()
        .web_exports()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(command) = revision.manifest().command() {
        selected.insert(command.to_owned());
    }
    let mut result = Inventory::default();
    let mut roots = Vec::new();
    for id in selected {
        let mut found = None;
        for module in modules {
            if let Some(function) = module
                .functions()
                .iter()
                .find(|function| function.id.as_str() == id)
            {
                if found.replace((module, function)).is_some() {
                    return Err(invalid());
                }
            }
        }
        let (module, function) = found.ok_or_else(invalid)?;
        let signature = signature(&function.params, &function.return_type, budget)?;
        roots.extend(function.params.iter().map(|parameter| parameter.ty.clone()));
        roots.push(function.return_type.clone());
        let row = json!({"id":function.id.as_str(),"name":function.name,
            "path":module.path(),"module":module.module(),"signature":signature,
            "effects":function.effects,"manifest_roles":manifest_roles(revision,id.as_str())});
        budget.fact(&row)?;
        result.functions.insert(id, row);
    }
    let mut seen = BTreeSet::new();
    for ty in roots {
        retain_nominals(
            &ty,
            &declarations,
            &all_declarations,
            &mut seen,
            &mut result.nominals,
            budget,
            0,
        )?;
    }
    let rows = target_facts.as_array().ok_or_else(invalid)?;
    for row in rows {
        let role = row["role"].as_str().ok_or_else(invalid)?;
        let lane = row["lane"].as_str().ok_or_else(invalid)?;
        let key = format!("{role}:{lane}");
        budget.fact(row)?;
        if result.targets.insert(key, row.clone()).is_some() {
            return Err(invalid());
        }
    }
    Ok(result)
}

fn manifest_roles(revision: &ProjectRevision, id: &str) -> Vec<&'static str> {
    let mut roles = Vec::new();
    if revision
        .manifest()
        .web_exports()
        .iter()
        .any(|item| item == id)
    {
        roles.push("web_export");
    }
    if revision.manifest().command() == Some(id) {
        roles.push("command");
    }
    roles
}

fn signature(
    params: &[ResolvedParam],
    result: &ResolvedType,
    budget: &mut Budget,
) -> Result<Value> {
    let mut parameters = Vec::with_capacity(params.len());
    for (index, param) in params.iter().enumerate() {
        parameters.push(json!({
            "index":index,"id":param.id.as_str(),"name":param.name,
            "ownership":ownership(param.ownership),"type_id":type_key(&param.ty,budget,0)?
        }));
    }
    let row = json!({"parameters":parameters,"result":{"type_id":type_key(result,budget,0)?,
        "ownership":"return_value_semantics_checked_by_type_facts_not_a_foreign_ABI_mode"}});
    budget.fact(&row)?;
    Ok(row)
}

fn retain_nominals<'a>(
    ty: &ResolvedType,
    declarations: &BTreeMap<
        &'a str,
        (
            &'a WorkspaceGraphProjectionModule,
            &'a ResolvedTypeDeclaration,
        ),
    >,
    all: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    seen: &mut BTreeSet<String>,
    output: &mut BTreeMap<String, Value>,
    budget: &mut Budget,
    depth: usize,
) -> Result<()> {
    budget.visit(depth)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    let key = type_key(ty, budget, depth)?;
    if !seen.insert(key.clone()) {
        return Ok(());
    }
    let Some((module, declaration_fact)) = declarations.get(declaration.as_str()).copied() else {
        if crate::prelude::is_compiler_owned_id(declaration.as_str()) {
            for argument in arguments {
                retain_nominals(argument, declarations, all, seen, output, budget, depth + 1)?;
            }
            return Ok(());
        }
        return Err(invalid());
    };
    let facts = DeclarationIndex::record_evolution_concrete_type_facts(ty, all)
        .map_err(|diagnostic| {
            if diagnostic.code == "SPX-G226" {
                capacity()
            } else {
                invalid()
            }
        })?
        .ok_or_else(invalid)?;
    let mut children = arguments.clone();
    let kind = match &declaration_fact.kind {
        ResolvedTypeDeclarationKind::Record { fields } => {
            let mut rows = Vec::with_capacity(fields.len());
            for field in fields {
                let concrete = substitute(
                    &field.ty,
                    declaration.as_str(),
                    arguments,
                    budget,
                    depth + 1,
                )?;
                let type_id = type_key(&concrete, budget, depth + 1)?;
                children.push(concrete.clone());
                rows.push(json!({"index":field.index,"id":field.id.as_str(),"name":field.name,"type_id":type_id}));
            }
            json!({"kind":"record","fields":rows})
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            let mut case_rows = Vec::with_capacity(cases.len());
            for case in cases {
                let mut field_rows = Vec::with_capacity(case.fields.len());
                for field in &case.fields {
                    let concrete = substitute(
                        &field.ty,
                        declaration.as_str(),
                        arguments,
                        budget,
                        depth + 1,
                    )?;
                    let type_id = type_key(&concrete, budget, depth + 1)?;
                    children.push(concrete.clone());
                    field_rows.push(json!({"index":field.index,"id":field.id.as_str(),"name":field.name,"type_id":type_id}));
                }
                case_rows.push(json!({"index":case.index,"id":case.id.as_str(),"name":case.name,"fields":field_rows}));
            }
            json!({"kind":"variant","cases":case_rows})
        }
        _ => return Err(invalid()),
    };
    let type_arguments = arguments
        .iter()
        .map(|argument| type_key(argument, budget, depth + 1))
        .collect::<Result<Vec<_>>>()?;
    let row = json!({"type_id":key,"declaration_id":declaration.as_str(),"name":declaration_fact.name,
        "path":module.path(),"module":module.module(),"type_arguments":type_arguments,
        "shape":kind,"checked_facts":facts_value(&facts)});
    budget.fact(&row)?;
    output.insert(key, row);
    for child in children {
        retain_nominals(&child, declarations, all, seen, output, budget, depth + 1)?;
    }
    Ok(())
}

fn substitute(
    ty: &ResolvedType,
    owner: &str,
    arguments: &[ResolvedType],
    budget: &mut Budget,
    depth: usize,
) -> Result<ResolvedType> {
    budget.visit(depth)?;
    Ok(match ty {
        ResolvedType::TypeParameter {
            owner: parameter_owner,
            index,
        } if parameter_owner.as_str() == owner => arguments
            .get(*index as usize)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        ResolvedType::Nominal {
            declaration,
            arguments: nested,
        } => ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: nested
                .iter()
                .map(|item| substitute(item, owner, arguments, budget, depth + 1))
                .collect::<Result<Vec<_>>>()?,
        },
        _ => ty.clone(),
    })
}

fn type_key(ty: &ResolvedType, budget: &mut Budget, depth: usize) -> Result<String> {
    preflight_type(ty, budget, depth)?;
    Ok(ty.identity_key())
}

fn preflight_type(ty: &ResolvedType, budget: &mut Budget, depth: usize) -> Result<()> {
    budget.visit(depth)?;
    if let ResolvedType::Nominal { arguments, .. } = ty {
        for argument in arguments {
            preflight_type(argument, budget, depth + 1)?;
        }
    }
    Ok(())
}

fn facts_value(facts: &TypeFacts) -> Value {
    json!({"copy":facts.copy,"needs_drop":facts.needs_drop,
    "contains_resource":facts.contains_resource,"sized":facts.sized,"layout_key":facts.layout_key})
}
fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

fn compare(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
    budget: &mut Budget,
) -> Result<Vec<Value>> {
    let ids = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::with_capacity(ids.len());
    for id in ids {
        let left = before.get(&id);
        let right = after.get(&id);
        let classification = match (left, right) {
            (None, Some(_)) => "added",
            (Some(_), None) => "removed",
            (Some(a), Some(b)) if a == b => "unchanged",
            (Some(_), Some(_)) => "changed",
            (None, None) => unreachable!(),
        };
        let row = json!({"id":id,"classification":classification,"base":left.cloned().unwrap_or(Value::Null),
            "candidate":right.cloned().unwrap_or(Value::Null)});
        budget.fact(&row)?;
        rows.push(row);
    }
    Ok(rows)
}

fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G522",
        "candidate ABI delta retained facts are inconsistent",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G523",
        "candidate ABI delta exceeds its bounded work or output",
    )]
}
fn verification() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G524",
        "candidate ABI delta failed exact independent replay",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::DeclarationId;

    #[test]
    fn concrete_generic_nominal_keys_and_owner_index_substitution_are_exact() {
        let owner = DeclarationId::new("generic.product.pair");
        let concrete = ResolvedType::Nominal {
            declaration: owner.clone(),
            arguments: vec![ResolvedType::Bytes, ResolvedType::Bool],
        };
        let swapped = ResolvedType::Nominal {
            declaration: owner.clone(),
            arguments: vec![ResolvedType::Bool, ResolvedType::Bytes],
        };
        let mut budget = Budget::default();
        let concrete_key = type_key(&concrete, &mut budget, 0).unwrap();
        let swapped_key = type_key(&swapped, &mut budget, 0).unwrap();
        assert_eq!(
            concrete_key,
            "nominal:20:generic.product.pair:2:5:bytes4:bool"
        );
        assert_eq!(
            swapped_key,
            "nominal:20:generic.product.pair:2:4:bool5:bytes"
        );
        assert_ne!(concrete_key, swapped_key);

        let left = substitute(
            &ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 0,
            },
            owner.as_str(),
            &[ResolvedType::Bytes, ResolvedType::Bool],
            &mut budget,
            0,
        )
        .unwrap();
        let right = substitute(
            &ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 1,
            },
            owner.as_str(),
            &[ResolvedType::Bytes, ResolvedType::Bool],
            &mut budget,
            0,
        )
        .unwrap();
        let foreign = substitute(
            &ResolvedType::TypeParameter {
                owner: DeclarationId::new("generic.product.foreign"),
                index: 0,
            },
            owner.as_str(),
            &[ResolvedType::Bytes, ResolvedType::Bool],
            &mut budget,
            0,
        )
        .unwrap();
        assert_eq!(left, ResolvedType::Bytes);
        assert_eq!(right, ResolvedType::Bool);
        assert!(matches!(foreign, ResolvedType::TypeParameter { .. }));
    }
}
