use super::*;

pub(super) fn parse_subject(
    subject: &str,
    budget: &mut Budget,
) -> Result<PackageSubject, Diagnostic> {
    validate_json_wire(subject, "subject")?;
    let (schema, subject_digest, declared_bytes, payload) =
        parse_wrapper(subject, SUBJECT_DIGEST_DOMAIN, "subject")?;
    if schema != SUBJECT_SCHEMA {
        return Err(grammar_error(format!(
            "subject schema must be {SUBJECT_SCHEMA}"
        )));
    }
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| grammar_error(format!("subject payload is not valid JSON: {error}")))?;
    expect_object_keys(
        &value,
        &[
            "capabilities",
            "dependencies",
            "licenses",
            "package",
            "provenance",
            "report",
            "schema",
            "version",
        ],
        "subject payload",
    )?;
    if value["schema"].as_str() != Some(SUBJECT_SCHEMA) {
        return Err(grammar_error(format!(
            "subject payload schema must be {SUBJECT_SCHEMA}"
        )));
    }

    let package = required_str(&value, "package", "subject")?.to_owned();
    validate_package_identity(&package)?;
    let version = required_str(&value, "version", "subject")?.to_owned();
    validate_version(&version)?;
    let coordinate = Coordinate { package, version };

    let report = value["report"]
        .as_object()
        .ok_or_else(|| grammar_error("subject report must be an object".to_owned()))?;
    expect_map_keys(
        report,
        &["bytes", "digest", "envelope", "schema"],
        "subject report",
    )?;
    let report_schema = required_str(&value["report"], "schema", "subject report")?;
    if report_schema != package_report::SCHEMA {
        return Err(confusion_error(format!(
            "subject report schema must be {}",
            package_report::SCHEMA
        )));
    }
    let report_digest = required_str(&value["report"], "digest", "subject report")?.to_owned();
    let report_bytes = required_usize(&value["report"], "bytes", "subject report")?;
    let report_envelope = required_str(&value["report"], "envelope", "subject report")?;
    if report_envelope.len() != report_bytes {
        return Err(integrity_error(
            "subject report byte count does not match the exact embedded envelope".to_owned(),
        ));
    }
    validate_package_report_wire(report_envelope)?;
    package_report::verify_envelope(report_envelope).map_err(|_| {
        integrity_error("embedded Interface Package Report v1 failed independent replay".to_owned())
    })?;
    let report_value: Value = serde_json::from_str(report_envelope)
        .map_err(|_| integrity_error("embedded report is not JSON".to_owned()))?;
    if report_value["digest"].as_str() != Some(report_digest.as_str()) {
        return Err(integrity_error(
            "subject report digest disagrees with the exact embedded report".to_owned(),
        ));
    }
    let report_package = report_value["payload"]["package"]["name"]
        .as_str()
        .ok_or_else(|| integrity_error("embedded report package name is missing".to_owned()))?;
    if report_package != coordinate.package {
        return Err(confusion_error(
            "subject package identity must exactly equal the report module identity".to_owned(),
        ));
    }
    let targets = parse_targets(&report_value["payload"]["targets"])?;

    let dependencies = parse_dependencies(&value["dependencies"])?;
    ensure_at_most(
        dependencies.len(),
        MAX_DEPENDENCIES_PER_PACKAGE,
        "dependencies_per_package",
    )?;
    let capabilities = parse_sorted_strings(
        &value["capabilities"],
        MAX_CAPABILITIES,
        "capabilities",
        validate_capability,
    )?;
    let licenses = parse_sorted_strings(
        &value["licenses"],
        MAX_LICENSES,
        "licenses",
        validate_license,
    )?;
    let provenance = parse_provenance(&value["provenance"])?;

    debit_work(
        budget,
        dependencies
            .len()
            .checked_add(capabilities.len())
            .and_then(|value| value.checked_add(licenses.len()))
            .and_then(|value| value.checked_add(provenance.len()))
            .ok_or_else(|| limit_error("subject fact work overflow".to_owned()))?,
    )?;

    checked_add(
        &mut budget.capability_facts,
        capabilities.len(),
        MAX_WORK_UNITS,
        "capability_facts",
    )?;
    checked_add(
        &mut budget.license_facts,
        licenses.len(),
        MAX_WORK_UNITS,
        "license_facts",
    )?;
    checked_add(
        &mut budget.provenance_facts,
        provenance.len(),
        MAX_WORK_UNITS,
        "provenance_facts",
    )?;

    let canonical_payload = render_subject_payload(
        &coordinate,
        report_schema,
        &report_digest,
        report_bytes,
        report_envelope,
        &dependencies,
        &capabilities,
        &licenses,
        &provenance,
    );
    if canonical_payload != payload {
        return Err(grammar_error(
            "subject payload is not in exact canonical form".to_owned(),
        ));
    }
    if declared_bytes != payload.len() {
        return Err(integrity_error(
            "subject payload byte count mismatch".to_owned(),
        ));
    }

    Ok(PackageSubject {
        coordinate,
        subject_digest,
        subject_bytes: subject.len(),
        report_digest,
        report_bytes,
        report_envelope_digest: domain_digest(
            REPORT_ENVELOPE_DIGEST_DOMAIN,
            report_envelope.as_bytes(),
        ),
        targets,
        dependencies,
        capabilities,
        licenses,
        provenance,
    })
}

pub(super) fn parse_wrapper<'a>(
    envelope: &'a str,
    domain: &[u8],
    label: &str,
) -> Result<(String, String, usize, &'a str), Diagnostic> {
    let value: Value = serde_json::from_str(envelope)
        .map_err(|error| grammar_error(format!("{label} is not valid JSON: {error}")))?;
    expect_object_keys(&value, &["bytes", "digest", "payload", "schema"], label)?;
    let schema = required_str(&value, "schema", label)?.to_owned();
    let digest = required_str(&value, "digest", label)?.to_owned();
    let bytes = required_usize(&value, "bytes", label)?;
    const MARKER: &str = "\"payload\":";
    let offset = envelope
        .find(MARKER)
        .ok_or_else(|| grammar_error(format!("{label} is missing payload")))?
        + MARKER.len();
    if !envelope.ends_with('}') {
        return Err(grammar_error(format!("{label} must end with `}}`")));
    }
    let payload = &envelope[offset..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(grammar_error(format!("{label} payload must be an object")));
    }
    if bytes != payload.len() {
        return Err(integrity_error(format!(
            "{label} payload byte count mismatch"
        )));
    }
    if digest != domain_digest(domain, payload.as_bytes()) {
        return Err(integrity_error(format!("{label} payload digest mismatch")));
    }
    let expected = format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(&schema),
        quote_json(&digest),
        bytes,
        payload
    );
    if expected != envelope {
        return Err(grammar_error(format!(
            "{label} wrapper is not in exact canonical form"
        )));
    }
    Ok((schema, digest, bytes, payload))
}

fn validate_package_report_wire(report: &str) -> Result<(), Diagnostic> {
    validate_json_wire(report, "embedded report")?;
    let value: Value = serde_json::from_str(report)
        .map_err(|error| integrity_error(format!("embedded report is not JSON: {error}")))?;
    expect_object_keys(
        &value,
        &["bytes", "digest", "payload", "schema"],
        "embedded report",
    )?;
    if top_level_object_keys(report)? != ["schema", "digest", "bytes", "payload"] {
        return Err(integrity_error(
            "embedded report wrapper key order is not canonical".to_owned(),
        ));
    }
    const PAYLOAD_MARKER: &str = "\"payload\":";
    let payload_offset = report
        .find(PAYLOAD_MARKER)
        .ok_or_else(|| integrity_error("embedded report payload is missing".to_owned()))?
        + PAYLOAD_MARKER.len();
    let raw_payload = &report[payload_offset..report.len() - 1];
    if top_level_object_keys(raw_payload)?
        != [
            "schema",
            "source",
            "limits",
            "package",
            "targets",
            "exports",
            "exclusions",
            "unavailable_capabilities",
            "nonclaims",
        ]
    {
        return Err(integrity_error(
            "embedded report payload key order is not canonical".to_owned(),
        ));
    }
    let payload = &value["payload"];
    expect_object_keys(
        payload,
        &[
            "exclusions",
            "exports",
            "limits",
            "nonclaims",
            "package",
            "schema",
            "source",
            "targets",
            "unavailable_capabilities",
        ],
        "embedded report payload",
    )?;
    expect_object_keys(
        &payload["source"],
        &["path", "revision", "sha256"],
        "report source",
    )?;
    expect_object_keys(&payload["limits"], &["max_bytes"], "report limits")?;
    let report_max_bytes = required_usize(&payload["limits"], "max_bytes", "report limits")?;
    package_report::PackageReportOptions::new(report_max_bytes).map_err(|_| {
        integrity_error("report max_bytes is outside the Package Report v1 bounds".to_owned())
    })?;
    expect_object_keys(
        &payload["package"],
        &[
            "exports_admitted",
            "exports_excluded",
            "functions_total",
            "name",
        ],
        "report package",
    )?;
    for target in payload["targets"]
        .as_array()
        .ok_or_else(|| integrity_error("report targets must be an array".to_owned()))?
    {
        expect_object_keys(target, &TARGET_KEYS, "report target")?;
    }
    for export in payload["exports"]
        .as_array()
        .ok_or_else(|| integrity_error("report exports must be an array".to_owned()))?
    {
        expect_object_keys(
            export,
            &[
                "effects",
                "ensures",
                "name",
                "native64",
                "parameters",
                "requires",
                "result",
                "stable_id",
            ],
            "report export",
        )?;
        expect_object_keys(
            &export["native64"],
            &["signature", "signature_sha256", "symbol"],
            "report native signature",
        )?;
    }
    for exclusion in payload["exclusions"]
        .as_array()
        .ok_or_else(|| integrity_error("report exclusions must be an array".to_owned()))?
    {
        expect_object_keys(
            exclusion,
            &["name", "reason", "stable_id"],
            "report exclusion",
        )?;
    }
    let nonclaims = payload["nonclaims"]
        .as_array()
        .ok_or_else(|| integrity_error("report nonclaims must be an array".to_owned()))?;
    if nonclaims.len() != PACKAGE_REPORT_NONCLAIMS.len()
        || nonclaims
            .iter()
            .zip(PACKAGE_REPORT_NONCLAIMS)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(integrity_error(
            "report nonclaims must be the exact closed Package Report v1 inventory".to_owned(),
        ));
    }
    Ok(())
}

fn parse_dependencies(value: &Value) -> Result<Vec<Coordinate>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar_error("dependencies must be an array".to_owned()))?;
    ensure_at_most(
        rows.len(),
        MAX_DEPENDENCIES_PER_PACKAGE,
        "dependencies_per_package",
    )?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        expect_object_keys(row, &["package", "version"], "dependency")?;
        let package = required_str(row, "package", "dependency")?.to_owned();
        validate_package_identity(&package)?;
        let version = required_str(row, "version", "dependency")?.to_owned();
        validate_version(&version)?;
        output.push(Coordinate { package, version });
    }
    if !strictly_sorted(&output) {
        return Err(confusion_error(
            "dependencies must be strictly coordinate-sorted and unique".to_owned(),
        ));
    }
    Ok(output)
}

fn parse_targets(value: &Value) -> Result<Vec<TargetFact>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| integrity_error("report targets must be an array".to_owned()))?;
    let mut targets = Vec::with_capacity(rows.len());
    for row in rows {
        let target = required_str(row, "target", "report target")?.to_owned();
        let available = row["available"].as_bool().ok_or_else(|| {
            integrity_error("report target availability must be boolean".to_owned())
        })?;
        targets.push(TargetFact { target, available });
    }
    if targets.is_empty() {
        return Err(confusion_error(
            "report target matrix cannot be empty".to_owned(),
        ));
    }
    Ok(targets)
}

fn parse_sorted_strings(
    value: &Value,
    maximum: usize,
    label: &str,
    validator: fn(&str) -> Result<(), Diagnostic>,
) -> Result<Vec<String>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar_error(format!("{label} must be an array")))?;
    ensure_at_most(rows.len(), maximum, label)?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        let fact = row
            .as_str()
            .ok_or_else(|| grammar_error(format!("{label} entries must be strings")))?;
        validator(fact)?;
        output.push(fact.to_owned());
    }
    if !strictly_sorted(&output) {
        return Err(confusion_error(format!(
            "{label} must be strictly byte-sorted and unique"
        )));
    }
    Ok(output)
}

fn parse_provenance(value: &Value) -> Result<Vec<ProvenanceFact>, Diagnostic> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar_error("provenance must be an array".to_owned()))?;
    ensure_at_most(rows.len(), MAX_PROVENANCE, "provenance")?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        expect_object_keys(row, &["kind", "value"], "provenance fact")?;
        let kind = required_str(row, "kind", "provenance fact")?.to_owned();
        if !PROVENANCE_KINDS.contains(&kind.as_str()) {
            return Err(confusion_error(
                "provenance kind is outside the closed vocabulary".to_owned(),
            ));
        }
        let value = required_str(row, "value", "provenance fact")?.to_owned();
        if value.is_empty() || value.len() > 1_024 || value.contains('\0') {
            return Err(confusion_error(
                "provenance value must be 1..1024 UTF-8 bytes without NUL".to_owned(),
            ));
        }
        output.push(ProvenanceFact { kind, value });
    }
    if !strictly_sorted(&output) {
        return Err(confusion_error(
            "provenance must be strictly (kind,value)-sorted and unique".to_owned(),
        ));
    }
    Ok(output)
}

pub(super) fn validate_json_wire(value: &str, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.starts_with('\u{feff}')
        || value.ends_with('\n')
        || value.contains('\r')
    {
        return Err(grammar_error(format!(
            "{label} must be compact UTF-8 without BOM, CRLF, or terminal LF"
        )));
    }
    let mut depth = 0usize;
    let mut maximum = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in value.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| limit_error("json_depth overflow".to_owned()))?;
                maximum = maximum.max(depth);
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| grammar_error(format!("{label} has unbalanced JSON")))?;
            }
            b' ' | b'\t' | b'\n' => {
                return Err(grammar_error(format!(
                    "{label} contains insignificant whitespace"
                )))
            }
            _ => {}
        }
    }
    if in_string || depth != 0 {
        return Err(grammar_error(format!("{label} has incomplete JSON")));
    }
    if maximum > MAX_JSON_DEPTH {
        return Err(limit_error(format!("json_depth exceeds {MAX_JSON_DEPTH}")));
    }
    Ok(())
}

fn top_level_object_keys(value: &str) -> Result<Vec<String>, Diagnostic> {
    if !value.starts_with('{') || !value.ends_with('}') {
        return Err(grammar_error(
            "canonical value must be an object".to_owned(),
        ));
    }
    let bytes = value.as_bytes();
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut index = 0usize;
    let mut expecting_key = false;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                depth += 1;
                if depth == 1 {
                    expecting_key = true;
                }
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b',' if depth == 1 => {
                expecting_key = true;
                index += 1;
            }
            b'"' => {
                let start = index;
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if escaped {
                        escaped = false;
                    } else if byte == b'\\' {
                        escaped = true;
                    } else if byte == b'"' {
                        break;
                    }
                }
                if index > bytes.len() || bytes.get(index - 1) != Some(&b'"') {
                    return Err(grammar_error("unterminated JSON string".to_owned()));
                }
                if expecting_key && depth == 1 {
                    let key: String = serde_json::from_str(&value[start..index])
                        .map_err(|_| grammar_error("invalid object key".to_owned()))?;
                    if bytes.get(index) != Some(&b':') {
                        return Err(grammar_error(
                            "object key must be followed by colon".to_owned(),
                        ));
                    }
                    keys.push(key);
                    expecting_key = false;
                }
            }
            _ => index += 1,
        }
    }
    Ok(keys)
}
