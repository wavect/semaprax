//! Independent Main043 predicates; raw owners and all table rows read RAW inputs.
use crate::source_reflows_merge::Main043Fixture as Fixture;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::OnceLock;
fn metadata(fixture: Fixture) -> &'static str {
    match fixture {
        Fixture::ExitPrefix => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::ExitDirectory => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::LegacyNominal => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Lifecycle => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Prefix => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Receiving => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Edge => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Selected => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Status => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_raw_metadata(fixture))
                .as_str()
        }
        Fixture::Formatting => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_metadata(fixture))
                .as_str()
        }
        Fixture::Receivers => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_metadata(fixture))
                .as_str()
        }
        Fixture::LiveAudits => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_metadata(fixture))
                .as_str()
        }
        Fixture::HistoricalCiExtras => {
            static VALUE: OnceLock<String> = OnceLock::new();
            VALUE
                .get_or_init(|| crate::source_reflows_merge::main043_metadata(fixture))
                .as_str()
        }
    }
}
static RECORDS: OnceLock<serde_json::Value> = OnceLock::new();
fn historical_ci_extra(source: &str, current: &str) -> String {
    crate::source_reflows_merge::main043_historical_ci_extra(source, current)
}
fn records() -> &'static [serde_json::Value] {
    RECORDS
        .get_or_init(|| serde_json::from_str(metadata(Fixture::Formatting)).unwrap())
        .as_array()
        .unwrap()
}
fn restore(record: &serde_json::Value, current: &str) -> String {
    let path = record["path"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        record["after_sha256"],
        "reviewed CI input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in record["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique CI inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        record["before_sha256"],
        "CI inverse did not preserve the complete predecessor: {path}"
    );
    restored
}
#[path = "main043/audit_0.rs"]
mod audit_0;
#[path = "main043/audit_1.rs"]
mod audit_1;
#[path = "main043/audit_2.rs"]
mod audit_2;
#[path = "main043/audit_3.rs"]
mod audit_3;
#[path = "main043/audit_4.rs"]
mod audit_4;
#[path = "main043/audit_5.rs"]
mod audit_5;
#[path = "main043/audit_6.rs"]
mod audit_6;
#[path = "main043/audit_7.rs"]
mod audit_7;
#[path = "main043/audit_8.rs"]
mod audit_8;
#[path = "main043/semantic_receivers.rs"]
mod semantic_receivers;
