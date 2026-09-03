//! Source-backed static protocol mappings. No runtime witness or dispatch data.
use super::{intent::IntentSummary, parse_revision, wire, ProjectCandidate};
use crate::ast::{
    Function, ModuleUse, ModuleUseKind, Program, ProtocolDeclaration, ProtocolImplementation,
    ProtocolImplementationMember, ProtocolMethod, Span, Type, TypeDeclaration, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, OwnershipMode, ResolvedType};
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
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(IntentSummary, ImplementationAddition)> {
    exact_implementation(request)?;
    let target = selector(request, "target")?;
    let protocol_id = selector(request, "protocol")?;
    let id = selector(request, "id")?;
    if id.starts_with("auto:") || id.starts_with("semaprax.") {
        return Err(invalid("implementation identity uses a reserved namespace"));
    }
    let receiver_owner = receiver_owner(programs, target)?.ok_or_else(|| {
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
    let receiver_program = &programs[receiver_owner];
    let receiver = receiver_program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == target)
        .unwrap()
        .clone();
    let (protocol_owner, protocol) =
        explicit_protocol(programs, protocol_id)?.ok_or_else(|| {
            invalid("implementation protocol must be one explicit Project declaration")
        })?;
    let protocol = protocol.clone();
    let destination = request
        .get("destination")
        .and_then(Value::as_str)
        .unwrap_or(receiver_program.module.as_str());
    let destination_owner = destination_module(programs, destination)?;
    if programs
        .iter()
        .flat_map(|program| &program.implementations)
        .any(|implementation| {
            implementation.receiver_id == target && implementation.protocol_id == protocol_id
        })
    {
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
        let (function_owner, function) =
            explicit_function(programs, function_id)?.ok_or_else(|| {
                invalid("implementation function must be one explicit Project function")
            })?;
        if !member_matches_project(
            programs,
            protocol_owner,
            &protocol,
            method,
            receiver_owner,
            &receiver,
            function_owner,
            function,
        )? {
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
    authenticate_checked_bindings(revision, programs, target, protocol_id, &implementation)?;
    plan_imports(
        programs,
        destination_owner,
        receiver_owner,
        &receiver,
        protocol_owner,
        &protocol,
        &implementation,
    )?;
    let program = &programs[destination_owner];
    let fact = binding_fact(program, &implementation);
    let addition = ImplementationAddition {
        id: id.to_owned(),
        owner: target.to_owned(),
        path: program.path.clone(),
        module: program.module.clone(),
        fact,
    };
    programs[destination_owner]
        .implementations
        .push(implementation);
    crate::static_protocol::validate_workspace(programs).map_err(|error| vec![error])?;
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
    /// Discover complete Project protocol requirements and compiler-matched
    /// existing functions. Discovery confers neither conformance nor authority.
    pub fn interface_catalog(&self, expected_candidate: &str, target: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let protocols = discover(self.revision(), target)?;
        wire::render(json!({"schema":"semaprax.project-interface-change-catalog.v1",
            "candidate_digest":self.candidate_digest(),"project_revision":self.revision().project_revision(),
            "target":target,"protocols":protocols,"admission":"compiler_signature_discovery_only",
            "requires_full_candidate_validation":true,"source_authority":false,
            "limits":{"max_members":MAX_MEMBERS,"max_items":MAX_ITEMS,"max_report_bytes":MAX_REPORT_BYTES},
            "nonclaims":["no_dynamic_dispatch","no_runtime_witness","no_generated_implementation","no_external_abi_or_package_conformance"]}), MAX_REPORT_BYTES)
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
    let mut protocols = programs
        .iter()
        .enumerate()
        .flat_map(|(protocol_owner, program)| {
            program
                .protocols
                .iter()
                .filter(|protocol| {
                    protocol.explicit_id
                        && crate::static_protocol::valid_binding_id(&protocol.stable_id)
                })
                .map(move |protocol| (protocol_owner, protocol))
        })
        .collect::<Vec<_>>();
    protocols.sort_by(|left, right| left.1.stable_id.cmp(&right.1.stable_id));
    let mut items = 0usize;
    let mut output = Vec::new();
    for (protocol_owner, protocol) in protocols {
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
            let mut functions = Vec::new();
            for (function_owner, function_program) in programs.iter().enumerate() {
                for function in &function_program.functions {
                    if crate::static_protocol::valid_binding_id(&function.stable_id)
                        && member_matches_project(
                            &programs,
                            protocol_owner,
                            protocol,
                            method,
                            owner,
                            receiver,
                            function_owner,
                            function,
                        )?
                    {
                        functions.push(function.stable_id.clone());
                    }
                }
            }
            functions.sort();
            items = items.saturating_add(1 + functions.len());
            if items > MAX_ITEMS {
                return Err(capacity("interface discovery inventory exceeds its bound"));
            }
            members.push(json!({"method":method.stable_id,"name":method.name,
                "parameters":method.params.iter().map(|parameter| json!({"name":parameter.name,"type":parameter.ty.to_string(),"mode":parameter.mode.text()})).collect::<Vec<_>>(),
                "return_type":method.return_type.to_string(),"eligible_implementations":functions}));
        }
        let existing = programs
            .iter()
            .flat_map(|program| &program.implementations)
            .find(|implementation| {
                implementation.receiver_id == target
                    && implementation.protocol_id == protocol.stable_id
            });
        let complete = complete_mapping(&members);
        output.push(json!({"protocol":protocol.stable_id,"name":protocol.name,"members":members,
            "protocol_module":programs[protocol_owner].module,
            "destination_modules":programs.iter().map(|program| program.module.as_str()).collect::<Vec<_>>(),
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
    exact_implementation(request)?;
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
    let destination = request
        .get("destination")
        .and_then(Value::as_str)
        .unwrap_or(program.module.as_str());
    let Ok(destination_owner) = destination_module(&programs, destination) else {
        return Ok(None);
    };
    let Some(receiver) = program
        .types
        .iter()
        .find(|declaration| declaration.stable_id == target)
    else {
        return Ok(None);
    };
    let Ok(Some((protocol_owner, protocol))) = explicit_protocol(&programs, protocol_id) else {
        return Ok(None);
    };
    if protocol.methods.len() != mapping.len() || protocol.methods.len() > MAX_MEMBERS {
        return Ok(None);
    }
    let pair_vacant = !programs
        .iter()
        .flat_map(|program| &program.implementations)
        .any(|implementation| {
            implementation.receiver_id == target && implementation.protocol_id == protocol_id
        });
    if !pair_vacant {
        return Ok(None);
    }

    let Some(checked_module) = revision
        .semantic
        .image_modules()
        .iter()
        .find(|module| module.path() == program.path)
    else {
        return Ok(None);
    };
    let Some(checked_receiver) = checked_module
        .types()
        .iter()
        .find(|declaration| declaration.id.as_str() == target)
    else {
        return Ok(None);
    };
    let crate::hir::ResolvedTypeDeclarationKind::Record {
        fields: checked_fields,
    } = &checked_receiver.kind
    else {
        return Ok(None);
    };

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
    let checked_receiver_fields = checked_fields
        .iter()
        .map(|field| {
            json!({
                "id":field.id.as_str(),
                "name":field.name,
                "index":field.index,
                "type_identity":field.ty.identity_key()
            })
        })
        .collect::<Vec<_>>();

    let mut method_facts = Vec::with_capacity(protocol.methods.len());
    let mut function_facts = BTreeMap::new();
    for method in &protocol.methods {
        if !method.explicit_id {
            return Ok(None);
        }
        let Some(function_id) = mapping.get(&method.stable_id) else {
            return Ok(None);
        };
        let Ok(Some((function_owner, function))) = explicit_function(&programs, function_id) else {
            return Ok(None);
        };
        if !member_matches_project(
            &programs,
            protocol_owner,
            protocol,
            method,
            owner,
            receiver,
            function_owner,
            function,
        )? {
            return Ok(None);
        }
        let Some(function_module) = revision
            .semantic
            .image_modules()
            .iter()
            .find(|module| module.path() == programs[function_owner].path)
        else {
            return Ok(None);
        };
        let Some(checked_function) = function_module
            .functions()
            .iter()
            .find(|candidate| candidate.id.as_str() == function.stable_id)
        else {
            return Ok(None);
        };
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
                "checked_signature":{
                    "parameters":checked_function.params.iter().map(|parameter|
                        parameter.ty.identity_key()).collect::<Vec<_>>(),
                    "return_type":checked_function.return_type.identity_key(),
                    "evidence_owner":"retained_checked_source_module_HIR"
                },
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
            "checked_fields":checked_receiver_fields,
            "path":program.path,
            "module":program.module
        },
        "protocol":{
            "id":protocol.stable_id,
            "explicit_id":protocol.explicit_id,
            "name":protocol.name,
            "members":method_facts,
            "path":programs[protocol_owner].path,
            "module":programs[protocol_owner].module
        },
        "functions":function_facts,
        "mapping":mapping,
        "implementation_id":implementation_id,
        "implementation_id_absent":implementation_id_absent,
        "pair_vacant":pair_vacant,
        "destination":{"path":programs[destination_owner].path,"module":programs[destination_owner].module},
        "planned_imports":programs[destination_owner].module_uses.iter().filter(|binding|
            binding.persistent_id == target || binding.persistent_id == protocol_id || mapping.values().any(|id| id == &binding.persistent_id)
        ).map(|binding| json!({"kind":match binding.kind { ModuleUseKind::Function => "function", ModuleUseKind::Type => "type", ModuleUseKind::Protocol => "protocol" },"id":binding.persistent_id,"provider":binding.target_module,"alias":binding.alias})).collect::<Vec<_>>()
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

fn exact_implementation(request: &Value) -> Result<()> {
    if request.get("destination").is_some() {
        exact(
            request,
            &["kind", "target", "protocol", "id", "destination", "members"],
        )
    } else {
        exact(request, &["kind", "target", "protocol", "id", "members"])
    }
}

fn destination_module(programs: &[Program], destination: &str) -> Result<usize> {
    if destination.is_empty() || destination.len() > 240 {
        return Err(placement(
            "implementation destination must be one bounded declared Project module",
        ));
    }
    let mut found = None;
    for (index, program) in programs.iter().enumerate() {
        if program.module == destination && found.replace(index).is_some() {
            return Err(placement("implementation destination module is ambiguous"));
        }
    }
    found.ok_or_else(|| placement("implementation destination module is absent"))
}

fn explicit_protocol<'a>(
    programs: &'a [Program],
    id: &str,
) -> Result<Option<(usize, &'a ProtocolDeclaration)>> {
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        for protocol in program
            .protocols
            .iter()
            .filter(|protocol| protocol.stable_id == id && protocol.explicit_id)
        {
            if found.replace((owner, protocol)).is_some() {
                return Err(authentication(
                    "implementation protocol identity is ambiguous",
                ));
            }
        }
    }
    Ok(found)
}

fn explicit_function<'a>(
    programs: &'a [Program],
    id: &str,
) -> Result<Option<(usize, &'a Function)>> {
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        for function in program
            .functions
            .iter()
            .filter(|function| function.stable_id == id && function.explicit_id)
        {
            if found.replace((owner, function)).is_some() {
                return Err(authentication(
                    "implementation function identity is ambiguous",
                ));
            }
        }
    }
    Ok(found)
}

#[allow(clippy::too_many_arguments)]
fn member_matches_project(
    programs: &[Program],
    protocol_owner: usize,
    protocol: &ProtocolDeclaration,
    method: &ProtocolMethod,
    receiver_owner: usize,
    receiver: &TypeDeclaration,
    function_owner: usize,
    function: &Function,
) -> Result<bool> {
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
    {
        return Ok(false);
    }
    for (index, (required, actual)) in method.params.iter().zip(&function.params).enumerate() {
        if required.mode != actual.mode {
            return Ok(false);
        }
        let required = if index == 0
            && matches!(&required.ty, Type::Named { name, arguments } if arguments.is_empty() && (name == "Self" || name == &protocol.name))
        {
            format!("id:{}", receiver.stable_id)
        } else {
            type_key(programs, protocol_owner, &required.ty, 0)?
        };
        let actual = type_key(programs, function_owner, &actual.ty, 0)?;
        if required != actual {
            return Ok(false);
        }
    }
    Ok(type_key(programs, protocol_owner, &method.return_type, 0)?
        == type_key(programs, function_owner, &function.return_type, 0)?
        && programs[receiver_owner]
            .types
            .iter()
            .any(|item| item == receiver))
}

fn type_key(programs: &[Program], owner: usize, ty: &Type, depth: usize) -> Result<String> {
    if depth > 64 {
        return Err(capacity("interface type identity exceeds its depth bound"));
    }
    let Type::Named { name, arguments } = ty else {
        return Ok(ty.to_string());
    };
    let program = &programs[owner];
    let local = program
        .types
        .iter()
        .find(|item| item.name == *name)
        .map(|item| item.stable_id.as_str());
    let imported = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type && item.alias == *name)
        .map(|item| item.persistent_id.as_str())
        .collect::<Vec<_>>();
    let prelude = crate::prelude::declarations()
        .iter()
        .find(|item| item.name == *name)
        .map(|item| item.stable_id.as_str());
    let identity = match (local, imported.as_slice()) {
        (Some(id), []) => format!("id:{id}"),
        (None, [id]) => format!("id:{id}"),
        (None, []) if prelude.is_some() => format!("id:{}", prelude.unwrap()),
        (None, []) => {
            return Err(authentication(
                "interface nominal type lacks an exact Project identity binding",
            ))
        }
        _ => {
            return Err(authentication(
                "interface nominal type binding is ambiguous",
            ))
        }
    };
    let arguments = arguments
        .iter()
        .map(|argument| type_key(programs, owner, argument, depth + 1))
        .collect::<Result<Vec<_>>>()?;
    // A monomorphic nominal type keys as its bare identity, exactly like the
    // receiver key the first-parameter `Self` case builds from `stable_id`.
    if arguments.is_empty() {
        return Ok(identity);
    }
    Ok(format!("{identity}<{}>", arguments.join(",")))
}

fn authenticate_checked_bindings(
    revision: &ProjectRevision,
    programs: &[Program],
    receiver_id: &str,
    protocol_id: &str,
    implementation: &ProtocolImplementation,
) -> Result<()> {
    let (receiver_owner, receiver) = receiver_owner(programs, receiver_id)?
        .and_then(|owner| {
            programs[owner]
                .types
                .iter()
                .find(|item| item.stable_id == receiver_id)
                .map(|item| (owner, item))
        })
        .ok_or_else(|| authentication("implementation receiver lost its source binding"))?;
    authenticate_source_program(revision, &programs[receiver_owner])?;
    let checked_receiver = revision
        .semantic
        .image_modules()
        .iter()
        .find(|module| module.path() == programs[receiver_owner].path)
        .and_then(|module| {
            module
                .types()
                .iter()
                .find(|item| item.id.as_str() == receiver_id)
        });
    let Some(checked_receiver) = checked_receiver else {
        return Err(authentication(
            "implementation receiver lacks exact retained record HIR",
        ));
    };
    let TypeDeclarationKind::Record { fields } = &receiver.kind else {
        return Err(authentication(
            "implementation receiver source is no longer a record",
        ));
    };
    let crate::hir::ResolvedTypeDeclarationKind::Record {
        fields: checked_fields,
    } = &checked_receiver.kind
    else {
        return Err(authentication(
            "implementation receiver lacks exact retained record HIR",
        ));
    };
    if checked_receiver.name != receiver.name
        || checked_receiver.span != receiver.span
        || !checked_receiver.type_parameters.is_empty()
        || checked_fields.len() != fields.len()
    {
        return Err(authentication(
            "implementation receiver source and retained record HIR disagree",
        ));
    }
    let mut work = 0usize;
    for (index, (field, checked)) in fields.iter().zip(checked_fields).enumerate() {
        let ty = resolved_source_type(&programs[receiver_owner], &field.ty, &mut work, 0)?;
        if checked.id.as_str() != field.stable_id
            || checked.name != field.name
            || checked.span != field.span
            || checked.index as usize != index
            || checked.ty != ty
        {
            return Err(authentication(
                "implementation receiver field source and retained HIR disagree",
            ));
        }
    }
    if explicit_protocol(programs, protocol_id)?.is_none() {
        return Err(authentication(
            "implementation protocol lost its retained source identity",
        ));
    }
    for member in &implementation.members {
        let (owner, function) = explicit_function(programs, &member.function_id)?
            .ok_or_else(|| authentication("implementation function lost its source identity"))?;
        authenticate_source_program(revision, &programs[owner])?;
        let checked = revision
            .semantic
            .image_modules()
            .iter()
            .find(|module| module.path() == programs[owner].path)
            .and_then(|module| {
                module
                    .functions()
                    .iter()
                    .find(|item| item.id.as_str() == function.stable_id)
            });
        let Some(checked) = checked else {
            return Err(authentication(
                "implementation function lacks exact retained source HIR",
            ));
        };
        if checked.name != function.name
            || checked.span != function.span
            || checked.params.len() != function.params.len()
            || checked.return_type
                != resolved_source_type(&programs[owner], &function.return_type, &mut work, 0)?
            || checked.effects != function.effects
            || checked.requires.len() != function.requires.len()
            || checked.ensures.len() != function.ensures.len()
        {
            return Err(authentication(
                "implementation function source and retained HIR signature, effects, or contracts disagree",
            ));
        }
        for (source, retained) in function.params.iter().zip(&checked.params) {
            let ownership =
                if source.mode == crate::ast::ParamMode::Value && source.ty == Type::String {
                    OwnershipMode::Own
                } else {
                    OwnershipMode::from(source.mode)
                };
            if retained.name != source.name
                || retained.span != source.span
                || retained.ownership != ownership
                || retained.ty != resolved_source_type(&programs[owner], &source.ty, &mut work, 0)?
            {
                return Err(authentication(
                    "implementation function parameter source and retained HIR disagree",
                ));
            }
        }
    }
    Ok(())
}

fn authenticate_source_program(revision: &ProjectRevision, program: &Program) -> Result<()> {
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == program.path)
        .ok_or_else(|| authentication("implementation source module is absent"))?;
    let retained = crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
    if retained != *program {
        return Err(authentication(
            "implementation source module differs from its authenticated revision",
        ));
    }
    Ok(())
}

fn resolved_source_type(
    program: &Program,
    source: &Type,
    work: &mut usize,
    depth: usize,
) -> Result<ResolvedType> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| capacity("interface checked type authentication work overflow"))?;
    if *work > MAX_ITEMS || depth > 64 {
        return Err(capacity(
            "interface checked type authentication exceeds its bound",
        ));
    }
    Ok(match source {
        Type::I64 => ResolvedType::I64,
        Type::I32 => ResolvedType::I32,
        Type::Char => ResolvedType::Char,
        Type::U8 => ResolvedType::U8,
        Type::Usize => ResolvedType::Usize,
        Type::ArrayU8(length) => ResolvedType::ArrayU8(*length),
        Type::F32 => ResolvedType::F32,
        Type::F64 => ResolvedType::F64,
        Type::Bool => ResolvedType::Bool,
        Type::String => ResolvedType::String,
        Type::Bytes => ResolvedType::Bytes,
        Type::Str => ResolvedType::Str,
        Type::SliceU8 => ResolvedType::SliceU8,
        Type::Named { name, arguments } => {
            *work = work
                .checked_add(program.types.len() + program.module_uses.len())
                .ok_or_else(|| capacity("interface nominal authentication work overflow"))?;
            if *work > MAX_ITEMS {
                return Err(capacity(
                    "interface nominal authentication exceeds its inventory bound",
                ));
            }
            let mut identities = BTreeSet::new();
            for declaration in &program.types {
                if declaration.name == *name {
                    identities.insert(declaration.stable_id.as_str());
                }
            }
            for binding in &program.module_uses {
                if binding.kind == ModuleUseKind::Type && binding.alias == *name {
                    identities.insert(binding.persistent_id.as_str());
                }
            }
            if let Some(prelude) = crate::prelude::declarations()
                .iter()
                .find(|declaration| declaration.name == *name)
            {
                identities.insert(prelude.stable_id.as_str());
            }
            if identities.len() != 1 {
                return Err(authentication(
                    "interface checked nominal type lacks one exact source identity",
                ));
            }
            let declaration = DeclarationId::new(*identities.iter().next().expect("one identity"));
            let mut resolved = Vec::with_capacity(arguments.len());
            for argument in arguments {
                resolved.push(resolved_source_type(program, argument, work, depth + 1)?);
            }
            ResolvedType::Nominal {
                declaration,
                arguments: resolved,
            }
        }
    })
}

fn plan_imports(
    programs: &mut [Program],
    destination: usize,
    receiver_owner: usize,
    receiver: &TypeDeclaration,
    protocol_owner: usize,
    protocol: &ProtocolDeclaration,
    implementation: &ProtocolImplementation,
) -> Result<()> {
    let mut required = Vec::new();
    if receiver_owner != destination {
        required.push((
            ModuleUseKind::Type,
            receiver.stable_id.clone(),
            programs[receiver_owner].module.clone(),
            receiver.name.clone(),
        ));
    }
    if protocol_owner != destination {
        required.push((
            ModuleUseKind::Protocol,
            protocol.stable_id.clone(),
            programs[protocol_owner].module.clone(),
            protocol.name.clone(),
        ));
    }
    for member in &implementation.members {
        let (owner, function) = explicit_function(programs, &member.function_id)?
            .ok_or_else(|| authentication("implementation function lost before import planning"))?;
        if owner != destination {
            required.push((
                ModuleUseKind::Function,
                function.stable_id.clone(),
                programs[owner].module.clone(),
                function.name.clone(),
            ));
        }
    }
    required.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    required.dedup_by(|left, right| left.0 == right.0 && left.1 == right.1);
    let mut occupied = programs[destination]
        .functions
        .iter()
        .map(|item| item.name.clone())
        .chain(
            programs[destination]
                .types
                .iter()
                .map(|item| item.name.clone()),
        )
        .chain(
            programs[destination]
                .protocols
                .iter()
                .map(|item| item.name.clone()),
        )
        .chain(
            programs[destination]
                .interfaces
                .iter()
                .map(|item| item.name.clone()),
        )
        .chain(
            programs[destination]
                .module_uses
                .iter()
                .map(|item| item.alias.clone()),
        )
        .collect::<BTreeSet<_>>();
    for (kind, id, provider, preferred) in required {
        let existing = programs[destination]
            .module_uses
            .iter()
            .filter(|item| item.persistent_id == id)
            .collect::<Vec<_>>();
        match existing.as_slice() {
            [binding] if binding.kind == kind && binding.target_module == provider => continue,
            [] => {}
            _ => {
                return Err(import_conflict(
                    "implementation dependency import conflicts with an existing binding",
                ))
            }
        }
        let alias = if occupied.insert(preferred.clone()) {
            preferred
        } else {
            let mut selected = None;
            for ordinal in 0..4096usize {
                let candidate = format!("_spx_impl_{ordinal}");
                if occupied.insert(candidate.clone()) {
                    selected = Some(candidate);
                    break;
                }
            }
            selected.ok_or_else(|| {
                import_conflict("implementation cannot allocate a bounded import alias")
            })?
        };
        programs[destination].module_uses.push(ModuleUse {
            kind,
            persistent_id: id,
            target_module: provider,
            alias,
            span: Span::default(),
        });
    }
    Ok(())
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
fn placement(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G497", message)]
}
fn import_conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G498", message)]
}
fn authentication(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G499", message)]
}
pub(super) fn mismatch() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G274",
        "candidate did not preserve exact source implementation identities and mappings",
    )]
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
