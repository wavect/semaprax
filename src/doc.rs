//! Documentation Projection v1: one checked module rendered for people and
//! coding agents from the declaration facts the semantic graph carries.
//!
//! `semaprax doc <file>` prints Markdown; `--json` prints the same facts as one
//! `semaprax.doc.v1` document. Both are deterministic functions of the
//! canonical program and its comments, and both carry the graph revision of
//! [`crate::graph::revision`], so a document can be matched to the exact graph
//! it describes. The renderer reads declarations, never bodies: identities,
//! signatures, ownership modes, effects, contracts, members, and the leading
//! comments that describe each declaration.

use std::fmt::Write as _;

use crate::ast::{
    Expr, Function, ImportFailure, ModuleUseKind, Param, Program, ResourceLifecycleKind, Span,
    Type, TypeDeclaration, TypeDeclarationKind, TypeParameterDeclaration,
};
use crate::diagnostic::quote_json;
use crate::format::comments::{Comments, Placement};
use crate::format::{
    write_escaped, write_joined, write_record_literal_delimited_expr, write_type,
    write_type_parameters,
};
use crate::graph;

/// Schema of the JSON projection.
pub const SCHEMA_V1: &str = "semaprax.doc.v1";

/// The documentation model of one module: the facts both renderings share.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub module: String,
    pub revision: String,
    pub permits: Vec<String>,
    pub uses: Vec<Use>,
    pub entries: Vec<Entry>,
}

/// One `use` line of the module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Use {
    pub kind: &'static str,
    pub id: String,
    pub module: String,
    pub alias: String,
}

/// Where a declaration or member is written: one-based line and column of its
/// name (or of the item when it has no name) and the byte offsets of that
/// token, exactly as diagnostics report locations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub start: usize,
    pub end: usize,
}

impl Location {
    fn of(span: Span) -> Self {
        Self {
            line: span.line,
            column: span.column,
            start: span.start,
            end: span.end,
        }
    }

    fn json(self) -> String {
        format!(
            "{{\"line\":{},\"column\":{},\"start\":{},\"end\":{}}}",
            self.line, self.column, self.start, self.end
        )
    }
}

/// One documented declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// `record`, `variant`, `class`, `resource`, `interface`, `protocol`,
    /// `implementation`, `function`, or `method`.
    pub kind: &'static str,
    pub id: String,
    pub name: String,
    /// `true` when the source declares the identity with `@id`, so it persists
    /// across revisions; automatic identities are revision-scoped.
    pub persistent: bool,
    /// The leading comments of the declaration, one line each, without `//`
    /// and without one leading space.
    pub description: Vec<String>,
    /// The declaration header in canonical source syntax, without bodies.
    pub signature: String,
    pub location: Location,
    pub facts: Vec<Fact>,
    pub members: Vec<Member>,
}

/// One labelled list of facts about a declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fact {
    pub label: &'static str,
    pub values: Vec<String>,
}

/// One identified member of a declaration: a field, case, case field, drop
/// lifecycle, import, protocol method, or implementation binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub kind: &'static str,
    pub id: String,
    pub name: String,
    pub persistent: bool,
    /// The member in canonical source syntax.
    pub text: String,
    pub location: Location,
}

/// Build the documentation model of a parsed program. Callers verify the
/// program first; the model describes declarations exactly as written.
#[must_use]
pub fn document(program: &Program, comments: &Comments) -> Document {
    let placement = Placement::new(program, comments);
    let mut entries = Vec::new();
    for declaration in &program.types {
        entries.push(type_entry(declaration, &placement));
        if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            for method in methods {
                let mut entry = function_entry(method, &placement, "method");
                entry.facts.insert(
                    0,
                    Fact {
                        label: "Owner",
                        values: vec![declaration.stable_id.clone()],
                    },
                );
                entries.push(entry);
            }
        }
    }
    for interface in &program.interfaces {
        entries.push(interface_entry(interface, &placement));
    }
    for protocol in &program.protocols {
        entries.push(protocol_entry(protocol, &placement));
    }
    for implementation in &program.implementations {
        entries.push(implementation_entry(implementation, &placement));
    }
    for function in &program.functions {
        entries.push(function_entry(function, &placement, "function"));
    }
    Document {
        module: program.module.clone(),
        revision: graph::revision(program),
        permits: program.permits.clone(),
        uses: program
            .module_uses
            .iter()
            .map(|module_use| Use {
                kind: match module_use.kind {
                    ModuleUseKind::Function => "function",
                    ModuleUseKind::Type => "type",
                    ModuleUseKind::Protocol => "protocol",
                },
                id: module_use.persistent_id.clone(),
                module: module_use.target_module.clone(),
                alias: module_use.alias.clone(),
            })
            .collect(),
        entries,
    }
}

/// The Markdown projection.
#[must_use]
pub fn markdown(program: &Program, comments: &Comments) -> String {
    render_markdown(&document(program, comments))
}

/// The `semaprax.doc.v1` JSON projection, one line, key order fixed.
#[must_use]
pub fn json(program: &Program, comments: &Comments) -> String {
    render_json(&document(program, comments))
}

fn description(placement: &Placement, start: usize) -> Vec<String> {
    placement
        .leading_texts(start)
        .iter()
        .map(|text| text.strip_prefix(' ').unwrap_or(text).to_owned())
        .collect()
}

fn type_text(ty: &Type) -> String {
    let mut output = String::new();
    write_type(&mut output, ty);
    output
}

fn param_text(param: &Param) -> String {
    format!(
        "{}: {}{}",
        param.name,
        param.mode.source_prefix(),
        type_text(&param.ty)
    )
}

fn contract_text(expr: &Expr) -> String {
    let mut output = String::new();
    write_record_literal_delimited_expr(&mut output, expr);
    output
}

fn write_id_line(output: &mut String, id: &str, indent: &str) {
    write!(output, "{indent}@id(\"").unwrap();
    write_escaped(output, id);
    output.push_str("\")\n");
}

fn write_params(output: &mut String, params: &[Param]) {
    output.push('(');
    for (index, param) in params.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&param_text(param));
    }
    output.push_str(") -> ");
}

fn write_function_header(output: &mut String, function: &Function, indent: &str) {
    if function.explicit_id {
        write_id_line(output, &function.stable_id, indent);
    }
    write!(output, "{indent}fn {}", function.name).unwrap();
    write_type_parameters(output, &function.type_parameters);
    write_params(output, &function.params);
    write_type(output, &function.return_type);
    output.push('\n');
    if !function.effects.is_empty() {
        write!(output, "{indent}    uses {{ ").unwrap();
        write_joined(output, &function.effects, ", ");
        output.push_str(" }\n");
    }
    for contract in &function.requires {
        write!(output, "{indent}    requires ").unwrap();
        write_record_literal_delimited_expr(output, contract);
        output.push('\n');
    }
    for contract in &function.ensures {
        write!(output, "{indent}    ensures ").unwrap();
        write_record_literal_delimited_expr(output, contract);
        output.push('\n');
    }
}

fn push_fact(facts: &mut Vec<Fact>, label: &'static str, values: Vec<String>) {
    if !values.is_empty() {
        facts.push(Fact { label, values });
    }
}

fn type_parameter_names(parameters: &[TypeParameterDeclaration]) -> Vec<String> {
    parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect()
}

fn function_entry(function: &Function, placement: &Placement, kind: &'static str) -> Entry {
    let mut signature = String::new();
    write_function_header(&mut signature, function, "");
    let mut facts = Vec::new();
    push_fact(
        &mut facts,
        "Type parameters",
        type_parameter_names(&function.type_parameters),
    );
    push_fact(
        &mut facts,
        "Parameters",
        function.params.iter().map(param_text).collect(),
    );
    facts.push(Fact {
        label: "Returns",
        values: vec![type_text(&function.return_type)],
    });
    push_fact(&mut facts, "Effects", function.effects.clone());
    push_fact(
        &mut facts,
        "Requires",
        function.requires.iter().map(contract_text).collect(),
    );
    push_fact(
        &mut facts,
        "Ensures",
        function.ensures.iter().map(contract_text).collect(),
    );
    Entry {
        kind,
        id: function.stable_id.clone(),
        name: function.name.clone(),
        persistent: function.explicit_id,
        description: description(placement, function.span.start),
        signature,
        location: Location::of(function.name_span),
        facts,
        members: Vec::new(),
    }
}

fn field_members(
    fields: &[crate::ast::FieldDeclaration],
    kind: &'static str,
    signature: &mut String,
    indent: &str,
    members: &mut Vec<Member>,
) {
    for field in fields {
        if field.explicit_id {
            write_id_line(signature, &field.stable_id, indent);
        }
        let text = format!("{}: {}", field.name, type_text(&field.ty));
        writeln!(signature, "{indent}{text},").unwrap();
        members.push(Member {
            kind,
            id: field.stable_id.clone(),
            name: field.name.clone(),
            persistent: field.explicit_id,
            text,
            location: Location::of(field.name_span),
        });
    }
}

fn type_entry(declaration: &TypeDeclaration, placement: &Placement) -> Entry {
    let mut signature = String::new();
    if declaration.explicit_id {
        write_id_line(&mut signature, &declaration.stable_id, "");
    }
    let mut members = Vec::new();
    let mut facts = Vec::new();
    push_fact(
        &mut facts,
        "Type parameters",
        type_parameter_names(&declaration.type_parameters),
    );
    let kind = match &declaration.kind {
        TypeDeclarationKind::Resource { lifecycles } => {
            write!(signature, "resource {}", declaration.name).unwrap();
            write_type_parameters(&mut signature, &declaration.type_parameters);
            if lifecycles.is_empty() {
                signature.push_str(";\n");
            } else {
                signature.push_str(" {\n");
                for lifecycle in lifecycles {
                    if let Some(id) = &lifecycle.stable_id {
                        write_id_line(&mut signature, id, "    ");
                    }
                    let text = match &lifecycle.kind {
                        ResourceLifecycleKind::Trivial => "drop trivial;".to_owned(),
                        ResourceLifecycleKind::Imported { import_key } => {
                            let mut text = String::from("drop import \"");
                            write_escaped(&mut text, import_key);
                            text.push_str("\";");
                            text
                        }
                    };
                    writeln!(signature, "    {text}").unwrap();
                    members.push(Member {
                        kind: "drop",
                        id: lifecycle.stable_id.clone().unwrap_or_default(),
                        name: "drop".to_owned(),
                        persistent: lifecycle.stable_id.is_some(),
                        text,
                        location: Location::of(lifecycle.span),
                    });
                }
                signature.push_str("}\n");
            }
            "resource"
        }
        TypeDeclarationKind::Record { fields } => {
            write!(signature, "record {}", declaration.name).unwrap();
            write_type_parameters(&mut signature, &declaration.type_parameters);
            signature.push_str(" {\n");
            field_members(fields, "field", &mut signature, "    ", &mut members);
            signature.push_str("}\n");
            "record"
        }
        TypeDeclarationKind::Variant { cases } => {
            write!(signature, "variant {}", declaration.name).unwrap();
            write_type_parameters(&mut signature, &declaration.type_parameters);
            signature.push_str(" {\n");
            for case in cases {
                if case.explicit_id {
                    write_id_line(&mut signature, &case.stable_id, "    ");
                }
                let text = if case.fields.is_empty() {
                    writeln!(signature, "    {},", case.name).unwrap();
                    case.name.clone()
                } else {
                    writeln!(signature, "    {} {{", case.name).unwrap();
                    let mut case_fields = Vec::new();
                    field_members(
                        &case.fields,
                        "case_field",
                        &mut signature,
                        "        ",
                        &mut case_fields,
                    );
                    signature.push_str("    },\n");
                    let text = format!(
                        "{} {{ {} }}",
                        case.name,
                        case_fields
                            .iter()
                            .map(|field| field.text.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    members.push(Member {
                        kind: "case",
                        id: case.stable_id.clone(),
                        name: case.name.clone(),
                        persistent: case.explicit_id,
                        text,
                        location: Location::of(case.name_span),
                    });
                    members.extend(case_fields);
                    continue;
                };
                members.push(Member {
                    kind: "case",
                    id: case.stable_id.clone(),
                    name: case.name.clone(),
                    persistent: case.explicit_id,
                    text,
                    location: Location::of(case.name_span),
                });
            }
            signature.push_str("}\n");
            "variant"
        }
        TypeDeclarationKind::Class { fields, methods } => {
            write!(signature, "class {}", declaration.name).unwrap();
            write_type_parameters(&mut signature, &declaration.type_parameters);
            if let Some(parent) = &declaration.extends {
                signature.push_str(" : ");
                write_type(&mut signature, parent);
                facts.push(Fact {
                    label: "Extends",
                    values: vec![type_text(parent)],
                });
            }
            if fields.is_empty() && methods.is_empty() {
                signature.push_str(" { }\n");
            } else {
                signature.push_str(" {\n");
                field_members(fields, "field", &mut signature, "    ", &mut members);
                for method in methods {
                    signature.push('\n');
                    write_function_header(&mut signature, method, "    ");
                }
                signature.push_str("}\n");
            }
            push_fact(
                &mut facts,
                "Methods",
                methods
                    .iter()
                    .map(|method| method.stable_id.clone())
                    .collect(),
            );
            "class"
        }
    };
    Entry {
        kind,
        id: declaration.stable_id.clone(),
        name: declaration.name.clone(),
        persistent: declaration.explicit_id,
        description: description(placement, declaration.span.start),
        signature,
        location: Location::of(declaration.name_span),
        facts,
        members,
    }
}

fn interface_entry(interface: &crate::ast::InterfaceDeclaration, placement: &Placement) -> Entry {
    let mut signature = String::new();
    if interface.explicit_id {
        write_id_line(&mut signature, &interface.stable_id, "");
    }
    writeln!(signature, "interface {}", interface.name).unwrap();
    signature.push_str("    permits { ");
    write_joined(&mut signature, &interface.permits, ", ");
    signature.push_str(" }\n{\n");
    let mut members = Vec::new();
    for import in &interface.imports {
        if import.explicit_id {
            write_id_line(&mut signature, &import.stable_id, "    ");
        }
        let mut text = format!(
            "import {}fn {}",
            if import.native_rust { "rust " } else { "" },
            import.name
        );
        write_params(&mut text, &import.params);
        write!(text, "{}", import.result).unwrap();
        writeln!(signature, "    {text}").unwrap();
        signature.push_str("        effects { ");
        write_joined(&mut signature, &import.effects, ", ");
        signature.push_str(" }\n");
        let terminator = if import.native_rust { ";" } else { "" };
        match &import.failure {
            ImportFailure::Infallible => {
                writeln!(signature, "        failure infallible{terminator}").unwrap();
            }
            ImportFailure::Status { domain_id } => {
                signature.push_str("        failure status \"");
                write_escaped(&mut signature, domain_id);
                writeln!(signature, "\"{terminator}").unwrap();
            }
        }
        if !import.native_rust {
            writeln!(signature, "        consumes {} always;", import.consumes).unwrap();
        }
        members.push(Member {
            kind: "import",
            id: import.stable_id.clone(),
            name: import.name.clone(),
            persistent: import.explicit_id,
            text,
            location: Location::of(import.name_span),
        });
    }
    signature.push_str("}\n");
    let mut facts = Vec::new();
    push_fact(&mut facts, "Permits", interface.permits.clone());
    Entry {
        kind: "interface",
        id: interface.stable_id.clone(),
        name: interface.name.clone(),
        persistent: interface.explicit_id,
        description: description(placement, interface.span.start),
        signature,
        location: Location::of(interface.name_span),
        facts,
        members,
    }
}

fn protocol_entry(protocol: &crate::ast::ProtocolDeclaration, placement: &Placement) -> Entry {
    let mut signature = String::new();
    if protocol.explicit_id {
        write_id_line(&mut signature, &protocol.stable_id, "");
    }
    writeln!(signature, "protocol {} {{", protocol.name).unwrap();
    let mut members = Vec::new();
    for method in &protocol.methods {
        if method.explicit_id {
            write_id_line(&mut signature, &method.stable_id, "    ");
        }
        let mut text = format!("fn {}", method.name);
        write_params(&mut text, &method.params);
        write_type(&mut text, &method.return_type);
        text.push(';');
        writeln!(signature, "    {text}").unwrap();
        members.push(Member {
            kind: "protocol_method",
            id: method.stable_id.clone(),
            name: method.name.clone(),
            persistent: method.explicit_id,
            text,
            location: Location::of(method.name_span),
        });
    }
    signature.push_str("}\n");
    Entry {
        kind: "protocol",
        id: protocol.stable_id.clone(),
        name: protocol.name.clone(),
        persistent: protocol.explicit_id,
        description: description(placement, protocol.span.start),
        signature,
        location: Location::of(protocol.name_span),
        facts: Vec::new(),
        members,
    }
}

fn implementation_entry(
    implementation: &crate::ast::ProtocolImplementation,
    placement: &Placement,
) -> Entry {
    let mut signature = String::new();
    if implementation.explicit_id {
        write_id_line(&mut signature, &implementation.stable_id, "");
    }
    signature.push_str("impl \"");
    write_escaped(&mut signature, &implementation.protocol_id);
    signature.push_str("\" for \"");
    write_escaped(&mut signature, &implementation.receiver_id);
    signature.push_str("\" {\n");
    let mut members = Vec::new();
    for member in &implementation.members {
        let mut text = String::from("\"");
        write_escaped(&mut text, &member.method_id);
        text.push_str("\" = \"");
        write_escaped(&mut text, &member.function_id);
        text.push_str("\";");
        writeln!(signature, "    {text}").unwrap();
        members.push(Member {
            kind: "binding",
            id: member.function_id.clone(),
            name: member.method_id.clone(),
            persistent: true,
            text,
            location: Location::of(member.span),
        });
    }
    signature.push_str("}\n");
    Entry {
        kind: "implementation",
        id: implementation.stable_id.clone(),
        name: format!(
            "{} for {}",
            implementation.protocol_id, implementation.receiver_id
        ),
        persistent: implementation.explicit_id,
        description: description(placement, implementation.span.start),
        signature: signature.clone(),
        location: Location::of(implementation.span),
        facts: vec![
            Fact {
                label: "Protocol",
                values: vec![implementation.protocol_id.clone()],
            },
            Fact {
                label: "Receiver",
                values: vec![implementation.receiver_id.clone()],
            },
        ],
        members,
    }
}

/// Section heading for one entry kind, in canonical declaration order.
const SECTIONS: &[(&str, &str)] = &[
    ("record", "Records"),
    ("variant", "Variants"),
    ("class", "Classes"),
    ("method", "Methods"),
    ("resource", "Resources"),
    ("interface", "Interfaces"),
    ("protocol", "Protocols"),
    ("implementation", "Implementations"),
    ("function", "Functions"),
];

fn member_heading(kind: &str) -> &'static str {
    match kind {
        "field" => "Fields",
        "case" => "Cases",
        "case_field" => "Case fields",
        "drop" => "Lifecycles",
        "import" => "Imports",
        "protocol_method" => "Methods",
        "binding" => "Bindings",
        _ => "Members",
    }
}

fn code(text: &str) -> String {
    format!("`{text}`")
}

fn render_markdown(document: &Document) -> String {
    let mut output = String::new();
    writeln!(output, "# Module `{}`\n", document.module).unwrap();
    output.push_str(
        "Generated by `semaprax doc` from the checked module. Signatures, identities,\neffects, and contracts are the graph's own facts; descriptions are the leading\ncomments of each declaration.\n\n",
    );
    writeln!(output, "- Graph revision: `{}`", document.revision).unwrap();
    if !document.permits.is_empty() {
        writeln!(
            output,
            "- Permits: {}",
            document
                .permits
                .iter()
                .map(|permit| code(permit))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    for module_use in &document.uses {
        writeln!(
            output,
            "- Uses {} `{}` from `{}` as `{}`",
            module_use.kind, module_use.id, module_use.module, module_use.alias
        )
        .unwrap();
    }
    output.push('\n');
    for (kind, heading) in SECTIONS {
        let entries: Vec<_> = document
            .entries
            .iter()
            .filter(|entry| entry.kind == *kind)
            .collect();
        if entries.is_empty() {
            continue;
        }
        writeln!(output, "## {heading}\n").unwrap();
        for entry in entries {
            writeln!(output, "### `{}`\n", entry.name).unwrap();
            if !entry.description.is_empty() {
                for line in &entry.description {
                    writeln!(output, "{line}").unwrap();
                }
                output.push('\n');
            }
            writeln!(output, "```spx\n{}```\n", entry.signature).unwrap();
            writeln!(
                output,
                "- Identity: `{}` ({})",
                entry.id,
                if entry.persistent {
                    "persistent"
                } else {
                    "automatic, revision-scoped"
                }
            )
            .unwrap();
            for fact in &entry.facts {
                writeln!(
                    output,
                    "- {}: {}",
                    fact.label,
                    fact.values
                        .iter()
                        .map(|value| code(value))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .unwrap();
            }
            let mut heading = None;
            for member in &entry.members {
                let label = member_heading(member.kind);
                if heading != Some(label) {
                    writeln!(output, "- {label}:").unwrap();
                    heading = Some(label);
                }
                write!(output, "  - `{}`", member.text).unwrap();
                if member.id.is_empty() {
                    output.push('\n');
                } else {
                    writeln!(
                        output,
                        " — `{}`{}",
                        member.id,
                        if member.persistent {
                            ""
                        } else {
                            " (automatic)"
                        }
                    )
                    .unwrap();
                }
            }
            output.push('\n');
        }
    }
    output
}

fn json_strings(values: &[String]) -> String {
    let mut output = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(value));
    }
    output.push(']');
    output
}

fn render_json(document: &Document) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"module\":{},\"revision\":{},\"permits\":{},\"uses\":[",
        quote_json(SCHEMA_V1),
        quote_json(&document.module),
        quote_json(&document.revision),
        json_strings(&document.permits)
    );
    for (index, module_use) in document.uses.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(
            output,
            "{{\"kind\":{},\"id\":{},\"module\":{},\"alias\":{}}}",
            quote_json(module_use.kind),
            quote_json(&module_use.id),
            quote_json(&module_use.module),
            quote_json(&module_use.alias)
        )
        .unwrap();
    }
    output.push_str("],\"declarations\":[");
    for (index, entry) in document.entries.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(
            output,
            "{{\"kind\":{},\"id\":{},\"name\":{},\"persistent\":{},\"description\":{},\"signature\":{},\"location\":{},\"facts\":[",
            quote_json(entry.kind),
            quote_json(&entry.id),
            quote_json(&entry.name),
            entry.persistent,
            json_strings(&entry.description),
            quote_json(&entry.signature),
            entry.location.json()
        )
        .unwrap();
        for (index, fact) in entry.facts.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write!(
                output,
                "{{\"label\":{},\"values\":{}}}",
                quote_json(fact.label),
                json_strings(&fact.values)
            )
            .unwrap();
        }
        output.push_str("],\"members\":[");
        for (index, member) in entry.members.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write!(
                output,
                "{{\"kind\":{},\"id\":{},\"name\":{},\"persistent\":{},\"text\":{},\"location\":{}}}",
                quote_json(member.kind),
                quote_json(&member.id),
                quote_json(&member.name),
                member.persistent,
                quote_json(&member.text),
                member.location.json()
            )
            .unwrap();
        }
        output.push_str("]}");
    }
    output.push_str("]}\n");
    output
}
