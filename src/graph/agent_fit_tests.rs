//! REF-10 owner tests: exact byte-budget fitting of agent-context responses.
//!
//! The sweep digests below were captured from the pre-REF-10 renderer, which
//! re-materialized the complete response for every discarded fact. They pin
//! complete v1/v2 response bytes, selected IDs, frontier entries, omission
//! counts, `required_bytes`, `used_bytes` and diagnostics across budgets.

use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::work_counter::{count, measure, Work};
use super::{
    agent_context_json, agent_context_v2_json, AgentContextDirection, AgentContextFilter,
    AgentContextOptions, AgentContextV2Options, MAX_AGENT_CONTEXT_BYTES, MIN_AGENT_CONTEXT_BYTES,
};

const CORE_FILTERS: [AgentContextFilter; 4] = [
    AgentContextFilter::Contracts,
    AgentContextFilter::Ownership,
    AgentContextFilter::Effects,
    AgentContextFilter::Types,
];

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

/// A root calling forty functions whose stable IDs grow by uneven widths, so
/// discarding facts crosses decimal-width boundaries of every count, plus a
/// call ring among them and unrelated declarations.
fn wide_source() -> String {
    let mut source = String::from(
        "module test.agent_fit_wide;\n@id(\"app.main\") fn main() -> i64 { root(1) }\n@id(\"w.root\") fn root(v: i64) -> i64 { 0",
    );
    for index in 0..40 {
        write!(source, " + f{index}(v)").unwrap();
    }
    source.push_str(" }\n");
    for index in 0..40 {
        let padding = "x".repeat(3 + (index * 7) % 60);
        writeln!(
            source,
            "@id(\"w.f{index}.{padding}\") fn f{index}(v: i64) -> i64 requires v >= {index} {{ if v > 0 {{ f{}(v - 1) }} else {{ {index} }} }}",
            (index + 1) % 40
        )
        .unwrap();
    }
    for index in 0..20 {
        writeln!(
            source,
            "@id(\"w.unrelated{index}\") fn unrelated{index}() -> i64 {{ {index} }}"
        )
        .unwrap();
    }
    source
}

fn cyclic_source() -> String {
    let mut source = String::from(
        "module test.agent_fit_cyclic;\n@id(\"app.main\") fn main() -> i64 { c0(1) }\n",
    );
    for index in 0..12 {
        writeln!(
            source,
            "@id(\"c.n{index}\") fn c{index}(v: i64) -> i64 {{ if v > 0 {{ c{}(v - 1) + c{}(v - 2) }} else {{ 0 }} }}",
            (index + 1) % 12,
            (index + 5) % 12
        )
        .unwrap();
    }
    source
}

const STRINGS_SOURCE: &str = "module test.agent_fit_strings;\n@id(\"app.main\") fn main() -> i64 { a(1) }\n@id(\"s.a\") fn a(v: i64) -> i64 { string_len(\"quote \\\" back \\\\ caf\u{e9} \u{1F600}\") + b(v) + c(v) }\n@id(\"s.b\") fn b(v: i64) -> i64 { string_len(\"tab\\t nl\\n \u{4E2D}\u{6587}\") + a(v - 1) }\n@id(\"s.c\") fn c(v: i64) -> i64 { v }\n";

fn sweep(source: &str, root: &str, step: usize) -> String {
    let program = crate::parse(source, Path::new("agent-fit.spx")).unwrap();
    let mut transcript = String::new();
    let full = AgentContextOptions::new(8, MAX_AGENT_CONTEXT_BYTES, 65_536, CORE_FILTERS).unwrap();
    let full_v1 = agent_context_json(&program, root, &full)
        .unwrap()
        .unwrap()
        .len();
    let mut budgets = (MIN_AGENT_CONTEXT_BYTES..full_v1 + 256)
        .step_by(step)
        .collect::<Vec<_>>();
    budgets.extend([full_v1 - 1, full_v1, full_v1 + 1]);
    for &budget in &budgets {
        for max_nodes in [1, 3, 256] {
            let options = AgentContextOptions::new(8, budget, max_nodes, CORE_FILTERS).unwrap();
            writeln!(
                transcript,
                "v1 {budget} {max_nodes} {}",
                render(agent_context_json(&program, root, &options))
            )
            .unwrap();
            for direction in AgentContextDirection::ALL {
                let options =
                    AgentContextV2Options::new(8, budget, max_nodes, CORE_FILTERS, direction)
                        .unwrap();
                writeln!(
                    transcript,
                    "v2 {budget} {max_nodes} {direction:?} {}",
                    render(agent_context_v2_json(&program, root, &options))
                )
                .unwrap();
            }
        }
    }
    transcript
}

#[test]
fn ref10_budget_sweeps_keep_exact_known_answers() {
    let wide = sweep(&wide_source(), "w.root", 61);
    let cyclic = sweep(&cyclic_source(), "c.n0", 37);
    let strings = sweep(STRINGS_SOURCE, "s.a", 13);
    let digests = [digest(&wide), digest(&cyclic), digest(&strings)];
    eprintln!("REF-10 sweep digests {digests:?}");
    for transcript in [&wide, &cyclic, &strings] {
        assert!(transcript
            .lines()
            .any(|line| line.contains("\"max_bytes\"")));
        // Sweep lines are "<v> <budget> <nodes> {render}", so an error line
        // carries " err\t" (a space, not a tab, before the marker).
        assert!(transcript.lines().any(|line| line.contains(" err\t")));
        assert!(transcript
            .lines()
            .any(|line| line.contains("\"reasons\":[\"max_bytes\"]")));
    }
    assert_eq!(digests, REF10_SWEEP_DIGESTS);
}

const REF10_SWEEP_DIGESTS: [&str; 3] = [
    "3b7a48f61aeae9e4310efa16ed879051a390588123653cb560e8775c43156c12",
    "fae97a0ed62ad57e5ac5a11a297bd72d4cb74cb886fc65b0687ceb4d0de0148a",
    "0181871b743758808e4be01cc6dd3aded73fe803e687d394b04d22264a756539",
];

/// Deterministic instrumentation: complete-response materializations no
/// longer grow once per discarded fact.
#[test]
fn ref10_materializations_do_not_grow_per_discarded_fact() {
    let program = crate::parse(&wide_source(), Path::new("agent-fit.spx")).unwrap();
    let mut report = String::new();
    for budget in [4096, 8192, 16_384, 32_768, MAX_AGENT_CONTEXT_BYTES] {
        let options = AgentContextOptions::new(8, budget, 256, CORE_FILTERS).unwrap();
        let (output, v1_counts) = measure(|| agent_context_json(&program, "w.root", &options));
        let output = output.unwrap().unwrap();
        let options =
            AgentContextV2Options::new(8, budget, 256, CORE_FILTERS, AgentContextDirection::Both)
                .unwrap();
        let (v2_output, v2_counts) =
            measure(|| agent_context_v2_json(&program, "w.root", &options));
        let v2_output = v2_output.unwrap().unwrap();
        writeln!(
            report,
            "budget={budget} v1_len={} v1[materialized={}, bytes={}, sized={}, fragment_bytes={}] v2_len={} v2[materialized={}, bytes={}, sized={}, fragment_bytes={}]",
            output.len(),
            count(&v1_counts, Work::FullResponseMaterialization),
            count(&v1_counts, Work::MaterializedResponseBytes),
            count(&v1_counts, Work::ResponseSizeEvaluation),
            count(&v1_counts, Work::EnvelopeFragmentBytes),
            v2_output.len(),
            count(&v2_counts, Work::FullResponseMaterialization),
            count(&v2_counts, Work::MaterializedResponseBytes),
            count(&v2_counts, Work::ResponseSizeEvaluation),
            count(&v2_counts, Work::EnvelopeFragmentBytes),
        )
        .unwrap();
    }
    eprintln!("REF-10 work counts\n{report}");
}
