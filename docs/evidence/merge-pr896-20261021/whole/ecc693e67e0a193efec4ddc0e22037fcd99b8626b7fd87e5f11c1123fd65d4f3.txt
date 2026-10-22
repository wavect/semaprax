use super::*;
use crate::source_reflows_merge::main_e177_final_inputs as owner;
use sha2::{Digest, Sha256};

#[test]
fn historical_data_hits_changes_errors_and_return_mutation_never_reuse_verdicts() {
    let mut data = Data::new();
    let a = "{\"a\":1}";
    let b = "{\"a\":2}";
    let first = data.parse(a).unwrap();
    let mut received = data.parse(a).unwrap();
    received["a"] = 99.into();
    assert_eq!(data.parse(a).unwrap(), first);
    for raw in ["", "{", "[1,]", "null trailing"] {
        let before = serde_json::from_str::<Value>(raw).unwrap_err();
        let current = data.parse(raw).unwrap_err();
        assert_eq!(current.to_string(), before.to_string());
        assert_eq!(
            (current.line(), current.column()),
            (before.line(), before.column())
        );
        assert_eq!(data.entry.as_ref().unwrap().0, a);
    }
    assert_eq!(
        data.parse(b).unwrap(),
        serde_json::from_str::<Value>(b).unwrap()
    );
    assert_eq!(data.entry.as_ref().unwrap().0, b);
    // Preserve the old parser's treatment of duplicate keys and whitespace.
    for raw in ["{\"a\":1,\"a\":2}", " { \"a\": 1 } \n", "null"] {
        assert_eq!(
            data.parse(raw).unwrap(),
            serde_json::from_str::<Value>(raw).unwrap()
        );
    }
    let large = format!("\"{}\"", "x".repeat(owner::SOURCE_CAP));
    let retained = data.entry.clone();
    assert_eq!(
        data.parse(&large).unwrap(),
        serde_json::from_str::<Value>(&large).unwrap()
    );
    assert_eq!(
        data.entry, retained,
        "oversized inputs never expand retained DATA"
    );
}

#[test]
fn historical_data_warm_reads_keep_dependency_order_and_exact_io_refusals() {
    let root = std::env::temp_dir().join(format!("semaprax-history-data-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let a = root.join("first.json");
    let b = root.join("second.json");
    let raw = "{\"a\":1}";
    let sha = owner::digest(raw.as_bytes());
    std::fs::write(&a, raw).unwrap();
    std::fs::write(&b, raw).unwrap();
    let mut data = Data::new();
    let mut acquire = |cached: bool| {
        let mut trace = Vec::new();
        let result = (|| {
            for path in [&a, &b] {
                trace.push(path.clone());
                let fresh = owner::held_4a6eb(path, raw.len(), &sha)?;
                let value = if cached {
                    data.parse(&fresh)
                } else {
                    serde_json::from_str(&fresh)
                };
                assert_eq!(value.unwrap(), serde_json::json!({"a":1}));
            }
            Ok::<_, &'static str>(())
        })();
        (trace, result)
    };
    assert_eq!(acquire(false), acquire(true));
    assert_eq!(
        acquire(false),
        acquire(true),
        "warm exact bytes still read both dependencies"
    );
    for path in [&b, &a] {
        for bad in [
            b"{\"a\":2}".as_slice(),
            b"{\"a\":1",
            b"{\"a\":1} ",
            &[255; 7],
        ] {
            std::fs::write(path, bad).unwrap();
            let before = acquire(false);
            assert!(before.1.is_err());
            assert_eq!(before, acquire(true));
        }
        std::fs::remove_file(path).unwrap();
        assert_eq!(acquire(false), acquire(true));
        std::fs::create_dir(path).unwrap();
        assert_eq!(acquire(false), acquire(true));
        std::fs::remove_dir(path).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(if path == &a { &b } else { &a }, path).unwrap();
            assert_eq!(acquire(false), acquire(true));
            std::fs::remove_file(path).unwrap();
        }
        std::fs::write(path, raw).unwrap();
        assert_eq!(acquire(false), acquire(true));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn historical_data_keeps_complete_old_loaders_and_all_sixteen_parse_frontiers() {
    let current = include_str!("main_e177_final_inputs.rs");
    let before = include_str!("main_e177_historical_before_data.rs");
    let start = current.find("#[rustfmt::skip]").unwrap();
    let end = current.find("fn same_file(").unwrap();
    let raw = &current[start..end];
    assert_eq!(raw.matches("historical_json!(").count(), 16);
    let digest_binding = "    let compiled_digest = digest(data.as_bytes());\n";
    assert_eq!(raw.matches(digest_binding).count(), 6);
    let raw = raw.replace(digest_binding, "");
    assert_eq!(raw.matches("compiled_digest.clone()").count(), 1);
    let raw = raw.replace("compiled_digest.clone()", "digest(data.as_bytes())");
    assert_eq!(raw.matches("compiled_digest").count(), 11);
    let raw = raw.replace("compiled_digest", "digest(data.as_bytes())");
    // Exactly four old explicit type arguments; other twelve inferred Value.
    let marker = "historical_json!(";
    let mut restored = raw.replace(marker, "serde_json::from_str(");
    for (from, to) in [
        (
            "serde_json::from_str(local_data)",
            "serde_json::from_str::<serde_json::Value>(local_data)",
        ),
        (
            "serde_json::from_str(&before)",
            "serde_json::from_str::<serde_json::Value>(&before)",
        ),
    ] {
        restored = restored.replace(from, to);
    }
    let old = before.split_once("#[rustfmt::skip]").unwrap().1;
    let old = format!("#[rustfmt::skip]{old}").replacen(
        "pub(super) fn packet_before_overlay()",
        "fn packet_before_overlay()",
        1,
    );
    assert_eq!(restored, old);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "67845c402b6089dc60b59d4d930360450c1c3713f69d5c37492bbcca59c7c6fe"
    );
    // Full live loader replay on both sides, including every historical inverse.
    assert_eq!(
        owner::packet_before_overlay(),
        owner::historical_before_data::packet_before_overlay()
    );
    assert_eq!(
        owner::packet_before_overlay(),
        owner::historical_before_data::packet_before_overlay()
    );
}
