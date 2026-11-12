use super::*;
use crate::ast::Span;
use crate::diagnostic::{before_writer as old, Severity};
use crate::test_allocation_observer::measure;
use std::fmt::Write as _;

fn span() -> Span {
    Span {
        start: 42,
        end: 51,
        line: 7,
        column: 3,
    }
}
fn current(full: bool, text: &str) -> Diagnostic {
    if full {
        Diagnostic::warning("SPX-W001", text, span())
            .at_path(text)
            .with_help(text)
    } else {
        Diagnostic::io("SPX-I001", text)
    }
}
fn previous(full: bool, text: &str) -> old::Diagnostic {
    if full {
        old::Diagnostic::warning("SPX-W001", text, span())
            .at_path(text)
            .with_help(text)
    } else {
        old::Diagnostic::io("SPX-I001", text)
    }
}

#[test]
fn writer_complete_old_json_human_fields_escaping_nulls_and_order_remain_exact() {
    for text in [
        "",
        "plain",
        "quoted \" slash \\ newline\n\r\t",
        "\0\u{8}\u{b}\u{c}\u{1b}\u{7f}\u{85}",
        "café € 🦀",
    ] {
        for full in [false, true] {
            let mut old = previous(full, text);
            let mut diagnostic = current(full, text);
            if text.is_empty() {
                old.code = "code\"\\\n";
                diagnostic.code = "code\"\\\n";
            }
            for json in [false, true] {
                let mut actual = String::new();
                diagnostic.write_to(&mut actual, json).unwrap();
                assert_eq!(actual, if json { old.json() } else { old.to_string() });
            }
            assert_eq!(
                diagnostic.json(),
                old.json(),
                "unchanged compatibility receiver"
            );
        }
    }
    let typed = crate::workspace_report::WorkspaceReportFailure::existing(vec![
        current(true, "first"),
        current(false, "second"),
    ]);
    assert!(!typed.has_paid_report());
    assert_eq!(
        typed.to_string(),
        format!("{}\n{}", previous(true, "first"), previous(false, "second"))
    );
    assert_eq!(typed.reported().unwrap().len(), 2);
    drop(typed);
    for code in 0u32..=0x100 {
        let text = char::from_u32(code).unwrap().to_string();
        let mut actual = String::new();
        current(true, &text).write_json(&mut actual).unwrap();
        assert_eq!(actual, previous(true, &text).json());
    }
    let diagnostics = [current(true, "first\n"), current(false, "second")];
    for json in [false, true] {
        let mut actual = String::new();
        write_lines(&mut actual, &diagnostics, json).unwrap();
        let expected: String = [previous(true, "first\n"), previous(false, "second")]
            .iter()
            .map(|d| {
                if json {
                    format!("{}\n", d.json())
                } else {
                    format!("{d}\n")
                }
            })
            .collect();
        assert_eq!(actual, expected);
        let mut empty = String::new();
        write_lines(&mut empty, &[], json).unwrap();
        assert!(empty.is_empty());
    }
}

#[test]
fn bounded_writer_keeps_every_original_quote_order_floor_sticky_and_partial_byte() {
    let text = "path \"é\"\n\u{85}";
    let full = previous(true, text).json().len();
    for limit in 0..=full + 1 {
        let old =
            crate::bounded_output::with_limit_usage_refusal(limit, || previous(true, text).json());
        let current = crate::bounded_output::with_limit_usage_refusal(limit, || {
            let mut output = String::new();
            self::current(true, text).write_json(&mut output).unwrap();
            output
        });
        assert_eq!(current, old, "exact logical compatibility at {limit}");
        for floor in [0, limit] {
            let old = crate::bounded_output::with_limit_usage_refusal(limit, || {
                assert!(crate::bounded_output::set_active_floor(floor));
                let first = previous(true, text).json();
                let retry = previous(false, "later").json();
                (first, retry)
            });
            let current = crate::bounded_output::with_limit_usage_refusal(limit, || {
                assert!(crate::bounded_output::set_active_floor(floor));
                let mut first = String::new();
                self::current(true, text).write_json(&mut first).unwrap();
                let mut retry = String::new();
                self::current(false, "later")
                    .write_json(&mut retry)
                    .unwrap();
                (first, retry)
            });
            assert_eq!(current, old, "floor/sticky order {limit}/{floor}");
        }
    }
}

// Caller-owned output is a fixed test array, not a certified compiler allowance.
// Diagnostic construction, renderer and complete Drop all remain observed.
struct Fixed {
    bytes: [u8; 8192],
    used: usize,
}
impl Fixed {
    fn new() -> Self {
        Self {
            bytes: [0; 8192],
            used: 0,
        }
    }
}
impl fmt::Write for Fixed {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.used.checked_add(value.len()).ok_or(fmt::Error)?;
        let slot = self.bytes.get_mut(self.used..end).ok_or(fmt::Error)?;
        slot.copy_from_slice(value.as_bytes());
        self.used = end;
        Ok(())
    }
}
#[test]
fn whole_renderer_drop_measures_actual_intermediate_strings_and_unfavorable_bounded_route() {
    for (full, text) in [(false, ""), (true, "quotes \" \\ é\n\u{85}")] {
        for bounded in [false, true] {
            let render = |old: bool| {
                measure(|| {
                    let run = || {
                        let mut sink = Fixed::new();
                        if old {
                            let diagnostic = previous(full, text);
                            sink.write_str(&diagnostic.json()).unwrap();
                            drop(diagnostic);
                        } else {
                            let diagnostic = current(full, text);
                            diagnostic.write_json(&mut sink).unwrap();
                            drop(diagnostic);
                        }
                        sink.used
                    };
                    if bounded {
                        crate::bounded_output::with_limit_usage(4096, run).0
                    } else {
                        run()
                    }
                })
            };
            let (old_bytes, old) = render(true);
            let (new_bytes, new) = render(false);
            assert_eq!(new_bytes, old_bytes);
            for measured in [old, new] {
                assert!(!measured.invalid && measured.live == 0);
                assert_eq!(measured.allocated, measured.freed);
            }
            eprintln!(
                "diagnostic-writer full={full} bounded={bounded} old={old:?} current={new:?}"
            );
            if bounded {
                assert_eq!(new.allocations, old.allocations);
                assert_eq!(new.reallocations, old.reallocations);
                assert_eq!(new.allocated, old.allocated);
                assert_eq!(new.highwater, old.highwater);
            } else {
                assert!(new.allocations < old.allocations);
                assert!(new.allocated < old.allocated);
                assert!(new.highwater <= old.highwater);
            }
        }
    }
}

struct Refuse {
    calls: usize,
}
impl io::Write for Refuse {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn writer_failure_keeps_first_io_error_and_does_not_touch_later_diagnostics() {
    let diagnostics = [current(true, "selected primary"), current(false, "later")];
    for json in [false, true] {
        let mut sink = Refuse { calls: 0 };
        let mut writer = IoWriter::new(&mut sink);
        assert!(write_lines(&mut writer, &diagnostics, json).is_err());
        let first = writer.error().unwrap() as *const io::Error;
        assert_eq!(writer.error().unwrap().kind(), io::ErrorKind::BrokenPipe);
        assert!(writer.write_str("retry").is_err());
        assert_eq!(writer.error().unwrap() as *const io::Error, first);
        drop(writer);
        assert_eq!(sink.calls, 1);
        assert_eq!(diagnostics[0].message, "selected primary");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }
}

#[test]
fn full_original_renderer_and_all_current_source_owners_remain_bound() {
    use sha2::{Digest, Sha256};
    let hash = |text: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
        )
    };
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/diagnostic-writer-846/source-inverses.json"
    )))
    .unwrap();
    let sources = [
        ("src/diagnostic.rs", include_str!("../../diagnostic.rs")),
        ("src/workspace_report.rs", include_str!("../../workspace_report.rs")),
        ("src/cli_driver/source_execution.rs", include_str!("../../cli_driver/source_execution.rs")),
        ("tests/cli_check_routing_v1.rs", include_str!("../../../tests/cli_check_routing_v1.rs")),
        ("src/source_reflows_merge.rs", include_str!("../../source_reflows_merge.rs")),
        ("tests/hir_module_boundaries.rs", include_str!("../../../tests/hir_module_boundaries.rs")),
        ("tests/workspace/semantic_graph/source_inventory.rs", include_str!("../../../tests/workspace/semantic_graph/source_inventory.rs")),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!("../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
        ("docs/ARCHITECTURE.md", include_str!("../../../docs/ARCHITECTURE.md")),
        ("CHANGELOG.md", include_str!("../../../CHANGELOG.md")),
    ];
    assert_eq!(sources.len(), rows.as_array().unwrap().len());
    for (path, raw) in sources {
        let current = crate::source_reflows_merge::diagnostic846_input(path, raw);
        let matching: Vec<_> = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["source"] == path)
            .collect();
        assert_eq!(matching.len(), 1);
        let row = matching[0];
        assert_eq!(hash(&current), row["current_sha256"]);
        assert_eq!(current, row["edits"][0]["new"].as_str().unwrap());
        let old = row["edits"][0]["old"].as_str().unwrap();
        assert_eq!(hash(old), row["original_sha256"]);
        if path == "src/diagnostic.rs" {
            let prefix = old.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
            assert_eq!(prefix, include_str!("../before_writer.rs"));
        }
    }
    let additions: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/diagnostic-writer-846/new-owners.json"
    )))
    .unwrap();
    for (path, raw) in [
        ("src/diagnostic/writer.rs", include_str!("../writer.rs")),
        ("src/diagnostic/writer/tests.rs", include_str!("tests.rs")),
        (
            "src/diagnostic/before_writer.rs",
            include_str!("../before_writer.rs"),
        ),
    ] {
        let current = crate::source_reflows_merge::diagnostic846_input(path, raw);
        let matching: Vec<_> = additions
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["source"] == path)
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(hash(&current), matching[0]["sha256"]);
        for audit in [include_str!("../../../tests/hir_module_boundaries.rs"), include_str!("../../../tests/workspace/semantic_graph/source_inventory.rs"), include_str!("../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")] {
            assert_eq!(audit.matches(&format!("{path}\"")).count(), 1);
        }
    }
}
