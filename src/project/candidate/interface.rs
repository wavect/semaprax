//! Source-backed static protocol mappings. No runtime witness or dispatch data.
use super::{intent::IntentSummary, parse_revision, wire, ProjectCandidate};
use crate::ast::{
    Program, ProtocolImplementation, ProtocolImplementationMember, Span, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_MEMBERS: usize = 64;
const MAX_ITEMS: usize = 65_536;
const MAX_REPORT_BYTES: usize = 1024 * 1024;

pub(super) struct ImplementationAddition {
    pub(super) id: String,
    pub(super) owner: String,
    pub(super) path: String,
    pub(super) module: String,
    pub(super) fact: Value,
}

pub(super) fn apply(
    _revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(IntentSummary, ImplementationAddition)> {
    exact(request, &["kind", "target", "protocol", "id", "members"])?;
    let target = selector(request, "target")?;
    let protocol_id = selector(request, "protocol")?;
    let id = selector(request, "id")?;
    if id.starts_with("auto:") || id.starts_with("semaprax.") {
        return Err(invalid("implementation identity uses a reserved namespace"));
    }
    let owner = receiver_owner(programs, target)?.ok_or_else(|| {
        invalid("implementation receiver must be one explicit local monomorphic record")
    })?;
    let identities = identities(programs)?;
    if identities.contains(id)
        || programs.iter().any(|program| {
            program
                .module_uses
                .iter()
                .any(|binding| binding.persistent_id == id)
        })
    {
        return Err(invalid(
            "implementation identity is already bound in this Project",
        ));
    }
    let program = &programs[owner];
    let receiver = program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == target)
        .unwrap();
    let protocol = program
        .protocols
        .iter()
        .find(|protocol| protocol.stable_id == protocol_id && protocol.explicit_id)
        .ok_or_else(|| {
            invalid(
                "implementation protocol must be an explicit declaration in the receiver module",
            )
        })?;
    if program.implementations.iter().any(|implementation| {
        implementation.receiver_id == target && implementation.protocol_id == protocol_id
    }) {
        return Err(invalid(
            "receiver already has an implementation of this protocol",
        ));
    }
    let members = request["members"]
        .as_array()
        .ok_or_else(|| invalid("implementation members must be an array"))?;
    if members.len() > MAX_MEMBERS || protocol.methods.len() > MAX_MEMBERS {
        return Err(capacity("implementation member table exceeds 64 entries"));
    }
    if members.len() != protocol.methods.len() || members.is_empty() {
        return Err(invalid(
            "implementation must cover every required protocol member exactly once",
        ));
    }
    let mut selected = BTreeMap::new();
    let mut selected_functions = BTreeSet::new();
    for member in members {
        exact(member, &["method", "implementation"])?;
        let method_id = selector(member, "method")?;
        let function_id = selector(member, "implementation")?;
        if !selected_functions.insert(function_id) {
            return Err(invalid(
                "implementation functions must be selected at most once",
            ));
        }
        let method = protocol
            .methods
            .iter()
            .find(|method| method.stable_id == method_id && method.explicit_id)
            .ok_or_else(|| {
                invalid("implementation selector is not an explicit required protocol member")
            })?;
        let function = program
            .functions
            .iter()
            .find(|function| function.stable_id == function_id)
            .ok_or_else(|| invalid("implementation function must belong to the receiver module"))?;
        if !crate::static_protocol::member_matches(protocol, method, receiver, function) {
            return Err(invalid(
                "implementation function does not match the compiler-required member signature",
            ));
        }
        if selected
            .insert(method_id.to_owned(), function_id.to_owned())
            .is_some()
        {
            return Err(invalid("implementation repeats a required protocol member"));
        }
    }
    let implementation = ProtocolImplementation {
        stable_id: id.to_owned(),
        explicit_id: true,
        protocol_id: protocol_id.to_owned(),
        receiver_id: target.to_owned(),
        members: selected
            .into_iter()
            .map(|(method_id, function_id)| ProtocolImplementationMember {
                method_id,
                function_id,
                span: Span::default(),
            })
            .collect(),
        span: Span::default(),
    };
    let fact = binding_fact(program, &implementation);
    let addition = ImplementationAddition {
        id: id.to_owned(),
        owner: target.to_owned(),
        path: program.path.clone(),
        module: program.module.clone(),
        fact,
    };
    programs[owner].implementations.push(implementation);
    crate::static_protocol::validate(&programs[owner]).map_err(|error| vec![error])?;
    Ok((
        IntentSummary {
            target_id: target.to_owned(),
            kind: "implement_interface".into(),
            migrated_calls: 0,
        },
        addition,
    ))
}

impl ProjectCandidate {
    /// Discover complete local protocol requirements and compiler-matched
    /// existing functions. Discovery confers neither conformance nor authority.
    pub fn interface_catalog(&self, expected_candidate: &str, target: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let protocols = discover(self.revision(), target)?;
        wire::render(json!({"schema":"semaprax.project-interface-change-catalog.v1",
            "candidate_digest":self.candidate_digest(),"project_revision":self.revision().project_revision(),
            "target":target,"protocols":protocols,"admission":"compiler_signature_discovery_only",
            "requires_full_candidate_validation":true,"source_authority":false,
            "limits":{"max_members":MAX_MEMBERS,"max_items":MAX_ITEMS,"max_report_bytes":MAX_REPORT_BYTES},
            "nonclaims":["no_dynamic_dispatch","no_runtime_witness","no_generated_implementation","no_external_module_conformance"]}), MAX_REPORT_BYTES)
            .map_err(|_| capacity("interface discovery exceeds its report byte bound"))
    }
}

pub(super) fn discover(revision: &ProjectRevision, target: &str) -> Result<Vec<Value>> {
    if target.is_empty() || target.len() > 4096 {
        return Err(invalid(
            "interface discovery target must be a bounded stable identity",
        ));
    }
    if !crate::static_protocol::valid_binding_id(target) {
        return Ok(vec![]);
    }
    let programs = parse_revision(revision)?;
    let Some(owner) = receiver_owner(&programs, target)? else {
        return Ok(vec![]);
    };
    let program = &programs[owner];
    let receiver = program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == target)
        .unwrap();
    let mut protocols = program
        .protocols
        .iter()
        .filter(|protocol| {
            protocol.explicit_id && crate::static_protocol::valid_binding_id(&protocol.stable_id)
        })
        .collect::<Vec<_>>();
    protocols.sort_by(|left, right| left.stable_id.cmp(&right.stable_id));
    let mut items = 0usize;
    let mut output = Vec::new();
    for protocol in protocols {
        if protocol.methods.len() > MAX_MEMBERS
            || protocol.methods.iter().any(|method| {
                !method.explicit_id || !crate::static_protocol::valid_binding_id(&method.stable_id)
            })
        {
            continue;
        }
        let mut methods = protocol.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| left.stable_id.cmp(&right.stable_id));
        let mut members = Vec::new();
        for method in methods {
            let mut functions = program
                .functions
                .iter()
                .filter(|function| {
                    crate::static_protocol::valid_binding_id(&function.stable_id)
                        && crate::static_protocol::member_matches(
                            protocol, method, receiver, function,
                        )
                })
                .map(|function| function.stable_id.clone())
                .collect::<Vec<_>>();
            functions.sort();
            items = items.saturating_add(1 + functions.len());
            if items > MAX_ITEMS {
                return Err(capacity("interface discovery inventory exceeds its bound"));
            }
            members.push(json!({"method":method.stable_id,"name":method.name,
                "parameters":method.params.iter().map(|parameter| json!({"name":parameter.name,"type":parameter.ty.to_string(),"mode":parameter.mode.text()})).collect::<Vec<_>>(),
                "return_type":method.return_type.to_string(),"eligible_implementations":functions}));
        }
        let existing = program.implementations.iter().find(|implementation| {
            implementation.receiver_id == target && implementation.protocol_id == protocol.stable_id
        });
        let complete = complete_mapping(&members);
        output.push(json!({"protocol":protocol.stable_id,"name":protocol.name,"members":members,
            "existing_implementation":existing.map(|implementation| implementation.stable_id.as_str()),
            "complete_mapping_available":complete && existing.is_none()}));
    }
    Ok(output)
}

/// Exact source facts that determine whether an admitted implementation
/// intention can be replayed on another base. Executable bodies and permitted
/// postconditions are deliberately outside static signature conformance.
pub(super) fn rebase_fingerprint(
    revision: &ProjectRevision,
    request: &Value,
) -> Result<Option<Value>> {
    exact(request, &["kind", "target", "protocol", "id", "members"])?;
    if request["kind"] != "implement_interface" {
        return Err(invalid(
            "interface rebase fingerprint requires an implementation intention",
        ));
    }
    let target = selector(request, "target")?;
    let protocol_id = selector(request, "protocol")?;
    let implementation_id = selector(request, "id")?;
    if implementation_id.starts_with("auto:") || implementation_id.starts_with("semaprax.") {
        return Err(invalid("implementation identity uses a reserved namespace"));
    }
    let requested = request["members"]
        .as_array()
        .ok_or_else(|| invalid("implementation members must be an array"))?;
    if requested.is_empty() || requested.len() > MAX_MEMBERS {
        return Err(invalid(
            "implementation must have a bounded nonempty member mapping",
        ));
    }
    let mut mapping = BTreeMap::new();
    let mut selected_functions = BTreeSet::new();
    for member in requested {
        exact(member, &["method", "implementation"])?;
        let method_id = selector(member, "method")?;
        let function_id = selector(member, "implementation")?;
        if mapping
            .insert(method_id.to_owned(), function_id.to_owned())
            .is_some()
        {
            return Err(invalid("implementation repeats a required protocol member"));
        }
        if !selected_functions.insert(function_id.to_owned()) {
            return Err(invalid(
                "implementation functions must be selected at most once",
            ));
        }
    }

    let programs = parse_revision(revision)?;
    let identities = identities(&programs)?;
    let implementation_id_absent = !identities.contains(implementation_id)
        && !programs.iter().any(|program| {
            program
                .module_uses
                .iter()
                .any(|binding| binding.persistent_id == implementation_id)
        });
    if !implementation_id_absent {
        return Ok(None);
    }
    // Ambiguity is destination drift here, rather than a malformed retained
    // intention. The originally admitted history necessarily had one owner.
    let Some(owner) = receiver_owner(&programs, target).ok().flatten() else {
        return Ok(None);
    };
    let program = &programs[owner];
    let Some(receiver) = program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == target)
    else {
        return Ok(None);
    };
    let Some(protocol) = program
        .protocols
        .iter()
        .find(|protocol| protocol.stable_id == protocol_id && protocol.explicit_id)
    else {
        return Ok(None);
    };
    if protocol.methods.len() != mapping.len() || protocol.methods.len() > MAX_MEMBERS {
        return Ok(None);
    }
    let pair_vacant = !program.implementations.iter().any(|implementation| {
        implementation.receiver_id == target && implementation.protocol_id == protocol_id
    });
    if !pair_vacant {
        return Ok(None);
    }

    let TypeDeclarationKind::Record { fields } = &receiver.kind else {
        return Ok(None);
    };
    let receiver_fields = fields
        .iter()
        .map(|field| {
            json!({
                "id":field.stable_id,
                "explicit_id":field.explicit_id,
                "name":field.name,
                "type":field.ty.to_string()
            })
        })
        .collect::<Vec<_>>();

    let mut methods = protocol.methods.iter().collect::<Vec<_>>();
    methods.sort_by(|left, right| left.stable_id.cmp(&right.stable_id));
    let mut method_facts = Vec::with_capacity(methods.len());
    let mut function_facts = BTreeMap::new();
    for method in methods {
        if !method.explicit_id {
            return Ok(None);
        }
        let Some(function_id) = mapping.get(&method.stable_id) else {
            return Ok(None);
        };
        let Some(function) = program
            .functions
            .iter()
            .find(|function| function.stable_id == *function_id)
        else {
            return Ok(None);
        };
        if !crate::static_protocol::member_matches(protocol, method, receiver, function) {
            return Ok(None);
        }
        method_facts.push(json!({
            "id":method.stable_id,
            "explicit_id":method.explicit_id,
            "name":method.name,
            "parameters":method.params.iter().map(|parameter| json!({
                "name":parameter.name,"mode":parameter.mode.text(),"type":parameter.ty.to_string()
            })).collect::<Vec<_>>(),
            "return_type":method.return_type.to_string()
        }));
        function_facts.insert(
            function.stable_id.clone(),
            json!({
                "id":function.stable_id,
                "explicit_id":function.explicit_id,
                "is_main":function.name == "main",
                "type_parameter_count":function.type_parameters.len(),
                "parameters":function.params.iter().map(|parameter| json!({
                    "mode":parameter.mode.text(),"type":parameter.ty.to_string()
                })).collect::<Vec<_>>(),
                "return_type":function.return_type.to_string(),
                "effects":function.effects,
                "requires_empty":function.requires.is_empty()
            }),
        );
    }
    let fingerprint = json!({
        "receiver":{
            "id":receiver.stable_id,
            "explicit_id":receiver.explicit_id,
            "name":receiver.name,
            "type_parameter_count":receiver.type_parameters.len(),
            "kind":"record",
            "fields":receiver_fields,
            "path":program.path,
            "module":program.module
        },
        "protocol":{
            "id":protocol.stable_id,
            "explicit_id":protocol.explicit_id,
            "name":protocol.name,
            "members":method_facts
        },
        "functions":function_facts,
        "mapping":mapping,
        "implementation_id":implementation_id,
        "implementation_id_absent":implementation_id_absent,
        "pair_vacant":pair_vacant
    });
    wire::render(fingerprint.clone(), MAX_REPORT_BYTES)
        .map_err(|_| capacity("interface rebase fingerprint exceeds its byte bound"))?;
    Ok(Some(fingerprint))
}

// Source conformance requires distinct functions, so nonempty per-member
// candidate lists alone do not establish availability. Paths have depth <=64.
fn complete_mapping(members: &[Value]) -> bool {
    fn assign(
        index: usize,
        candidates: &[Vec<&str>],
        assigned: &mut BTreeMap<String, usize>,
        seen: &mut BTreeSet<String>,
    ) -> bool {
        for function in &candidates[index] {
            if !seen.insert((*function).to_owned()) {
                continue;
            }
            let previous = assigned.get(*function).copied();
            if previous.is_none() || assign(previous.unwrap(), candidates, assigned, seen) {
                assigned.insert((*function).to_owned(), index);
                return true;
            }
        }
        false
    }
    let candidates = members
        .iter()
        .map(|member| {
            member["eligible_implementations"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut assigned = BTreeMap::new();
    (0..candidates.len())
        .all(|index| assign(index, &candidates, &mut assigned, &mut BTreeSet::new()))
}

fn receiver_owner(programs: &[Program], target: &str) -> Result<Option<usize>> {
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        for declaration in &program.types {
            if declaration.stable_id == target
                && declaration.explicit_id
                && declaration.type_parameters.is_empty()
                && matches!(declaration.kind, TypeDeclarationKind::Record { .. })
                && found.replace(owner).is_some()
            {
                return Err(invalid("implementation receiver identity is ambiguous"));
            }
        }
    }
    Ok(found)
}

fn binding_fact(program: &Program, implementation: &ProtocolImplementation) -> Value {
    let members = implementation
        .members
        .iter()
        .map(|member| (member.method_id.clone(), member.function_id.clone()))
        .collect::<BTreeMap<_, _>>();
    json!({"id":implementation.stable_id,"kind":"protocol_implementation","identity_origin":if implementation.explicit_id {"explicit"} else {"derived"},
        "owner":implementation.receiver_id,"protocol":implementation.protocol_id,"path":program.path,"module":program.module,
        "members":members,"evidence_owner":"source_static_protocol_validation","runtime_graph_declaration":false})
}

pub(super) fn inventory(programs: &[Program]) -> Result<BTreeMap<String, Value>> {
    let mut result = BTreeMap::new();
    for program in programs {
        for implementation in &program.implementations {
            if result.len() >= MAX_ITEMS {
                return Err(capacity("implementation inventory exceeds its bound"));
            }
            if result
                .insert(
                    implementation.stable_id.clone(),
                    binding_fact(program, implementation),
                )
                .is_some()
            {
                return Err(invalid(
                    "implementation identity is duplicated across source modules",
                ));
            }
        }
    }
    Ok(result)
}

pub(super) fn binding(revision: &ProjectRevision, target: &str) -> Result<Option<Value>> {
    Ok(inventory(&parse_revision(revision)?)?.remove(target))
}

pub(super) fn related(revision: &ProjectRevision, target: &str) -> Result<Vec<Value>> {
    Ok(inventory(&parse_revision(revision)?)?
        .into_values()
        .filter(|fact| {
            fact["id"] == target
                || fact["owner"] == target
                || fact["protocol"] == target
                || fact["members"].as_object().is_some_and(|members| {
                    members
                        .iter()
                        .any(|(method, function)| method == target || function == target)
                })
        })
        .collect())
}

/// Covers identities omitted by the current runtime graph as well. Other
/// candidate operations cannot silently reuse a protocol/implementation ID.
pub(super) fn identities(programs: &[Program]) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    let mut put = |id: &str| -> Result<()> {
        if ids.len() >= MAX_ITEMS {
            return Err(capacity(
                "source declaration identity inventory exceeds its bound",
            ));
        }
        if !ids.insert(id.to_owned()) {
            return Err(invalid(
                "source declaration identity is duplicated across the Project",
            ));
        }
        Ok(())
    };
    for program in programs {
        for function in &program.functions {
            put(&function.stable_id)?;
        }
        for declaration in &program.types {
            put(&declaration.stable_id)?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        put(&field.stable_id)?;
                    }
                    if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
                        for method in methods {
                            put(&method.stable_id)?;
                        }
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        put(&case.stable_id)?;
                        for field in &case.fields {
                            put(&field.stable_id)?;
                        }
                    }
                }
                TypeDeclarationKind::Resource { lifecycles } => {
                    for lifecycle in lifecycles {
                        if let Some(id) = &lifecycle.stable_id {
                            put(id)?;
                        }
                    }
                }
            }
        }
        for interface in &program.interfaces {
            put(&interface.stable_id)?;
            for import in &interface.imports {
                put(&import.stable_id)?;
            }
        }
        for protocol in &program.protocols {
            put(&protocol.stable_id)?;
            for method in &protocol.methods {
                put(&method.stable_id)?;
            }
        }
        for implementation in &program.implementations {
            put(&implementation.stable_id)?;
        }
    }
    Ok(ids)
}

fn exact(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("interface intention requires a closed object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid("interface intention has missing or unknown fields"));
    }
    Ok(())
}
fn selector<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let id = value[field]
        .as_str()
        .ok_or_else(|| invalid("interface selector must be text"))?;
    if !crate::static_protocol::valid_binding_id(id) {
        return Err(invalid(
            "interface selector must be a bounded ASCII stable identity",
        ));
    }
    Ok(id)
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G272", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G273", message)]
}
pub(super) fn mismatch() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G274",
        "candidate did not preserve exact source implementation identities and mappings",
    )]
}
pub(super) fn rebase_conflict() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G275","static implementation intention requires fresh discovery on a new base; protocol and member bindings are not implicitly remapped")]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_requires_a_complete_distinct_function_matching() {
        assert!(!complete_mapping(&[
            json!({"eligible_implementations":["same"]}),
            json!({"eligible_implementations":["same"]}),
        ]));
        assert!(complete_mapping(&[
            json!({"eligible_implementations":["first","second"]}),
            json!({"eligible_implementations":["first"]}),
        ]));
        assert!(!complete_mapping(&[json!({"eligible_implementations":[]})]));
    }
}
