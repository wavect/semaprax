use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::graph::{
    self, AgentContextDirection, AgentContextFilter, AgentContextOptions, AgentContextV2Options,
    MAX_AGENT_CONTEXT_BYTES, MIN_AGENT_CONTEXT_BYTES,
};
use semaprax::parse;
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"
module test.agent_context_v2;

@id("ctx.phantom")
fn phantom<T>(value: i64) -> i64 { value }

@id("ctx.leaf")
fn leaf(value: i64) -> i64 { value + 1 }

@id("ctx.left")
fn left(value: i64) -> i64 { leaf(value) }

@id("ctx.right")
fn right(value: i64) -> i64 { leaf(value) }

@id("ctx.root")
fn root(value: i64) -> i64 { left(value) + right(value) }

@id("ctx.caller.a")
fn caller_a(value: i64) -> i64 { root(value) }

@id("ctx.caller.b")
fn caller_b(value: i64) -> i64 { root(value) }

@id("ctx.generic.caller")
fn generic_caller(value: i64) -> i64 { phantom<bool>(value) }

@id("app.main")
fn main() -> i64 { caller_a(1) + caller_b(2) + generic_caller(3) }
"#;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("agent-context-v2.spx")).unwrap()
}

fn options(direction: AgentContextDirection, depth: usize) -> AgentContextV2Options {
    AgentContextV2Options::new(
        depth,
        128 * 1024,
        64,
        [AgentContextFilter::Effects],
        direction,
    )
    .unwrap()
}

fn context(direction: AgentContextDirection, depth: usize) -> String {
    graph::agent_context_v2_json(&program(), "ctx.root", &options(direction, depth))
        .unwrap()
        .unwrap()
}

#[test]
fn modern_byte_data_context_projects_while_statements() {
    let source = include_str!("../examples/text_analytics.spx");
    let program = parse(source, Path::new("examples/text_analytics.spx")).unwrap();
    let output = graph::agent_context_v2_json(
        &program,
        "text.count_byte",
        &options(AgentContextDirection::Forward, 1),
    )
    .unwrap()
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["source_graph_schema"], "semaprax.graph.v23");
    assert!(output.contains("\"kind\":\"while\""));
}

#[test]
fn v2_forward_reverse_and_both_have_exact_directional_closure() {
    let forward = context(AgentContextDirection::Forward, 2);
    let reverse = context(AgentContextDirection::Reverse, 1);
    let both = context(AgentContextDirection::Both, 1);

    for (json, direction) in [
        (&forward, "forward"),
        (&reverse, "reverse"),
        (&both, "both"),
    ] {
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(parsed["schema"], "semaprax.agent-context.v2");
        assert_eq!(parsed["source_graph_schema"], "semaprax.graph.v14");
        assert_eq!(parsed["query"]["direction"], direction);
        assert_eq!(parsed["budget"]["used_bytes"], json.len());
    }

    assert_eq!(
        fact_ids(&forward),
        ["ctx.root", "ctx.left", "ctx.right", "ctx.leaf"]
    );
    assert_eq!(
        fact_ids(&reverse),
        ["ctx.root", "ctx.caller.a", "ctx.caller.b"]
    );
    assert_eq!(
        fact_ids(&both),
        [
            "ctx.root",
            "ctx.caller.a",
            "ctx.caller.b",
            "ctx.left",
            "ctx.right",
        ]
    );
    assert!(reverse.contains("\"called_by\":[\"ctx.caller.a\",\"ctx.caller.b\"]"));
    assert!(both.contains("\"directions\":[\"forward\"]"));
    assert!(both.contains("\"directions\":[\"reverse\"]"));
}

#[test]
fn traversal_and_reference_frontiers_have_disjoint_honest_counts() {
    let reverse = context(AgentContextDirection::Reverse, 0);
    let parsed: serde_json::Value = serde_json::from_str(&reverse).unwrap();

    assert_eq!(
        parsed["truncation"]["reasons"],
        serde_json::json!(["depth"])
    );
    assert_eq!(parsed["truncation"]["omitted_known_nodes"], 2);
    assert_eq!(parsed["truncation"]["deferred_known_nodes"], 0);
    assert_eq!(
        parsed["reference_closure"]["referenced_unselected_nodes"],
        2
    );
    assert_eq!(ids(&parsed["frontier"]), ["ctx.caller.a", "ctx.caller.b"]);
    assert_eq!(
        ids(&parsed["reference_frontier"]),
        ["ctx.left", "ctx.right"]
    );
    for item in parsed["frontier"].as_array().unwrap() {
        assert_eq!(item["directions"], serde_json::json!(["reverse"]));
        assert_eq!(item["resume"]["direction"], "reverse");
    }
    for item in parsed["reference_frontier"].as_array().unwrap() {
        assert_eq!(item["relations"], serde_json::json!(["calls"]));
        assert_eq!(item["resume"]["direction"], "reverse");
    }
}

#[test]
fn v1_bytes_remain_exact_and_v2_outputs_have_frozen_kats() {
    let program = program();
    let v1_options =
        AgentContextOptions::new(1, 128 * 1024, 64, [AgentContextFilter::Effects]).unwrap();
    let v1_first = graph::agent_context_json(&program, "ctx.root", &v1_options)
        .unwrap()
        .unwrap();
    let v1_second = graph::agent_context_json(&program, "ctx.root", &v1_options)
        .unwrap()
        .unwrap();
    assert_eq!(v1_first, v1_second);
    assert!(v1_first.starts_with("{\"schema\":\"semaprax.agent-context.v1\""));

    let outputs = [
        context(AgentContextDirection::Forward, 1),
        context(AgentContextDirection::Reverse, 1),
        context(AgentContextDirection::Both, 1),
    ];
    let expected = [
        "922404133444942ab86607772362098e0f5656add6bea607a890be2bcfe5b7c9",
        "9a2ebfe569926e67f436379cf2b5c96d510daadd11d0a295ed54903cb612627b",
        "4ec8a62a17551e87dc301d08f0a09c6159445757bca6dd9920a7db4e3790ce17",
    ];
    for (output, expected) in outputs.iter().zip(expected) {
        assert_eq!(
            format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(output.as_bytes()))
            ),
            expected
        );
        assert_eq!(
            output,
            &graph::agent_context_v2_json(
                &program,
                "ctx.root",
                &AgentContextV2Options::new(
                    1,
                    128 * 1024,
                    64,
                    [AgentContextFilter::Effects],
                    serde_direction(output),
                )
                .unwrap(),
            )
            .unwrap()
            .unwrap()
        );
    }
}

#[test]
fn direction_api_and_cli_are_closed_and_v1_is_the_default() {
    assert_eq!(
        AgentContextDirection::from_name("forward"),
        Some(AgentContextDirection::Forward)
    );
    assert_eq!(AgentContextDirection::from_name("Forward"), None);
    assert_eq!(AgentContextDirection::from_name("sideways"), None);

    let fixture = CliFixture::new();
    let default = fixture.run(&[]);
    assert!(default.status.success());
    assert!(String::from_utf8(default.stdout)
        .unwrap()
        .starts_with("{\"schema\":\"semaprax.agent-context.v1\""));

    let reverse = fixture.run(&["--direction", "reverse", "--depth", "0"]);
    assert!(reverse.status.success());
    let reverse = String::from_utf8(reverse.stdout).unwrap();
    assert!(reverse.starts_with("{\"schema\":\"semaprax.agent-context.v2\""));
    assert!(reverse.contains("\"direction\":\"reverse\""));

    for arguments in [
        vec!["--direction"],
        vec!["--direction", "Forward"],
        vec!["--direction", "sideways"],
        vec!["--direction", "forward", "--direction", "reverse"],
    ] {
        let output = fixture.run(&arguments);
        assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn traversal_and_reference_cursors_replay_with_direction_bound_progress() {
    let program = program();
    let reverse_root = graph::agent_context_v2_json(
        &program,
        "ctx.root",
        &options(AgentContextDirection::Reverse, 0),
    )
    .unwrap()
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&reverse_root).unwrap();
    let reference = &parsed["reference_frontier"][0];
    let reference_target = reference["resume"]["target"].as_str().unwrap();
    let reference_min = reference["resume"]["min_bytes"].as_u64().unwrap() as usize;
    let replayed_reference = graph::agent_context_v2_json(
        &program,
        reference_target,
        &AgentContextV2Options::new(
            0,
            reference_min,
            64,
            [AgentContextFilter::Effects],
            AgentContextDirection::Reverse,
        )
        .unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(fact_ids(&replayed_reference)[0], reference_target);
    assert!(replayed_reference.contains("\"direction\":\"reverse\""));

    let node_bounded = AgentContextV2Options::new(
        3,
        128 * 1024,
        1,
        [AgentContextFilter::Effects],
        AgentContextDirection::Both,
    )
    .unwrap();
    let node_page = graph::agent_context_v2_json(&program, "ctx.root", &node_bounded)
        .unwrap()
        .unwrap();
    let node_page: serde_json::Value = serde_json::from_str(&node_page).unwrap();
    assert_eq!(node_page["budget"]["used_nodes"], 1);
    assert!(node_page["truncation"]["reasons"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("max_nodes")));
    assert!(node_page["frontier"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| {
            item["resume"]["direction"] == "both"
                && item["resume"]["min_bytes"] == MAX_AGENT_CONTEXT_BYTES
        }));

    let byte_page = (MIN_AGENT_CONTEXT_BYTES..=8192)
        .step_by(64)
        .find_map(|max_bytes| {
            let options = AgentContextV2Options::new(
                1,
                max_bytes,
                64,
                [AgentContextFilter::Effects],
                AgentContextDirection::Forward,
            )
            .unwrap();
            let json = graph::agent_context_v2_json(&program, "ctx.root", &options)
                .ok()
                .flatten()?;
            (json.contains("\"used_nodes\":0") && json.contains("\"max_bytes\"")).then_some(json)
        });
    let byte_page = byte_page.expect("a v2 zero-fact byte page must exist");
    let byte_page: serde_json::Value = serde_json::from_str(&byte_page).unwrap();
    let resume = &byte_page["frontier"][0]["resume"];
    let resumed = graph::agent_context_v2_json(
        &program,
        resume["target"].as_str().unwrap(),
        &AgentContextV2Options::new(
            1,
            resume["min_bytes"].as_u64().unwrap() as usize,
            64,
            [AgentContextFilter::Effects],
            AgentContextDirection::Forward,
        )
        .unwrap(),
    )
    .unwrap()
    .unwrap();
    assert!(!fact_ids(&resumed).is_empty());
}

#[test]
fn reverse_template_edges_and_both_direction_cycles_are_exact() {
    let template = graph::agent_context_v2_json(
        &program(),
        "ctx.phantom",
        &options(AgentContextDirection::Reverse, 1),
    )
    .unwrap()
    .unwrap();
    assert_eq!(fact_ids(&template), ["ctx.phantom", "ctx.generic.caller"]);

    let cycle = parse(
        r#"
module test.agent_context_v2_cycle;
@id("ctx.a") fn a(value: i64) -> i64 { b(value) }
@id("ctx.b") fn b(value: i64) -> i64 { a(value) }
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("agent-context-v2-cycle.spx"),
    )
    .unwrap();
    let both = graph::agent_context_v2_json(
        &cycle,
        "ctx.a",
        &AgentContextV2Options::new(
            8,
            128 * 1024,
            64,
            [AgentContextFilter::Effects],
            AgentContextDirection::Both,
        )
        .unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(fact_ids(&both), ["ctx.a", "ctx.b"]);
    let parsed: serde_json::Value = serde_json::from_str(&both).unwrap();
    assert_eq!(parsed["budget"]["max_depth_used"], 1);
    assert_eq!(parsed["truncation"]["omitted_known_nodes"], 0);
}

#[test]
fn oversized_v2_fact_is_permanently_unavailable() {
    let huge_name = "a".repeat(MAX_AGENT_CONTEXT_BYTES);
    let source = format!(
        "module test.agent_context_v2_huge; @id(\"ctx.huge\") fn {huge_name}() -> i64 {{ 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    let program = parse(&source, Path::new("agent-context-v2-huge.spx")).unwrap();
    let error = graph::agent_context_v2_json(
        &program,
        "ctx.huge",
        &AgentContextV2Options::new(
            0,
            MAX_AGENT_CONTEXT_BYTES,
            1,
            [AgentContextFilter::Effects],
            AgentContextDirection::Forward,
        )
        .unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G004");
    assert!(error[0].message.contains("permanently unavailable"));
}

#[test]
fn every_graph_lattice_schema_is_reported_without_migration() {
    let legacy = r#"
module test.agent_context_v2_legacy;
@id("ctx.legacy") fn legacy(value: i64) -> i64 { value + 1 }
@id("app.main") fn main() -> i64 { legacy(1) }
"#;
    let option = r#"
module test.agent_context_v2_option;
@id("ctx.option")
fn option(input: Option<i64>) -> Option<bool> {
    let checked = input?;
    Option<bool>::Some { value: checked > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let cases = [
        (legacy, "ctx.legacy", "semaprax.graph.v10"),
        (option, "ctx.option", "semaprax.graph.v11"),
        (
            include_str!("../platform-tests/component-runtime/v7.spx"),
            "component.transform-i64-bool",
            "semaprax.graph.v12",
        ),
        (
            include_str!("../platform-tests/component-runtime/v8.spx"),
            "component.pattern.preserve-phantom-i64",
            "semaprax.graph.v13",
        ),
        (SOURCE, "ctx.root", "semaprax.graph.v14"),
    ];
    for (source, root, schema) in cases {
        let program = parse(source, Path::new("agent-context-v2-lattice.spx")).unwrap();
        let output = graph::agent_context_v2_json(
            &program,
            root,
            &AgentContextV2Options::new(
                0,
                128 * 1024,
                64,
                [AgentContextFilter::Effects],
                AgentContextDirection::Both,
            )
            .unwrap(),
        )
        .unwrap()
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["source_graph_schema"], schema);
    }
}

#[test]
fn cli_context_accepts_an_authenticated_project_without_graph_transfer() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project = root.join("examples/calculator-project");
    let manifest = project.join("semaprax.toml");
    let graph_bytes = semaprax::project::with_authenticated_project(&manifest, |snapshot| {
        Ok(snapshot.semantic_graph().len())
    })
    .unwrap();
    let mut exact = None;
    for selector in [&project, &manifest] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("context")
            .arg(selector)
            .arg("calculator.add")
            .args([
                "--direction",
                "both",
                "--depth",
                "1",
                "--max-bytes",
                "2048",
                "--max-nodes",
                "16",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert!(output.stdout.len() <= 2049);
        assert!(output
            .stdout
            .starts_with(b"{\"schema\":\"semaprax.project-agent-context.v1\""));
        assert_eq!(
            output.stdout.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
        if let Some(expected) = &exact {
            assert_eq!(&output.stdout, expected);
        } else {
            exact = Some(output.stdout.clone());
        }
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["schema"], "semaprax.project-agent-context.v1");
        assert_eq!(value["target"][0], "calculator.add");
        assert_eq!(value["target"][2], "src/core.spx");
        assert_eq!(value["query"][0], "both");
        assert_eq!(value["query"][2], 2048);
        assert_eq!(value["query"][4], 16 * 1024 * 1024);
        assert!(value["nodes"].as_array().unwrap().iter().any(|node| {
            node[0] == "calculator.app.main" && node[1] == "declaration" && node[2] == "function"
        }));
        assert!(value["project_revision"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(value["graph_revision"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }
    let exact = exact.as_ref().unwrap();
    assert!(
        exact.len() * 6 < graph_bytes,
        "context={} graph={graph_bytes}",
        exact.len()
    );
    assert!(semaprax::agent_economics::lexical_tokens(std::str::from_utf8(exact).unwrap()) <= 600);

    let filtered = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["context"])
        .arg(&project)
        .args(["calculator.add", "--filters", "contracts"])
        .output()
        .unwrap();
    assert_eq!(filtered.status.code(), Some(2));
    assert!(String::from_utf8(filtered.stderr)
        .unwrap()
        .contains("--filters is unavailable for Project inputs"));

    // Project support does not weaken the standalone source contract.
    let library = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["context"])
        .arg(project.join("src/core.spx"))
        .arg("calculator.add")
        .output()
        .unwrap();
    assert_eq!(library.status.code(), Some(1));
    assert!(String::from_utf8(library.stderr)
        .unwrap()
        .contains("SPX-T105"));
}

fn fact_ids(json: &str) -> Vec<String> {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    parsed["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fact| fact["id"].as_str().unwrap().to_owned())
        .collect()
}

fn ids<const N: usize>(items: &serde_json::Value) -> [&str; N] {
    let values = items.as_array().unwrap();
    assert_eq!(values.len(), N);
    std::array::from_fn(|index| values[index]["id"].as_str().unwrap())
}

fn serde_direction(output: &str) -> AgentContextDirection {
    match serde_json::from_str::<serde_json::Value>(output).unwrap()["query"]["direction"]
        .as_str()
        .unwrap()
    {
        "forward" => AgentContextDirection::Forward,
        "reverse" => AgentContextDirection::Reverse,
        "both" => AgentContextDirection::Both,
        other => panic!("unexpected direction {other}"),
    }
}

struct CliFixture {
    directory: PathBuf,
    source: PathBuf,
}

impl CliFixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-agent-context-v2-cli-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let source = directory.join("context-v2.spx");
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
