//! Same-call borrowing only; no stored packet or acquisition cache.
use super::SOURCE_CAP;
use super::{actual, clippy_stage, digest, packet, read_view, value_4a6eb};
use serde_json::Value;
use std::path::Path;

#[rustfmt::skip] // Exact extracted historical lookup body.
pub(super) fn from_packet(packet: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
    else {
        return crate::source_reflows_merge::next_big_batch_inputs::value_local(
            source, expected, supplied,
        );
    };
    let Some(prior) = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["sha256"] == expected)
    else {
        return supplied.to_owned();
    };
    let current = actual(row);
    let previous = read_view(prior);
    if supplied == current || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}

#[rustfmt::skip] // Exact historical continuation; only first lookup is borrowed.
pub(super) fn with_packet(authenticated: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {
    // Current and prior logical-table views retain their complete scope before
    // the separately authenticated historical Clippy-stage acquisition.
    let direct = from_packet(authenticated, source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }
    if let Some(value) = clippy_stage::requested(source, expected, supplied) {
        let _ = packet();
        return value;
    }
    let normalized = clippy_stage::normalize(source, supplied);
    let result = value_4a6eb(source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::from_packet;
    use crate::source_reflows_merge::main_e177_final_inputs::digest;
    use std::path::PathBuf;

    struct Files(PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn files() -> Files {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("semaprax-packet-borrow-{}-{n}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Files(dir)
    }
    fn panic_text(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let panic = std::panic::catch_unwind(f).expect_err("fresh authentication must refuse");
        if let Some(text) = panic.downcast_ref::<String>() {
            text.clone()
        } else {
            panic.downcast_ref::<&str>().unwrap().to_string()
        }
    }

    #[test]
    fn registered_context_preserves_the_entire_original_reader_and_every_guard() {
        fn body<'a>(text: &'a str, signature: &str) -> &'a str {
            let start = text.find(&format!("\n{signature}")).unwrap() + 1;
            let length = text[start..].find("\n}\n").unwrap() + 3;
            &text[start..start + length]
        }
        let current = include_str!("main_e177_packet_lookup.rs");
        let before = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/1e31aba22d551ebabbfb57e2f0b1d36f6bb1517f816a1ed6413ebd732df66981"
        ));
        let original = body(before, "pub(super) fn registered_file(");
        let saved = body(
            current,
            "pub(super) fn registered_file_before_packet_context(",
        )
        .replacen(
            "registered_file_before_packet_context(",
            "registered_file(",
            1,
        );
        assert_eq!(saved, original);
        let restored = body(current, "pub(super) fn registered_file(")
            .replacen("actual_with_packet(&packet, row)", "super::actual(row)", 1)
            .replacen(
                "read_view_with_packet(&packet, selected)",
                "super::read_view(selected)",
                1,
            );
        assert_eq!(restored, original);
        // Keep the complete existing current/history/cap/child/unknown-input controls.
        super::registered_digest_reads_keep_current_history_and_each_original_file_bound();
    }

    #[test]
    fn borrowed_registered_reader_matches_original_complete_current_and_history() {
        let packet = super::super::packet();
        let row = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == "src/ast/program_source/decl_slices.rs")
            .unwrap();
        let source = row["source"].as_str().unwrap();
        for pin in [&row["current"], &row["views"][0]] {
            let expected = pin["sha256"].as_str().unwrap();
            let old = super::registered_file_before_packet_context(source, expected, usize::MAX);
            let borrowed = super::registered_file(source, expected, usize::MAX);
            assert_eq!(borrowed, old);
            assert_eq!(
                borrowed.unwrap(),
                std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join(pin["archive"].as_str().unwrap())
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn borrowed_lookup_rechecks_current_on_each_invocation_before_history() {
        let files = files();
        let current = files.0.join("current.rs");
        let old = files.0.join("prior.rs");
        std::fs::write(&current, b"new!").unwrap();
        std::fs::write(&old, b"old!").unwrap();
        let source = current.to_str().unwrap();
        let expected = digest(b"old!");
        let packet = serde_json::json!({"rows":[{"source":source,"compiled":false,
            "current":{"archive":source,"length":4,"sha256":digest(b"new!")},
            "views":[{"archive":old.to_str().unwrap(),"length":4,"sha256":expected}]}]});
        assert_eq!(from_packet(&packet, source, &expected, "new!"), "old!");
        assert_eq!(
            from_packet(&packet, source, &expected, "foreign"),
            "foreign"
        );
        // A second invocation with the same borrowed metadata still reads the
        // physical current channel; missing history cannot precede its refusal.
        std::fs::write(&current, b"drft").unwrap();
        std::fs::remove_file(&old).unwrap();
        assert!(panic_text(|| {
            from_packet(&packet, source, &expected, "old!");
        })
        .contains("length-or-digest"));
        std::fs::write(&current, b"new!extra").unwrap();
        assert!(panic_text(|| {
            from_packet(&packet, source, &expected, "new!");
        })
        .contains("length-or-identity"));
        std::fs::write(&current, [0xff; 4]).unwrap();
        let mut invalid_utf8 = packet.clone();
        invalid_utf8["rows"][0]["current"]["sha256"] = digest(&[0xff; 4]).into();
        assert!(panic_text(|| {
            from_packet(&invalid_utf8, source, &expected, "new!");
        })
        .contains("utf8"));
        std::fs::write(&current, b"new!").unwrap();
        assert!(panic_text(|| {
            from_packet(&packet, source, &expected, "new!");
        })
        .contains("missing"));
    }

    #[test]
    fn packet_borrowing_preserves_complete_old_root_and_both_fresh_fallbacks() {
        let before = include_str!("main_e177_packet_lookup_before.rs.txt");
        assert_eq!(
            digest(before.as_bytes()),
            "b8b5ca81b4e0001c7dc8ffc354da34cf9966f7dee840b58f7e1265c2806479e6"
        );
        let current = super::root_before_nested_context(include_str!("main_e177_final_inputs.rs"));
        let child = include_str!("main_e177_packet_lookup.rs");
        let old_value = before
            .split_once("pub(crate) fn value(source:")
            .unwrap()
            .1
            .split_once("fn value_4a6eb(")
            .unwrap()
            .0;
        let old_lookup = before
            .split_once("fn value_4a6eb(")
            .unwrap()
            .1
            .split_once("#[test]\nfn latest_main")
            .unwrap()
            .0;
        let child_lookup = child.split_once("pub(super) fn from_packet(packet: &serde_json::Value, source:").unwrap().1
            .split_once("#[rustfmt::skip] // Exact historical continuation; only first lookup is borrowed.\npub(super) fn with_packet(").unwrap().0;
        assert_eq!(
            child_lookup,
            old_lookup.strip_prefix("source:").unwrap().replacen(
                "    let packet = packet();\n",
                "",
                1
            ) + "\n"
        );
        let child_value = child
            .split_once("pub(super) fn with_packet(authenticated: &serde_json::Value, source:")
            .unwrap()
            .1
            .split_once("#[cfg(test)]\nmod tests")
            .unwrap()
            .0;
        assert_eq!(
            child_value,
            old_value.replacen(
                "let direct = value_4a6eb(source, expected, supplied);",
                "let direct = from_packet(authenticated, source, expected, supplied);",
                1
            ) + "\n"
        );
        let mut restored = current.replace(
            "#[path = \"main_e177_packet_lookup.rs\"]\nmod packet_lookup;\n",
            "",
        );
        let invocation_export = "pub(crate) use packet_lookup::ProjectionInvocation;\n";
        assert_eq!(restored.matches(invocation_export).count(), 1);
        restored = restored.replacen(invocation_export, "", 1);
        let new_value = "pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {\n    packet_lookup::with_packet(&packet(), source, expected, supplied)\n}\n";
        let new_lookup = "fn value_4a6eb(source: &str, expected: &str, supplied: &str) -> String {\n    packet_lookup::from_packet(&packet(), source, expected, supplied)\n}\n";
        assert_eq!(restored.matches(new_value).count(), 1);
        restored = restored.replacen(
            new_value,
            &("pub(crate) fn value(source:".to_owned() + old_value),
            1,
        );
        assert_eq!(restored.matches(new_lookup).count(), 1);
        restored = restored.replacen(new_lookup, &("fn value_4a6eb(".to_owned() + old_lookup), 1);
        let reuse = "        packet_lookup::with_packet(&current, source, expected, supplied)\n";
        assert_eq!(restored.matches(reuse).count(), 1);
        restored = restored.replacen(reuse, "        value(source, expected, supplied)\n", 1);
        assert_eq!(restored, before, "complete independently pinned old root");
    }

    #[test]
    fn same_invocation_file_context_reuses_metadata_and_keeps_fresh_reads_foreign_refusals_and_drop(
    ) {
        use super::ProjectionInvocation;
        let files = files();
        let current = files.0.join("current.rs");
        let old = files.0.join("prior.rs");
        std::fs::write(&current, b"new!").unwrap();
        std::fs::write(&old, b"old!").unwrap();
        let source = current.to_str().unwrap();
        let expected = digest(b"old!");
        let fixture = serde_json::json!({"unknown_root":{"retained":[1,2]},
            "rows":[{"source":source,"compiled":false,"unknown_metadata":{"retained":"whole"},
            "current":{"archive":source,"length":4,"sha256":digest(b"new!")},
            "views":[{"archive":old.to_str().unwrap(),"length":4,"sha256":expected}]}]});
        let ((), heap) = crate::test_allocation_observer::measure(|| {
            let mut context = ProjectionInvocation::new();
            assert!(
                context.packet.is_none(),
                "a new invocation owns no prior packet"
            );
            assert!(context
                .file(std::path::Path::new("foreign"), 4, &expected)
                .is_none());
            assert!(
                context.packet.is_none(),
                "foreign paths retain the pre-packet refusal"
            );
            // Only this private control installs a synthetic immutable packet.
            // The public constructor always acquires the complete real packet.
            context.packet = Some(fixture.clone());
            let identity = context.metadata() as *const serde_json::Value;
            for _ in 0..2 {
                assert_eq!(
                    context.file_row(source, 4, &expected),
                    Some("old!".to_owned())
                );
                assert_eq!(from_packet(&fixture, source, &expected, "new!"), "old!");
                assert_eq!(context.value_direct_row(source, &expected, "new!"), "old!");
                assert_eq!(context.value_direct(source, &expected, "old!"), "old!");
                assert_eq!(
                    from_packet(&fixture, source, &expected, "foreign"),
                    "foreign"
                );
                assert_eq!(context.metadata() as *const serde_json::Value, identity);
                assert_eq!(
                    context.metadata(),
                    &fixture,
                    "every unknown field remains whole"
                );
            }
            assert!(context.file_row(source, 5, &expected).is_none());
            assert!(context.file_row(source, 4, &"0".repeat(64)).is_none());
            assert!(context.file_row("foreign", 4, &expected).is_none());
            drop(context);
        });
        assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
        // Refusal panics are outside the complete normal-Drop measurement.
        let mut context = ProjectionInvocation::new();
        context.packet = Some(fixture.clone());
        assert_eq!(
            context.file_row(source, 4, &expected),
            Some("old!".to_owned())
        );
        std::fs::write(&current, b"drft").unwrap();
        std::fs::remove_file(&old).unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.file_row(source, 4, &expected);
        }))
        .contains("length-or-digest"));
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.value_direct(source, &expected, "new!");
        }))
        .contains("length-or-digest"));
        drop(context);
        let mut next = ProjectionInvocation::new();
        assert!(
            next.packet.is_none(),
            "Drop cannot publish metadata into the next invocation"
        );
        next.packet = Some(fixture);
        std::fs::write(&current, b"new!").unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            next.file_row(source, 4, &expected);
        }))
        .contains("missing"));
        // Common public endpoint methods use one lazy loader for this call only.
        let loads = std::cell::Cell::new(0usize);
        std::fs::write(&old, b"old!").unwrap();
        let fixture = serde_json::json!({"unknown_root":{"kept":true},"rows":[{
            "source":source,"compiled":false,"unknown_metadata":[1,2],
            "current":{"archive":source,"length":4,"sha256":digest(b"new!")},
            "views":[{"archive":old.to_str().unwrap(),"length":4,"sha256":expected}]}]});
        let ((), heap) = crate::test_allocation_observer::measure(|| {
            for invocation in 1..=2 {
                let mut context = ProjectionInvocation {
                    packet: None,
                    acquire: || {
                        loads.set(loads.get() + 1);
                        fixture.clone()
                    },
                };
                assert_eq!(context.value(source, &expected, "new!"), "old!");
                assert_eq!(context.value_direct(source, &expected, "new!"), "old!");
                assert_eq!(
                    context.file_row_with_context(source, 4, &expected),
                    Some("old!".to_owned())
                );
                assert_eq!(
                    super::from_packet_with_context(
                        context.metadata(),
                        source,
                        &expected,
                        "foreign"
                    ),
                    "foreign"
                );
                assert_eq!(loads.get(), invocation);
                assert_eq!(context.metadata(), &fixture);
                drop(context);
            }
        });
        assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
        // Metadata reuse never hides fresh-current failure or the missing archive.
        let mut context = ProjectionInvocation {
            packet: None,
            acquire: || fixture.clone(),
        };
        assert_eq!(context.value(source, &expected, "new!"), "old!");
        std::fs::write(&current, b"drft").unwrap();
        std::fs::remove_file(&old).unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.value(source, &expected, "new!");
        }))
        .contains("length-or-digest"));
        std::fs::write(&current, b"new!").unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.file_row_with_context(source, 4, &expected);
        }))
        .contains("missing"));
        // The borrowed row lookup is the complete original file lookup suffix.
        let root = include_str!("main_e177_final_inputs.rs");
        let old = root
            .split_once("fn file_4a6eb(")
            .unwrap()
            .1
            .split_once("    let packet = packet();\n")
            .unwrap()
            .1
            .split_once("pub(crate) fn value(")
            .unwrap()
            .0;
        let child = include_str!("main_e177_packet_lookup.rs");
        let actual = child
            .rsplit_once("    fn file_row(&mut self,")
            .unwrap()
            .1
            .split_once("        let packet = self.metadata();\n")
            .unwrap()
            .1
            .split_once("\n}\n// END INVOCATION-LOCAL PROJECTION METADATA")
            .unwrap()
            .0;
        assert_eq!(actual, old.trim_end_matches('\n'));
    }
}

fn held_with_packet(
    authenticated: &serde_json::Value,
    path: &Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    if let Some(result) =
        super::overlay_v2::held_source_with_packet(authenticated, path, length, expected)
    {
        return result;
    }
    super::held_4a6eb(path, length, expected)
}
fn read_view_with_packet(authenticated: &serde_json::Value, view: &serde_json::Value) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    held_with_packet(
        authenticated,
        &root.join(view["archive"].as_str().unwrap()),
        view["length"].as_u64().unwrap() as usize,
        view["sha256"].as_str().unwrap(),
    )
    .unwrap()
}
fn actual_with_packet(authenticated: &serde_json::Value, row: &serde_json::Value) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = row["source"].as_str().unwrap();
    let current = &row["current"];
    let value = held_with_packet(
        authenticated,
        &root.join(source),
        current["length"].as_u64().unwrap() as usize,
        current["sha256"].as_str().unwrap(),
    )
    .unwrap();
    if row["compiled"].as_bool().unwrap() {
        assert_eq!(
            Some(value.as_str()),
            super::compiled(source),
            "complete actual compiled source"
        );
    }
    assert_eq!(
        value,
        read_view_with_packet(authenticated, current),
        "complete actual archive"
    );
    if let Some(children) = row.get("fragments") {
        for child in children.as_array().unwrap() {
            assert!(child.get("fragments").is_none());
            // Authenticate actual same-scope compiled bytes before returning
            // the shortened parent's complete historical source view.
            actual_with_packet(authenticated, child);
        }
    }

    value
}
#[rustfmt::skip]
fn from_packet_with_context(packet: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
    else {
        return crate::source_reflows_merge::next_big_batch_inputs::value_local(
            source, expected, supplied,
        );
    };
    let Some(prior) = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["sha256"] == expected)
    else {
        return supplied.to_owned();
    };
    let current = actual_with_packet(packet, row);
    let previous = read_view_with_packet(packet, prior);
    if supplied == current || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}

#[rustfmt::skip]
fn with_packet_context(authenticated: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {
    // Current and prior logical-table views retain their complete scope before
    // the separately authenticated historical Clippy-stage acquisition.
    let direct = from_packet_with_context(authenticated, source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }
    if let Some(value) = clippy_stage::requested(source, expected, supplied) {
        let _ = authenticated;
        return value;
    }
    let normalized = clippy_stage::normalize(source, supplied);
    let result = value_4a6eb(source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}

#[cfg(test)]
fn root_before_nested_context(current: &str) -> String {
    // Main adds exactly one API forwarder and its executable regression.
    // Remove only their complete bytes before the unchanged release inverse.
    let registered_predecessor;
    let current = if current.contains("pub(crate) fn registered_file(") {
        let mut restored = current.to_owned();
        assert_eq!(restored.matches("pub(crate) fn registered_file(source: &str, expected: &str, cap: usize) -> Option<String> {\n    packet_lookup::registered_file(source, expected, cap)\n}\n\n").count(), 1);
        restored = restored.replacen("pub(crate) fn registered_file(source: &str, expected: &str, cap: usize) -> Option<String> {\n    packet_lookup::registered_file(source, expected, cap)\n}\n\n", "", 1);
        assert_eq!(restored.matches("\n#[test]\nfn registered_digest_reads_keep_current_history_and_each_original_file_bound() {\n    packet_lookup::registered_digest_reads_keep_current_history_and_each_original_file_bound();\n}\n").count(), 1);
        restored = restored.replacen("\n#[test]\nfn registered_digest_reads_keep_current_history_and_each_original_file_bound() {\n    packet_lookup::registered_digest_reads_keep_current_history_and_each_original_file_bound();\n}\n", "", 1);
        registered_predecessor = restored;
        registered_predecessor.as_str()
    } else {
        current
    };
    assert_eq!(current.len(), 112797);
    assert_eq!(
        digest(current.as_bytes()),
        "038f6b2515f59c4df7a11a8718b66f0904b2d07cb9544346eef2e3482a205467"
    );
    let mut restored = current.to_owned();
    assert_eq!(restored.matches("pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value_direct(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_direct_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {").count(), 1);
    restored = restored.replacen("pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value_direct(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_direct_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {", "pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {", 1);
    assert_eq!(restored.matches("pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {").count(), 1);
    restored = restored.replacen("pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {", "pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {", 1);
    assert_eq!(restored.matches("pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {\n    ProjectionInvocation::new().file(path, length, expected)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn file_before_nested_context(path: &Path, length: usize, expected: &str) -> Option<String> {").count(), 1);
    restored = restored.replacen("pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {\n    ProjectionInvocation::new().file(path, length, expected)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn file_before_nested_context(path: &Path, length: usize, expected: &str) -> Option<String> {", "pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {", 1);
    assert_eq!(restored.len(), 112069);
    assert_eq!(
        digest(restored.as_bytes()),
        "c99aa4c8f163fd93f2c5ab7dc050bbd69e0d2a5568f1faf2a02e8c62faf533a5"
    );
    restored
}

// BEGIN INVOCATION-LOCAL PROJECTION METADATA
// Each outer projection acquisition owns one lazy immutable metadata packet.
// Current sources, compiled bytes and current/prior archives are never cached.
pub(crate) struct ProjectionInvocation<F = fn() -> serde_json::Value> {
    packet: Option<serde_json::Value>,
    acquire: F,
}
impl ProjectionInvocation {
    pub(crate) fn new() -> Self {
        Self {
            packet: None,
            acquire: packet,
        }
    }
}
#[rustfmt::skip] // Keep the complete old file-lookup suffix available verbatim.
impl<F: FnMut() -> serde_json::Value> ProjectionInvocation<F> {
    fn metadata(&mut self) -> &serde_json::Value {
        self.packet.get_or_insert_with(&mut self.acquire)
    }
    pub(crate) fn value(&mut self, source: &str, expected: &str, supplied: &str) -> String {
        with_packet_context(self.metadata(), source, expected, supplied)
    }
    pub(crate) fn file(&mut self, path: &std::path::Path, length: usize, expected: &str) -> Option<String> {
        // Original Clippy read precedes metadata acquisition on every lookup.
        if let Some(Ok(value)) = clippy_stage::read(path, length, expected) {
            let _ = self.metadata();
            return Some(value);
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let relative = path.strip_prefix(root).ok()?;
        if !relative
            .components()
            .all(|p| matches!(p, std::path::Component::Normal(_)))
        {
            return None;
        }
        let source = relative.to_str()?;
        self.file_row_with_context(source, length, expected)
    }

    // Reuse only immutable canonical metadata; all lookup readers and fallbacks stay live.
    pub(crate) fn value_direct(&mut self, source: &str, expected: &str, supplied: &str) -> String {
        // Preserve the original nondelegating domain before any Clippy stage.
        if !self.metadata()["rows"].as_array().unwrap().iter().any(|row| row["source"] == source) {
            return supplied.to_owned();
        }
        // A whole logical-table view is selected only after its actual current
        // physical, compiled and archive channels have authenticated. Preserve the
        // complete historical stage fallback below for every other input.
        let direct = self.value_direct_row(source, expected, supplied);
        if digest(direct.as_bytes()) == expected {
            return direct;
        }

        if let Some(value) = clippy_stage::requested(source, expected, supplied) {
            let _ = self.metadata();
            return value;
        }
        let normalized = clippy_stage::normalize(source, supplied);
        let result = self.value_direct_row(source, expected, normalized.as_ref());
        if normalized.as_ref() != supplied
            && result == normalized.as_ref()
            && digest(result.as_bytes()) != expected
        {
            supplied.to_owned()
        } else {
            result
        }
    }

    fn value_direct_row(&mut self, source: &str, expected: &str, supplied: &str) -> String {
        let current = self.metadata();
        if current["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["source"] == source)
        {
            with_packet_context(current, source, expected, supplied)
        } else {
            supplied.to_owned()
        }
    }

    fn combined_side_actual(&mut self, row: &serde_json::Value) -> String {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = row["source"].as_str().unwrap();
        let current = &row["current"];
        let actual = self.file(
            &root.join(source),
            current["length"].as_u64().unwrap() as usize,
            current["sha256"].as_str().unwrap(),
        )
        .unwrap_or_else(|| {
            held_with_packet(self.metadata(),
                &root.join(source),
                current["length"].as_u64().unwrap() as usize,
                current["sha256"].as_str().unwrap(),
            )
            .unwrap()
        });
        let compiled = self.value_direct(
            source,
            current["sha256"].as_str().unwrap(),
            super::combined_side_compiled(source).unwrap(),
        );
        assert_eq!(
            actual, compiled,
            "complete combined current compiled source"
        );
        assert_eq!(
            actual,
            read_view_with_packet(self.metadata(), current),
            "complete combined raw-carrier archive"
        );
        actual
    }

    pub(crate) fn combined_side_file(&mut self, path: &Path, length: usize, expected: &str) -> Option<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let relative = path.strip_prefix(root).ok()?;
        if !relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return None;
        }
        let source = relative.to_str()?;
        let packet = super::combined_side_packet();
        let row = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)?;
        let previous =
            row["views"].as_array().unwrap().iter().find(|old| {
                old["length"].as_u64() == Some(length as u64) && old["sha256"] == expected
            })?;
        self.combined_side_actual(row);
        Some(read_view_with_packet(self.metadata(), previous))
    }

    fn file_row_with_context(&mut self, source: &str, length: usize, expected: &str) -> Option<String> {
        let authenticated = self.metadata();
        let row = authenticated["rows"].as_array().unwrap().iter().find(|r| r["source"] == source)?;
        let prior = row["views"].as_array().unwrap().iter()
            .find(|v| v["length"].as_u64() == Some(length as u64) && v["sha256"] == expected)?;
        actual_with_packet(authenticated, row);
        Some(read_view_with_packet(authenticated, prior))
    }
    #[allow(dead_code, reason = "Invocable complete predecessor lookup suffix")]
    fn file_row(&mut self, source: &str, length: usize, expected: &str) -> Option<String> {
        let packet = self.metadata();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)?;
    let prior = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["length"].as_u64() == Some(length as u64) && v["sha256"] == expected)?;
    actual(row);
    Some(read_view(prior))
}
}
// END INVOCATION-LOCAL PROJECTION METADATA

// Caller-cap lookup retained verbatim in the already audited sibling.
#[rustfmt::skip]
pub(super) fn registered_file(source: &str, expected: &str, cap: usize) -> Option<String> {
    if cap == 0
        || !Path::new(source)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        || expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let cap = cap.min(SOURCE_CAP);
    let packet = super::packet();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)?;
    let selected = std::iter::once(&row["current"])
        .chain(row["views"].as_array().unwrap().iter())
        .find(|pin| pin["sha256"] == expected)?;
    let fits = |pin: &Value| pin["length"].as_u64().unwrap() <= cap as u64;
    if !fits(selected) || !fits(&row["current"]) {
        return None;
    }
    if let Some(children) = row.get("fragments") {
        for child in children.as_array().unwrap() {
            assert!(child.get("fragments").is_none());
            if !fits(&child["current"]) {
                return None;
            }
        }
    }
    let current = actual_with_packet(&packet, row);
    if selected == &row["current"] {
        Some(current)
    } else {
        Some(read_view_with_packet(&packet, selected))
    }
}

pub(super) fn registered_digest_reads_keep_current_history_and_each_original_file_bound() {
    use super::registered_file;
    let packet = super::packet();
    let rows = packet["rows"].as_array().unwrap();
    let source = "src/source_reflows_merge/runtime_json.rs";
    let row = rows.iter().find(|row| row["source"] == source).unwrap();
    let current = super::actual(row);
    let current_sha = row["current"]["sha256"].as_str().unwrap();
    assert_eq!(
        registered_file(source, current_sha, SOURCE_CAP),
        Some(current.clone())
    );
    assert!(registered_file(source, current_sha, current.len() - 1).is_none());
    assert!(registered_file(source, current_sha, 0).is_none());
    assert!(registered_file(source, &"0".repeat(64), SOURCE_CAP).is_none());
    assert!(registered_file("foreign-unregistered.rs", current_sha, SOURCE_CAP).is_none());
    assert!(registered_file(
        "../src/source_reflows_merge/runtime_json.rs",
        current_sha,
        SOURCE_CAP
    )
    .is_none());
    assert!(registered_file(source, "not-a-digest", SOURCE_CAP).is_none());
    let prior = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pin| pin["length"].as_u64().unwrap() < current.len() as u64)
        .unwrap();
    let prior_sha = prior["sha256"].as_str().unwrap();
    let before = super::read_view(prior);
    assert_eq!(
        registered_file(source, prior_sha, SOURCE_CAP),
        Some(before.clone())
    );
    // A short historical output cannot hide a larger current source read.
    assert!(registered_file(source, prior_sha, before.len()).is_none());
    let parent = rows
        .iter()
        .find(|row| {
            row.get("fragments").is_some_and(|children| {
                children.as_array().unwrap().iter().any(|child| {
                    child["current"]["length"].as_u64().unwrap()
                        > row["current"]["length"].as_u64().unwrap()
                })
            })
        })
        .unwrap();
    let source = parent["source"].as_str().unwrap();
    let sha = parent["current"]["sha256"].as_str().unwrap();
    let length = parent["current"]["length"].as_u64().unwrap() as usize;
    assert!(registered_file(source, sha, length).is_none());
    assert_eq!(
        registered_file(source, sha, SOURCE_CAP),
        Some(super::actual(parent))
    );
}

#[test]
fn combined_side_invocation_checks_current_and_complete_prior_before_returning() {
    let packet = super::combined_side_packet();
    let row = &packet["rows"].as_array().unwrap()[0];
    let pin = &row["views"].as_array().unwrap()[0];
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(row["source"].as_str().unwrap());
    let mut invocation = ProjectionInvocation::new();
    assert_eq!(
        invocation.combined_side_file(
            &path,
            pin["length"].as_u64().unwrap() as usize,
            pin["sha256"].as_str().unwrap()
        ),
        Some(super::read_view(pin))
    );
    assert!(invocation
        .combined_side_file(
            &path,
            pin["length"].as_u64().unwrap() as usize,
            &"0".repeat(64)
        )
        .is_none());
    assert!(invocation
        .combined_side_file(&root.join("foreign-unregistered.rs"), 0, &"0".repeat(64))
        .is_none());
}

// REGISTERED_CONTEXT_PREDECESSOR_BEGIN
#[allow(dead_code, reason = "Complete original fresh reader remains invocable for comparison")]
#[rustfmt::skip]
pub(super) fn registered_file_before_packet_context(source: &str, expected: &str, cap: usize) -> Option<String> {
    if cap == 0
        || !Path::new(source)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        || expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let cap = cap.min(SOURCE_CAP);
    let packet = super::packet();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)?;
    let selected = std::iter::once(&row["current"])
        .chain(row["views"].as_array().unwrap().iter())
        .find(|pin| pin["sha256"] == expected)?;
    let fits = |pin: &Value| pin["length"].as_u64().unwrap() <= cap as u64;
    if !fits(selected) || !fits(&row["current"]) {
        return None;
    }
    if let Some(children) = row.get("fragments") {
        for child in children.as_array().unwrap() {
            assert!(child.get("fragments").is_none());
            if !fits(&child["current"]) {
                return None;
            }
        }
    }
    let current = super::actual(row);
    if selected == &row["current"] {
        Some(current)
    } else {
        Some(super::read_view(selected))
    }
}
// REGISTERED_CONTEXT_PREDECESSOR_END
