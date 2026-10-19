//! Same-call borrowing only; no stored packet or acquisition cache.
use super::{actual, clippy_stage, digest, packet, read_view, value_4a6eb};
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
    fn same_invocation_file_context_reuses_metadata_and_keeps_fresh_reads_foreign_refusals_and_drop() {
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
        // This real legacy-only subject has no row in this invocation packet.
        // The retained old route authenticates physical/compiled/archive bytes
        // and selects its historical view; the stricter public route must not.
        let legacy_source = "src/workspace_graph/constructor_failure/report/stages.rs";
        let legacy_current = include_str!("../workspace_graph/constructor_failure/report/stages.rs");
        let legacy_before = include_str!("../../docs/evidence/clippy-4a6eb/YyMz16gZziO27eJCd12onKvQ2XldkIBHpW6o7FjczCo.txt");
        let legacy_expected = digest(legacy_before.as_bytes());
        assert_eq!(legacy_expected, "632333d7a819ce23b6ede242775da89cabd0d9795d908047a56ea8ec58dccc2a");
        assert_eq!(digest(legacy_current.as_bytes()), "fdfbc8c20782be5b3af2537c4cee8cda100937c62bdd6f3bb049497a24eee1f9");
        let mut legacy_context = ProjectionInvocation::new();
        assert!(!fixture["rows"].as_array().unwrap().iter().any(|row| row["source"] == legacy_source));
        legacy_context.packet = Some(fixture.clone());
        assert_eq!(legacy_context.value_direct_before_closed_membership(legacy_source, &legacy_expected, legacy_current), legacy_before);
        assert_ne!(legacy_current, legacy_before);
        assert_eq!(legacy_context.value_direct(legacy_source, &legacy_expected, legacy_current), legacy_current);
        for foreign in [String::new(), format!("{legacy_current} "), legacy_current[..legacy_current.len()-1].to_owned(), format!("{legacy_current}{legacy_current}"), legacy_before.to_owned()] {
            assert_eq!(legacy_context.value_direct(legacy_source, &legacy_expected, &foreign), foreign);
        }
        assert_eq!(legacy_context.metadata(), &fixture);
        drop(legacy_context);
        let ((), heap) = crate::test_allocation_observer::measure(|| {
            let mut context = ProjectionInvocation::new();
            assert!(context.packet.is_none(), "a new invocation owns no prior packet");
            assert!(context.file(std::path::Path::new("foreign"), 4, &expected).is_none());
            assert!(context.packet.is_none(), "foreign paths retain the pre-packet refusal");
            // Only this private control installs a synthetic immutable packet.
            // The public constructor always acquires the complete real packet.
            context.packet = Some(fixture.clone());
            let identity = context.metadata() as *const serde_json::Value;
            for _ in 0..2 {
                assert_eq!(context.file_row(source, 4, &expected), Some("old!".to_owned()));
                assert_eq!(from_packet(&fixture, source, &expected, "new!"), "old!");
                assert_eq!(context.value_direct_row(source, &expected, "new!"), "old!");
                assert_eq!(context.value_direct(source, &expected, "old!"), "old!");
                assert_eq!(from_packet(&fixture, source, &expected, "foreign"), "foreign");
                assert_eq!(context.metadata() as *const serde_json::Value, identity);
                assert_eq!(context.metadata(), &fixture, "every unknown field remains whole");
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
        assert_eq!(context.file_row(source, 4, &expected), Some("old!".to_owned()));
        std::fs::write(&current, b"drft").unwrap();
        std::fs::remove_file(&old).unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.file_row(source, 4, &expected);
        })).contains("length-or-digest"));
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.value_direct(source, &expected, "new!");
        })).contains("length-or-digest"));
        drop(context);
        let mut next = ProjectionInvocation::new();
        assert!(next.packet.is_none(), "Drop cannot publish metadata into the next invocation");
        next.packet = Some(fixture);
        std::fs::write(&current, b"new!").unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            next.file_row(source, 4, &expected);
        })).contains("missing"));
        // Common public endpoint methods use one lazy loader for this call only.
        let loads = std::cell::Cell::new(0usize);
        std::fs::write(&old, b"old!").unwrap();
        let fixture = serde_json::json!({"unknown_root":{"kept":true},"rows":[{
            "source":source,"compiled":false,"unknown_metadata":[1,2],
            "current":{"archive":source,"length":4,"sha256":digest(b"new!")},
            "views":[{"archive":old.to_str().unwrap(),"length":4,"sha256":expected}]}]});
        let ((), heap) = crate::test_allocation_observer::measure(|| {
            for invocation in 1..=2 {
                let mut context = ProjectionInvocation { packet: None, stage: None, acquire_stage: super::clippy_stage::rows, acquire: || {
                    loads.set(loads.get() + 1);
                    fixture.clone()
                }};
                assert_eq!(context.value(source, &expected, "new!"), "old!");
                assert_eq!(context.value_direct(source, &expected, "new!"), "old!");
                assert_eq!(context.file_row_with_context(source, 4, &expected), Some("old!".to_owned()));
                assert_eq!(super::from_packet_with_context(context.metadata(), source, &expected, "foreign"), "foreign");
                assert_eq!(loads.get(), invocation);
                assert_eq!(context.metadata(), &fixture);
                drop(context);
            }
        });
        assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
        // Metadata reuse never hides fresh-current failure or the missing archive.
        let mut context = ProjectionInvocation { packet: None, stage: None, acquire_stage: super::clippy_stage::rows, acquire: || fixture.clone() };
        assert_eq!(context.value(source, &expected, "new!"), "old!");
        std::fs::write(&current, b"drft").unwrap();
        std::fs::remove_file(&old).unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.value(source, &expected, "new!");
        })).contains("length-or-digest"));
        std::fs::write(&current, b"new!").unwrap();
        assert!(panic_text(std::panic::AssertUnwindSafe(|| {
            context.file_row_with_context(source, 4, &expected);
        })).contains("missing"));
        // The borrowed row lookup is the complete original file lookup suffix.
        let root = include_str!("main_e177_final_inputs.rs");
        let old = root.split_once("fn file_4a6eb(").unwrap().1
            .split_once("    let packet = packet();\n").unwrap().1
            .split_once("pub(crate) fn value(").unwrap().0;
        let child = include_str!("main_e177_packet_lookup.rs");
        let actual = child.rsplit_once("    fn file_row(&mut self,").unwrap().1
            .split_once("        let packet = self.metadata();\n").unwrap().1
            .split_once("\n}\n// END INVOCATION-LOCAL PROJECTION METADATA").unwrap().0;
        assert_eq!(actual, old.trim_end_matches('\n'));
    }
}

fn held_with_packet(authenticated: &serde_json::Value, path: &Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if let Some(result) = super::overlay_v2::held_source_with_packet(authenticated, path, length, expected) {
        return result;
    }
    super::held_4a6eb(path, length, expected)
}
fn read_view_with_packet(authenticated: &serde_json::Value, view: &serde_json::Value) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    held_with_packet(authenticated,
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
    let value = held_with_packet(authenticated,
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
    assert_eq!(value, read_view_with_packet(authenticated, current), "complete actual archive");
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
    let current = current.replace("    pub(super) fn compiled(source:", "    fn compiled(source:")
        .replace("    pub(super) fn pin(pin:", "    fn pin(pin:");
    let current = current.as_str();
    assert_eq!(current.len(), 112797);
    assert_eq!(digest(current.as_bytes()), "038f6b2515f59c4df7a11a8718b66f0904b2d07cb9544346eef2e3482a205467");
    let mut restored = current.to_owned();
    assert_eq!(restored.matches("pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value_direct(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_direct_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {").count(), 1);
    restored = restored.replacen("pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value_direct(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_direct_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {", "pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {", 1);
    assert_eq!(restored.matches("pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {").count(), 1);
    restored = restored.replacen("pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {\n    ProjectionInvocation::new().value(source, expected, supplied)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn value_before_nested_context(source: &str, expected: &str, supplied: &str) -> String {", "pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {", 1);
    assert_eq!(restored.matches("pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {\n    ProjectionInvocation::new().file(path, length, expected)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn file_before_nested_context(path: &Path, length: usize, expected: &str) -> Option<String> {").count(), 1);
    restored = restored.replacen("pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {\n    ProjectionInvocation::new().file(path, length, expected)\n}\n#[allow(dead_code, reason = \"Invocable complete predecessor acquisition route\")]\nfn file_before_nested_context(path: &Path, length: usize, expected: &str) -> Option<String> {", "pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {", 1);
    assert_eq!(restored.len(), 112069);
    assert_eq!(digest(restored.as_bytes()), "c99aa4c8f163fd93f2c5ab7dc050bbd69e0d2a5568f1faf2a02e8c62faf533a5");
    restored
}

// BEGIN INVOCATION-LOCAL PROJECTION METADATA
// Each outer projection acquisition owns one lazy immutable metadata packet.
// Current sources, compiled bytes and current/prior archives are never cached.
pub(crate) struct ProjectionInvocation<F = fn() -> serde_json::Value, S = fn() -> serde_json::Value> {
    packet: Option<serde_json::Value>,
    acquire: F,
    stage: Option<serde_json::Value>,
    acquire_stage: S,
}
impl ProjectionInvocation {
    pub(crate) fn new() -> Self {
        Self { packet: None, acquire: packet, stage: None, acquire_stage: clippy_stage::rows }
    }
}
#[rustfmt::skip] // Keep the complete old file-lookup suffix available verbatim.
impl<F: FnMut() -> serde_json::Value, S: FnMut() -> serde_json::Value> ProjectionInvocation<F, S> {
    #[cfg(test)]
    pub(super) fn with_loaders(acquire: F, acquire_stage: S) -> Self {
        Self { packet: None, acquire, stage: None, acquire_stage }
    }
    fn metadata(&mut self) -> &serde_json::Value {
        self.packet.get_or_insert_with(&mut self.acquire)
    }
    fn direct_metadata(&mut self, source: &str) -> (&serde_json::Value, Option<&serde_json::Value>) {
        let packet = self.packet.get_or_insert_with(&mut self.acquire);
        let stage = if clippy_stage::compiled(source).is_some() {
            Some(self.stage.get_or_insert_with(&mut self.acquire_stage) as &serde_json::Value)
        } else { None };
        (packet, stage)
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
    #[cfg(test)]
    pub(super) fn independent_old_value_direct(&mut self, source: &str, expected: &str, supplied: &str) -> String {
        let direct = from_packet_with_context(self.metadata(), source, expected, supplied);
        if digest(direct.as_bytes()) == expected { return direct; }
        let (authenticated, stage) = self.direct_metadata(source);
        independent_old_direct(authenticated, stage, source, expected, supplied)
    }
    pub(crate) fn value_direct(&mut self, source: &str, expected: &str, supplied: &str) -> String {
        // This intentionally closes the noncanonical-known-Clippy boundary.
        // A foreign subject never gains a historical acquisition route here.
        if !self.metadata()["rows"].as_array().unwrap().iter().any(|row| row["source"] == source) {
            return supplied.to_owned();
        }
        // A current-table hit needs no stage metadata or fallback allocation.
        let direct = from_packet_with_context(self.metadata(), source, expected, supplied);
        if digest(direct.as_bytes()) == expected {
            return direct;
        }
        self.value_direct_with_stage_context(source, expected, supplied)
    }

    // Borrow only invocation metadata; current/prior source reads remain live.
    fn value_direct_with_stage_context(&mut self, source: &str, expected: &str, supplied: &str) -> String {
        // A whole logical-table view is selected only after its actual current
        // physical, compiled and archive channels have authenticated. Preserve the
        // complete historical stage fallback below for every other input.
        let (authenticated, stage) = self.direct_metadata(source);
        let direct = direct_row_context(authenticated, stage, source, expected, supplied);
        if digest(direct.as_bytes()) == expected {
            return direct;
        }

        if let Some(value) = requested_context(authenticated, stage, source, expected, supplied) {
            let _ = authenticated;
            return value;
        }
        let normalized = normalize_context(authenticated, stage, source, supplied);
        let result = direct_row_context(authenticated, stage, source, expected, normalized.as_ref());
        if normalized.as_ref() != supplied
            && result == normalized.as_ref()
            && digest(result.as_bytes()) != expected
        {
            supplied.to_owned()
        } else {
            result
        }
    }

    // Entire immediate predecessor, independently exercised by the foreign
    // known-Clippy control; canonical-member fallback bodies stay byte-exact.
    fn value_direct_before_closed_membership(&mut self, source: &str, expected: &str, supplied: &str) -> String {
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

    #[allow(dead_code, reason = "Complete predecessor row acquisition") ]
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

// BEGIN BORROWED INDEPENDENT OLD DIRECT CONTROL
#[cfg(test)]
fn independent_old_direct(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {
    // A whole logical-table view is selected only after its actual current
    // physical, compiled and archive channels have authenticated. Preserve the
    // complete historical stage fallback below for every other input.
    let direct = independent_old_row(authenticated, stage, source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }

    if let Some(value) = requested_context(authenticated, stage, source, expected, supplied) {
        let _ = authenticated;
        return value;
    }
    let normalized = normalize_context(authenticated, stage, source, supplied);
    let result = independent_old_row(authenticated, stage, source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}
// END BORROWED INDEPENDENT OLD DIRECT CONTROL

// BEGIN BORROWED INDEPENDENT OLD ROW CONTROL
#[cfg(test)]
fn independent_old_row(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {
    let current = authenticated;
    if current["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["source"] == source)
    {
        with_packet_direct_context(current, stage, source, expected, supplied)
    } else {
        supplied.to_owned()
    }
}
// END BORROWED INDEPENDENT OLD ROW CONTROL

#[cfg(test)]
pub(super) fn assert_independent_old_direct_source() {
    assert_transitive_context_source();
        let child = include_str!("main_e177_packet_lookup.rs");
        let borrowed = child.split_once("// BEGIN BORROWED INDEPENDENT OLD DIRECT CONTROL\n")
            .unwrap().1.split_once("// END BORROWED INDEPENDENT OLD DIRECT CONTROL").unwrap().0;
        let restored = borrowed.strip_prefix("#[cfg(test)]\n").unwrap()
            .replacen("fn independent_old_direct(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str)", "pub(super) fn value_direct(source: &str, expected: &str, supplied: &str)", 1)
            .replacen("independent_old_row(authenticated, stage, source, expected, supplied)", "super::value_direct_4a6eb(source, expected, supplied)", 1)
            .replacen("let _ = authenticated;", "let _ = super::packet();", 1)
            .replacen("independent_old_row(authenticated, stage, source, expected, normalized.as_ref())", "super::value_direct_4a6eb(source, expected, normalized.as_ref())", 1)
            .replace("requested_context(authenticated, stage, source, expected, supplied)", "super::clippy_stage::requested(source, expected, supplied)")
            .replace("normalize_context(authenticated, stage, source, supplied)", "super::clippy_stage::normalize(source, supplied)");
        let current = include_str!("main_e177_overlay_v2.rs");
        assert_eq!(restored, current.rsplit_once("// Nondelegating current/prior view front, followed by the entire old stage path.\n").unwrap().1);
        let borrowed_row = child.split_once("// BEGIN BORROWED INDEPENDENT OLD ROW CONTROL\n").unwrap().1.split_once("// END BORROWED INDEPENDENT OLD ROW CONTROL").unwrap().0;
        let restored_row = borrowed_row.strip_prefix("#[cfg(test)]\n").unwrap()
            .replacen("fn independent_old_row(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source:", "fn value_direct_4a6eb(source:", 1)
            .replacen("let current = authenticated;", "let current = packet();", 1)
            .replacen("with_packet_direct_context(current, stage, source, expected, supplied)", "packet_lookup::with_packet(&current, source, expected, supplied)", 1);
        let root = include_str!("main_e177_final_inputs.rs");
        assert_eq!(restored_row.trim_end(), format!("fn value_direct_4a6eb({}", root.split_once("fn value_direct_4a6eb(").unwrap().1.split_once("// Exact finite successor").unwrap().0).trim_end());
}

// BEGIN EXPLICIT BORROWED STAGE METADATA
fn stage_with_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> Option<serde_json::Value> {
    let source = row["source"].as_str().unwrap();
    if !super::overlay_v2::REPLACEMENTS.contains(&source) && !super::overlay_v2::ADDITIONS.contains(&source) { return None; }
    let current_row = authenticated["rows"].as_array().unwrap().iter().find(|r| r["source"] == source).unwrap().clone();
    assert_eq!(
        canonical["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == source),
        Some(row),
        "closed original stage row"
    );
    super::overlay_v2::current(&current_row).unwrap();
    Some(current_row)
}
fn stage_normalize_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value, supplied: &str) -> Option<String> {
    let current_row = stage_with_context(authenticated, canonical, row)?;
    let raw = super::overlay_v2::current(&current_row).unwrap();
    if supplied == raw {
        Some(super::overlay_v2::image(&row["before"]).unwrap())
    } else {
        None
    }
}
fn stage_requested_context(authenticated: &serde_json::Value, canonical: &serde_json::Value,
    row: &serde_json::Value,
    expected: &str,
    supplied: &str,
) -> Option<Option<String>> {
    let current_row = stage_with_context(authenticated, canonical, row)?;
    let raw = super::overlay_v2::current(&current_row).unwrap();
    let selected = [&row["before"], &row["current"]]
        .into_iter()
        .chain(
            row.get("views")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten(),
        )
        .find(|p| p["sha256"] == expected);
    Some(selected.and_then(|p| {
        let prior = super::overlay_v2::image(p).unwrap();
        if supplied == raw || supplied == prior {
            Some(prior)
        } else {
            None
        }
    }))
}
fn requested_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> Option<String> {
        clippy_stage::compiled(source)?;
        let stage = stage.expect("compiled stage metadata belongs to this invocation");
        let packet = stage;
        let row = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)?;
        if let Some(value) = stage_requested_context(authenticated, stage, row, expected, supplied) {
            return value;
        }
        let actual = current_context(authenticated, stage, row);
        let selected = [&row["before"], &row["current"]]
            .into_iter()
            .chain(
                row.get("views")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten(),
            )
            .find(|pin| pin["sha256"] == expected)?;
        let previous = clippy_stage::pin(selected);
        if supplied == actual || supplied == previous {
            Some(previous)
        } else {
            None
        }
    }
fn normalize_context<'a>(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, supplied: &'a str) -> std::borrow::Cow<'a, str> {
        if clippy_stage::compiled(source).is_none() {
            return std::borrow::Cow::Borrowed(supplied);
        }
        let stage = stage.expect("compiled stage metadata belongs to this invocation");
        let packet = stage;
        let Some(row) = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
        else {
            return std::borrow::Cow::Borrowed(supplied);
        };
        if let Some(value) = stage_normalize_context(authenticated, stage, row, supplied) {
            return std::borrow::Cow::Owned(value);
        }
        let actual = current_context(authenticated, stage, row);
        if supplied == actual {
            std::borrow::Cow::Owned(clippy_stage::pin(&row["before"]))
        } else {
            std::borrow::Cow::Borrowed(supplied)
        }
    }
fn with_packet_direct_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {
    // Current and prior logical-table views retain their complete scope before
    // the separately authenticated historical Clippy-stage acquisition.
    let direct = from_packet_with_context(authenticated, source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }
    if let Some(value) = requested_context(authenticated, stage, source, expected, supplied) {
        let _ = authenticated;
        return value;
    }
    let normalized = normalize_context(authenticated, stage, source, supplied);
    let result = from_packet_with_context(authenticated, source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}
fn direct_row_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {
    if authenticated["rows"].as_array().unwrap().iter().any(|row| row["source"] == source) {
        with_packet_direct_context(authenticated, stage, source, expected, supplied)
    } else { supplied.to_owned() }
}
// END EXPLICIT BORROWED STAGE METADATA

fn stage_current_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> Option<String> {
    stage_with_context(authenticated, canonical, row)?;
    Some(super::overlay_v2::image(&row["current"]).unwrap())
}
fn current_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> String {
        if let Some(value) = stage_current_context(authenticated, canonical, row) {
            return value;
        }
        let source = row["source"].as_str().unwrap();
        let expected = &row["current"];
        let raw = super::held_4a6eb(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join(source),
            expected["length"].as_u64().unwrap() as usize,
            expected["sha256"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            Some(raw.as_str()),
            clippy_stage::compiled(source),
            "complete successor compiled source"
        );
        assert_eq!(raw, clippy_stage::pin(expected), "complete successor archive");
        raw
    }

// END BORROWED CURRENT CONTINUATION

#[cfg(test)]
fn assert_transitive_context_source() {
    let child = include_str!("main_e177_packet_lookup.rs");
    let overlay = include_str!("main_e177_overlay_v2.rs");
    let root = include_str!("main_e177_final_inputs.rs");
    {
        let mut restored = ("fn stage_current_context".to_owned() + child.split_once("\nfn stage_current_context").unwrap().1.split_once("\nfn current_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> String {").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (0usize, "fn stage_current_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> Option<String> {\n    stage_with_context(authenticated, canonical, row)?;\n    Some(super::overlay_v2::image(&row[\"current\"]).unwrap())\n", "pub(super) fn stage_current(row: &Value) -> Option<String> {\n    stage(row)?;\n    Some(image(&row[\"current\"]).unwrap())\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "pub(super) fn stage_current(".to_owned() + overlay.split_once("pub(super) fn stage_current(").unwrap().1.split_once("pub(super) fn stage_normalize").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole stage_current_context source inverse");
    }
    {
        let mut restored = ("fn current_context".to_owned() + child.split_once("\nfn current_context").unwrap().1.split_once("\n// END BORROWED CURRENT CONTINUATION").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (732usize, "        assert_eq!(raw, clippy_stage::pin(expected), \"complete successor archive\");\n", "        assert_eq!(raw, pin(expected), \"complete successor archive\");\n"),
            (628usize, "            clippy_stage::compiled(source),\n", "            compiled(source),\n"),
            (336usize, "        let raw = super::held_4a6eb(\n", "        let raw = held_4a6eb(\n"),
            (0usize, "fn current_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> String {\n        if let Some(value) = stage_current_context(authenticated, canonical, row) {\n", "    fn current(row: &serde_json::Value) -> String {\n        if let Some(value) = overlay_v2::stage_current(row) {\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "    fn current(".to_owned() + root.split_once("    fn current(").unwrap().1.split_once("    pub(super) fn compiled(").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole current_context source inverse");
    }
    {
        let mut restored = ("fn stage_with_context".to_owned() + child.split_once("\nfn stage_with_context").unwrap().1.split_once("\nfn stage_normalize_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value, supplied: &str) -> Option<String> {").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (657usize, "    super::overlay_v2::current(&current_row).unwrap();\n", "    current(&current_row).unwrap();\n"),
            (194usize, "    if !super::overlay_v2::REPLACEMENTS.contains(&source) && !super::overlay_v2::ADDITIONS.contains(&source) { return None; }\n    let current_row = authenticated[\"rows\"].as_array().unwrap().iter().find(|r| r[\"source\"] == source).unwrap().clone();\n", "    let current_row = affected_row(source)?;\n    let canonical = super::clippy_stage::rows();\n"),
            (0usize, "fn stage_with_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value) -> Option<serde_json::Value> {\n", "fn stage(row: &Value) -> Option<Value> {\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "fn stage(row:".to_owned() + overlay.split_once("fn stage(row:").unwrap().1.split_once("pub(super) fn stage_current").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole stage_with_context source inverse");
    }
    {
        let mut restored = ("fn stage_normalize_context".to_owned() + child.split_once("\nfn stage_normalize_context").unwrap().1.split_once("\nfn stage_requested_context(authenticated: &serde_json::Value, canonical: &serde_json::Value,").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (318usize, "        Some(super::overlay_v2::image(&row[\"before\"]).unwrap())\n", "        Some(image(&row[\"before\"]).unwrap())\n"),
            (0usize, "fn stage_normalize_context(authenticated: &serde_json::Value, canonical: &serde_json::Value, row: &serde_json::Value, supplied: &str) -> Option<String> {\n    let current_row = stage_with_context(authenticated, canonical, row)?;\n    let raw = super::overlay_v2::current(&current_row).unwrap();\n", "pub(super) fn stage_normalize(row: &Value, supplied: &str) -> Option<String> {\n    let current_row = stage(row)?;\n    let raw = current(&current_row).unwrap();\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "pub(super) fn stage_normalize(".to_owned() + overlay.split_once("pub(super) fn stage_normalize(").unwrap().1.split_once("pub(super) fn stage_requested").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole stage_normalize_context source inverse");
    }
    {
        let mut restored = ("fn stage_requested_context".to_owned() + child.split_once("\nfn stage_requested_context").unwrap().1.split_once("\nfn requested_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> Option<String> {").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (649usize, "        let prior = super::overlay_v2::image(p).unwrap();\n", "        let prior = image(p).unwrap();\n"),
            (450usize, "                .and_then(serde_json::Value::as_array)\n", "                .and_then(Value::as_array)\n"),
            (192usize, "    let current_row = stage_with_context(authenticated, canonical, row)?;\n    let raw = super::overlay_v2::current(&current_row).unwrap();\n", "    let current_row = stage(row)?;\n    let raw = current(&current_row).unwrap();\n"),
            (0usize, "fn stage_requested_context(authenticated: &serde_json::Value, canonical: &serde_json::Value,\n    row: &serde_json::Value,\n", "pub(super) fn stage_requested(\n    row: &Value,\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "pub(super) fn stage_requested(".to_owned() + overlay.split_once("pub(super) fn stage_requested(").unwrap().1.split_once("#[path = \"main_e177_overlay_compiled.rs\"]").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole stage_requested_context source inverse");
    }
    {
        let mut restored = ("fn requested_context".to_owned() + child.split_once("\nfn requested_context").unwrap().1.split_once("\nfn normalize_context<'a>(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, supplied: &'a str) -> std::borrow::Cow<'a, str> {").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (993usize, "        let previous = clippy_stage::pin(selected);\n", "        let previous = pin(selected);\n"),
            (602usize, "        let actual = current_context(authenticated, stage, row);\n", "        let actual = current(row);\n"),
            (464usize, "        if let Some(value) = stage_requested_context(authenticated, stage, row, expected, supplied) {\n", "        if let Some(value) = overlay_v2::stage_requested(row, expected, supplied) {\n"),
            (0usize, "fn requested_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> Option<String> {\n        clippy_stage::compiled(source)?;\n        let stage = stage.expect(\"compiled stage metadata belongs to this invocation\");\n        let packet = stage;\n", "    pub(super) fn requested(source: &str, expected: &str, supplied: &str) -> Option<String> {\n        compiled(source)?;\n        let packet = rows();\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "    pub(super) fn requested(".to_owned() + root.split_once("    pub(super) fn requested(").unwrap().1.split_once("    pub(super) fn normalize").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole requested_context source inverse");
    }
    {
        let mut restored = ("fn normalize_context".to_owned() + child.split_once("\nfn normalize_context").unwrap().1.split_once("\nfn with_packet_direct_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (971usize, "            std::borrow::Cow::Borrowed(supplied)\n", "            Cow::Borrowed(supplied)\n"),
            (883usize, "            std::borrow::Cow::Owned(clippy_stage::pin(&row[\"before\"]))\n", "            Cow::Owned(pin(&row[\"before\"]))\n"),
            (786usize, "        let actual = current_context(authenticated, stage, row);\n", "        let actual = current(row);\n"),
            (633usize, "        if let Some(value) = stage_normalize_context(authenticated, stage, row, supplied) {\n            return std::borrow::Cow::Owned(value);\n", "        if let Some(value) = overlay_v2::stage_normalize(row, supplied) {\n            return Cow::Owned(value);\n"),
            (565usize, "            return std::borrow::Cow::Borrowed(supplied);\n", "            return Cow::Borrowed(supplied);\n"),
            (280usize, "        let stage = stage.expect(\"compiled stage metadata belongs to this invocation\");\n        let packet = stage;\n", "        let packet = rows();\n"),
            (0usize, "fn normalize_context<'a>(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, supplied: &'a str) -> std::borrow::Cow<'a, str> {\n        if clippy_stage::compiled(source).is_none() {\n            return std::borrow::Cow::Borrowed(supplied);\n", "    pub(super) fn normalize<'a>(source: &str, supplied: &'a str) -> Cow<'a, str> {\n        if compiled(source).is_none() {\n            return Cow::Borrowed(supplied);\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "    pub(super) fn normalize".to_owned() + root.split_once("    pub(super) fn normalize").unwrap().1.split_once("    pub(super) fn read(").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole normalize_context source inverse");
    }
    {
        let mut restored = ("fn with_packet_direct_context".to_owned() + child.split_once("\nfn with_packet_direct_context").unwrap().1.split_once("\nfn direct_row_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (627usize, "    let normalized = normalize_context(authenticated, stage, source, supplied);\n    let result = from_packet_with_context(authenticated, source, expected, normalized.as_ref());\n", "    let normalized = clippy_stage::normalize(source, supplied);\n    let result = value_4a6eb(source, expected, normalized.as_ref());\n"),
            (473usize, "    if let Some(value) = requested_context(authenticated, stage, source, expected, supplied) {\n        let _ = authenticated;\n", "    if let Some(value) = clippy_stage::requested(source, expected, supplied) {\n        let _ = packet();\n"),
            (311usize, "    let direct = from_packet_with_context(authenticated, source, expected, supplied);\n", "    let direct = from_packet(authenticated, source, expected, supplied);\n"),
            (0usize, "fn with_packet_direct_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {\n", "pub(super) fn with_packet(authenticated: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "pub(super) fn with_packet(authenticated:".to_owned() + child.split_once("pub(super) fn with_packet(authenticated:").unwrap().1.split_once("#[cfg(test)]\nmod tests").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole with_packet_direct_context source inverse");
    }
    {
        let mut restored = ("fn from_packet_with_context".to_owned() + child.split_once("\nfn from_packet_with_context").unwrap().1.split_once("\n#[rustfmt::skip]").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (579usize, "    let current = actual_with_packet(packet, row);\n    let previous = read_view_with_packet(packet, prior);\n", "    let current = actual(row);\n    let previous = read_view(prior);\n"),
            (0usize, "fn from_packet_with_context(packet: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {\n", "pub(super) fn from_packet(packet: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "pub(super) fn from_packet(packet:".to_owned() + child.split_once("pub(super) fn from_packet(packet:").unwrap().1.split_once("#[rustfmt::skip] // Exact historical continuation").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole from_packet_with_context source inverse");
    }
    {
        let mut restored = ("fn direct_row_context".to_owned() + child.split_once("\nfn direct_row_context").unwrap().1.split_once("\n// END EXPLICIT BORROWED STAGE METADATA").unwrap().0).trim_end().to_owned();
        for (offset, after, before) in [
            (0usize, "fn direct_row_context(authenticated: &serde_json::Value, stage: Option<&serde_json::Value>, source: &str, expected: &str, supplied: &str) -> String {\n    if authenticated[\"rows\"].as_array().unwrap().iter().any(|row| row[\"source\"] == source) {\n        with_packet_direct_context(authenticated, stage, source, expected, supplied)\n    } else { supplied.to_owned() }\n", "fn value_direct_4a6eb(source: &str, expected: &str, supplied: &str) -> String {\n    let current = packet();\n    if current[\"rows\"]\n        .as_array()\n        .unwrap()\n        .iter()\n        .any(|row| row[\"source\"] == source)\n    {\n        packet_lookup::with_packet(&current, source, expected, supplied)\n    } else {\n        supplied.to_owned()\n    }\n"),
        ] {
            assert_eq!(&restored[offset..offset + after.len()], after);
            restored.replace_range(offset..offset + after.len(), before);
        }
        let original = "fn value_direct_4a6eb(".to_owned() + root.split_once("fn value_direct_4a6eb(").unwrap().1.split_once("// Exact finite successor").unwrap().0;
        assert_eq!(restored, original.trim_end(), "whole direct_row_context source inverse");
    }
}
