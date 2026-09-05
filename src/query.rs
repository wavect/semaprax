//! Declaration Query v1: bounded search over one checked module's declarations
//! with call-graph predicates.
//!
//! `semaprax query <file> [filters] [--json]` selects declarations from the
//! same model `semaprax doc` renders ([`crate::doc`]), so every match carries
//! the declaration's identity, canonical signature, and facts, and the result
//! names the graph revision of the module. Call predicates come from the
//! persistent call index the graph and impact analyses use. The query is
//! read-only and deterministic; a filter that names an unknown kind or
//! declaration fails closed instead of matching nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::ast::Program;
use crate::call_index::PersistentCallIndex;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::doc::{self, Entry};
use crate::format::comments::Comments;
use crate::hir::{self, DeclarationId};

/// Schema of the JSON result.
pub const SCHEMA_V1: &str = "semaprax.query.v1";

/// The declaration kinds a `--kind` filter may name, in canonical order.
pub const KINDS: &[&str] = &[
    "record",
    "variant",
    "class",
    "method",
    "resource",
    "interface",
    "protocol",
    "implementation",
    "function",
];

/// The conjunction of filters one query applies. Every set field must hold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueryFilters {
    /// Admitted kinds; empty admits every kind.
    pub kinds: Vec<String>,
    /// Substring of the display name.
    pub name: Option<String>,
    /// Prefix of the stable identity.
    pub id_prefix: Option<String>,
    /// An effect the declaration uses.
    pub effect: Option<String>,
    /// Match callables that call this declaration.
    pub calls: Option<String>,
    /// Match callables that this declaration calls.
    pub called_by: Option<String>,
}

/// One matching declaration with its call facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    pub entry: Entry,
    /// Persistent callables this declaration calls, sorted.
    pub calls: Vec<String>,
    /// Persistent callables that call this declaration, sorted.
    pub called_by: Vec<String>,
}

/// The result of one query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryResult {
    pub module: String,
    pub revision: String,
    pub filters: QueryFilters,
    pub matches: Vec<Match>,
}

fn effects(entry: &Entry) -> &[String] {
    entry
        .facts
        .iter()
        .find(|fact| fact.label == "Effects")
        .map_or(&[], |fact| fact.values.as_slice())
}

fn ids(set: Option<&BTreeSet<DeclarationId>>) -> Vec<String> {
    set.map(|ids| ids.iter().map(|id| id.as_str().to_owned()).collect())
        .unwrap_or_default()
}

/// Run the query. The program must already be verified.
pub fn run(
    program: &Program,
    comments: &Comments,
    filters: &QueryFilters,
) -> Result<QueryResult, Vec<Diagnostic>> {
    for kind in &filters.kinds {
        if !KINDS.contains(&kind.as_str()) {
            return Err(vec![Diagnostic::io(
                "SPX-V211",
                format!(
                    "unknown declaration kind `{kind}`; admitted: {}",
                    KINDS.join(", ")
                ),
            )]);
        }
    }
    let document = doc::document(program, comments);
    let known: BTreeSet<&str> = document
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    for target in [&filters.calls, &filters.called_by].into_iter().flatten() {
        if !known.contains(target.as_str()) {
            return Err(vec![Diagnostic::io(
                "SPX-V212",
                format!(
                    "`{target}` is not a declaration of module `{}`",
                    document.module
                ),
            )]);
        }
    }
    let resolved = hir::resolve(program)?;
    let index = PersistentCallIndex::build(&resolved).map_err(|error| vec![error])?;
    let calls_by_owner: &BTreeMap<_, _> = index.calls_by_owner();
    let callers_by_callee: &BTreeMap<_, _> = index.callers_by_callee();

    let mut matches = Vec::new();
    for entry in &document.entries {
        let id = DeclarationId::new(entry.id.clone());
        let calls = ids(calls_by_owner.get(&id));
        let called_by = ids(callers_by_callee.get(&id));
        let admitted = (filters.kinds.is_empty()
            || filters.kinds.iter().any(|kind| kind == entry.kind))
            && filters
                .name
                .as_ref()
                .is_none_or(|needle| entry.name.contains(needle.as_str()))
            && filters
                .id_prefix
                .as_ref()
                .is_none_or(|prefix| entry.id.starts_with(prefix.as_str()))
            && filters
                .effect
                .as_ref()
                .is_none_or(|effect| effects(entry).contains(effect))
            && filters
                .calls
                .as_ref()
                .is_none_or(|target| calls.contains(target))
            && filters
                .called_by
                .as_ref()
                .is_none_or(|target| called_by.contains(target));
        if admitted {
            matches.push(Match {
                entry: entry.clone(),
                calls,
                called_by,
            });
        }
    }
    Ok(QueryResult {
        module: document.module,
        revision: document.revision,
        filters: filters.clone(),
        matches,
    })
}

/// The first signature line that is not an `@id` attribute.
fn header(entry: &Entry) -> &str {
    entry
        .signature
        .lines()
        .find(|line| !line.trim_start().starts_with("@id("))
        .unwrap_or("")
}

/// One tab-separated line per match: kind, identity, header.
#[must_use]
pub fn text(result: &QueryResult) -> String {
    let mut output = String::new();
    for found in &result.matches {
        writeln!(
            output,
            "{}\t{}\t{}",
            found.entry.kind,
            found.entry.id,
            header(&found.entry)
        )
        .unwrap();
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

fn json_option(value: Option<&String>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| quote_json(value))
}

/// The one-line `semaprax.query.v1` result.
#[must_use]
pub fn json(result: &QueryResult) -> String {
    let filters = &result.filters;
    let mut output = format!(
        "{{\"schema\":{},\"module\":{},\"revision\":{},\"filters\":{{\"kinds\":{},\"name\":{},\"id_prefix\":{},\"effect\":{},\"calls\":{},\"called_by\":{}}},\"matches\":[",
        quote_json(SCHEMA_V1),
        quote_json(&result.module),
        quote_json(&result.revision),
        json_strings(&filters.kinds),
        json_option(filters.name.as_ref()),
        json_option(filters.id_prefix.as_ref()),
        json_option(filters.effect.as_ref()),
        json_option(filters.calls.as_ref()),
        json_option(filters.called_by.as_ref()),
    );
    for (index, found) in result.matches.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(
            output,
            "{{\"kind\":{},\"id\":{},\"name\":{},\"persistent\":{},\"signature\":{},\"location\":{{\"line\":{},\"column\":{},\"start\":{},\"end\":{}}},\"effects\":{},\"calls\":{},\"called_by\":{}}}",
            quote_json(found.entry.kind),
            quote_json(&found.entry.id),
            quote_json(&found.entry.name),
            found.entry.persistent,
            quote_json(&found.entry.signature),
            found.entry.location.line,
            found.entry.location.column,
            found.entry.location.start,
            found.entry.location.end,
            json_strings(effects(&found.entry)),
            json_strings(&found.calls),
            json_strings(&found.called_by)
        )
        .unwrap();
    }
    output.push_str("]}\n");
    output
}
