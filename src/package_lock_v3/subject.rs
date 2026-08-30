use std::collections::BTreeMap;

use serde_json::Value;

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::package_report_v2;

use super::model::Subject;
use super::wire::{
    authentication_error, charge, confusion_error, domain_digest, limit_error, parse_wrapper,
    render_wrapper, required_str, wire_error,
};
use super::{
    Coordinate, DependencyRequirement, MAX_CAPABILITIES, MAX_DEPENDENCIES, MAX_SUBJECT_BYTES,
    REPORT_DOMAIN, SUBJECT_DOMAIN, SUBJECT_SCHEMA,
};

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

pub(super) fn create_subject(
    coordinate: &Coordinate,
    report: &str,
    dependencies: &[DependencyRequirement],
    capabilities: &[String],
) -> Result<String, Vec<Diagnostic>> {
    let receipt = package_report_v2::verify_envelope(report).map_err(|error| {
        vec![if matches!(error.code, "SPX-P401" | "SPX-P402") {
            limit_error("semantic subject v3 nested report bound failed")
        } else {
            authentication_error("semantic subject v3 report replay failed")
        }]
    })?;
    if receipt.package != coordinate.package {
        return Err(vec![confusion_error(
            "subject package differs from replayed v2 report",
        )]);
    }
    validate_coordinate(coordinate).map_err(|error| vec![error])?;
    validate_dependencies(coordinate, dependencies).map_err(|error| vec![error])?;
    validate_capabilities(capabilities).map_err(|error| vec![error])?;
    let (envelope, overflowed) = bounded_output::with_limit(64 * 1024 * 1024, || {
        let payload = render_subject_payload(coordinate, report, dependencies, capabilities);
        render_wrapper(SUBJECT_SCHEMA, SUBJECT_DOMAIN, &payload)
    });
    if overflowed {
        return Err(vec![limit_error("semantic subject render budget exceeded")]);
    }
    if envelope.len() > MAX_SUBJECT_BYTES {
        return Err(vec![limit_error(
            "semantic subject exceeds max_subject_bytes",
        )]);
    }
    Ok(envelope)
}

pub(super) fn parse_subject(bytes: &str, work: &mut usize) -> Result<Subject, Diagnostic> {
    parse_subject_impl(bytes, work, false)
}

pub(super) fn parse_subject_for_resolution(
    bytes: &str,
    work: &mut usize,
) -> Result<Subject, Diagnostic> {
    parse_subject_impl(bytes, work, true)
}

fn parse_subject_impl(
    bytes: &str,
    work: &mut usize,
    preserve_report_bound: bool,
) -> Result<Subject, Diagnostic> {
    if bytes.len() > MAX_SUBJECT_BYTES {
        return Err(limit_error("semantic subject exceeds max_subject_bytes"));
    }
    let payload = parse_wrapper(bytes, SUBJECT_SCHEMA, SUBJECT_DOMAIN, "subject")?;
    let value: Value = serde_json::from_str(payload)
        .map_err(|_| wire_error("semantic subject payload is not JSON"))?;
    let dependency_rows = value["dependencies"]
        .as_array()
        .ok_or_else(|| wire_error("dependencies must be array"))?;
    let capability_rows = value["capabilities"]
        .as_array()
        .ok_or_else(|| wire_error("capabilities must be array"))?;
    if dependency_rows.len() > MAX_DEPENDENCIES {
        return Err(limit_error("dependencies exceed limit"));
    }
    if capability_rows.len() > MAX_CAPABILITIES {
        return Err(limit_error("capabilities exceed limit"));
    }
    let package = required_str(&value, "package")?;
    let version = required_str(&value, "version")?;
    validate_identity(package)?;
    crate::package_range::parse_version(version, confusion_owned)?;
    let coordinate = Coordinate {
        package: package.to_owned(),
        version: version.to_owned(),
    };
    validate_coordinate(&coordinate)?;
    let report_bytes = exact_report_bytes(payload)?;
    if report_bytes.len() > package_report_v2::MAX_OUTPUT_BYTES {
        return Err(limit_error(
            "semantic subject v3 nested report bound failed",
        ));
    }
    let report = report_bytes.to_owned();
    let declared_report_bytes = value["report_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| wire_error("report_bytes missing"))?;
    if declared_report_bytes != report.len()
        || required_str(&value, "report_digest")? != domain_digest(REPORT_DOMAIN, report.as_bytes())
    {
        return Err(authentication_error(
            "semantic report byte/digest binding failed",
        ));
    }
    let receipt = if preserve_report_bound {
        package_report_v2::verify_envelope_for_resolution(&report)
    } else {
        package_report_v2::verify_envelope(&report)
    }
    .map_err(|error| {
        if preserve_report_bound && matches!(error.code, "SPX-P401" | "SPX-P402") {
            limit_error("semantic subject v3 nested report bound failed")
        } else {
            authentication_error("semantic subject v3 report replay failed")
        }
    })?;
    if receipt.package != coordinate.package {
        return Err(confusion_error(
            "semantic subject package differs from report",
        ));
    }
    let dependencies = parse_requirements(&value["dependencies"])?;
    validate_dependencies(&coordinate, &dependencies)?;
    let capabilities = parse_strings(&value["capabilities"])?;
    validate_capabilities(&capabilities)?;
    charge(
        work,
        dependencies
            .len()
            .saturating_add(capabilities.len())
            .saturating_add(1),
    )?;
    let report_value: Value = serde_json::from_str(&report)
        .map_err(|_| authentication_error("verified report is not JSON"))?;
    let source_bytes = report_value["payload"]["source"]["bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| authentication_error("v2 source byte fact missing"))?;
    let report_facts = report_value["payload"]["exports"]
        .as_array()
        .map_or(0, Vec::len)
        .saturating_add(
            report_value["payload"]["types"]
                .as_array()
                .map_or(0, Vec::len),
        )
        .saturating_add(
            report_value["payload"]["targets"]
                .as_array()
                .map_or(0, Vec::len),
        );
    charge(work, source_bytes.saturating_add(report_facts))?;
    let mut targets = BTreeMap::new();
    for row in report_value["payload"]["targets"]
        .as_array()
        .ok_or_else(|| authentication_error("v2 targets missing"))?
    {
        targets.insert(
            required_str(row, "target")?.to_owned(),
            required_str(row, "status")?.to_owned(),
        );
    }
    let canonical = render_subject_payload(&coordinate, &report, &dependencies, &capabilities);
    if canonical != payload {
        return Err(wire_error("semantic subject is not canonical"));
    }
    Ok(Subject {
        coordinate,
        digest: domain_digest(SUBJECT_DOMAIN, payload.as_bytes()),
        bytes: bytes.len(),
        report_digest: domain_digest(REPORT_DOMAIN, report.as_bytes()),
        report,
        revision: receipt.source_revision,
        targets,
        dependencies,
        capabilities,
    })
}

fn render_subject_payload(
    coordinate: &Coordinate,
    report: &str,
    dependencies: &[DependencyRequirement],
    capabilities: &[String],
) -> String {
    bf!("{{\"schema\":{},\"package\":{},\"version\":{},\"report_digest\":{},\"report_bytes\":{},\"report\":{},\"dependencies\":[{}],\"capabilities\":[{}]}}", quote_json(SUBJECT_SCHEMA), quote_json(&coordinate.package), quote_json(&coordinate.version), quote_json(&domain_digest(REPORT_DOMAIN, report.as_bytes())), report.len(), report, dependencies.iter().map(render_requirement).collect::<Vec<_>>().budgeted_join(","), capabilities.iter().map(|v| quote_json(v)).collect::<Vec<_>>().budgeted_join(","))
}

fn exact_report_bytes(payload: &str) -> Result<&str, Diagnostic> {
    const START: &str = "\"report\":";
    const END: &str = ",\"dependencies\":";
    let start = payload
        .find(START)
        .ok_or_else(|| wire_error("exact report member missing"))?
        + START.len();
    let end = structural_object_end(payload.as_bytes(), start)?;
    if !payload[end..].starts_with(END) {
        return Err(wire_error("exact report terminator missing"));
    }
    Ok(&payload[start..end])
}

fn structural_object_end(bytes: &[u8], start: usize) -> Result<usize, Diagnostic> {
    if bytes.get(start) != Some(&b'{') {
        return Err(wire_error("embedded report must be a JSON object"));
    }
    let mut depth = 0usize;
    let mut string = false;
    let mut escaped = false;
    for (offset, &byte) in bytes.iter().enumerate().skip(start) {
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
        } else if byte == b'"' {
            string = true;
        } else if matches!(byte, b'{' | b'[') {
            depth = depth
                .checked_add(1)
                .ok_or_else(|| limit_error("report depth overflow"))?;
        } else if matches!(byte, b'}' | b']') {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| wire_error("report nesting invalid"))?;
            if depth == 0 {
                return Ok(offset + 1);
            }
        }
    }
    Err(wire_error("embedded report is unterminated"))
}

pub(super) fn validate_dependencies(
    owner: &Coordinate,
    values: &[DependencyRequirement],
) -> Result<(), Diagnostic> {
    if values.len() > MAX_DEPENDENCIES {
        return Err(limit_error("dependencies exceed limit"));
    }
    let mut last = None;
    for value in values {
        validate_identity(&value.package)?;
        crate::package_range::parse_range(&value.range, confusion_owned)?;
        if value.package == owner.package {
            return Err(confusion_error("self dependency"));
        }
        if last.is_some_and(|v: &String| v >= &value.package) {
            return Err(confusion_error(
                "dependencies must be strictly package-sorted",
            ));
        }
        last = Some(&value.package);
    }
    Ok(())
}

fn validate_capabilities(values: &[String]) -> Result<(), Diagnostic> {
    if values.len() > MAX_CAPABILITIES {
        return Err(limit_error("capabilities exceed limit"));
    }
    let mut last = None;
    for value in values {
        if value.is_empty()
            || value.len() > 255
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            return Err(confusion_error("invalid capability"));
        }
        if last.is_some_and(|v: &String| v >= value) {
            return Err(confusion_error("capabilities must be strictly sorted"));
        }
        last = Some(value);
    }
    Ok(())
}

fn validate_identity(package: &str) -> Result<(), Diagnostic> {
    if package.is_empty()
        || package.len() > 255
        || !package
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(confusion_error("invalid package identity"));
    }
    Ok(())
}

fn validate_coordinate(value: &Coordinate) -> Result<(), Diagnostic> {
    validate_identity(&value.package)?;
    crate::package_range::parse_version(&value.version, confusion_owned).map(|_| ())
}

fn parse_requirements(value: &Value) -> Result<Vec<DependencyRequirement>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| wire_error("dependencies must be array"))?;
    if rows.len() > MAX_DEPENDENCIES {
        return Err(limit_error("dependencies exceed limit"));
    }
    rows.iter()
        .map(|v| {
            let package = required_str(v, "package")?;
            let range = required_str(v, "range")?;
            validate_identity(package)?;
            crate::package_range::parse_range(range, confusion_owned)?;
            Ok(DependencyRequirement {
                package: package.to_owned(),
                range: range.to_owned(),
            })
        })
        .collect()
}

fn parse_strings(value: &Value) -> Result<Vec<String>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| wire_error("capabilities must be array"))?;
    if rows.len() > MAX_CAPABILITIES {
        return Err(limit_error("capabilities exceed limit"));
    }
    rows.iter()
        .map(|v| {
            let value = v
                .as_str()
                .ok_or_else(|| wire_error("capability must be string"))?;
            validate_identity(value)?;
            Ok(value.to_owned())
        })
        .collect()
}

pub(super) fn render_coordinate(value: &Coordinate) -> String {
    bf!(
        "{{\"package\":{},\"version\":{}}}",
        quote_json(&value.package),
        quote_json(&value.version)
    )
}

pub(super) fn render_requirement(value: &DependencyRequirement) -> String {
    bf!(
        "{{\"package\":{},\"range\":{}}}",
        quote_json(&value.package),
        quote_json(&value.range)
    )
}
fn confusion_owned(message: String) -> Diagnostic {
    confusion_error(message)
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn report_boundary_is_structural_even_with_nested_dependency_text() {
        let payload = r#"{"report":{"nested":{"dependencies":[]},"text":",\"dependencies\": still inside","escaped":"\\","rows":[{"x":"}"}]},"dependencies":[]}"#;
        assert_eq!(
            exact_report_bytes(payload).unwrap(),
            r#"{"nested":{"dependencies":[]},"text":",\"dependencies\": still inside","escaped":"\\","rows":[{"x":"}"}]}"#
        );
        assert!(exact_report_bytes(r#"{"report":{},"other":[],"dependencies":[]}"#).is_err());
    }

    #[test]
    fn parser_byte_and_array_counts_reject_before_field_clones() {
        let mut work = 0;
        assert_eq!(
            parse_subject_impl(&"x".repeat(MAX_SUBJECT_BYTES + 1), &mut work, true)
                .err()
                .unwrap()
                .code,
            "SPX-PL606"
        );
        assert_eq!(work, 0);
        let row = serde_json::json!({"package":"dep","range":"=1.0.0"});
        assert!(parse_requirements(&Value::Array(vec![row.clone(); MAX_DEPENDENCIES])).is_ok());
        assert_eq!(
            parse_requirements(&Value::Array(vec![row; MAX_DEPENDENCIES + 1]))
                .unwrap_err()
                .code,
            "SPX-PL606"
        );
        let row = Value::String("read".into());
        assert!(parse_strings(&Value::Array(vec![row.clone(); MAX_CAPABILITIES])).is_ok());
        assert_eq!(
            parse_strings(&Value::Array(vec![row; MAX_CAPABILITIES + 1]))
                .unwrap_err()
                .code,
            "SPX-PL606"
        );
    }
}
