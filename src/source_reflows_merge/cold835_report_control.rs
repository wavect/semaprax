//! Exact entry into the original cold formatter source consumer.
use std::{borrow::Cow, path::Path};

type Edit = (usize, &'static str, &'static str);
type SourceInverse = (
    &'static str,
    &'static str,
    usize,
    &'static str,
    &'static [Edit],
);

pub(super) fn entry<'a>(domain: &str, source: &str, supplied: &'a str) -> Cow<'a, str> {
    if domain != "cold835" {
        return Cow::Borrowed(supplied);
    }
    let (compiled, current_sha, old_len, old_sha, edits): SourceInverse = match source {
        "src/format/comments.rs" => (
            include_str!("../format/comments.rs"),
            "e8cfdd112b18bda68da2fd4f5cec2a12bb9d3b0f7077077670b34c59fe14b72c",
            15917,
            "aab12d29902ab888d7ec9ea5386f27d8752194ade2019175bba2949f84094174",
            &[(
                2085,
                "#[cfg(test)]\npub(crate) use compare::with_materialized_control;\n",
                "",
            )],
        ),
        "src/format/comments/compare.rs" => (
            include_str!("../format/comments/compare.rs"),
            "96fa2d33d01c4f45b442b0f929321d26402572194fc79808096d2e56afbfb77b",
            1780,
            "b6407f4ce1b02e619e420b179d48b766191373239ea436308c3a3226a4c98f8d",
            &[
                (
                    1403,
                    "// The owning cold builder supplies both arms' exact current report controls.\n#[cfg(test)]\nthread_local! {\n    static MATERIALIZED_CONTROL: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };\n}\n\n#[cfg(test)]\npub(crate) fn with_materialized_control<T>(operation: impl FnOnce() -> T) -> (T, usize) {\n    struct Reset;\n    impl Drop for Reset {\n        fn drop(&mut self) {\n            MATERIALIZED_CONTROL.set(None);\n        }\n    }\n    assert!(MATERIALIZED_CONTROL.get().is_none());\n    MATERIALIZED_CONTROL.set(Some(0));\n    let reset = Reset;\n    let output = operation();\n    let calls = MATERIALIZED_CONTROL.get().unwrap();\n    drop(reset);\n    (output, calls)\n}\n\n",
                    "",
                ),
                (
                    2211,
                    "    #[cfg(test)]\n    if let Some(calls) = MATERIALIZED_CONTROL.get() {\n        MATERIALIZED_CONTROL.set(Some(calls + 1));\n        let output = super::canonical_with_comments(program, comments);\n        return Comparison {\n            bytes: Some(output.len()),\n            matches: output == expected,\n        };\n    }\n",
                    "",
                ),
            ],
        ),
        "src/workspace_graph/cold_canonical/tests.rs" => (
            include_str!("../workspace_graph/cold_canonical/tests.rs"),
            "d0243b4269d6a985bd1d12e24ea5df4f9281850947ee5e66ca68bd4e227d0ed1",
            16268,
            "e2eba449e74c96adea795867aa18830c952a2e83225007808e0e0a07f25e88ec",
            &[
                (
                    6093,
                    "    let expected_sources = sources.len();\n    let build =\n        || build_owned_retaining_sources_with_frontend_limit(sources, limit, None, false, None);\n",
                    "",
                ),
                (
                    6261,
                    "        let (result, formatted) = format::comments::with_materialized_control(build);\n        if result.is_ok() {\n            assert_eq!(\n                formatted, expected_sources,\n                \"materialized control covers every module\"\n            );\n        }\n        result\n",
                    "        before::build_owned_retaining_sources_with_frontend_limit(sources, limit, None, false, None)\n",
                ),
                (
                    6556,
                    "        build()\n",
                    "        build_owned_retaining_sources_with_frontend_limit(sources, limit, None, false, None)\n",
                ),
                (
                    6580,
                    "\n// Keep the complete historical builder executable with its own report tariff.\n#[test]\nfn complete_frozen_cold_builder_still_authenticates_original_inputs() {\n    let input = sources(2);\n    let (build, observed) = before::build_owned_retaining_sources_with_frontend_limit(\n        input.clone(),\n        MAX_BUILDER_BYTES,\n        None,\n        false,\n        None,\n    )\n    .unwrap();\n    assert_eq!(observed.len(), input.len());\n    for (source, expected) in observed.iter().zip(input) {\n        assert_eq!(source.input().path, expected.path);\n        assert_eq!(source.input().source, expected.source);\n    }\n    assert_eq!(\n        build\n            .linked_scalar_program(\"cold.app\")\n            .unwrap()\n            .entrypoint\n            .as_str(),\n        \"cold.main\"\n    );\n}\n\n",
                    "",
                ),
            ],
        ),
        _ => return Cow::Borrowed(supplied),
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = super::super::main_e177_final_inputs::held_4a6eb(
        &root.join(source),
        compiled.len(),
        current_sha,
    )
    .expect("exact current cold formatter source");
    assert_eq!(physical, compiled);
    let mut previous = physical;
    for &(start, current, old) in edits.iter().rev() {
        let end = start.checked_add(current.len()).unwrap();
        assert_eq!(previous.get(start..end), Some(current));
        previous.replace_range(start..end, old);
    }
    assert_eq!(previous.len(), old_len);
    assert_eq!(super::super::source_sha(&previous), old_sha);
    assert!(supplied == compiled || supplied == previous);
    Cow::Owned(previous)
}

pub(super) fn historical_project_entry(
    source: &str,
    expected: &str,
    supplied: &str,
) -> Option<String> {
    if source != "src/workspace_graph/cold_canonical/tests.rs"
        || expected != "ecc2a07ac366adeaddc986c5bf1c676000e95ec98f5dd526c37c78f65406d62c"
        || super::super::source_sha(supplied)
            != "e2eba449e74c96adea795867aa18830c952a2e83225007808e0e0a07f25e88ec"
    {
        return None;
    }
    let previous = entry("cold835", source, supplied);
    let mut restored = previous.into_owned();
    for (offset, current, old) in [
        (11969, "    let packet: serde_json::Value =\n        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n            crate::source_reflows_merge::ReceiverFixture::ColdCanonical,\n        ))\n        .unwrap();\n", "    let packet: serde_json::Value = serde_json::from_str(include_str!(\n        \"../../../docs/evidence/canonical-comment-comparison-835/source-inverses.json\"\n    ))\n    .unwrap();\n"),
        (14420, "    assert_eq!(crate::source_reflows_merge::receiver_child_entry(\n        \"src/workspace_graph/cold_canonical/before.rs\", include_str!(\"before.rs\")\n    ),format!(\"//! Complete immutable940 cold preflight; no comparison-sink delegation.\\nuse super::super::*;\\n{body}\\n{wrapper}\"));\n", "    assert_eq!(include_str!(\"before.rs\"),format!(\"//! Complete immutable940 cold preflight; no comparison-sink delegation.\\nuse super::super::*;\\n{body}\\n{wrapper}\"));\n"),
        (14717, "        crate::source_reflows::original_fixture_source(\n            \"src/format.rs\",\n            \"a0afc1d0c8a137742ae524f57e1e2fc585c2cbff808c932e1bd1e0081b358a6e\",\n            include_str!(\"../../format.rs\")\n        ),\n", "        include_str!(\"../../format.rs\"),\n"),
    ].into_iter().rev() {
        let end = offset + current.len();
        assert_eq!(restored.get(offset..end), Some(current));
        restored.replace_range(offset..end, old);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let archived = super::super::main_e177_final_inputs::held_4a6eb(
        &root.join("e").join(expected),
        15940,
        expected,
    )
    .expect("complete cold formatter projector predecessor");
    assert_eq!(restored, archived);
    Some(restored)
}
