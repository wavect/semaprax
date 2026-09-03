//! Exact retained-instance guard for a generic declaration display rename.
//!
//! The generic template and every concrete retained instance are authenticated
//! before source mutation. After ordinary full-Project replay, only the display
//! name may differ: instance identities, exact type arguments, checked bodies,
//! contracts, ownership, cleanup and loan plans must remain byte-for-byte
//! equivalent as retained HIR values.

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedFunctionInstance, ResolvedFunctionTemplate};

use super::ProjectRevision;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

const MAX_RETAINED_INSTANCES: usize = 4096;

#[derive(Clone)]
pub(super) struct GenericRenamePlan {
    target: String,
    path: String,
    module: String,
    template: ResolvedFunctionTemplate,
    instances: Vec<ResolvedFunctionInstance>,
}

pub(super) fn plan(
    revision: &ProjectRevision,
    programs: &[Program],
    intent: &serde_json::Value,
) -> Result<Option<GenericRenamePlan>> {
    if intent.get("kind").and_then(serde_json::Value::as_str) != Some("rename_declaration") {
        return Ok(None);
    }
    let Some(target) = intent.get("target").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    let mut source = None;
    for program in programs {
        for function in &program.functions {
            if function.stable_id == target && !function.type_parameters.is_empty() {
                if source.replace((program, function)).is_some() {
                    return Err(invalid("generic rename source identity is ambiguous"));
                }
            }
        }
    }
    let Some((program, function)) = source else {
        return Ok(None);
    };
    if !function.explicit_id || function.name == "main" {
        return Err(invalid(
            "generic rename requires an explicit non-entry template identity",
        ));
    }

    let mut retained = None;
    for owner in revision.semantic.image_modules() {
        for template in owner
            .function_templates()
            .iter()
            .filter(|template| template.id.as_str() == target)
        {
            if retained.replace((owner, template)).is_some() {
                return Err(invalid("retained generic template identity is ambiguous"));
            }
        }
    }
    let Some((owner, template)) = retained else {
        return Err(invalid(
            "generic rename requires an authenticated retained template",
        ));
    };
    if owner.path() != program.path
        || owner.module() != program.module
        || template.name != function.name
        || template.type_parameters.len() != function.type_parameters.len()
    {
        return Err(invalid(
            "generic source declaration disagrees with its retained template",
        ));
    }
    let (template, instances) = normalized(owner, template, target)?;
    Ok(Some(GenericRenamePlan {
        target: target.to_owned(),
        path: owner.path().to_owned(),
        module: owner.module().to_owned(),
        template,
        instances,
    }))
}

pub(super) fn validate(revision: &ProjectRevision, plan: &GenericRenamePlan) -> Result<()> {
    let mut retained = None;
    for owner in revision.semantic.image_modules() {
        for template in owner
            .function_templates()
            .iter()
            .filter(|template| template.id.as_str() == plan.target)
        {
            if retained.replace((owner, template)).is_some() {
                return Err(stale(
                    "candidate generic template identity became ambiguous",
                ));
            }
        }
    }
    let Some((owner, template)) = retained else {
        return Err(stale("candidate lost the retained generic template"));
    };
    if owner.path() != plan.path || owner.module() != plan.module {
        return Err(stale("candidate moved the retained generic template"));
    }
    let (candidate_template, candidate_instances) = normalized(owner, template, &plan.target)?;
    if candidate_template != plan.template || candidate_instances != plan.instances {
        return Err(stale(
            "generic rename changed the checked template or concrete instance inventory",
        ));
    }
    Ok(())
}

fn normalized(
    owner: &crate::workspace_graph::WorkspaceGraphProjectionModule,
    template: &ResolvedFunctionTemplate,
    target: &str,
) -> Result<(ResolvedFunctionTemplate, Vec<ResolvedFunctionInstance>)> {
    let mut normalized_template = template.clone();
    normalized_template.name.clear();
    let mut instances = owner
        .function_instances()
        .iter()
        .filter(|instance| instance.template.as_str() == target)
        .cloned()
        .collect::<Vec<_>>();
    if instances.len() > MAX_RETAINED_INSTANCES {
        return Err(capacity(
            "generic rename concrete instance inventory exceeds its bound",
        ));
    }
    for instance in &mut instances {
        if instance.function.id.as_str() != target
            || instance.type_arguments.len() != template.type_parameters.len()
        {
            return Err(invalid(
                "retained concrete instance disagrees with its generic template",
            ));
        }
        instance.function.name.clear();
    }
    instances.sort_by(|left, right| left.id.cmp(&right.id));
    if instances.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(invalid("retained concrete instance identity is duplicated"));
    }
    Ok((normalized_template, instances))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G503", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G504", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G505", message)]
}
