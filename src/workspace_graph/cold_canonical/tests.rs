use super::super::*;
use super::before;
use crate::bounded_output::with_limit_usage_refusal;
use crate::format::comments::canonical_comparison;
use crate::test_allocation_observer::measure;

const SOURCE: &str = "// héader 🌍\r\nmodule compare.comments;\r\n\r\n// declaration\r\n@id(\"compare.main\") fn main()->i64 {\r\n let x=1; // trailing λ\r\n while x<0 { // nested\r\n  x<0 // tail\r\n }\r\n x\r\n // closing\r\n}\r\n// final Ω  \r\n";

#[test]
fn complete_old_and_comparison_keep_exact_short_zero_sticky_and_nested_formatter_debits() {
    let (program, comments) = crate::parse_with_comments(SOURCE, Path::new("compare.spx")).unwrap();
    let expected = crate::format::comments::canonical_with_comments(&program, &comments);
    let (_, _, needed, _) = with_limit_usage_refusal(usize::MAX, || {
        let output = crate::format::comments::canonical_with_comments(&program, &comments);
        drop(output);
    });
    for limit in [
        0,
        needed.saturating_sub(1),
        needed,
        needed.saturating_add(1),
    ] {
        for sticky in [false, true] {
            crate::format::reset_private_scratch_high_water();
            let old = with_limit_usage_refusal(limit, || {
                if sticky {
                    assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
                }
                let output = crate::format::comments::canonical_with_comments(&program, &comments);
                let length = output.len();
                drop(output);
                length
            });
            let complete_walk = crate::format::private_scratch_high_water();
            for text in [expected.as_str(), "mismatch at the first byte"] {
                crate::format::reset_private_scratch_high_water();
                let current = with_limit_usage_refusal(limit, || {
                    if sticky {
                        assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
                    }
                    canonical_comparison(&program, &comments, text)
                        .bytes
                        .unwrap()
                });
                assert_eq!(
                    old, current,
                    "full traversal and every existing debit survive mismatch"
                );
                assert_eq!(crate::format::private_scratch_high_water(), complete_walk);
            }
        }
    }
    // The inner comparison does not carry a payer, restore a foreign budget,
    // relax a floor, or alter the enclosing ledger's retained refusal.
    let nested = |old: bool| {
        with_limit_usage_refusal(1024, || {
            assert!(crate::bounded_output::set_active_floor(512));
            let result = with_limit_usage_refusal(256, || {
                if old {
                    drop(crate::format::comments::canonical_with_comments(
                        &program, &comments,
                    ));
                } else {
                    let _ = canonical_comparison(&program, &comments, "wrong");
                }
            });
            crate::bounded_output::clear_active_floor();
            result
        })
    };
    assert_eq!(nested(true), nested(false));
}

#[test]
fn final_string_backing_is_removed_with_actual_allocated_reserved_peak_and_whole_drop_observation()
{
    for count in [0, 1, 64] {
        let mut source = String::from("module compare.physical;\n");
        for _ in 0..count {
            source.push_str(
                "// long comment café λ repeated immutable payload without a final output owner\n",
            );
        }
        source.push_str("@id(\"compare.physical.main\") fn main()->i64 {1+2*3}\n");
        let (program, comments) =
            crate::parse_with_comments(&source, Path::new("physical.spx")).unwrap();
        let expected = crate::format::comments::canonical_with_comments(&program, &comments);
        let old = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let output = crate::format::comments::canonical_with_comments(&program, &comments);
                assert_eq!(output, expected);
                drop(output);
            })
        });
        let current = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let output = canonical_comparison(&program, &comments, &expected);
                assert!(output.matches);
                assert_eq!(output.bytes, Some(expected.len()));
            })
        });
        assert_eq!((old.1, old.2, old.3), (current.1, current.2, current.3));
        let before = old.0 .1;
        let after = current.0 .1;
        for metrics in [before, after] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        assert!(after.allocated < before.allocated);
        assert!(after.allocations < before.allocations);
        assert!(after.reallocations <= before.reallocations);
        eprintln!(
            "canonical-comment-835 count={count} reserved={} old={before:?} current={after:?}",
            old.2
        );
    }
}

fn sources(comments: usize) -> Vec<WorkspaceSource> {
    let mut app = String::from("module cold.app;\n");
    for _ in 0..comments {
        app.push_str("// borrowed source comment café 🌍\n");
    }
    app.push_str("@id(\"cold.main\") fn main()->i64 { 1 + 2 }\n");
    [
        ("cold/app.spx", app),
        (
            "cold/leaf.spx",
            "module cold.leaf;\n@id(\"cold.leaf\") fn leaf()->i64 {0}\n".to_owned(),
        ),
    ]
    .into_iter()
    .map(|(path, raw)| {
        let (program, comments) = crate::parse_with_comments(&raw, Path::new(path)).unwrap();
        WorkspaceSource {
            path: path.to_owned(),
            source: format::comments::canonical_with_comments(&program, &comments),
        }
    })
    .collect()
}
fn run(
    old: bool,
    sources: Vec<WorkspaceSource>,
    limit: usize,
) -> Result<(WorkspaceGraphBuild, current_source::SourceFacts), Vec<Diagnostic>> {
    if old {
        before::build_owned_retaining_sources_with_frontend_limit(sources, limit, None, false, None)
    } else {
        build_owned_retaining_sources_with_frontend_limit(sources, limit, None, false, None)
    }
}
fn assert_same(
    old: &Result<(WorkspaceGraphBuild, current_source::SourceFacts), Vec<Diagnostic>>,
    current: &Result<(WorkspaceGraphBuild, current_source::SourceFacts), Vec<Diagnostic>>,
) {
    match (old, current) {
        (Err(old), Err(current)) => {
            assert_eq!(old, current, "all six Diagnostic fields and their order")
        }
        (Ok((old, old_sources)), Ok((current, current_sources))) => {
            assert_eq!(old.usage, current.usage);
            assert_eq!(old.edges, current.edges);
            assert_eq!(old.hir.declarations, current.hir.declarations);
            assert_eq!(old.hir.module_paths, current.hir.module_paths);
            assert_eq!(old.hir.dependency_depths, current.hir.dependency_depths);
            assert_eq!(old.hir.shared_prelude_ids, current.hir.shared_prelude_ids);
            assert_eq!(old_sources.len(), current_sources.len());
            for (a, b) in old_sources.iter().zip(current_sources.iter()) {
                assert_eq!(a.input().path, b.input().path);
                assert_eq!(a.input().source, b.input().source);
            }
            let input = old_sources
                .iter()
                .find(|source| source.input().path == "cold/app.spx")
                .unwrap()
                .input();
            let authored = crate::check(&input.source, &input.path).unwrap();
            let revision = crate::graph::revision(&authored);
            let old_hir = old.linked_scalar_program("cold.app").unwrap();
            let current_hir = current.linked_scalar_program("cold.app").unwrap();
            assert_eq!(
                crate::graph::to_hir_json(&old_hir, &revision).unwrap(),
                crate::graph::to_hir_json(&current_hir, &revision).unwrap()
            );
        }
        _ => panic!("complete cold preflight outcome changed"),
    }
}

#[test]
fn whole_cold_preflight_keeps_complete_old_source_graph_diagnostics_and_priority() {
    for count in [0, 1, 32] {
        let input = sources(count);
        let old = run(true, input.clone(), MAX_BUILDER_BYTES);
        let current = run(false, input.clone(), MAX_BUILDER_BYTES);
        assert!(old.is_ok());
        assert_same(&old, &current);
        for change in 0..5 {
            let mut altered = input.clone();
            match change {
                0 => altered[0].source.insert(0, ' '),
                1 => altered[0].source = altered[0].source.replace('\n', "\r\n"),
                2 => altered[1].path = altered[0].path.clone(),
                3 => altered[0].source.push_str("@id(\"broken\") fn broken(\n"),
                _ => altered[0].source.push_str("// misplaced trailing  \n"),
            }
            let old = run(true, altered.clone(), MAX_BUILDER_BYTES);
            let current = run(false, altered, MAX_BUILDER_BYTES);
            assert!(old.is_err());
            assert_same(&old, &current);
        }
    }
}

#[test]
fn whole_cold_preflight_exact_short_zero_builder_bounds_and_early_mismatch_keep_old_refusal() {
    let input = sources(2);
    let mut low = 0;
    let mut high = MAX_BUILDER_BYTES;
    assert!(run(true, input.clone(), high).is_ok());
    while low < high {
        let mid = low + (high - low) / 2;
        if run(true, input.clone(), mid).is_ok() {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    for limit in [0, low - 1, low, MAX_BUILDER_BYTES] {
        assert_same(
            &run(true, input.clone(), limit),
            &run(false, input.clone(), limit),
        );
    }
    assert!(run(false, input.clone(), low).is_ok());
    assert!(run(false, input.clone(), low - 1).is_err());
    let mut mismatch = input;
    mismatch[0].source.insert(0, ' ');
    for limit in [0, 1, low - 1, MAX_BUILDER_BYTES] {
        assert_same(
            &run(true, mismatch.clone(), limit),
            &run(false, mismatch.clone(), limit),
        );
    }
}

#[test]
fn whole_cold_preflight_observes_same_cold_setup_reserved_peak_and_complete_drop_without_output_string(
) {
    for count in [0, 1, 32] {
        let input = sources(count);
        // Both paths are cold (no frontend cache); initialize shared test TLS
        // before either observation, using the same complete owning receiver.
        assert_same(
            &run(true, input.clone(), MAX_BUILDER_BYTES),
            &run(false, input.clone(), MAX_BUILDER_BYTES),
        );
        let old = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let result = run(true, input.clone(), MAX_BUILDER_BYTES);
                assert!(result.is_ok());
                drop(result);
            })
        });
        let current = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let result = run(false, input.clone(), MAX_BUILDER_BYTES);
                assert!(result.is_ok());
                drop(result);
            })
        });
        assert_eq!((old.1, old.2, old.3), (current.1, current.2, current.3));
        let before = old.0 .1;
        let after = current.0 .1;
        for metrics in [before, after] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        assert!(after.allocated < before.allocated);
        assert!(after.allocations < before.allocations);
        eprintln!(
            "canonical-comment-835 whole-cold count={count} reserved={} old={before:?} current={after:?}",
            old.2
        );
    }
}

#[test]
fn complete_old_commented_formatter_preflight_sources_and_all_current_children_remain_exact() {
    let packet: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/canonical-comment-comparison-835/source-inverses.json"
    ))
    .unwrap();
    for row in packet["owners"].as_array().unwrap() {
        let (mut current, original) = match row["source"].as_str().unwrap() {
            "src/format/comments.rs" => (include_str!("../../format/comments.rs").to_owned(), include_str!("../../../docs/evidence/canonical-comment-comparison-835/original/comments.rs")),
            "src/workspace_graph.rs" => (include_str!("../../workspace_graph.rs").to_owned(), include_str!("../../../docs/evidence/canonical-comment-comparison-835/original/workspace_graph.rs")),
            "tests/workspace/semantic_graph/source_inventory.rs" => (include_str!("../../../tests/workspace/semantic_graph/source_inventory.rs").to_owned(), include_str!("../../../docs/evidence/canonical-comment-comparison-835/original/source_inventory.rs")),
            _ => panic!("unexpected complete source owner"),
        };
        current = crate::source_reflows_merge::consumer_input(
            "cold835",
            row["source"].as_str().unwrap(),
            &current,
        );
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            let old = edit["before"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1);
            current = current.replacen(new, old, 1);
        }
        assert_eq!(current, original);
    }
    let original = include_str!(
        "../../../docs/evidence/canonical-comment-comparison-835/original/workspace_graph.rs"
    );
    let start = original.find("fn build_owned_inner(").unwrap();
    let end = original[start..]
        .find("\nfn build_resolved_core_current(")
        .unwrap()
        + start;
    let body = original[start..end].replacen(
        "fn build_owned_inner(",
        "pub(super) fn build_owned_inner(",
        1,
    );
    let start = original
        .find("fn build_owned_retaining_sources_with_frontend_limit(")
        .unwrap();
    let end = original[start..]
        .find("\nfn active_builder_limit()")
        .unwrap()
        + start;
    let wrapper = original[start..end].replacen(
        "fn build_owned_retaining_sources_with_frontend_limit(",
        "pub(super) fn build_owned_retaining_sources_with_frontend_limit(",
        1,
    );
    assert_eq!(include_str!("before.rs"),format!("//! Complete immutable940 cold preflight; no comparison-sink delegation.\nuse super::super::*;\n{body}\n{wrapper}"));
    assert_eq!(
        include_str!("../../format.rs"),
        include_str!("../../../docs/evidence/canonical-comment-comparison-835/original/format.rs")
    );
    let children = [
        (
            "src/workspace_graph/cold_canonical.rs",
            include_str!("../cold_canonical.rs"),
        ),
        (
            "src/workspace_graph/cold_canonical/before.rs",
            include_str!("before.rs"),
        ),
        (
            "src/workspace_graph/cold_canonical/tests.rs",
            include_str!("tests.rs"),
        ),
        (
            "src/format/comments/compare.rs",
            include_str!("../../format/comments/compare.rs"),
        ),
        (
            "src/format/comments/compare/tests.rs",
            include_str!("../../format/comments/compare/tests.rs"),
        ),
    ];
    for (source, bytes) in children {
        let row = packet["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == source)
            .unwrap();
        let projected = crate::source_reflows_merge::consumer_child("cold835", source, bytes);
        let bytes = projected.as_str();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
    }
}
