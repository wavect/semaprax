//! Independent expected canonical JSON from literal or provisioner-supplied
//! expectations. Never derive these expectations from an observed report.
use super::fixture::{architecture, SELECTOR};
use super::observe::Observation;

pub fn expected(target: &str, tools: &[(&str, &str, &str)], build: &str) -> Vec<u8> {
    expected_for_selector(SELECTOR, target, tools, build)
}

pub(super) fn expected_for_selector(
    selector: &str,
    target: &str,
    tools: &[(&str, &str, &str)],
    build: &str,
) -> Vec<u8> {
    let arch = if architecture() == 1 {
        "x86_64"
    } else {
        "aarch64"
    };
    let profile = format!("offline profile `{selector}`; checks describe this profile only");
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
    require_for_selector(observation, SELECTOR, target, tools, status);
}

pub(super) fn require_for_selector(
    observation: Observation,
    selector: &str,
    target: &str,
    tools: &[(&str, &str, &str)],
    status: i32,
) {
    assert_eq!(observation.status.code(), Some(status));
    assert!(observation.stderr.is_empty(), "{:?}", observation.stderr);
    assert!(
        observation.stdout == expected_for_selector(selector, target, tools, "debug")
            || observation.stdout == expected_for_selector(selector, target, tools, "release"),
        "{:?}",
        observation.stdout
    );
}

#[test]
fn default_selector_preserves_literal_canonical_report_bytes() {
    let arch = if architecture() == 1 {
        "x86_64"
    } else {
        "aarch64"
    };
    let literal = format!(concat!(
        "{{\"schema\":\"semaprax.doctor.v1\",\"target\":\"native\",\"checks\":[",
        "{{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.2.0\"}},",
        "{{\"id\":\"os\",\"required\":true,\"status\":\"ok\",\"detail\":\"linux\"}},",
        "{{\"id\":\"arch\",\"required\":true,\"status\":\"ok\",\"detail\":\"{}\"}},",
        "{{\"id\":\"release\",\"required\":true,\"status\":\"ok\",\"detail\":\"debug\"}},",
        "{{\"id\":\"profile\",\"required\":true,\"status\":\"ok\",\"detail\":\"offline profile `collector-fixture`; checks describe this profile only\"}},",
        "{{\"id\":\"clang\",\"required\":true,\"status\":\"ok\",\"detail\":\"/bin/clang (clang version 1.0.0)\"}}]}}\n"
    ), arch);
    let tools = [("clang", "ok", "/bin/clang (clang version 1.0.0)")];
    assert_eq!(expected("native", &tools, "debug"), literal.as_bytes());
    assert_eq!(
        expected_for_selector("collector-fixture", "native", &tools, "debug"),
        literal.as_bytes()
    );
}

#[test]
fn explicit_selector_preserves_order_and_escapes_independent_detail_bytes() {
    let arch = if architecture() == 1 {
        "x86_64"
    } else {
        "aarch64"
    };
    let literal = format!(concat!(
        "{{\"schema\":\"semaprax.doctor.v1\",\"target\":\"all\",\"checks\":[",
        "{{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.2.0\"}},",
        "{{\"id\":\"os\",\"required\":true,\"status\":\"ok\",\"detail\":\"linux\"}},",
        "{{\"id\":\"arch\",\"required\":true,\"status\":\"ok\",\"detail\":\"{}\"}},",
        "{{\"id\":\"release\",\"required\":true,\"status\":\"ok\",\"detail\":\"release\"}},",
        "{{\"id\":\"profile\",\"required\":true,\"status\":\"ok\",\"detail\":\"offline profile `real-tools-42`; checks describe this profile only\"}},",
        "{{\"id\":\"clang\",\"required\":true,\"status\":\"ok\",\"detail\":\"/bin/clang (clang \\\"Q\\\" \\\\ λ)\"}},",
        "{{\"id\":\"node\",\"required\":true,\"status\":\"ok\",\"detail\":\"v22.0.0\"}},",
        "{{\"id\":\"rust\",\"required\":true,\"status\":\"ok\",\"detail\":\"rustc 1.88.0\"}}]}}\n"
    ), arch);
    let tools = [
        ("clang", "ok", "/bin/clang (clang \"Q\" \\ λ)"),
        ("node", "ok", "v22.0.0"),
        ("rust", "ok", "rustc 1.88.0"),
    ];
    assert_eq!(
        expected_for_selector("real-tools-42", "all", &tools, "release"),
        literal.as_bytes()
    );
}
