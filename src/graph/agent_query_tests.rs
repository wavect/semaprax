//! REF-09 and REF-10 owner tests: query-local call/schema reuse, exact
//! response fitting, and deterministic work counts for agent-context queries.

use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::work_counter::{count, measure, Work};
use super::{
    agent_context_json, agent_context_v2_json, AgentContextDirection, AgentContextFilter,
    AgentContextOptions, AgentContextV2Options, MAX_AGENT_CONTEXT_BYTES,
};

const CORE_FILTERS: [AgentContextFilter; 4] = [
    AgentContextFilter::Contracts,
    AgentContextFilter::Ownership,
    AgentContextFilter::Effects,
    AgentContextFilter::Types,
];

fn parse(source: &str) -> crate::ast::Program {
    crate::parse(source, Path::new("agent-query.spx")).unwrap()
}

fn render<T: std::fmt::Debug>(result: Result<Option<String>, T>) -> String {
    match result {
        Ok(Some(text)) => format!("ok\t{text}"),
        Ok(None) => "none".to_owned(),
        Err(error) => format!("err\t{error:?}"),
    }
}

fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::new(), |mut output, byte| {
            write!(output, "{byte:02x}").unwrap();
            output
        })
}

fn v1(depth: usize, max_bytes: usize, max_nodes: usize) -> AgentContextOptions {
    AgentContextOptions::new(depth, max_bytes, max_nodes, CORE_FILTERS).unwrap()
}

fn v2(
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    direction: AgentContextDirection,
) -> AgentContextV2Options {
    AgentContextV2Options::new(depth, max_bytes, max_nodes, CORE_FILTERS, direction).unwrap()
}

/// A straight call chain of `reached` functions rooted at `chain.c0`, plus
/// `unrelated` declarations the query never reaches.
fn chain_source(reached: usize, unrelated: usize) -> String {
    let mut source = String::from(
        "module test.agent_query_chain;\n@id(\"app.main\") fn main() -> i64 { c0(1) }\n",
    );
    for index in 0..reached {
        if index + 1 < reached {
            writeln!(
                source,
                "@id(\"chain.c{index}\") fn c{index}(v: i64) -> i64 {{ c{}(v) + 1 }}",
                index + 1
            )
            .unwrap();
        } else {
            writeln!(
                source,
                "@id(\"chain.c{index}\") fn c{index}(v: i64) -> i64 {{ v }}"
            )
            .unwrap();
        }
    }
    for index in 0..unrelated {
        writeln!(
            source,
            "@id(\"chain.u{index}\") fn u{index}(v: i64) -> i64 {{ v }}"
        )
        .unwrap();
    }
    source
}

fn chain_counts(reached: usize, unrelated: usize) -> [usize; 8] {
    let program = parse(&chain_source(reached, unrelated));
    let (first, v1_counts) = measure(|| {
        agent_context_json(
            &program,
            "chain.c0",
            &v1(1024, MAX_AGENT_CONTEXT_BYTES, 65_536),
        )
    });
    let first = first.unwrap().unwrap();
    let (second, v2_counts) = measure(|| {
        agent_context_v2_json(
            &program,
            "chain.c0",
            &v2(
                1024,
                MAX_AGENT_CONTEXT_BYTES,
                65_536,
                AgentContextDirection::Forward,
            ),
        )
    });
    let second = second.unwrap().unwrap();
    assert!(first.contains(&format!("\"used_nodes\":{reached},")));
    assert!(second.contains(&format!("\"used_nodes\":{reached},")));
    [
        count(&v1_counts, Work::CallableMembershipBuild),
        count(&v1_counts, Work::CallableMembershipEntry),
        count(&v1_counts, Work::CallSetComputation),
        count(&v1_counts, Work::SchemaSelection),
        count(&v2_counts, Work::CallableMembershipBuild),
        count(&v2_counts, Work::CallableMembershipEntry),
        count(&v2_counts, Work::CallSetComputation),
        count(&v2_counts, Work::SchemaSelection),
    ]
}

#[test]
fn ref09_work_counts_do_not_scale_with_reached_times_declared() {
    let mut report = String::new();
    let mut rows = Vec::new();
    for (reached, unrelated) in [(1, 0), (4, 0), (16, 0), (4, 16), (4, 64), (16, 64)] {
        let counts = chain_counts(reached, unrelated);
        writeln!(report, "R={reached} U={unrelated} {counts:?}").unwrap();
        rows.push((reached, unrelated, counts));
    }
    eprintln!("REF-09 work counts [v1 builds, v1 entries, v1 call sets, v1 schemas, v2 builds, v2 entries, v2 call sets, v2 schemas]\n{report}");
    for (reached, unrelated, counts) in rows {
        let declared = reached + unrelated + 1;
        // v1: one query-local membership view, one call-set walk per reached
        // function, and a constant number of schema selections per query.
        assert_eq!(counts[0], 1, "{report}");
        assert_eq!(counts[1], declared, "{report}");
        assert_eq!(counts[2], reached, "{report}");
        assert_eq!(counts[3], REF09_V1_SCHEMA_SELECTIONS, "{report}");
        // v2: the persistent index owns traversal edges; the legacy fact
        // `calls` field walks each emitted function once, plus once for the
        // unselected caller `app.main` whose resume target must be checked,
        // and the fact schema is selected a constant number of times.
        assert_eq!(counts[4], 1, "{report}");
        assert_eq!(counts[5], declared, "{report}");
        assert_eq!(counts[6], reached + 1, "{report}");
        assert_eq!(counts[7], REF09_V2_SCHEMA_SELECTIONS, "{report}");
    }
}

const REF09_V1_SCHEMA_SELECTIONS: usize = 1;
const REF09_V2_SCHEMA_SELECTIONS: usize = 2;

const COVERAGE_SOURCE: &str = r#"
module test.agent_query_coverage;

@id("cov.pick")
fn pick<T>(value: i64) -> i64 { value }

@id("cov.double")
fn double(value: i64) -> i64 { value * 2 }

@id("cov.left")
fn left(value: i64) -> i64 { if value > 0 { value - 1 } else { 0 } }

@id("cov.size")
fn size() -> usize { let owned = bytes_zeroed(2usize); let view = bytes_as_slice(owned); byte_len(view) }

@id("cov.root")
fn root(value: i64) -> i64 { pick<bool>(value) + double(value) + left(value) + if size() == 2usize { 1 } else { 0 } }

@id("cov.unreached")
fn unreached() -> i64 { 0 }

@id("app.main")
fn main() -> i64 { root(1) }
"#;

/// Exact v1/v2 bytes for functions, templates and concrete instances,
/// function references, compiler-owned callees, a call cycle, unreachable
/// declarations, and a missing root, pinned from the pre-REF-09 renderer.
#[test]
fn ref09_known_answers_cover_callable_kinds_cycles_and_missing_roots() {
    let program = parse(COVERAGE_SOURCE);
    let mut transcript = String::new();
    for root in [
        "cov.root",
        "cov.pick",
        "cov.left",
        "cov.size",
        "cov.unreached",
        "cov.missing",
    ] {
        for depth in [0, 1, 4] {
            let v1_result = agent_context_json(&program, root, &v1(depth, 64 * 1024, 256));
            writeln!(transcript, "v1 {root} {depth} {}", render(v1_result)).unwrap();
            for direction in AgentContextDirection::ALL {
                let v2_result =
                    agent_context_v2_json(&program, root, &v2(depth, 64 * 1024, 256, direction));
                writeln!(
                    transcript,
                    "v2 {root} {depth} {direction:?} {}",
                    render(v2_result)
                )
                .unwrap();
            }
        }
    }
    eprintln!("REF-09 coverage digest {}", digest(&transcript));
    let root = render(agent_context_json(
        &program,
        "cov.root",
        &v1(4, 64 * 1024, 256),
    ));
    assert!(root.starts_with("ok\t"), "{root}");
    assert!(
        root.contains("\"calls\":[\"cov.double\",\"cov.left\",\"cov.pick\",\"cov.size\"]"),
        "{root}"
    );
    assert!(!root.contains("\"id\":\"cov.unreached\""), "{root}");
    assert_eq!(
        render(agent_context_json(
            &program,
            "cov.missing",
            &v1(1, 64 * 1024, 256)
        )),
        "none"
    );
    assert_eq!(digest(&transcript), REF09_COVERAGE_DIGEST, "{transcript}");
}

const REF09_COVERAGE_DIGEST: &str =
    "a705ff501289c548f9daa41cceac3a60afe883ca69cca66525d1742e83d333f1";
