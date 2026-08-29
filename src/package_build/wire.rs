use std::collections::BTreeSet;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};

use super::model::{
    BuildFacts, OfflinePackageBuildOptions, EVIDENCE_SCHEMA, MANIFEST_SCHEMA, NONCLAIMS, PROFILE,
    RUNTIME_IMPORTS,
};

const MANIFEST_DOMAIN: &[u8] = b"semaprax.offline-effect-free-wasm-package-build.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.offline-effect-free-wasm-package-build-evidence.v1\0";
const SOURCE_SET_DOMAIN: &[u8] = b"semaprax.offline-effect-free-wasm-package-source-set.v1\0";
const LINK_DOMAIN: &[u8] = b"semaprax.offline-effect-free-wasm-package-link.v1\0";
/// Closed structural parser bound. Generated v1 artifacts currently use at
/// most four levels; the larger frozen ceiling admits no unbounded recursion.
const MAX_JSON_DEPTH: usize = 32;
/// Structural work bounds are deliberately far above the generated v1 shape
/// (at most 32 exports) while preventing a byte-bounded hostile array from
/// expanding into millions of DOM nodes or per-object key inventories.
const MAX_JSON_VALUES: usize = 4_096;
const MAX_JSON_OBJECTS: usize = 512;
const MAX_JSON_KEYS: usize = 4_096;

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

pub(crate) fn source_set_digest(source: &str) -> String {
    domain_digest(SOURCE_SET_DOMAIN, source.as_bytes())
}

pub(crate) fn link_digest(graph: &str) -> String {
    domain_digest(LINK_DOMAIN, graph.as_bytes())
}

pub(crate) fn wasm_digest(wasm: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(wasm);
    bf!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(crate) fn wrapper_digest(value: &str, label: &str) -> Result<String, Diagnostic> {
    let parsed: Value = serde_json::from_str(value)
        .map_err(|_| super::authentication_error(format!("package-build {label} is not JSON")))?;
    parsed["digest"].as_str().map(str::to_owned).ok_or_else(|| {
        super::authentication_error(format!("package-build {label} digest is missing"))
    })
}

pub(crate) fn render_manifest(
    facts: &BuildFacts,
    options: &OfflinePackageBuildOptions,
    wasm_digest: &str,
    wasm_bytes: usize,
) -> String {
    let root = coordinate(&facts.coordinate);
    let packages = bf!(
        "{{\"package\":{},\"version\":{},\"subject_digest\":{},\"source_revision\":{}}}",
        quote_json(&facts.coordinate.package),
        quote_json(&facts.coordinate.version),
        quote_json(&facts.subject_digest),
        quote_json(&facts.source_revision),
    );
    let exports = export_rows(facts);
    let runtime_imports = RUNTIME_IMPORTS
        .iter()
        .map(|name| {
            bf!(
                "{{\"module\":\"env\",\"name\":{},\"kind\":\"function\"}}",
                quote_json(name)
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let nonclaims = string_rows(&NONCLAIMS);
    bf!(
        "{{\"schema\":{},\"profile\":{},\"root\":{},\"packages\":[{}],\"exports\":[{}],\"runtime_imports\":[{}],\"module\":{{\"path\":\"module.wasm\",\"sha256\":{},\"bytes\":{}}},\"compiler\":{{\"package\":\"semaprax\",\"version\":{}}},\"limits\":{{\"max_artifact_bytes\":{},\"max_evidence_bytes\":{}}},\"nonclaims\":[{}]}}",
        quote_json(MANIFEST_SCHEMA),
        quote_json(PROFILE),
        root,
        packages,
        exports,
        runtime_imports,
        quote_json(wasm_digest),
        wasm_bytes,
        quote_json(env!("CARGO_PKG_VERSION")),
        options.max_artifact_bytes,
        options.max_evidence_bytes,
        nonclaims,
    )
}

pub(crate) fn render_evidence(
    facts: &BuildFacts,
    options: &OfflinePackageBuildOptions,
    manifest: &str,
    manifest_digest: &str,
    wasm_digest: &str,
    wasm_bytes: usize,
) -> Result<String, Diagnostic> {
    let root = coordinate(&facts.coordinate);
    let subjects = bf!(
        "{{\"package\":{},\"version\":{},\"subject_digest\":{},\"subject_bytes\":{},\"report_digest\":{},\"source_revision\":{}}}",
        quote_json(&facts.coordinate.package),
        quote_json(&facts.coordinate.version),
        quote_json(&facts.subject_digest),
        facts.subject_bytes,
        quote_json(&facts.report_digest),
        quote_json(&facts.source_revision),
    );
    let exports = export_rows(facts);
    let nonclaims = string_rows(&NONCLAIMS);
    let mut evidence_bytes = 0usize;
    for _ in 0..32 {
        let artifact_bytes = wasm_bytes
            .checked_add(manifest.len())
            .and_then(|value| value.checked_add(evidence_bytes))
            .ok_or_else(|| super::limit_error("package-build artifact byte sum overflowed"))?;
        let payload = bf!(
            "{{\"schema\":{},\"resolution_digest\":{},\"resolution_bytes\":{},\"lock_digest\":{},\"lock_bytes\":{},\"subjects\":[{}],\"root\":{},\"exports\":[{}],\"package_source_set_digest\":{},\"package_link_digest\":{},\"manifest_digest\":{},\"manifest_bytes\":{},\"wasm_digest\":{},\"wasm_bytes\":{},\"limits\":{{\"max_artifact_bytes\":{},\"max_evidence_bytes\":{}}},\"budget\":{{\"used_source_bytes\":{},\"used_wasm_bytes\":{},\"used_manifest_bytes\":{},\"used_evidence_bytes\":{},\"used_artifact_bytes\":{}}},\"nonclaims\":[{}]}}",
            quote_json(EVIDENCE_SCHEMA),
            quote_json(&facts.resolution_digest),
            facts.resolution_bytes,
            quote_json(&facts.lock_digest),
            facts.lock_bytes,
            subjects,
            root,
            exports,
            quote_json(&facts.source_set_digest),
            quote_json(&facts.link_digest),
            quote_json(manifest_digest),
            manifest.len(),
            quote_json(wasm_digest),
            wasm_bytes,
            options.max_artifact_bytes,
            options.max_evidence_bytes,
            facts.source_bytes,
            wasm_bytes,
            manifest.len(),
            evidence_bytes,
            artifact_bytes,
            nonclaims,
        );
        let evidence = bf!(
            "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
            quote_json(EVIDENCE_SCHEMA),
            quote_json(&domain_digest(EVIDENCE_DOMAIN, payload.as_bytes())),
            payload.len(),
            payload,
        );
        if evidence.len() == evidence_bytes {
            return Ok(evidence);
        }
        evidence_bytes = evidence.len();
    }
    Err(super::limit_error(
        "package-build evidence byte accounting did not converge",
    ))
}

pub(crate) fn manifest_digest(manifest: &str) -> String {
    domain_digest(MANIFEST_DOMAIN, manifest.as_bytes())
}

pub(crate) fn validate_submitted_manifest(value: &str, maximum: usize) -> Result<(), Diagnostic> {
    let object_keys = validate_compact_json(value, maximum, "manifest")?;
    let parsed: Value = serde_json::from_str(value)
        .map_err(|_| super::wire_error("package-build manifest is not JSON"))?;
    let mut order = ObjectOrder::new(&object_keys);
    require_object(
        &parsed,
        &mut order,
        &[
            "schema",
            "profile",
            "root",
            "packages",
            "exports",
            "runtime_imports",
            "module",
            "compiler",
            "limits",
            "nonclaims",
        ],
    )?;
    require_exact_string(&parsed["schema"], MANIFEST_SCHEMA)?;
    require_string(&parsed["profile"])?;
    require_coordinate(&parsed["root"], &mut order)?;
    require_object_array(&parsed["packages"], &mut order, require_manifest_package)?;
    require_object_array(&parsed["exports"], &mut order, require_export)?;
    require_object_array(
        &parsed["runtime_imports"],
        &mut order,
        require_runtime_import,
    )?;
    require_object(&parsed["module"], &mut order, &["path", "sha256", "bytes"])?;
    require_string(&parsed["module"]["path"])?;
    require_string(&parsed["module"]["sha256"])?;
    require_u64(&parsed["module"]["bytes"])?;
    require_object(&parsed["compiler"], &mut order, &["package", "version"])?;
    require_string(&parsed["compiler"]["package"])?;
    require_string(&parsed["compiler"]["version"])?;
    require_limits(&parsed["limits"], &mut order)?;
    require_string_array(&parsed["nonclaims"])?;
    order.finish()?;
    Ok(())
}

pub(crate) fn validate_submitted_evidence(value: &str, maximum: usize) -> Result<(), Diagnostic> {
    let object_keys = validate_compact_json(value, maximum, "evidence")?;
    let parsed: Value = serde_json::from_str(value)
        .map_err(|_| super::wire_error("package-build evidence is not JSON"))?;
    let mut order = ObjectOrder::new(&object_keys);
    require_object(
        &parsed,
        &mut order,
        &["schema", "digest", "bytes", "payload"],
    )?;
    require_exact_string(&parsed["schema"], EVIDENCE_SCHEMA)?;
    require_string(&parsed["digest"])?;
    require_u64(&parsed["bytes"])?;
    let payload = &parsed["payload"];
    require_object(
        payload,
        &mut order,
        &[
            "schema",
            "resolution_digest",
            "resolution_bytes",
            "lock_digest",
            "lock_bytes",
            "subjects",
            "root",
            "exports",
            "package_source_set_digest",
            "package_link_digest",
            "manifest_digest",
            "manifest_bytes",
            "wasm_digest",
            "wasm_bytes",
            "limits",
            "budget",
            "nonclaims",
        ],
    )?;
    require_exact_string(&payload["schema"], EVIDENCE_SCHEMA)?;
    for key in [
        "resolution_digest",
        "lock_digest",
        "package_source_set_digest",
        "package_link_digest",
        "manifest_digest",
        "wasm_digest",
    ] {
        require_string(&payload[key])?;
    }
    for key in [
        "resolution_bytes",
        "lock_bytes",
        "manifest_bytes",
        "wasm_bytes",
    ] {
        require_u64(&payload[key])?;
    }
    require_object_array(&payload["subjects"], &mut order, require_subject)?;
    require_coordinate(&payload["root"], &mut order)?;
    require_object_array(&payload["exports"], &mut order, require_export)?;
    require_limits(&payload["limits"], &mut order)?;
    require_object(
        &payload["budget"],
        &mut order,
        &[
            "used_source_bytes",
            "used_wasm_bytes",
            "used_manifest_bytes",
            "used_evidence_bytes",
            "used_artifact_bytes",
        ],
    )?;
    for key in [
        "used_source_bytes",
        "used_wasm_bytes",
        "used_manifest_bytes",
        "used_evidence_bytes",
        "used_artifact_bytes",
    ] {
        require_u64(&payload["budget"][key])?;
    }
    require_string_array(&payload["nonclaims"])?;
    order.finish()?;
    let _ = exact_payload(value)?;
    Ok(())
}

struct ObjectOrder<'a> {
    objects: &'a [Vec<String>],
    next: usize,
}

impl<'a> ObjectOrder<'a> {
    fn new(objects: &'a [Vec<String>]) -> Self {
        Self { objects, next: 0 }
    }

    fn take(&mut self, expected: &[&str]) -> Result<(), Diagnostic> {
        let Some(actual) = self.objects.get(self.next) else {
            return Err(super::wire_error(
                "package-build JSON object inventory is incomplete",
            ));
        };
        self.next += 1;
        if actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
        {
            Ok(())
        } else {
            Err(super::wire_error(
                "package-build JSON object member order is noncanonical",
            ))
        }
    }

    fn finish(&self) -> Result<(), Diagnostic> {
        if self.next == self.objects.len() {
            Ok(())
        } else {
            Err(super::wire_error(
                "package-build JSON contains an unvalidated object",
            ))
        }
    }
}

fn require_object(
    value: &Value,
    order: &mut ObjectOrder<'_>,
    keys: &[&str],
) -> Result<(), Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| super::wire_error("package-build JSON member must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(super::wire_error(
            "package-build JSON object member inventory is invalid",
        ));
    }
    order.take(keys)
}

fn require_string(value: &Value) -> Result<(), Diagnostic> {
    value
        .as_str()
        .map(|_| ())
        .ok_or_else(|| super::wire_error("package-build JSON member must be a string"))
}

fn require_exact_string(value: &Value, expected: &str) -> Result<(), Diagnostic> {
    if value.as_str() == Some(expected) {
        Ok(())
    } else {
        Err(super::wire_error(
            "package-build JSON schema identity is invalid",
        ))
    }
}

fn require_u64(value: &Value) -> Result<(), Diagnostic> {
    value
        .as_u64()
        .map(|_| ())
        .ok_or_else(|| super::wire_error("package-build JSON member must be an unsigned integer"))
}

fn require_string_array(value: &Value) -> Result<(), Diagnostic> {
    let values = value
        .as_array()
        .ok_or_else(|| super::wire_error("package-build JSON member must be an array"))?;
    values.iter().try_for_each(require_string)
}

fn require_object_array(
    value: &Value,
    order: &mut ObjectOrder<'_>,
    validate: impl Fn(&Value, &mut ObjectOrder<'_>) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let values = value
        .as_array()
        .ok_or_else(|| super::wire_error("package-build JSON member must be an array"))?;
    for value in values {
        validate(value, order)?;
    }
    Ok(())
}

fn require_coordinate(value: &Value, order: &mut ObjectOrder<'_>) -> Result<(), Diagnostic> {
    require_object(value, order, &["package", "version"])?;
    require_string(&value["package"])?;
    require_string(&value["version"])
}

fn require_manifest_package(value: &Value, order: &mut ObjectOrder<'_>) -> Result<(), Diagnostic> {
    require_object(
        value,
        order,
        &["package", "version", "subject_digest", "source_revision"],
    )?;
    for key in ["package", "version", "subject_digest", "source_revision"] {
        require_string(&value[key])?;
    }
    Ok(())
}

fn require_subject(value: &Value, order: &mut ObjectOrder<'_>) -> Result<(), Diagnostic> {
    require_object(
        value,
        order,
        &[
            "package",
            "version",
            "subject_digest",
            "subject_bytes",
            "report_digest",
            "source_revision",
        ],
    )?;
    for key in [
        "package",
        "version",
        "subject_digest",
        "report_digest",
        "source_revision",
    ] {
        require_string(&value[key])?;
    }
    require_u64(&value["subject_bytes"])
}

fn require_export(value: &Value, order: &mut ObjectOrder<'_>) -> Result<(), Diagnostic> {
    require_object(
        value,
        order,
        &["stable_id", "wasm_export", "parameters", "result"],
    )?;
    require_string(&value["stable_id"])?;
    require_string(&value["wasm_export"])?;
    require_string_array(&value["parameters"])?;
    require_string(&value["result"])
}

fn require_runtime_import(value: &Value, order: &mut ObjectOrder<'_>) -> Result<(), Diagnostic> {
    require_object(value, order, &["module", "name", "kind"])?;
    require_string(&value["module"])?;
    require_string(&value["name"])?;
    require_string(&value["kind"])
}

fn require_limits(value: &Value, order: &mut ObjectOrder<'_>) -> Result<(), Diagnostic> {
    require_object(value, order, &["max_artifact_bytes", "max_evidence_bytes"])?;
    require_u64(&value["max_artifact_bytes"])?;
    require_u64(&value["max_evidence_bytes"])
}

fn validate_compact_json(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<Vec<Vec<String>>, Diagnostic> {
    if value.is_empty()
        || value.len() > maximum
        || value.starts_with('\u{feff}')
        || value.ends_with('\n')
        || value.contains('\r')
    {
        return Err(super::wire_error(format!(
            "package-build {label} is not bounded compact UTF-8"
        )));
    }
    let mut parser = CanonicalJsonParser {
        bytes: value.as_bytes(),
        offset: 0,
        depth: 0,
        values: 0,
        objects: 0,
        keys: 0,
        object_keys: Vec::new(),
    };
    parser.value()?;
    if parser.offset != parser.bytes.len() {
        return Err(super::wire_error(format!(
            "package-build {label} has trailing JSON data"
        )));
    }
    Ok(parser.object_keys)
}

struct CanonicalJsonParser<'a> {
    bytes: &'a [u8],
    offset: usize,
    depth: usize,
    values: usize,
    objects: usize,
    keys: usize,
    object_keys: Vec<Vec<String>>,
}

impl CanonicalJsonParser<'_> {
    fn value(&mut self) -> Result<(), Diagnostic> {
        self.values = self
            .values
            .checked_add(1)
            .ok_or_else(|| super::wire_error("package-build JSON value count overflowed"))?;
        if self.values > MAX_JSON_VALUES {
            return Err(super::wire_error(
                "package-build JSON value inventory exceeds the closed bound",
            ));
        }
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(|_| ()),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'0'..=b'9') => self.number(),
            _ => Err(super::wire_error(
                "package-build JSON token is invalid or noncanonical",
            )),
        }
    }

    fn object(&mut self) -> Result<(), Diagnostic> {
        self.enter(b'{')?;
        self.objects = self
            .objects
            .checked_add(1)
            .ok_or_else(|| super::wire_error("package-build JSON object count overflowed"))?;
        if self.objects > MAX_JSON_OBJECTS {
            return Err(super::wire_error(
                "package-build JSON object inventory exceeds the closed bound",
            ));
        }
        let object_index = self.object_keys.len();
        self.object_keys.push(Vec::new());
        let mut keys = BTreeSet::new();
        if self.take(b'}') {
            return self.leave();
        }
        loop {
            let key = self.string()?;
            self.keys = self
                .keys
                .checked_add(1)
                .ok_or_else(|| super::wire_error("package-build JSON key count overflowed"))?;
            if self.keys > MAX_JSON_KEYS {
                return Err(super::wire_error(
                    "package-build JSON key inventory exceeds the closed bound",
                ));
            }
            self.object_keys[object_index].push(key.clone());
            if !keys.insert(key) {
                return Err(super::wire_error(
                    "package-build JSON contains a duplicate key",
                ));
            }
            self.expect(b':')?;
            self.value()?;
            if self.take(b'}') {
                return self.leave();
            }
            self.expect(b',')?;
        }
    }

    fn array(&mut self) -> Result<(), Diagnostic> {
        self.enter(b'[')?;
        if self.take(b']') {
            return self.leave();
        }
        loop {
            self.value()?;
            if self.take(b']') {
                return self.leave();
            }
            self.expect(b',')?;
        }
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        let start = self.offset;
        self.expect(b'"')?;
        let mut escaped = false;
        while let Some(byte) = self.peek() {
            self.offset += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let slice = std::str::from_utf8(&self.bytes[start..self.offset])
                    .map_err(|_| super::wire_error("package-build JSON is not UTF-8"))?;
                let decoded: String = serde_json::from_str(slice)
                    .map_err(|_| super::wire_error("package-build JSON string is invalid"))?;
                if quote_json(&decoded) != slice {
                    return Err(super::wire_error(
                        "package-build JSON string is not canonically quoted",
                    ));
                }
                return Ok(decoded);
            } else if byte < 0x20 {
                return Err(super::wire_error(
                    "package-build JSON string contains a control byte",
                ));
            }
        }
        Err(super::wire_error(
            "package-build JSON string is unterminated",
        ))
    }

    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        let slice = std::str::from_utf8(&self.bytes[start..self.offset])
            .map_err(|_| super::wire_error("package-build JSON integer is invalid"))?;
        if slice.is_empty() || (slice.len() > 1 && slice.starts_with('0')) {
            return Err(super::wire_error(
                "package-build JSON integer is not canonical",
            ));
        }
        slice
            .parse::<u64>()
            .map(|_| ())
            .map_err(|_| super::wire_error("package-build JSON integer overflows u64"))
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), Diagnostic> {
        if self.bytes.get(self.offset..self.offset + literal.len()) == Some(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            Err(super::wire_error("package-build JSON literal is invalid"))
        }
    }

    fn enter(&mut self, byte: u8) -> Result<(), Diagnostic> {
        self.expect(byte)?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| super::wire_error("package-build JSON depth overflowed"))?;
        if self.depth > MAX_JSON_DEPTH {
            return Err(super::wire_error(
                "package-build JSON depth exceeds the closed bound",
            ));
        }
        Ok(())
    }

    fn leave(&mut self) -> Result<(), Diagnostic> {
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or_else(|| super::wire_error("package-build JSON depth underflowed"))?;
        Ok(())
    }

    fn expect(&mut self, byte: u8) -> Result<(), Diagnostic> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(super::wire_error(
                "package-build JSON punctuation is invalid",
            ))
        }
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }
}

fn exact_payload(value: &str) -> Result<&str, Diagnostic> {
    const MARKER: &str = "\"payload\":";
    let start = value
        .find(MARKER)
        .map(|offset| offset + MARKER.len())
        .ok_or_else(|| super::wire_error("package-build evidence payload is missing"))?;
    let end = value
        .len()
        .checked_sub(1)
        .ok_or_else(|| super::wire_error("package-build evidence payload is truncated"))?;
    let payload = value
        .get(start..end)
        .ok_or_else(|| super::wire_error("package-build evidence payload boundary is invalid"))?;
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(super::wire_error(
            "package-build evidence payload must be an object",
        ));
    }
    Ok(payload)
}

fn coordinate(value: &crate::package_lock_v2::Coordinate) -> String {
    bf!(
        "{{\"package\":{},\"version\":{}}}",
        quote_json(&value.package),
        quote_json(&value.version)
    )
}

fn export_rows(facts: &BuildFacts) -> String {
    facts
        .exports
        .iter()
        .map(|export| {
            let parameters = export
                .parameters
                .iter()
                .map(|value| quote_json(value))
                .collect::<Vec<_>>()
                .budgeted_join(",");
            bf!(
                "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                parameters,
                quote_json(export.result),
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",")
}

fn string_rows(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",")
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    bf!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}
#[cfg(test)]
mod structural_bound_tests {
    use super::*;

    fn parse(value: &str) -> Result<(), Diagnostic> {
        let mut parser = CanonicalJsonParser {
            bytes: value.as_bytes(),
            offset: 0,
            depth: 0,
            values: 0,
            objects: 0,
            keys: 0,
            object_keys: Vec::new(),
        };
        parser.value()?;
        if parser.offset == parser.bytes.len() {
            Ok(())
        } else {
            Err(super::super::wire_error("test JSON has trailing data"))
        }
    }

    #[test]
    fn structural_depth_bound_accepts_32_and_rejects_33() {
        let at_limit = format!("{}0{}", "[".repeat(32), "]".repeat(32));
        parse(&at_limit).expect("depth 32");
        let over_limit = format!("{}0{}", "[".repeat(33), "]".repeat(33));
        assert_eq!(parse(&over_limit).unwrap_err().code, "SPX-PB506");
    }

    #[test]
    fn structural_value_and_object_amplification_is_bounded() {
        let at_value_limit = format!("[{}]", vec!["0"; MAX_JSON_VALUES - 1].join(","));
        parse(&at_value_limit).expect("value bound");
        let over_value_limit = format!("[{}]", vec!["0"; MAX_JSON_VALUES].join(","));
        assert_eq!(parse(&over_value_limit).unwrap_err().code, "SPX-PB506");

        let at_object_limit = format!("[{}]", vec!["{}"; MAX_JSON_OBJECTS].join(","));
        parse(&at_object_limit).expect("object bound");
        let over_object_limit = format!("[{}]", vec!["{}"; MAX_JSON_OBJECTS + 1].join(","));
        assert_eq!(parse(&over_object_limit).unwrap_err().code, "SPX-PB506");
    }
}
