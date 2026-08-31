//! Exact expected canonical JSON for closed literal fixture inputs only.
use super::fixture::{architecture, SELECTOR};
use super::observe::Observation;

pub fn expected(target: &str, tools: &[(&str, &str, &str)], build: &str) -> Vec<u8> {
    let arch = if architecture() == 1 {
        "x86_64"
    } else {
        "aarch64"
    };
    let profile = format!("offline profile `{SELECTOR}`; checks describe this profile only");
    let mut rows = vec![
        ("semaprax", "ok", "0.2.0"),
        ("os", "ok", "linux"),
        ("arch", "ok", arch),
        ("release", "ok", build),
        ("profile", "ok", &profile),
    ];
    rows.extend_from_slice(tools);
    let checks = rows.into_iter().map(|(id, status, detail)| {
        // The fixture supplies no control characters. Quotes/backslashes are
        // explicit inputs in the report-sink case, not production serialization.
        assert!(!detail.chars().any(char::is_control));
        let detail = detail.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{{\"id\":\"{id}\",\"required\":true,\"status\":\"{status}\",\"detail\":\"{detail}\"}}")
    }).collect::<Vec<_>>().join(",");
    format!("{{\"schema\":\"semaprax.doctor.v1\",\"target\":\"{target}\",\"checks\":[{checks}]}}\n")
        .into_bytes()
}

pub fn require(observation: Observation, target: &str, tools: &[(&str, &str, &str)], status: i32) {
    assert_eq!(observation.status.code(), Some(status));
    assert!(observation.stderr.is_empty(), "{:?}", observation.stderr);
    assert!(
        observation.stdout == expected(target, tools, "debug")
            || observation.stdout == expected(target, tools, "release"),
        "{:?}",
        observation.stdout
    );
}
