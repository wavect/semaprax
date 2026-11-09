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
        old_lookup
            .strip_prefix("source:")
            .unwrap()
            .replacen("    let packet = packet();\n", "", 1)
            + "\n"
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
    let legacy_before = include_str!(
        "../../docs/evidence/clippy-4a6eb/YyMz16gZziO27eJCd12onKvQ2XldkIBHpW6o7FjczCo.txt"
    );
    let legacy_expected = digest(legacy_before.as_bytes());
    assert_eq!(
        legacy_expected,
        "632333d7a819ce23b6ede242775da89cabd0d9795d908047a56ea8ec58dccc2a"
    );
    assert_eq!(
        digest(legacy_current.as_bytes()),
        "fdfbc8c20782be5b3af2537c4cee8cda100937c62bdd6f3bb049497a24eee1f9"
    );
    let mut legacy_context = ProjectionInvocation::new();
    assert!(!fixture["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["source"] == legacy_source));
    legacy_context.packet = Some(fixture.clone());
    assert_eq!(
        legacy_context.value_direct_before_closed_membership(
            legacy_source,
            &legacy_expected,
            legacy_current
        ),
        legacy_before
    );
    assert_ne!(legacy_current, legacy_before);
    assert_eq!(
        legacy_context.value_direct(legacy_source, &legacy_expected, legacy_current),
        legacy_current
    );
    for foreign in [
        String::new(),
        format!("{legacy_current} "),
        legacy_current[..legacy_current.len() - 1].to_owned(),
        format!("{legacy_current}{legacy_current}"),
        legacy_before.to_owned(),
    ] {
        assert_eq!(
            legacy_context.value_direct(legacy_source, &legacy_expected, &foreign),
            foreign
        );
    }
    assert_eq!(legacy_context.metadata(), &fixture);
    drop(legacy_context);
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
                guard: super::super::overlay_v2::InvocationGuard::Disabled,
                packet: None,
                ordinary_packet_loader: false,
                stage: None,
                acquire_stage: super::clippy_stage::rows,
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
                super::from_packet_with_context(context.metadata(), source, &expected, "foreign"),
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
        guard: super::super::overlay_v2::InvocationGuard::Disabled,
        packet: None,
        ordinary_packet_loader: false,
        stage: None,
        acquire_stage: super::clippy_stage::rows,
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
