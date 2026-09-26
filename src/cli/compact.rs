//! Compact projections are read-only views; replay always regenerates selection.
use semaprax::compact_semantic_projection::{
    self as compact, CompactProjection, ProjectionSelection, ProjectionSource,
};
use semaprax::diagnostic::Diagnostic;
use semaprax::graph::{AgentContextDirection, AgentContextFilter, AgentContextV2Options};
use semaprax::project::{with_authenticated_project, ProjectCandidate};
use semaprax::semantic_task_context::{CompilationBudget, CompilationGoal, CompilationSeed};
use std::path::{Path, PathBuf};

/// Local parser bounds for the structured task-context CLI input. These keep
/// malformed or hostile argument collections bounded before source loading;
/// semantic selection and token accounting remain owned by the library.
const TASK_MAX_SEEDS: usize = 32;
const TASK_MAX_FIELD_BYTES: usize = 4096;
const TASK_MAX_REVISION_BYTES: usize = 256;
const TASK_MAX_TOKENIZER_BYTES: usize = 64;
const TASK_MAX_CONTEXT_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct Options {
    profile: String,
    input: PathBuf,
    selection: Option<String>,
    encoding: compact::negotiation::ProjectionEncoding,
    replay: Option<PathBuf>,
    max_bytes: usize,
    max_tokens: usize,
    task_seeds: Vec<TaskSeed>,
    task_revision: Option<String>,
    task_tokenizer: String,
}

struct TaskSeed {
    id: String,
    priority: u32,
    reason: String,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, u8> {
    parse_inner(args).map_err(|message| {
        eprintln!("compact: {message}");
        2
    })
}
fn parse_inner(args: &[String]) -> Result<Options, &'static str> {
    let [profile, input, rest @ ..] = args else {
        return Err("requires <profile> <input> [selection] [options]");
    };
    let needs_selection = match profile.as_str() {
        "context" | "task-context" | "candidate-diff" => true,
        "graph" | "api-surface" | "agent-definition" => false,
        _ => return Err("unknown profile"),
    };
    if input.is_empty() || input.starts_with('-') {
        return Err("input must be an explicit path");
    }
    let (selection, rest) = if needs_selection {
        let [selection, rest @ ..] = rest else {
            return Err("profile requires a stable ID or candidate capsule path");
        };
        if selection.is_empty() || selection.starts_with('-') {
            return Err("missing profile selection");
        }
        if profile == "task-context" && selection.len() > TASK_MAX_FIELD_BYTES {
            return Err("task-context root seed id exceeds 4096 bytes");
        }
        (Some(selection.clone()), rest)
    } else {
        (None, rest)
    };
    let mut options = Options {
        profile: profile.clone(),
        input: input.into(),
        selection,
        encoding: compact::negotiation::ProjectionEncoding::Text,
        replay: None,
        max_bytes: 65_536,
        max_tokens: 65_536,
        task_seeds: Vec::new(),
        task_revision: None,
        task_tokenizer: "byte-v1".to_owned(),
    };
    if profile == "task-context" {
        return parse_task_context_options(options, rest);
    }
    let mut seen = std::collections::BTreeSet::new();
    let (chunks, remainder) = rest.as_chunks::<2>();
    for pair in chunks {
        if !seen.insert(pair[0].as_str()) {
            return Err("duplicate option");
        }
        match pair[0].as_str() {
            "--encoding" => {
                options.encoding = match pair[1].as_str() {
                    "text" => compact::negotiation::ProjectionEncoding::Text,
                    "binary" => compact::negotiation::ProjectionEncoding::Binary,
                    "model-text" => compact::negotiation::ProjectionEncoding::ModelText,
                    _ => return Err("encoding must be text, binary or model-text"),
                }
            }
            "--replay" if !pair[1].is_empty() && !pair[1].starts_with('-') => {
                options.replay = Some((&pair[1]).into())
            }
            "--max-bytes" if matches!(profile.as_str(), "context" | "task-context") => {
                options.max_bytes = pair[1].parse().map_err(|_| "invalid byte limit")?
            }
            "--max-tokens" if profile == "task-context" => {
                options.max_tokens = pair[1].parse().map_err(|_| "invalid token limit")?
            }
            _ => return Err("unknown or inapplicable option"),
        }
    }
    if !remainder.is_empty() {
        return Err("option requires a value");
    }
    if matches!(profile.as_str(), "graph" | "api-surface" | "candidate-diff") {
        options.input = super::project::resolve_positional(options.input);
    }
    Ok(options)
}

fn parse_task_context_options(
    mut options: Options,
    rest: &[String],
) -> Result<Options, &'static str> {
    let root = options
        .selection
        .take()
        .expect("task-context selection was validated by parse_inner");
    let mut root_priority = 1u32;
    let mut root_reason = "explicit CLI task root".to_owned();
    let mut active_seed: Option<usize> = None;
    let mut priority_seen = false;
    let mut reason_seen = false;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 0;
    while index < rest.len() {
        let option = rest[index].as_str();
        let value = rest.get(index + 1).ok_or("option requires a value")?;
        if value.is_empty() || value.starts_with('-') {
            return Err("option requires a nonempty value");
        }
        match option {
            "--seed" => {
                if options.task_seeds.len() >= TASK_MAX_SEEDS - 1 {
                    return Err("task-context accepts at most 32 total seeds");
                }
                if value.len() > TASK_MAX_FIELD_BYTES {
                    return Err("task-context seed id exceeds 4096 bytes");
                }
                options.task_seeds.push(TaskSeed {
                    id: value.clone(),
                    priority: 0,
                    reason: "explicit CLI task seed".to_owned(),
                });
                active_seed = Some(options.task_seeds.len() - 1);
                priority_seen = false;
                reason_seen = false;
            }
            "--priority" => {
                if priority_seen {
                    return Err("duplicate task-context option `--priority` for seed");
                }
                let priority = canonical_u32(value).ok_or("invalid task-context priority")?;
                if let Some(seed) = active_seed {
                    options.task_seeds[seed].priority = priority;
                } else {
                    root_priority = priority;
                }
                priority_seen = true;
            }
            "--reason" => {
                if reason_seen {
                    return Err("duplicate task-context option `--reason` for seed");
                }
                if value.len() > TASK_MAX_FIELD_BYTES {
                    return Err("task-context reason exceeds 4096 bytes");
                }
                if let Some(seed) = active_seed {
                    options.task_seeds[seed].reason = value.clone();
                } else {
                    root_reason = value.clone();
                }
                reason_seen = true;
            }
            "--goal" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                if value.len() > TASK_MAX_FIELD_BYTES {
                    return Err("task-context goal exceeds 4096 bytes");
                }
                // The library's structured goal carries opaque reason data on
                // each seed. Preserve the optional natural-language goal as
                // that root seed's reason; it remains untrusted and has no
                // role in resolution or budget selection.
                root_reason = value.clone();
            }
            "--revision" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                if value.len() > TASK_MAX_REVISION_BYTES {
                    return Err("task-context revision exceeds 256 bytes");
                }
                options.task_revision = Some(value.clone());
            }
            "--tokenizer" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                if value.len() > TASK_MAX_TOKENIZER_BYTES {
                    return Err("task-context tokenizer exceeds 64 bytes");
                }
                options.task_tokenizer = value.clone();
            }
            "--max-bytes" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                options.max_bytes = canonical_usize(value).ok_or("invalid byte limit")?;
            }
            "--max-tokens" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                options.max_tokens = canonical_usize(value).ok_or("invalid token limit")?;
            }
            "--encoding" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                options.encoding = match value.as_str() {
                    "text" => compact::negotiation::ProjectionEncoding::Text,
                    "binary" => compact::negotiation::ProjectionEncoding::Binary,
                    "model-text" => compact::negotiation::ProjectionEncoding::ModelText,
                    _ => return Err("encoding must be text, binary or model-text"),
                };
            }
            "--replay" => {
                if !seen.insert(option) {
                    return Err("duplicate option");
                }
                options.replay = Some(value.into());
            }
            _ => return Err("unknown or inapplicable option"),
        }
        index += 2;
    }
    let mut seeds = Vec::with_capacity(1 + options.task_seeds.len());
    seeds.push(TaskSeed {
        id: root,
        priority: root_priority,
        reason: root_reason,
    });
    seeds.append(&mut options.task_seeds);
    options.task_seeds = seeds;
    if options
        .max_bytes
        .checked_mul(options.task_seeds.len())
        .is_none_or(|bytes| bytes > TASK_MAX_CONTEXT_BYTES)
    {
        return Err("task-context aggregate per-seed byte capacity exceeds 8 MiB");
    }
    Ok(options)
}

fn canonical_usize(value: &str) -> Option<usize> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

fn canonical_u32(value: &str) -> Option<u32> {
    canonical_usize(value).and_then(|value| u32::try_from(value).ok())
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Vec<Diagnostic>> {
    super::project_image::read_bounded(path, limit).map_err(|error| vec![error])
}
fn text(path: &Path, limit: usize) -> Result<String, Vec<Diagnostic>> {
    String::from_utf8(read(path, limit)?)
        .map_err(|_| vec![Diagnostic::io("SPX-Z904", "compact input must be UTF-8")])
}
fn context_options(max_bytes: usize) -> Result<AgentContextV2Options, Vec<Diagnostic>> {
    AgentContextV2Options::new(
        1,
        max_bytes,
        256,
        [
            AgentContextFilter::Contracts,
            AgentContextFilter::Ownership,
            AgentContextFilter::Effects,
            AgentContextFilter::Types,
            AgentContextFilter::Diagnostics,
            AgentContextFilter::Tests,
        ],
        AgentContextDirection::Forward,
    )
    .map_err(|error| vec![error])
}
fn encode(options: &Options) -> Result<CompactProjection, Vec<Diagnostic>> {
    match options.profile.as_str() {
        "graph" if super::project::is_project_manifest(&options.input) => {
            with_authenticated_project(&options.input, |snapshot| {
                compact::encode_bytes(
                    snapshot.semantic_graph().as_bytes(),
                    "full-graph",
                    "*",
                    snapshot.project_revision(),
                )
                .map_err(|error| vec![error])
            })
        }
        "api-surface" => with_authenticated_project(&options.input, |snapshot| {
            compact::encode_selected(ProjectionSelection::ApiSurface {
                revision: &snapshot.retain_revision(),
            })
            .map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| {
                        if error.code == "SPX-J105" && error.help.is_none() {
                            error.with_help(
                                "api-surface describes owned-data-api.v1 exports only; use semaprax doc or semaprax query for other projects",
                            )
                        } else {
                            error
                        }
                    })
                    .collect()
            })
        }),
        "candidate-diff" => {
            let capsule = super::project_candidate::read_capsule(Path::new(
                options.selection.as_ref().expect("parsed selection"),
            ))
            .map_err(|error| vec![error])?;
            with_authenticated_project(&options.input, |snapshot| {
                let candidate = ProjectCandidate::restore(
                    snapshot.retain_revision(),
                    snapshot.project_revision(),
                    &capsule,
                )?;
                compact::encode_selected(ProjectionSelection::CandidateDiff {
                    candidate: &candidate,
                    expected_candidate: candidate.candidate_digest(),
                })
            })
        }
        "agent-definition" => {
            let source = text(&options.input, 1_310_720)?;
            let definition = semaprax::agent_definition::compile_agent_definition(&source)?;
            compact::encode_selected(ProjectionSelection::AgentDefinition {
                definition: &definition,
            })
        }
        _ => {
            if options.profile == "task-context"
                && options
                    .max_bytes
                    .checked_mul(options.task_seeds.len())
                    .is_none_or(|bytes| bytes > TASK_MAX_CONTEXT_BYTES)
            {
                return Err(vec![Diagnostic::io(
                    "SPX-Z801",
                    "task-context aggregate per-seed byte capacity exceeds 8 MiB",
                )]);
            }
            let source = text(&options.input, compact::MAX_SOURCE_BYTES)?;
            let program = semaprax::check(&source, &options.input)?;
            match options.profile.as_str() {
                "graph" => compact::encode_profile(&program, ProjectionSource::FullGraph),
                "context" => compact::encode_profile(
                    &program,
                    ProjectionSource::AgentContextV2 {
                        symbol: options.selection.as_ref().expect("parsed selection"),
                        options: &context_options(options.max_bytes)?,
                    },
                ),
                "task-context" => {
                    let revision = semaprax::graph::revision(&program);
                    if let Some(expected) = &options.task_revision {
                        if expected != &revision {
                            return Err(vec![Diagnostic::io(
                                "SPX-Z801",
                                format!(
                                    "task-context source revision `{revision}` does not match requested revision `{expected}`"
                                ),
                            )]);
                        }
                    }
                    let seeds = options
                        .task_seeds
                        .iter()
                        .map(|seed| CompilationSeed::new(&seed.id, seed.priority, &seed.reason))
                        .collect();
                    let goal = CompilationGoal::new(seeds).map_err(|error| vec![error])?;
                    let budget =
                        CompilationBudget::new(options.max_tokens, &options.task_tokenizer)
                            .map_err(|error| vec![error])?;
                    compact::encode_selected(ProjectionSelection::TaskContext {
                        program: &program,
                        goal: &goal,
                        options: &context_options(options.max_bytes)?,
                        budget,
                    })
                }
                _ => unreachable!("closed profile parser"),
            }
        }
    }
}

pub(crate) fn output(options: &Options) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let expected = encode(options)?;
    if let Some(path) = &options.replay {
        let bytes = read(path, compact::MAX_ENCODED_BYTES)?;
        let decoded = match options.encoding {
            compact::negotiation::ProjectionEncoding::Binary => compact::decode_binary_and_verify(
                &bytes,
                expected.profile(),
                expected.root(),
                expected.source_revision(),
            ),
            compact::negotiation::ProjectionEncoding::Text => {
                let encoded = std::str::from_utf8(&bytes)
                    .map_err(|_| vec![Diagnostic::io("SPX-Z904", "compact text must be UTF-8")])?;
                compact::decode_text_and_verify(
                    encoded,
                    expected.profile(),
                    expected.root(),
                    expected.source_revision(),
                )
            }
            compact::negotiation::ProjectionEncoding::ModelText => {
                compact::decode_model_text_and_verify(
                    &bytes,
                    expected.profile(),
                    expected.root(),
                    expected.source_revision(),
                )
            }
        }
        .map_err(|error| vec![error])?;
        let full = decoded.reconstructed().map_err(|error| vec![error])?;
        if full != expected.reconstructed().map_err(|error| vec![error])? {
            return Err(vec![Diagnostic::io(
                "SPX-Z910",
                "compact replay differs from freshly selected content",
            )]);
        }
        Ok(full)
    } else {
        match options.encoding {
            compact::negotiation::ProjectionEncoding::Binary => Ok(expected.to_binary()),
            compact::negotiation::ProjectionEncoding::Text => Ok(expected.to_text().into_bytes()),
            compact::negotiation::ProjectionEncoding::ModelText => {
                compact::encode_model_text(&expected)
                    .map(String::into_bytes)
                    .map_err(|error| vec![error])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }
    fn banking_ledger_path() -> PathBuf {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let direct = manifest.join("examples/banking_ledger.spx");
        if direct.exists() {
            direct
        } else {
            manifest.join("../../examples/banking_ledger.spx")
        }
    }
    #[test]
    fn compact_cli_grammar_is_closed_and_profile_specific() {
        assert_eq!(
            parse_inner(&args(&["graph", "input.spx", "--encoding", "binary"]))
                .unwrap()
                .encoding,
            compact::negotiation::ProjectionEncoding::Binary
        );
        for input in [
            vec!["unknown", "input"],
            vec!["graph", "input", "extra"],
            vec!["context", "input"],
            vec!["graph", "input", "--max-tokens", "3"],
            vec![
                "graph",
                "input",
                "--encoding",
                "text",
                "--encoding",
                "binary",
            ],
        ] {
            assert!(parse_inner(&args(&input)).is_err());
        }
    }

    #[test]
    fn task_context_accepts_an_explicit_goal_and_ordered_seed_metadata() {
        let options = parse_inner(&args(&[
            "task-context",
            "input.spx",
            "app.main",
            "--goal",
            "repair the checked entry point",
            "--priority",
            "7",
            "--reason",
            "the entry is the requested operation",
            "--seed",
            "helper.call",
            "--priority",
            "3",
            "--reason",
            "the callee supplies the return type",
            "--tokenizer",
            "lexical-v1",
            "--revision",
            "sha256:revision",
        ]))
        .unwrap();

        assert_eq!(options.task_tokenizer, "lexical-v1");
        assert_eq!(options.task_revision.as_deref(), Some("sha256:revision"));
        assert_eq!(options.task_seeds.len(), 2);
        assert_eq!(options.task_seeds[0].id, "app.main");
        assert_eq!(options.task_seeds[0].priority, 7);
        assert_eq!(
            options.task_seeds[0].reason,
            "the entry is the requested operation"
        );
        assert_eq!(options.task_seeds[1].id, "helper.call");
        assert_eq!(options.task_seeds[1].priority, 3);
        assert_eq!(
            options.task_seeds[1].reason,
            "the callee supplies the return type"
        );
    }

    #[test]
    fn task_context_rejects_malformed_or_profile_specific_arguments() {
        for input in [
            vec!["task-context", "input.spx", "app.main", "--tokenizer"],
            vec!["task-context", "input.spx", "app.main", "--priority", "01"],
            vec![
                "task-context",
                "input.spx",
                "app.main",
                "--max-tokens",
                "01",
            ],
            vec![
                "task-context",
                "input.spx",
                "app.main",
                "--tokenizer",
                "byte-v1",
                "--tokenizer",
                "lexical-v1",
            ],
            vec!["context", "input.spx", "app.main", "--seed", "helper.call"],
        ] {
            assert!(parse_inner(&args(&input)).is_err(), "accepted {input:?}");
        }
    }

    #[test]
    fn task_context_parser_refuses_each_local_bound() {
        let long = "x".repeat(TASK_MAX_FIELD_BYTES + 1);
        let long_revision = "x".repeat(TASK_MAX_REVISION_BYTES + 1);
        let long_tokenizer = "x".repeat(TASK_MAX_TOKENIZER_BYTES + 1);
        let mut too_many_seeds = vec![
            "task-context".to_owned(),
            "input.spx".to_owned(),
            "app.main".to_owned(),
        ];
        for index in 0..TASK_MAX_SEEDS {
            too_many_seeds.extend(["--seed".to_owned(), format!("seed.{index}")]);
        }
        let mut aggregate_overbound = vec![
            "task-context".to_owned(),
            "input.spx".to_owned(),
            "app.main".to_owned(),
            "--max-bytes".to_owned(),
            "262145".to_owned(),
        ];
        for index in 0..(TASK_MAX_SEEDS - 1) {
            aggregate_overbound.extend(["--seed".to_owned(), format!("seed.{index}")]);
        }
        let cases = vec![
            vec![
                "task-context".to_owned(),
                "input.spx".to_owned(),
                long.clone(),
            ],
            vec![
                "task-context".to_owned(),
                "input.spx".to_owned(),
                "app.main".to_owned(),
                "--goal".to_owned(),
                long.clone(),
            ],
            vec![
                "task-context".to_owned(),
                "input.spx".to_owned(),
                "app.main".to_owned(),
                "--seed".to_owned(),
                long.clone(),
            ],
            vec![
                "task-context".to_owned(),
                "input.spx".to_owned(),
                "app.main".to_owned(),
                "--reason".to_owned(),
                long,
            ],
            vec![
                "task-context".to_owned(),
                "input.spx".to_owned(),
                "app.main".to_owned(),
                "--revision".to_owned(),
                long_revision,
            ],
            vec![
                "task-context".to_owned(),
                "input.spx".to_owned(),
                "app.main".to_owned(),
                "--tokenizer".to_owned(),
                long_tokenizer,
            ],
            aggregate_overbound,
            too_many_seeds,
        ];
        for input in cases {
            assert!(parse_inner(&input).is_err(), "accepted overbound {input:?}");
        }
    }

    #[test]
    fn task_context_cli_preserves_library_tokenizer_and_seed_diagnostics() {
        let path = banking_ledger_path();
        let unknown_tokenizer = parse_inner(&args(&[
            "task-context",
            path.to_str().unwrap(),
            "app.main",
            "--tokenizer",
            "model-v1",
        ]))
        .unwrap();
        let errors = output(&unknown_tokenizer).unwrap_err();
        assert_eq!(errors[0].code, "SPX-Z803");

        let unknown_seed = parse_inner(&args(&[
            "task-context",
            path.to_str().unwrap(),
            "missing.seed",
        ]))
        .unwrap();
        let errors = output(&unknown_seed).unwrap_err();
        assert_eq!(errors[0].code, "SPX-Z804");

        let stale_revision = parse_inner(&args(&[
            "task-context",
            path.to_str().unwrap(),
            "app.main",
            "--revision",
            "sha256:stale",
        ]))
        .unwrap();
        let errors = output(&stale_revision).unwrap_err();
        assert_eq!(errors[0].code, "SPX-Z801");
    }

    #[test]
    fn task_context_cli_emits_the_selected_lexical_accounting_metadata() {
        let path = banking_ledger_path();
        let options = parse_inner(&args(&[
            "task-context",
            path.to_str().unwrap(),
            "app.main",
            "--seed",
            "ledger.apply",
            "--priority",
            "0",
            "--reason",
            "callee context",
            "--tokenizer",
            "lexical-v1",
        ]))
        .unwrap();
        let wire = output(&options).unwrap();
        let projection = compact::decode_text(std::str::from_utf8(&wire).unwrap()).unwrap();
        let context: serde_json::Value =
            serde_json::from_slice(&projection.reconstructed().unwrap()).unwrap();
        assert_eq!(context["schema"], "semaprax.semantic-task-context.v1");
        assert_eq!(context["budget"]["tokenizer"], "lexical-v1");
        assert_eq!(context["budget"]["exactness"], "approximate");
        assert_eq!(context["seeds"].as_array().unwrap().len(), 2);
        assert_eq!(context["seeds"][1]["id"], "ledger.apply");
        assert_eq!(context["seeds"][1]["reason"], "callee context");
    }

    #[test]
    fn task_context_default_route_matches_the_legacy_byte_wire() {
        let path = banking_ledger_path();
        let options =
            parse_inner(&args(&["task-context", path.to_str().unwrap(), "app.main"])).unwrap();
        let cli_wire = output(&options).unwrap();

        let source = std::fs::read_to_string(&path).unwrap();
        let program = semaprax::check(&source, &path).unwrap();
        let goal = CompilationGoal::new(vec![CompilationSeed::new(
            "app.main",
            1,
            "explicit CLI task root",
        )])
        .unwrap();
        let expected = compact::encode_selected(ProjectionSelection::TaskContext {
            program: &program,
            goal: &goal,
            options: &context_options(65_536).unwrap(),
            budget: CompilationBudget::new(65_536, "byte-v1").unwrap(),
        })
        .unwrap();
        assert_eq!(cli_wire, expected.to_text().into_bytes());
    }

    #[test]
    fn actual_graph_cli_output_is_independently_reconstructed() {
        let path = banking_ledger_path();
        let options = parse_inner(&args(&["graph", path.to_str().unwrap()])).unwrap();
        let encoded = output(&options).unwrap();
        let decoded = compact::decode_text(std::str::from_utf8(&encoded).unwrap()).unwrap();
        let model_options = parse_inner(&args(&[
            "graph",
            path.to_str().unwrap(),
            "--encoding",
            "model-text",
        ]))
        .unwrap();
        let model_wire = output(&model_options).unwrap();
        assert_eq!(
            compact::decode_model_text(&model_wire)
                .unwrap()
                .reconstructed()
                .unwrap(),
            decoded.reconstructed().unwrap()
        );
        let source = std::fs::read_to_string(&path).unwrap();
        let program = semaprax::parse(&source, &path).unwrap();
        assert_eq!(
            decoded.reconstructed().unwrap(),
            semaprax::graph::to_json(&program).unwrap().as_bytes()
        );
    }

    #[test]
    fn api_surface_on_plain_project_points_at_doc_and_query() {
        let options = parse_inner(&args(&["api-surface", "examples/calculator-project"])).unwrap();
        let errors = encode(&options).expect_err("plain projects have no owned-data api-surface");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-J105");
        assert_eq!(
            errors[0].help.as_deref(),
            Some("api-surface describes owned-data-api.v1 exports only; use semaprax doc or semaprax query for other projects")
        );
    }
}
