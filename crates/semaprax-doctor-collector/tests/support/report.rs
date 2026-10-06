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
        ("semaprax", "ok", "0.9.0"),
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
    assert_eq!(
        observation.status.code(),
        Some(status),
        "checks [{}]; {}",
        checks_summary(&observation.stdout, tools),
        observation.describe()
    );
    assert!(observation.stderr.is_empty(), "{:?}", observation.stderr);
    assert!(
        observation.stdout == expected_for_selector(selector, target, tools, "debug")
            || observation.stdout == expected_for_selector(selector, target, tools, "release"),
        "{:?}",
        observation.stdout
    );
}

/// A one-line `id=status:detail` summary of every check the contracted
/// `semaprax.doctor.v1` report carries, so a status-code mismatch names which
/// check(s) actually failed instead of only the numeric exit code. This reads
/// nothing beyond `observation.stdout` -- the same contracted report bytes
/// `describe()` already dumps in full -- it only makes them legible without a
/// manual read of the raw JSON. It cannot say *why* a check failed beyond the
/// fixed, contracted detail string: that finer cause (exited-with-code vs
/// killed-by-signal) is only observable one layer down, in the confined
/// worker's own wire reply, which this outer collector-process observation
/// never sees.
fn checks_summary(stdout: &[u8], tools: &[(&str, &str, &str)]) -> String {
    const PREAMBLE: [&str; 5] = ["semaprax", "os", "arch", "release", "profile"];
    PREAMBLE
        .into_iter()
        .chain(tools.iter().map(|(id, _, _)| *id))
        .map(|id| match observed_check(stdout, id) {
            Some((status, detail)) => format!("{id}={status}:{detail}"),
            None => format!("{id}=<missing>"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Best-effort extraction of one check's `status`/`detail` from the canonical
/// report's flat, single-line JSON. Diagnostic-only: a parse miss just omits
/// that check from the summary rather than panicking, so this can never turn
/// an unrelated report shape into a spurious test failure of its own.
fn observed_check(stdout: &[u8], id: &str) -> Option<(String, String)> {
    let text = std::str::from_utf8(stdout).ok()?;
    let marker = format!("\"id\":\"{id}\",\"required\":true,\"status\":\"");
    let after_status = &text[text.find(&marker)? + marker.len()..];
    let status_end = after_status.find('"')?;
    let status = &after_status[..status_end];
    let detail_marker = format!("{status}\",\"detail\":\"");
    let after_detail = &after_status[after_status.find(&detail_marker)? + detail_marker.len()..];
    let detail_end = unescaped_quote(after_detail)?;
    Some((status.to_string(), after_detail[..detail_end].to_string()))
}

/// Byte offset of the first `"` in `text` not escaped by a preceding
/// odd-length run of backslashes, matching this report's own JSON escaping.
fn unescaped_quote(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let backslashes = bytes[..index]
                .iter()
                .rev()
                .take_while(|b| **b == b'\\')
                .count();
            if backslashes % 2 == 0 {
                return Some(index);
            }
        }
        index += 1;
    }
    None
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
        "{{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.9.0\"}},",
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
        "{{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.9.0\"}},",
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
