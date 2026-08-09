//! Deterministic offline economics for checked agent-context maintenance cases.
//!
//! The lexical-token counter is deliberately not a model tokenizer. It exists
//! only as a stable repository-owned unit alongside exact UTF-8 bytes and
//! emitted function-node counts.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph::{self, AgentContextFilter, AgentContextOptions};
use crate::parse;

const MANIFEST_SCHEMA: &str = "semaprax.agent-context-benchmark.v1";
const OUTPUT_SCHEMA: &str = "semaprax.agent-context-economics.v1";
const TOKEN_SCHEMA: &str = "semaprax.lexical-token.v1";

#[derive(Clone)]
struct Case {
    id: String,
    question: String,
    source: String,
    root: String,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    filters: Vec<AgentContextFilter>,
    relevant: Vec<String>,
    evidence: Vec<String>,
}

/// Run a checked-in, offline benchmark manifest and return canonical JSON.
pub fn benchmark_manifest(path: &Path) -> Result<String, Diagnostic> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        benchmark_error(format!("cannot read benchmark {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(benchmark_error(format!(
            "benchmark manifest {} must be a regular non-symlink file",
            path.display()
        )));
    }
    let manifest_bytes = std::fs::read(path).map_err(|error| {
        benchmark_error(format!("cannot read benchmark {}: {error}", path.display()))
    })?;
    let manifest = std::str::from_utf8(&manifest_bytes)
        .map_err(|_| benchmark_error("benchmark manifest is not valid UTF-8"))?;
    let cases = parse_manifest(manifest)?;
    let canonical_manifest = path.canonicalize().map_err(|error| {
        benchmark_error(format!(
            "cannot canonicalize benchmark {}: {error}",
            path.display()
        ))
    })?;
    let parent = canonical_manifest
        .parent()
        .ok_or_else(|| benchmark_error("benchmark manifest has no canonical parent"))?;
    let mut rendered = Vec::with_capacity(cases.len());
    let mut aggregate = Aggregate::default();
    for case in cases {
        let source_path = canonical_source(parent, &case.source)?;
        let source = std::fs::read_to_string(&source_path).map_err(|error| {
            benchmark_error(format!(
                "cannot read benchmark source {}: {error}",
                source_path.display()
            ))
        })?;
        let program = parse(&source, &source_path)
            .map_err(|error| benchmark_error(format!("benchmark case `{}`: {error}", case.id)))?;
        let options = AgentContextOptions::new(
            case.depth,
            case.max_bytes,
            case.max_nodes,
            case.filters.iter().copied(),
        )
        .map_err(|error| benchmark_error(format!("benchmark case `{}`: {error}", case.id)))?;
        let label_options = AgentContextOptions::new(
            0,
            graph::MAX_AGENT_CONTEXT_BYTES,
            1,
            [AgentContextFilter::Effects],
        )
        .expect("closed benchmark label options are valid");
        for id in &case.relevant {
            if graph::agent_context_json(&program, id, &label_options)
                .map_err(|errors| {
                    benchmark_error(format!(
                        "benchmark case `{}` label `{id}`: {}",
                        case.id,
                        errors
                            .first()
                            .map_or("context failed", |error| error.message.as_str())
                    ))
                })?
                .is_none()
            {
                return Err(benchmark_error(format!(
                    "benchmark case `{}` relevant ID `{id}` was not found",
                    case.id
                )));
            }
        }
        let context = graph::agent_context_json(&program, &case.root, &options)
            .map_err(|errors| {
                benchmark_error(format!(
                    "benchmark case `{}`: {}",
                    case.id,
                    errors
                        .first()
                        .map_or("context failed", |error| error.message.as_str())
                ))
            })?
            .ok_or_else(|| {
                benchmark_error(format!(
                    "benchmark case `{}` root `{}` was not found",
                    case.id, case.root
                ))
            })?;
        let facts = context
            .split_once("\"facts\":[")
            .map(|(_, facts)| facts)
            .ok_or_else(|| benchmark_error("context output has no canonical facts array"))?;
        let emitted_nodes = json_number(&context, "\"used_nodes\":")?;
        let relevance_hits = case
            .relevant
            .iter()
            .filter(|id| fact_is_emitted(facts, id))
            .count();
        let evidence_hits = case
            .evidence
            .iter()
            .filter(|id| fact_is_emitted(facts, id))
            .count();
        let source_tokens = lexical_tokens(&source);
        let question_tokens = lexical_tokens(&case.question);
        let context_tokens = lexical_tokens(&context);
        aggregate.source_bytes += source.len();
        aggregate.source_tokens += source_tokens;
        aggregate.question_bytes += case.question.len();
        aggregate.question_tokens += question_tokens;
        aggregate.context_bytes += context.len();
        aggregate.context_tokens += context_tokens;
        aggregate.context_nodes += emitted_nodes;
        aggregate.relevance_hits += relevance_hits;
        aggregate.relevance_total += emitted_nodes;
        aggregate.evidence_hits += evidence_hits;
        aggregate.evidence_total += case.evidence.len();
        rendered.push(format!(
            "{{\"id\":{},\"question\":{},\"source\":{},\"root\":{},\"revision\":{},\"query\":{{\"depth\":{},\"max_bytes\":{},\"max_nodes\":{},\"filters\":[{}]}},\"input\":{{\"source_bytes\":{},\"source_lexical_tokens\":{},\"question_bytes\":{},\"question_lexical_tokens\":{}}},\"context\":{{\"sha256\":{},\"bytes\":{},\"lexical_tokens\":{},\"nodes\":{}}},\"labels\":{{\"relevant_ids\":[{}],\"evidence_ids\":[{}]}},\"score\":{{\"relevance\":{{\"hits\":{},\"emitted\":{},\"ratio\":{}}},\"evidence_recall\":{{\"hits\":{},\"expected\":{},\"ratio\":{}}}}}}}",
            quote_json(&case.id),
            quote_json(&case.question),
            quote_json(&case.source),
            quote_json(&case.root),
            quote_json(&graph::revision(&program)),
            case.depth,
            case.max_bytes,
            case.max_nodes,
            case.filters
                .iter()
                .map(|filter| quote_json(filter.name()))
                .collect::<Vec<_>>()
                .join(","),
            source.len(),
            source_tokens,
            case.question.len(),
            question_tokens,
            quote_json(&sha256(context.as_bytes())),
            context.len(),
            context_tokens,
            emitted_nodes,
            case.relevant
                .iter()
                .map(|id| quote_json(id))
                .collect::<Vec<_>>()
                .join(","),
            case.evidence
                .iter()
                .map(|id| quote_json(id))
                .collect::<Vec<_>>()
                .join(","),
            relevance_hits,
            emitted_nodes,
            quote_json(&ratio(relevance_hits, emitted_nodes)),
            evidence_hits,
            case.evidence.len(),
            quote_json(&ratio(evidence_hits, case.evidence.len())),
        ));
    }
    Ok(format!(
        "{{\"schema\":{schema},\"manifest\":{{\"schema\":{manifest},\"sha256\":{manifest_digest}}},\"token_unit\":{{\"schema\":{token},\"model_tokens\":false}},\"cases\":[{cases}],\"aggregate\":{{\"questions\":{questions},\"input\":{{\"source_bytes\":{source_bytes},\"source_lexical_tokens\":{source_tokens},\"question_bytes\":{question_bytes},\"question_lexical_tokens\":{question_tokens}}},\"context\":{{\"bytes\":{context_bytes},\"lexical_tokens\":{context_tokens},\"nodes\":{context_nodes},\"to_source_bytes\":{byte_ratio},\"to_source_lexical_tokens\":{token_ratio}}},\"score\":{{\"relevance\":{{\"hits\":{relevance_hits},\"emitted\":{relevance_total},\"ratio\":{relevance_ratio}}},\"evidence_recall\":{{\"hits\":{evidence_hits},\"expected\":{evidence_total},\"ratio\":{evidence_ratio}}}}}}}}}",
        schema = quote_json(OUTPUT_SCHEMA),
        manifest = quote_json(MANIFEST_SCHEMA),
        manifest_digest = quote_json(&sha256(&manifest_bytes)),
        token = quote_json(TOKEN_SCHEMA),
        cases = rendered.join(","),
        questions = rendered.len(),
        source_bytes = aggregate.source_bytes,
        source_tokens = aggregate.source_tokens,
        question_bytes = aggregate.question_bytes,
        question_tokens = aggregate.question_tokens,
        context_bytes = aggregate.context_bytes,
        context_tokens = aggregate.context_tokens,
        context_nodes = aggregate.context_nodes,
        byte_ratio = quote_json(&ratio(aggregate.context_bytes, aggregate.source_bytes)),
        token_ratio = quote_json(&ratio(aggregate.context_tokens, aggregate.source_tokens)),
        relevance_hits = aggregate.relevance_hits,
        relevance_total = aggregate.relevance_total,
        relevance_ratio = quote_json(&ratio(aggregate.relevance_hits, aggregate.relevance_total)),
        evidence_hits = aggregate.evidence_hits,
        evidence_total = aggregate.evidence_total,
        evidence_ratio = quote_json(&ratio(aggregate.evidence_hits, aggregate.evidence_total)),
    ))
}

/// Count repository-defined lexical units, not model tokens.
#[must_use]
pub fn lexical_tokens(text: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if !character.is_whitespace() {
                count += 1;
            }
        }
    }
    count
}

#[derive(Default)]
struct Aggregate {
    source_bytes: usize,
    source_tokens: usize,
    question_bytes: usize,
    question_tokens: usize,
    context_bytes: usize,
    context_tokens: usize,
    context_nodes: usize,
    relevance_hits: usize,
    relevance_total: usize,
    evidence_hits: usize,
    evidence_total: usize,
}

fn parse_manifest(manifest: &str) -> Result<Vec<Case>, Diagnostic> {
    let mut lines = manifest.lines();
    if lines.next() != Some(&format!("schema\t{MANIFEST_SCHEMA}")) {
        return Err(benchmark_error(format!(
            "benchmark manifest must start with `schema\\t{MANIFEST_SCHEMA}`"
        )));
    }
    let mut cases = Vec::new();
    let mut ids = BTreeSet::new();
    for (offset, line) in lines.enumerate() {
        let line_number = offset + 2;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 10 || fields.iter().any(|field| field.is_empty()) {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line_number} requires 10 nonempty tab-separated fields"
            )));
        }
        if fields
            .iter()
            .any(|field| field.chars().any(char::is_control))
        {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line_number} contains a control character"
            )));
        }
        if !ids.insert(fields[0].to_owned()) {
            return Err(benchmark_error(format!(
                "benchmark case `{}` is duplicated",
                fields[0]
            )));
        }
        validate_relative_source(fields[2], line_number)?;
        let filters = parse_filters(fields[7], line_number)?;
        let relevant = parse_ids(fields[8], "relevant", line_number)?;
        let evidence = parse_ids(fields[9], "evidence", line_number)?;
        if evidence.iter().any(|id| !relevant.contains(id)) {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line_number} evidence must be a subset of relevant IDs"
            )));
        }
        cases.push(Case {
            id: fields[0].to_owned(),
            question: fields[1].to_owned(),
            source: fields[2].to_owned(),
            root: fields[3].to_owned(),
            depth: canonical_usize(fields[4], "depth", line_number)?,
            max_bytes: canonical_usize(fields[5], "max_bytes", line_number)?,
            max_nodes: canonical_usize(fields[6], "max_nodes", line_number)?,
            filters,
            relevant,
            evidence,
        });
    }
    if cases.is_empty() {
        return Err(benchmark_error("benchmark manifest has no cases"));
    }
    Ok(cases)
}

fn validate_relative_source(source: &str, line: usize) -> Result<(), Diagnostic> {
    let path = Path::new(source);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || source.contains('\\')
        || source.split('/').any(str::is_empty)
    {
        return Err(benchmark_error(format!(
            "benchmark manifest line {line} source must stay below the manifest directory"
        )));
    }
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line} source must use canonical components"
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            benchmark_error(format!(
                "benchmark manifest line {line} source must be valid UTF-8"
            ))
        })?;
        if !portable_windows_component(component) {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line} source is not portable across Windows filesystems"
            )));
        }
    }
    Ok(())
}

fn portable_windows_component(component: &str) -> bool {
    if component.ends_with('.')
        || component.ends_with(' ')
        || component.contains(':')
        || component
            .bytes()
            .any(|byte| matches!(byte, b'<' | b'>' | b'"' | b'|' | b'?' | b'*'))
    {
        return false;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return false;
    }
    for prefix in ["COM", "LPT"] {
        let Some(suffix) = stem.strip_prefix(prefix) else {
            continue;
        };
        let ascii_device = suffix.len() == 1 && (b'1'..=b'9').contains(&suffix.as_bytes()[0]);
        if ascii_device || matches!(suffix, "¹" | "²" | "³") {
            return false;
        }
    }
    true
}

fn canonical_source(parent: &Path, source: &str) -> Result<PathBuf, Diagnostic> {
    let mut candidate = parent.to_path_buf();
    for component in Path::new(source).components() {
        let Component::Normal(component) = component else {
            return Err(benchmark_error(
                "benchmark source alias is not a canonical relative path",
            ));
        };
        let exact_entry = std::fs::read_dir(&candidate)
            .map_err(|error| {
                benchmark_error(format!(
                    "cannot inspect benchmark source {}: {error}",
                    candidate.display()
                ))
            })?
            .filter_map(Result::ok)
            .any(|entry| entry.file_name() == component);
        candidate.push(component);
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
            benchmark_error(format!(
                "cannot resolve benchmark source {}: {error}",
                candidate.display()
            ))
        })?;
        if !exact_entry {
            return Err(benchmark_error(format!(
                "benchmark source {} does not use exact directory-entry spelling",
                candidate.display()
            )));
        }
        if metadata.file_type().is_symlink() {
            return Err(benchmark_error(format!(
                "benchmark source {} traverses a symlink",
                candidate.display()
            )));
        }
    }
    let metadata = std::fs::metadata(&candidate).map_err(|error| {
        benchmark_error(format!(
            "cannot read benchmark source {}: {error}",
            candidate.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(benchmark_error(format!(
            "benchmark source {} must be a regular file",
            candidate.display()
        )));
    }
    let canonical = candidate.canonicalize().map_err(|error| {
        benchmark_error(format!(
            "cannot canonicalize benchmark source {}: {error}",
            candidate.display()
        ))
    })?;
    if canonical.strip_prefix(parent).is_err() {
        return Err(benchmark_error(format!(
            "benchmark source {} escapes the manifest directory",
            candidate.display()
        )));
    }
    Ok(canonical)
}

fn parse_filters(value: &str, line: usize) -> Result<Vec<AgentContextFilter>, Diagnostic> {
    let mut filters = Vec::new();
    let mut seen = BTreeSet::new();
    for name in value.split(',') {
        let filter = AgentContextFilter::from_name(name).ok_or_else(|| {
            benchmark_error(format!(
                "benchmark manifest line {line} has unknown filter `{name}`"
            ))
        })?;
        if !matches!(
            filter,
            AgentContextFilter::Contracts
                | AgentContextFilter::Ownership
                | AgentContextFilter::Effects
                | AgentContextFilter::Types
        ) {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line} cannot score unavailable Graph v6 filter `{name}`"
            )));
        }
        if !seen.insert(filter) {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line} duplicates filter `{name}`"
            )));
        }
        filters.push(filter);
    }
    Ok(filters)
}

fn parse_ids(value: &str, label: &str, line: usize) -> Result<Vec<String>, Diagnostic> {
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    for id in value.split(',') {
        if id.is_empty() || !seen.insert(id) {
            return Err(benchmark_error(format!(
                "benchmark manifest line {line} has empty or duplicate {label} ID"
            )));
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}

fn canonical_usize(value: &str, label: &str, line: usize) -> Result<usize, Diagnostic> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(benchmark_error(format!(
            "benchmark manifest line {line} {label} is not a canonical integer"
        )));
    }
    value.parse().map_err(|_| {
        benchmark_error(format!(
            "benchmark manifest line {line} {label} is outside usize"
        ))
    })
}

fn json_number(json: &str, marker: &str) -> Result<usize, Diagnostic> {
    let start = json
        .find(marker)
        .map(|start| start + marker.len())
        .ok_or_else(|| benchmark_error(format!("context output has no `{marker}`")))?;
    let end = json[start..]
        .find(|character: char| !character.is_ascii_digit())
        .map_or(json.len(), |offset| start + offset);
    json[start..end]
        .parse()
        .map_err(|_| benchmark_error(format!("context output has malformed `{marker}`")))
}

fn fact_is_emitted(facts: &str, id: &str) -> bool {
    facts.contains(&format!("\"id\":{},\"kind\":\"function\"", quote_json(id)))
}

fn ratio(numerator: usize, denominator: usize) -> String {
    if denominator == 0 {
        return "0/0".to_owned();
    }
    let divisor = gcd(numerator, denominator);
    format!("{}/{}", numerator / divisor, denominator / divisor)
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.max(1)
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::from("sha256:");
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn benchmark_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error("SPX-G005", message.into(), crate::ast::Span::default())
}

#[cfg(test)]
mod tests {
    use super::lexical_tokens;

    #[test]
    fn lexical_unit_is_closed_and_deterministic() {
        assert_eq!(lexical_tokens("alpha_beta + 42\nγ"), 4);
        assert_eq!(lexical_tokens("{\"x\":true}"), 7);
    }
}
