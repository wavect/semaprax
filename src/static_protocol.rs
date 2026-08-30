//! Source-backed static conformance to local protocol method requirements.
//! Bindings name ordinary functions by persistent ID; there is no dispatch ABI.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ast::{
    Function, Program, ProtocolDeclaration, ProtocolMethod, Type, TypeDeclaration,
    TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;

pub const SCHEMA: &str = "semaprax.static-protocol-conformance.v1";
pub const MAX_IMPLEMENTATIONS: usize = 256;
pub const MAX_IMPLEMENTATION_MEMBERS: usize = 256;
pub const MAX_TOTAL_MEMBERS: usize = 4096;
pub const MAX_STABLE_ID_BYTES: usize = 240;
pub const MAX_FACT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_METHOD_PARAMETERS: usize = 64;

/// Closed selector grammar for new conformance declarations. Declaration
/// display names are never binding authority.
pub fn valid_binding_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_STABLE_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

/// Signature eligibility only. Local declaration identity and complete member
/// inventory must additionally pass `validate`; function bodies still require
/// the ordinary source/HIR verifier.
pub fn member_matches(
    protocol: &ProtocolDeclaration,
    method: &ProtocolMethod,
    receiver: &TypeDeclaration,
    function: &Function,
) -> bool {
    if !receiver.explicit_id
        || !receiver.type_parameters.is_empty()
        || !matches!(receiver.kind, TypeDeclarationKind::Record { .. })
        || !function.explicit_id
        || function.name == "main"
        || !function.type_parameters.is_empty()
        || !function.effects.is_empty()
        || !function.requires.is_empty()
        || method.params.is_empty()
        || method.params.len() != function.params.len()
        || method.return_type != function.return_type
    {
        return false;
    }
    let first = &method.params[0].ty;
    if !matches!(first, Type::Named { name, arguments } if arguments.is_empty() && (name == "Self" || name == &protocol.name))
    {
        return false;
    }
    method.params.iter().zip(&function.params).enumerate().all(|(index, (required, actual))| {
        required.mode == actual.mode && if index == 0 {
            matches!(&actual.ty, Type::Named { name, arguments } if arguments.is_empty() && name == &receiver.name)
        } else { required.ty == actual.ty }
    })
}

/// Validate source-owned static facts before any backend receives resolved HIR.
/// Safe to call on original workspace modules: cross-file imports do not widen
/// this strictly local conformance vocabulary.
pub fn validate(program: &Program) -> Result<(), Diagnostic> {
    if program.protocols.is_empty() && program.implementations.is_empty() {
        return Ok(());
    }
    if program.protocols.len() > MAX_IMPLEMENTATIONS
        || program.implementations.len() > MAX_IMPLEMENTATIONS
    {
        return Err(error(
            "SPX-Q109",
            "static protocol declaration inventory exceeds its bound",
        ));
    }
    let mut methods = 0usize;
    let mut names = BTreeSet::new();
    for protocol in &program.protocols {
        methods = methods.saturating_add(protocol.methods.len());
        if protocol.methods.len() > MAX_IMPLEMENTATION_MEMBERS || methods > MAX_TOTAL_MEMBERS {
            return Err(error(
                "SPX-Q109",
                "static protocol method inventory exceeds its bound",
            ));
        }
        if protocol
            .methods
            .iter()
            .any(|method| method.params.len() > MAX_METHOD_PARAMETERS)
        {
            return Err(error(
                "SPX-Q109",
                "static protocol method parameter inventory exceeds its bound",
            ));
        }
        let mut member_names = BTreeSet::new();
        if protocol.methods.is_empty()
            || !names.insert(protocol.name.as_str())
            || protocol
                .methods
                .iter()
                .any(|method| !member_names.insert(method.name.as_str()))
        {
            return Err(error(
                "SPX-Q106",
                "static protocol names and nonempty method inventories must be unique",
            ));
        }
    }
    crate::protocol_check::validate_program(program)?;
    let mut ids = BTreeSet::new();
    for id in crate::prelude::all_ids() {
        ids.insert(id.to_owned());
    }
    let mut insert = |id: &str| -> Result<(), Diagnostic> {
        if ids.insert(id.to_owned()) {
            Ok(())
        } else {
            Err(error(
                "SPX-Q108",
                "static protocol declaration identity collides with another declaration",
            ))
        }
    };
    for function in &program.functions {
        insert(&function.stable_id)?;
    }
    for declaration in &program.types {
        insert(&declaration.stable_id)?;
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                for field in fields {
                    insert(&field.stable_id)?;
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                for field in fields {
                    insert(&field.stable_id)?;
                }
                for method in methods {
                    insert(&method.stable_id)?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    insert(&case.stable_id)?;
                    for field in &case.fields {
                        insert(&field.stable_id)?;
                    }
                }
            }
            TypeDeclarationKind::Resource { lifecycles } => {
                for lifecycle in lifecycles {
                    if let Some(id) = &lifecycle.stable_id {
                        insert(id)?;
                    }
                }
            }
        }
    }
    for interface in &program.interfaces {
        insert(&interface.stable_id)?;
        for import in &interface.imports {
            insert(&import.stable_id)?;
        }
    }
    for protocol in &program.protocols {
        insert(&protocol.stable_id)?;
        for method in &protocol.methods {
            insert(&method.stable_id)?;
        }
    }
    let protocols = program
        .protocols
        .iter()
        .map(|item| (item.stable_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let receivers = program
        .types
        .iter()
        .map(|item| (item.stable_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let functions = program
        .functions
        .iter()
        .map(|item| (item.stable_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = BTreeSet::new();
    let mut total = 0usize;
    for implementation in &program.implementations {
        if !implementation.explicit_id
            || !valid_binding_id(&implementation.stable_id)
            || implementation.stable_id.starts_with("auto:")
            || implementation.stable_id.starts_with("semaprax.")
            || !valid_binding_id(&implementation.protocol_id)
            || !valid_binding_id(&implementation.receiver_id)
        {
            return Err(error(
                "SPX-Q106",
                "static impl requires bounded explicit declaration and target identities",
            ));
        }
        insert(&implementation.stable_id)?;
        total = total.saturating_add(implementation.members.len());
        if implementation.members.len() > MAX_IMPLEMENTATION_MEMBERS || total > MAX_TOTAL_MEMBERS {
            return Err(error(
                "SPX-Q109",
                "static implementation member inventory exceeds its bound",
            ));
        }
        if !pairs.insert((
            implementation.protocol_id.as_str(),
            implementation.receiver_id.as_str(),
        )) {
            return Err(error(
                "SPX-Q108",
                "a protocol and receiver may have only one local static implementation",
            ));
        }
        let protocol = protocols
            .get(implementation.protocol_id.as_str())
            .filter(|item| item.explicit_id)
            .ok_or_else(|| {
                error(
                    "SPX-Q106",
                    "static impl protocol must be a local explicit declaration",
                )
            })?;
        let receiver = receivers
            .get(implementation.receiver_id.as_str())
            .filter(|item| {
                item.explicit_id
                    && item.type_parameters.is_empty()
                    && matches!(item.kind, TypeDeclarationKind::Record { .. })
            })
            .ok_or_else(|| {
                error(
                    "SPX-Q106",
                    "static impl receiver must be a local explicit monomorphic record",
                )
            })?;
        if implementation.members.len() != protocol.methods.len() {
            return Err(error(
                "SPX-Q107",
                "static impl must bind every required method exactly once",
            ));
        }
        let required = protocol
            .methods
            .iter()
            .map(|item| (item.stable_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut bound_methods = BTreeSet::new();
        let mut bound_functions = BTreeSet::new();
        for member in &implementation.members {
            if !valid_binding_id(&member.method_id) || !valid_binding_id(&member.function_id) {
                return Err(error(
                    "SPX-Q106",
                    "static impl member selectors must be bounded stable identities",
                ));
            }
            if !bound_methods.insert(member.method_id.as_str())
                || !bound_functions.insert(member.function_id.as_str())
            {
                return Err(error("SPX-Q107", "static impl bindings must be one-to-one"));
            }
            let method = required
                .get(member.method_id.as_str())
                .filter(|item| item.explicit_id)
                .ok_or_else(|| {
                    error(
                        "SPX-Q107",
                        "static impl member is not an explicit requirement of its protocol",
                    )
                })?;
            let function = functions.get(member.function_id.as_str()).ok_or_else(|| {
                error(
                    "SPX-Q107",
                    "static impl member must name an ordinary local function",
                )
            })?;
            if !member_matches(protocol, method, receiver, function) {
                return Err(error("SPX-Q107", "static impl function signature, modes, effects, or preconditions do not satisfy its requirement"));
            }
        }
    }
    Ok(())
}

/// Fully source-admitted single-module facts. This does not run any target.
pub fn facts(program: &Program) -> Result<Value, Vec<Diagnostic>> {
    let mut facts = declaration_facts(program)?;
    crate::hir::resolve(program)?;
    facts["full_source_admitted"] = json!(true);
    Ok(facts)
}

/// Hidden protocol/implementation IDs share the workspace-wide declaration
/// namespace even though they do not enter the runtime declaration graph.
pub(crate) fn validate_workspace(programs: &[Program]) -> Result<(), Diagnostic> {
    if programs
        .iter()
        .all(|program| program.protocols.is_empty() && program.implementations.is_empty())
    {
        return Ok(());
    }
    if programs.len() > 16 {
        return Err(error(
            "SPX-Q109",
            "static protocol workspace exceeds its module bound",
        ));
    }
    let mut ids = crate::prelude::all_ids()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut count = ids.len();
    for program in programs {
        validate(program)?;
        visit_ids(program, |id| {
            count += 1;
            if count > 65_536 {
                return Err(error(
                    "SPX-Q109",
                    "static protocol workspace identity inventory exceeds its bound",
                ));
            }
            if !ids.insert(id) {
                return Err(error(
                    "SPX-Q108",
                    "static protocol workspace declaration identity collides across source modules",
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn visit_ids<'a>(
    program: &'a Program,
    mut visit: impl FnMut(&'a str) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    for function in &program.functions {
        visit(&function.stable_id)?;
    }
    for declaration in &program.types {
        visit(&declaration.stable_id)?;
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                for field in fields {
                    visit(&field.stable_id)?;
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                for field in fields {
                    visit(&field.stable_id)?;
                }
                for method in methods {
                    visit(&method.stable_id)?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    visit(&case.stable_id)?;
                    for field in &case.fields {
                        visit(&field.stable_id)?;
                    }
                }
            }
            TypeDeclarationKind::Resource { lifecycles } => {
                for lifecycle in lifecycles {
                    if let Some(id) = &lifecycle.stable_id {
                        visit(id)?;
                    }
                }
            }
        }
    }
    for interface in &program.interfaces {
        visit(&interface.stable_id)?;
        for import in &interface.imports {
            visit(&import.stable_id)?;
        }
    }
    for protocol in &program.protocols {
        visit(&protocol.stable_id)?;
        for method in &protocol.methods {
            visit(&method.stable_id)?;
        }
    }
    for implementation in &program.implementations {
        visit(&implementation.stable_id)?;
    }
    Ok(())
}

/// Original-source table for a caller already holding the independently
/// admitted workspace. The result deliberately does not claim body admission.
pub(crate) fn declaration_facts(program: &Program) -> Result<Value, Vec<Diagnostic>> {
    validate(program).map_err(|error| vec![error])?;
    // Charge conservative escaping and entry overhead before cloning report
    // strings/arrays. This is a construction charge, not an allocator bound.
    let mut budget = 4096usize;
    let mut charge = |text: &str, overhead: usize| -> Result<(), Vec<Diagnostic>> {
        if text.len() > 4096 {
            return Err(vec![error(
                "SPX-Q109",
                "static conformance report string exceeds its bound",
            )]);
        }
        budget = budget
            .saturating_add(text.len().saturating_mul(6))
            .saturating_add(overhead);
        if budget > MAX_FACT_BYTES {
            return Err(vec![error(
                "SPX-Q109",
                "static conformance report exceeds its construction bound",
            )]);
        }
        Ok(())
    };
    charge(&program.path, 256)?;
    charge(&program.module, 256)?;
    for protocol in &program.protocols {
        charge(&protocol.stable_id, 512)?;
        charge(&protocol.name, 128)?;
        for method in &protocol.methods {
            if method.params.len() > MAX_METHOD_PARAMETERS {
                return Err(vec![error(
                    "SPX-Q109",
                    "static protocol method parameter inventory exceeds its bound",
                )]);
            }
            charge(&method.stable_id, 512)?;
            charge(&method.name, 128)?;
            charge(&bounded_type_label(&method.return_type)?, 128)?;
            for param in &method.params {
                charge(&param.name, 256)?;
                charge(&bounded_type_label(&param.ty)?, 128)?;
            }
        }
    }
    for implementation in &program.implementations {
        charge(&implementation.stable_id, 512)?;
        charge(&implementation.protocol_id, 128)?;
        charge(&implementation.receiver_id, 128)?;
        for member in &implementation.members {
            charge(&member.method_id, 256)?;
            charge(&member.function_id, 128)?;
        }
    }
    let mut protocols = program.protocols.iter().collect::<Vec<_>>();
    protocols.sort_by(|a, b| a.stable_id.cmp(&b.stable_id));
    let protocols = protocols.into_iter().map(|protocol| {
        let mut methods = protocol.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|a,b| a.stable_id.cmp(&b.stable_id));
        json!({"id":protocol.stable_id,"name":protocol.name,
            "identity_origin":if protocol.explicit_id {"explicit"} else {"derived"},
            "methods":methods.into_iter().map(|method| json!({"id":method.stable_id,"name":method.name,
                "identity_origin":if method.explicit_id {"explicit"} else {"derived"},
                "params":method.params.iter().map(|param| json!({"name":param.name,"type":param.ty.to_string(),"ownership":param.mode.text()})).collect::<Vec<_>>(),
                "return_type":method.return_type.to_string()})).collect::<Vec<_>>()})
    }).collect::<Vec<_>>();
    let mut implementations = program.implementations.iter().collect::<Vec<_>>();
    implementations.sort_by(|a, b| a.stable_id.cmp(&b.stable_id));
    let entries = implementations.into_iter().map(|implementation| {
        let mut members = implementation.members.iter().collect::<Vec<_>>();
        members.sort_by(|a,b| a.method_id.cmp(&b.method_id));
        json!({"id":implementation.stable_id,"identity_origin":"explicit",
            "protocol_id":implementation.protocol_id,"receiver_id":implementation.receiver_id,
            "members":members.into_iter().map(|member| json!({"method_id":member.method_id,"function_id":member.function_id})).collect::<Vec<_>>(),
            "evidence":"source_checked_static_signature_conformance"})
    }).collect::<Vec<_>>();
    Ok(
        json!({"schema":SCHEMA,"path":program.path,"module":program.module,
        "protocols":protocols,"type_representation":"source_display_not_resolved_HIR_identity",
        "implementations":entries,"full_source_admitted":false,"source_authority":false,
        "nonclaims":["no_dynamic_dispatch","no_runtime_witness_table","no_target_execution","no_source_publication_authority"]}),
    )
}

fn error(code: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::io(code, message)
}

fn bounded_type_label(ty: &Type) -> Result<String, Vec<Diagnostic>> {
    if matches!(ty, Type::Named { name, arguments } if name.len() > 4096 || !arguments.is_empty()) {
        return Err(vec![error(
            "SPX-Q109",
            "static protocol type label exceeds its bound",
        )]);
    }
    Ok(ty.to_string())
}
