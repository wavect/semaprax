use sha2::{Digest as _, Sha256};

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};

use super::model::{
    BuildFacts, LinkedOfflinePackageBuildOptions, EVIDENCE_SCHEMA, MANIFEST_SCHEMA, NONCLAIMS,
    PROFILE,
};

const MANIFEST_DOMAIN: &[u8] = b"semaprax.offline-linked-scalar-wasm-package-build.v2\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.offline-linked-scalar-wasm-package-build-evidence.v2\0";

macro_rules! bf { ($($arg:tt)*) => { bounded_output::budgeted_format(format_args!($($arg)*)) }; }

pub(crate) fn wasm_digest(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    bf!("sha256:{:x}", crate::digest_hex::LowerHex(h.finalize()))
}
fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(bytes);
    bf!("sha256:{:x}", crate::digest_hex::LowerHex(h.finalize()))
}
pub(crate) fn manifest_digest(bytes: &str) -> String {
    digest(MANIFEST_DOMAIN, bytes.as_bytes())
}
fn coordinate(value: &crate::package_lock_v2::Coordinate) -> String {
    bf!(
        "{{\"package\":{},\"version\":{}}}",
        quote_json(&value.package),
        quote_json(&value.version)
    )
}
fn strings(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| quote_json(v))
        .collect::<Vec<_>>()
        .budgeted_join(",")
}
fn packages(facts: &BuildFacts) -> String {
    facts
        .packages
        .iter()
        .map(coordinate)
        .collect::<Vec<_>>()
        .budgeted_join(",")
}
fn exports(facts: &BuildFacts) -> String {
    facts
        .exports
        .iter()
        .map(|e| {
            bf!(
                "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
                quote_json(&e.stable_id),
                quote_json(&e.wasm_export),
                strings(&e.parameters),
                quote_json(e.result)
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",")
}
fn imports() -> String {
    crate::package_build::RUNTIME_IMPORTS
        .iter()
        .map(|name| {
            bf!(
                "{{\"module\":\"env\",\"name\":{},\"kind\":\"function\"}}",
                quote_json(name)
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",")
}
pub(crate) fn render_manifest(
    f: &BuildFacts,
    o: &LinkedOfflinePackageBuildOptions,
    wasm: &str,
    wasm_bytes: usize,
) -> String {
    bf!("{{\"schema\":{},\"profile\":{},\"root\":{},\"packages\":[{}],\"inputs\":{{\"capsule_schema\":{},\"capsule_digest\":{},\"capsule_bytes\":{},\"source_set_digest\":{},\"link_digest\":{}}},\"exports\":[{}],\"runtime_imports\":[{}],\"module\":{{\"path\":\"module.wasm\",\"sha256\":{},\"bytes\":{}}},\"compiler\":{{\"package\":\"semaprax\",\"version\":{}}},\"limits\":{{\"max_artifact_bytes\":{},\"max_evidence_bytes\":{}}},\"nonclaims\":[{}]}}", quote_json(MANIFEST_SCHEMA), quote_json(PROFILE), coordinate(&f.root), packages(f), quote_json(&f.capsule_schema), quote_json(&f.capsule_digest), f.capsule_bytes, quote_json(&f.source_set_digest), quote_json(&f.link_digest), exports(f), imports(), quote_json(wasm), wasm_bytes, quote_json(env!("CARGO_PKG_VERSION")), o.max_artifact_bytes, o.max_evidence_bytes, strings(&NONCLAIMS))
}
pub(crate) fn render_evidence(
    f: &BuildFacts,
    o: &LinkedOfflinePackageBuildOptions,
    manifest: &str,
    manifest_hash: &str,
    wasm: &str,
    wasm_bytes: usize,
) -> Result<String, Diagnostic> {
    let mut evidence_bytes = 0usize;
    for _ in 0..32 {
        let artifact_bytes = wasm_bytes
            .checked_add(manifest.len())
            .and_then(|v| v.checked_add(evidence_bytes))
            .ok_or_else(|| {
                super::limit_error("linked package-build artifact byte sum overflowed")
            })?;
        let payload = bf!("{{\"schema\":{},\"capsule_schema\":{},\"capsule_digest\":{},\"capsule_bytes\":{},\"root\":{},\"packages\":[{}],\"exports\":[{}],\"source_set_digest\":{},\"link_digest\":{},\"manifest_digest\":{},\"manifest_bytes\":{},\"wasm_digest\":{},\"wasm_bytes\":{},\"limits\":{{\"max_artifact_bytes\":{},\"max_evidence_bytes\":{}}},\"budget\":{{\"used_source_bytes\":{},\"used_wasm_bytes\":{},\"used_manifest_bytes\":{},\"used_evidence_bytes\":{},\"used_artifact_bytes\":{}}},\"nonclaims\":[{}]}}", quote_json(EVIDENCE_SCHEMA), quote_json(&f.capsule_schema), quote_json(&f.capsule_digest), f.capsule_bytes, coordinate(&f.root), packages(f), exports(f), quote_json(&f.source_set_digest), quote_json(&f.link_digest), quote_json(manifest_hash), manifest.len(), quote_json(wasm), wasm_bytes, o.max_artifact_bytes, o.max_evidence_bytes, f.source_bytes, wasm_bytes, manifest.len(), evidence_bytes, artifact_bytes, strings(&NONCLAIMS));
        let result = bf!(
            "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
            quote_json(EVIDENCE_SCHEMA),
            quote_json(&digest(EVIDENCE_DOMAIN, payload.as_bytes())),
            payload.len(),
            payload
        );
        if result.len() == evidence_bytes {
            return Ok(result);
        }
        evidence_bytes = result.len();
    }
    Err(super::limit_error(
        "linked package-build evidence byte accounting did not converge",
    ))
}

fn parse_wire(
    value: &str,
    maximum: usize,
    schema: &str,
    label: &str,
) -> Result<(serde_json::Value, ObjectOrder), Diagnostic> {
    let keys = crate::package_build::wire::validate_compact_json_keys(value, maximum, label)
        .map_err(|_| {
            super::wire_error(format!(
                "linked package-build {label} is not bounded canonical JSON"
            ))
        })?;
    let parsed: serde_json::Value = serde_json::from_str(value)
        .map_err(|_| super::wire_error(format!("linked package-build {label} is not JSON")))?;
    let object = parsed.as_object().ok_or_else(|| {
        super::wire_error(format!("linked package-build {label} is not an object"))
    })?;
    if object.get("schema").and_then(serde_json::Value::as_str) != Some(schema) {
        return Err(super::wire_error(format!(
            "linked package-build {label} schema is not exact"
        )));
    }
    Ok((parsed, ObjectOrder { keys, next: 0 }))
}

struct ObjectOrder {
    keys: Vec<Vec<String>>,
    next: usize,
}

impl ObjectOrder {
    fn require(&mut self, value: &serde_json::Value, expected: &[&str]) -> Result<(), Diagnostic> {
        if !value.is_object()
            || self.keys.get(self.next).map(Vec::as_slice)
                != Some(
                    &expected
                        .iter()
                        .map(|key| (*key).to_owned())
                        .collect::<Vec<_>>()[..],
                )
        {
            return Err(super::wire_error(
                "linked package-build object shape or field order is not exact",
            ));
        }
        self.next += 1;
        Ok(())
    }

    fn finish(self) -> Result<(), Diagnostic> {
        if self.next == self.keys.len() {
            Ok(())
        } else {
            Err(super::wire_error(
                "linked package-build object inventory is not exact",
            ))
        }
    }
}

fn require_coordinate(
    value: &serde_json::Value,
    order: &mut ObjectOrder,
) -> Result<(), Diagnostic> {
    order.require(value, &["package", "version"])?;
    require_strings(value, &["package", "version"])
}

fn require_strings(value: &serde_json::Value, keys: &[&str]) -> Result<(), Diagnostic> {
    if keys.iter().all(|key| value[*key].is_string()) {
        Ok(())
    } else {
        Err(super::wire_error(
            "linked package-build string field is not exact",
        ))
    }
}

fn require_numbers(value: &serde_json::Value, keys: &[&str]) -> Result<(), Diagnostic> {
    if keys.iter().all(|key| value[*key].as_u64().is_some()) {
        Ok(())
    } else {
        Err(super::wire_error(
            "linked package-build integer field is not exact",
        ))
    }
}

fn require_string_array(value: &serde_json::Value) -> Result<(), Diagnostic> {
    if value
        .as_array()
        .is_some_and(|rows| rows.iter().all(serde_json::Value::is_string))
    {
        Ok(())
    } else {
        Err(super::wire_error(
            "linked package-build string array is not exact",
        ))
    }
}

fn require_export(value: &serde_json::Value, order: &mut ObjectOrder) -> Result<(), Diagnostic> {
    order.require(value, &["stable_id", "wasm_export", "parameters", "result"])?;
    require_strings(value, &["stable_id", "wasm_export", "result"])?;
    if value["parameters"]
        .as_array()
        .is_some_and(|values| values.iter().all(serde_json::Value::is_string))
    {
        Ok(())
    } else {
        Err(super::wire_error(
            "linked package-build export parameters are not exact",
        ))
    }
}

pub(crate) fn validate_submitted_manifest(value: &str, maximum: usize) -> Result<(), Diagnostic> {
    let (parsed, mut order) = parse_wire(value, maximum, MANIFEST_SCHEMA, "manifest")?;
    order.require(
        &parsed,
        &[
            "schema",
            "profile",
            "root",
            "packages",
            "inputs",
            "exports",
            "runtime_imports",
            "module",
            "compiler",
            "limits",
            "nonclaims",
        ],
    )?;
    if parsed["profile"].as_str() != Some(PROFILE) {
        return Err(super::wire_error(
            "linked package-build manifest profile is not exact",
        ));
    }
    require_coordinate(&parsed["root"], &mut order)?;
    for package in parsed["packages"]
        .as_array()
        .ok_or_else(|| super::wire_error("linked package-build packages are not an array"))?
    {
        require_coordinate(package, &mut order)?;
    }
    order.require(
        &parsed["inputs"],
        &[
            "capsule_schema",
            "capsule_digest",
            "capsule_bytes",
            "source_set_digest",
            "link_digest",
        ],
    )?;
    require_strings(
        &parsed["inputs"],
        &[
            "capsule_schema",
            "capsule_digest",
            "source_set_digest",
            "link_digest",
        ],
    )?;
    require_numbers(&parsed["inputs"], &["capsule_bytes"])?;
    for export in parsed["exports"]
        .as_array()
        .ok_or_else(|| super::wire_error("linked package-build exports are not an array"))?
    {
        require_export(export, &mut order)?;
    }
    for import in parsed["runtime_imports"]
        .as_array()
        .ok_or_else(|| super::wire_error("linked package-build imports are not an array"))?
    {
        order.require(import, &["module", "name", "kind"])?;
        require_strings(import, &["module", "name", "kind"])?;
    }
    order.require(&parsed["module"], &["path", "sha256", "bytes"])?;
    require_strings(&parsed["module"], &["path", "sha256"])?;
    require_numbers(&parsed["module"], &["bytes"])?;
    order.require(&parsed["compiler"], &["package", "version"])?;
    require_strings(&parsed["compiler"], &["package", "version"])?;
    order.require(
        &parsed["limits"],
        &["max_artifact_bytes", "max_evidence_bytes"],
    )?;
    require_numbers(
        &parsed["limits"],
        &["max_artifact_bytes", "max_evidence_bytes"],
    )?;
    require_string_array(&parsed["nonclaims"])?;
    order.finish()
}
pub(crate) fn validate_submitted_evidence(value: &str, maximum: usize) -> Result<(), Diagnostic> {
    let (parsed, mut order) = parse_wire(value, maximum, EVIDENCE_SCHEMA, "evidence")?;
    order.require(&parsed, &["schema", "digest", "bytes", "payload"])?;
    require_strings(&parsed, &["schema", "digest"])?;
    require_numbers(&parsed, &["bytes"])?;
    let payload = &parsed["payload"];
    order.require(
        payload,
        &[
            "schema",
            "capsule_schema",
            "capsule_digest",
            "capsule_bytes",
            "root",
            "packages",
            "exports",
            "source_set_digest",
            "link_digest",
            "manifest_digest",
            "manifest_bytes",
            "wasm_digest",
            "wasm_bytes",
            "limits",
            "budget",
            "nonclaims",
        ],
    )?;
    require_strings(
        payload,
        &[
            "schema",
            "capsule_schema",
            "capsule_digest",
            "source_set_digest",
            "link_digest",
            "manifest_digest",
            "wasm_digest",
        ],
    )?;
    if payload["schema"].as_str() != Some(EVIDENCE_SCHEMA) {
        return Err(super::wire_error(
            "linked package-build evidence payload schema is not exact",
        ));
    }
    require_numbers(payload, &["capsule_bytes", "manifest_bytes", "wasm_bytes"])?;
    require_coordinate(&payload["root"], &mut order)?;
    for package in payload["packages"]
        .as_array()
        .ok_or_else(|| super::wire_error("linked package-build packages are not an array"))?
    {
        require_coordinate(package, &mut order)?;
    }
    for export in payload["exports"]
        .as_array()
        .ok_or_else(|| super::wire_error("linked package-build exports are not an array"))?
    {
        require_export(export, &mut order)?;
    }
    order.require(
        &payload["limits"],
        &["max_artifact_bytes", "max_evidence_bytes"],
    )?;
    require_numbers(
        &payload["limits"],
        &["max_artifact_bytes", "max_evidence_bytes"],
    )?;
    order.require(
        &payload["budget"],
        &[
            "used_source_bytes",
            "used_wasm_bytes",
            "used_manifest_bytes",
            "used_evidence_bytes",
            "used_artifact_bytes",
        ],
    )?;
    require_numbers(
        &payload["budget"],
        &[
            "used_source_bytes",
            "used_wasm_bytes",
            "used_manifest_bytes",
            "used_evidence_bytes",
            "used_artifact_bytes",
        ],
    )?;
    require_string_array(&payload["nonclaims"])?;
    order.finish()
}
