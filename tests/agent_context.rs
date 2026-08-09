use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::graph::{self, AgentContextFilter, AgentContextOptions, MAX_AGENT_CONTEXT_BYTES};
use semaprax::parse;

const SOURCE: &str = r#"
module test.agent_context;
permit { clock.read }

@id("ctx.leaf")
fn leaf(value: i64) -> i64 uses { clock.read } { value + 1 }

@id("ctx.left")
fn left(value: i64) -> i64 uses { clock.read } { leaf(value) }

@id("ctx.right")
fn right(value: i64) -> i64 uses { clock.read } requires value >= 0 { leaf(value) }

@id("ctx.root")
fn root(value: i64) -> i64 uses { clock.read } ensures result >= value {
    left(value) + right(value)
}

@id("app.main")
fn main() -> i64 uses { clock.read } { root(40) }
"#;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("agent-context.spx")).unwrap()
}

fn options(
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    filters: &[AgentContextFilter],
) -> AgentContextOptions {
    AgentContextOptions::new(depth, max_bytes, max_nodes, filters.iter().copied()).unwrap()
}

#[test]
fn canonical_agent_context_v1_matches_golden_and_replays_exactly() {
    let program = program();
    let options = options(
        0,
        4096,
        8,
        &[
            AgentContextFilter::Effects,
            AgentContextFilter::Targets,
            AgentContextFilter::Tests,
        ],
    );
    let first = graph::agent_context_json(&program, "ctx.root", &options)
        .unwrap()
        .unwrap();
    let second = graph::agent_context_json(&program, "ctx.root", &options)
        .unwrap()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        format!("{first}\n"),
        include_str!("snapshots/agent_context.v1.json")
    );
    assert!(first.contains("\"used_bytes\":"));
    assert_eq!(first.len(), used_bytes(&first));
    assert!(first.contains("\"reasons\":[\"depth\",\"unavailable_filters\"]"));
    assert!(first.contains("\"unavailable\":[\"targets\",\"tests\"]"));
}

#[test]
fn budgets_emit_exact_counts_reasons_and_reexpandable_frontier() {
    let program = program();
    let bounded = options(1, 4096, 1, &[AgentContextFilter::Effects]);
    let json = graph::agent_context_json(&program, "ctx.root", &bounded)
        .unwrap()
        .unwrap();
    assert!(json.len() <= bounded.max_bytes());
    assert_eq!(json.len(), used_bytes(&json));
    assert!(json.contains("\"used_nodes\":1"));
    assert!(json.contains("\"reasons\":[\"max_nodes\"]"));
    for id in ["ctx.left", "ctx.right"] {
        assert!(json.contains(&format!("\"id\":\"{id}\",\"kind\":\"function\"")));
        assert!(json.contains(&format!("\"symbol\":\"{id}\"")));
        let expanded = graph::agent_context_json(&program, id, &bounded)
            .unwrap()
            .unwrap();
        assert!(expanded.contains(&format!("\"root\":\"{id}\"")));
        assert!(expanded.contains(&format!("\"id\":\"{id}\",\"kind\":\"function\"")));
    }

    let byte_bounded = options(1, 1024, 8, &[AgentContextFilter::Ownership]);
    let json = graph::agent_context_json(&program, "ctx.root", &byte_bounded)
        .unwrap()
        .unwrap();
    assert!(json.len() <= 1024);
    assert_eq!(json.len(), used_bytes(&json));
    assert!(json.contains("\"reasons\":[\"max_bytes\"]"));
    assert!(json.contains("\"omitted_fact_bytes\":"));
    assert!(json.contains("\"symbol\":\"ctx.root\""));

    let meaning = parse(
        include_str!("../examples/meaning.spx"),
        Path::new("meaning.spx"),
    )
    .unwrap();
    let minimum = options(1, 1024, 256, &[AgentContextFilter::Ownership]);
    let minimum_json = graph::agent_context_json(&meaning, "app.main", &minimum)
        .unwrap()
        .unwrap();
    assert!(minimum_json.len() <= 1024);
    assert_eq!(minimum_json.len(), used_bytes(&minimum_json));
    assert!(minimum_json.contains("\"used_nodes\":0"));
    assert!(minimum_json.contains(
        "\"frontier\":[{\"id\":\"app.main\",\"kind\":\"function\",\"reasons\":[\"max_bytes\"]"
    ));
    assert!(minimum_json.contains("\"deferred_known_nodes\":1"));
    assert!(minimum_json.contains(
        "\"resume_contract\":{\"depth\":\"query.depth\",\"max_nodes\":\"query.max_nodes\",\"filters\":\"query.filters\",\"max_bytes\":\"frontier.resume.min_bytes\"}"
    ));
    assert_independent_json_parse(&minimum_json);

    let resume_bytes = json_number(&minimum_json, "\"min_bytes\":");
    assert!(resume_bytes > minimum.max_bytes());
    let resumed_options = options(
        minimum.depth(),
        resume_bytes,
        minimum.max_nodes(),
        &[AgentContextFilter::Ownership],
    );
    let resumed = graph::agent_context_json(&meaning, "app.main", &resumed_options)
        .unwrap()
        .unwrap();
    assert!(resumed.contains("\"used_nodes\":1"));
    assert!(resumed.contains("\"facts\":[{\"id\":\"app.main\",\"kind\":\"function\""));
    assert_independent_json_parse(&resumed);

    let multi_callee = (1024..=4096).step_by(64).find_map(|max_bytes| {
        let bounded = options(1, max_bytes, 8, &[AgentContextFilter::Effects]);
        let json = graph::agent_context_json(&program, "ctx.root", &bounded)
            .ok()
            .flatten()?;
        (json.contains("\"used_nodes\":1")
            && json.contains("\"reasons\":[\"depth\",\"max_bytes\"]"))
        .then_some(json)
    });
    let multi_callee = multi_callee.expect("a bounded root-only page must exist");
    assert!(multi_callee.contains("\"calls\":[\"ctx.left\",\"ctx.right\"]"));
    for id in ["ctx.left", "ctx.right"] {
        assert!(multi_callee.contains(&format!(
            "\"id\":\"{id}\",\"kind\":\"function\",\"reasons\":[\"max_bytes\"]"
        )));
    }
    assert!(multi_callee.contains("\"omitted_known_nodes\":3"));
    assert!(multi_callee.contains("\"deferred_known_nodes\":0"));
}

#[test]
fn oversized_fact_is_permanently_unavailable_instead_of_a_capped_cursor() {
    let huge_name = "a".repeat(MAX_AGENT_CONTEXT_BYTES);
    let source = format!(
        "module test.agent_oversized; @id(\"ctx.huge\") fn {huge_name}() -> i64 {{ 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    let program = parse(&source, Path::new("agent-oversized.spx")).unwrap();
    let maximum = options(
        0,
        MAX_AGENT_CONTEXT_BYTES,
        1,
        &[AgentContextFilter::Effects],
    );
    let diagnostic = graph::agent_context_json(&program, "ctx.huge", &maximum)
        .unwrap_err()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(diagnostic.code, "SPX-G004");
    assert!(diagnostic.message.contains("permanently unavailable"));
    assert!(diagnostic.message.contains("ctx.huge"));
}

#[test]
fn aggregate_larger_than_contract_maximum_paginates_individually_small_facts() {
    const FACT_COUNT: usize = 24;
    const ID_PADDING: usize = 300_000;

    let padding = "x".repeat(ID_PADDING);
    let ids = (0..FACT_COUNT)
        .map(|index| format!("ctx.aggregate.{index:02}.{padding}"))
        .collect::<Vec<_>>();
    let aggregate_reference_bytes = ids
        .iter()
        .take(FACT_COUNT - 1)
        .map(|id| id.len() * 3)
        .sum::<usize>();
    assert!(aggregate_reference_bytes > MAX_AGENT_CONTEXT_BYTES);
    let mut source =
        String::from("module test.agent_aggregate; @id(\"app.main\") fn main() -> i64 { f0() } ");
    for (index, id) in ids.iter().enumerate() {
        source.push_str(&format!("@id(\"{id}\") fn f{index}() -> i64 {{ "));
        if index + 1 == FACT_COUNT {
            source.push('0');
        } else {
            source.push_str(&format!("f{}()", index + 1));
        }
        source.push_str(" } ");
    }
    let program = parse(&source, Path::new("agent-aggregate.spx")).unwrap();
    let minimum = options(
        FACT_COUNT,
        1024,
        FACT_COUNT + 1,
        &[AgentContextFilter::Effects],
    );
    let first = graph::agent_context_json(&program, "app.main", &minimum)
        .unwrap()
        .unwrap();
    assert!(first.contains("\"used_nodes\":0"));
    assert!(first.contains("\"id\":\"app.main\""));

    let root_budget = json_number(&first, "\"min_bytes\":");
    assert!(root_budget < MAX_AGENT_CONTEXT_BYTES);
    let root_page = graph::agent_context_json(
        &program,
        "app.main",
        &options(
            FACT_COUNT,
            root_budget,
            FACT_COUNT + 1,
            &[AgentContextFilter::Effects],
        ),
    )
    .unwrap()
    .unwrap();
    assert!(root_page.contains("\"facts\":[{\"id\":\"app.main\",\"kind\":\"function\""));
    assert!(root_page.contains(&format!(
        "\"id\":{}",
        semaprax::diagnostic::quote_json(&ids[0])
    )));

    let next_budget = json_number(&root_page, "\"min_bytes\":");
    let next_page = graph::agent_context_json(
        &program,
        &ids[0],
        &options(
            FACT_COUNT,
            next_budget,
            FACT_COUNT + 1,
            &[AgentContextFilter::Effects],
        ),
    )
    .unwrap()
    .unwrap();
    assert!(next_page.contains(&format!(
        "\"facts\":[{{\"id\":{},\"kind\":\"function\"",
        semaprax::diagnostic::quote_json(&ids[0])
    )));
}

#[test]
fn filters_are_exact_and_unsupported_graph_facets_are_honest() {
    let program = program();
    let filtered = options(
        0,
        8192,
        2,
        &[
            AgentContextFilter::Contracts,
            AgentContextFilter::Effects,
            AgentContextFilter::Diagnostics,
        ],
    );
    let json = graph::agent_context_json(&program, "ctx.root", &filtered)
        .unwrap()
        .unwrap();
    assert!(json.contains("\"contracts\":{"));
    assert!(json.contains("\"effects\":[\"clock.read\"]"));
    assert!(!json.contains("\"ownership\":{"));
    assert!(!json.contains("\"types\":{"));
    assert!(json.contains("\"unavailable\":[\"diagnostics\"]"));
    assert!(json.contains("\"reasons\":[\"depth\",\"unavailable_filters\"]"));
}

#[test]
fn supported_facets_close_nominal_references_without_lifecycle_or_import_edges() {
    let source = r#"
module test.agent_reference_closure;
permit { filesystem.handle.release }
@id("io.file")
resource File { @id("io.file.drop") drop import "io.file.finalize"; }
@id("io.file.host")
interface FileHost permits { filesystem.handle.release } {
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes file always;
}
@id("file.identity")
fn identity(value: own File) -> File uses { filesystem.handle.release } { value }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("agent-reference-closure.spx")).unwrap();
    let options = options(
        0,
        16 * 1024,
        4,
        &[AgentContextFilter::Ownership, AgentContextFilter::Types],
    );
    let json = graph::agent_context_json(&program, "file.identity", &options)
        .unwrap()
        .unwrap();
    assert!(json.contains(
        "\"declarations\":[{\"id\":\"io.file\",\"kind\":\"resource\",\"drop_strategy\":\"imported\"}]"
    ));
    assert!(!json.contains("io.file.drop"));
    assert!(!json.contains("io.file.finalize"));
    assert!(!json.contains("\"kind\":\"import\""));
    assert!(!json.contains("\"lifecycle\":"));
    assert!(!json.contains("\"cleanup\":"));
    assert_independent_json_parse(&json);
}

#[test]
fn source_revision_references_and_mutations_are_bound_deterministically() {
    let first = program();
    let whitespace = parse(
        &format!("\n\n{SOURCE}\n"),
        Path::new("agent-context-whitespace.spx"),
    )
    .unwrap();
    let options = options(1, 16 * 1024, 8, &[AgentContextFilter::Types]);
    let first_json = graph::agent_context_json(&first, "ctx.root", &options)
        .unwrap()
        .unwrap();
    let whitespace_json = graph::agent_context_json(&whitespace, "ctx.root", &options)
        .unwrap()
        .unwrap();
    assert_eq!(first_json, whitespace_json);
    assert!(first_json.contains(&format!(
        "\"revision\":{}",
        semaprax::diagnostic::quote_json(&graph::revision(&first))
    )));
    for reference in ["ctx.left", "ctx.right"] {
        assert!(
            first_json.contains(&format!("\"calls\":[\"{reference}"))
                || first_json.contains(&format!(",\"{reference}\"]"))
                || first_json.contains(&format!("\"{reference}\""))
        );
    }

    let mutated_source = SOURCE.replace("root(40)", "root(41)");
    let mutated = parse(&mutated_source, Path::new("agent-context-mutated.spx")).unwrap();
    assert_ne!(graph::revision(&first), graph::revision(&mutated));
    let mutated_json = graph::agent_context_json(&mutated, "ctx.root", &options)
        .unwrap()
        .unwrap();
    assert_ne!(first_json, mutated_json);
    assert_eq!(
        mutated_json,
        graph::agent_context_json(&mutated, "ctx.root", &options)
            .unwrap()
            .unwrap()
    );
    assert!(mutated_json.contains(&format!(
        "\"revision\":{}",
        semaprax::diagnostic::quote_json(&graph::revision(&mutated))
    )));
}

#[test]
fn option_api_and_cli_reject_unknown_duplicate_and_malformed_inputs() {
    assert_eq!(
        AgentContextOptions::new(0, 1023, 1, [AgentContextFilter::Effects])
            .unwrap_err()
            .code,
        "SPX-G004"
    );
    assert_eq!(
        AgentContextOptions::new(0, 1024, 0, [AgentContextFilter::Effects])
            .unwrap_err()
            .code,
        "SPX-G004"
    );
    assert_eq!(
        AgentContextOptions::new(0, 1024, 1, []).unwrap_err().code,
        "SPX-G004"
    );
    assert_eq!(
        AgentContextOptions::new(
            0,
            1024,
            1,
            [AgentContextFilter::Effects, AgentContextFilter::Effects]
        )
        .unwrap_err()
        .code,
        "SPX-G004"
    );

    let fixture = CliFixture::new();
    for arguments in [
        vec!["--unknown", "1"],
        vec!["--depth", "01"],
        vec!["--depth", "nope"],
        vec!["--depth"],
        vec!["--max-nodes", "0"],
        vec!["--max-bytes", "1023"],
        vec!["--filters", "effects,wat"],
        vec!["--filters", "effects,effects"],
        vec!["--depth", "1", "--depth", "2"],
    ] {
        let output = fixture.run(&arguments);
        assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
        assert!(!output.stderr.is_empty(), "arguments: {arguments:?}");
    }

    let success = fixture.run(&[
        "--depth",
        "0",
        "--max-bytes",
        "2048",
        "--max-nodes",
        "1",
        "--filters",
        "effects,targets",
    ]);
    assert!(success.status.success());
    let stdout = String::from_utf8(success.stdout).unwrap();
    assert!(stdout.starts_with("{\"schema\":\"semaprax.agent-context.v1\""));
    assert!(stdout.contains("\"unavailable\":[\"targets\"]"));
    assert!(stdout.len() - 1 <= 2048);
}

fn used_bytes(json: &str) -> usize {
    json_number(json, "\"used_bytes\":")
}

fn json_number(json: &str, marker: &str) -> usize {
    let start = json.find(marker).unwrap() + marker.len();
    let end = json[start..]
        .find(|byte: char| !byte.is_ascii_digit())
        .map(|offset| start + offset)
        .unwrap_or(json.len());
    json[start..end].parse().unwrap()
}

fn assert_independent_json_parse(json: &str) {
    let output = Command::new("node")
        .arg("-e")
        .arg("JSON.parse(process.argv[1]);")
        .arg(json)
        .output()
        .expect("Node.js is required for the independent agent-context JSON gate");
    assert!(
        output.status.success(),
        "independent JSON parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct CliFixture {
    directory: PathBuf,
    source: PathBuf,
}

impl CliFixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-agent-context-cli-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let source = directory.join("context.spx");
        fs::write(&source, SOURCE).unwrap();
        Self { directory, source }
    }

    fn run(&self, options: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("context")
            .arg(&self.source)
            .arg("ctx.root")
            .args(options)
            .output()
            .unwrap()
    }
}

impl Drop for CliFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}
