use std::collections::BTreeSet;

use sha2::{Digest as _, Sha256};

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};

use super::model::{
    LinkedPackageImportFact, LinkedPackageSourceFact, SourceCapsuleOptions, MAX_IMPORTS,
    MAX_OUTPUT_BYTES, MAX_PACKAGES, MAX_RENDER_BYTES, MAX_SOURCE_BYTES, MAX_TOTAL_SOURCE_BYTES,
    MIN_PACKAGES, SCHEMA,
};

const CAPSULE_DOMAIN: &[u8] = b"semaprax.offline-multi-package-source-capsule.v1\0";
const SOURCE_SET_DOMAIN: &[u8] = b"semaprax.offline-multi-package-source-set.v1\0";
const SOURCE_DOMAIN: &[u8] = b"semaprax.offline-multi-package-source.v1\0";
const LINK_DOMAIN: &[u8] = b"semaprax.offline-multi-package-link.v1\0";
const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_VALUES: usize = 16_384;
const MAX_JSON_OBJECTS: usize = 2_048;
const MAX_JSON_KEYS: usize = 16_384;

pub(crate) struct RenderInput<'a> {
    pub(crate) resolution_digest: &'a str,
    pub(crate) resolution_bytes: usize,
    pub(crate) lock_digest: &'a str,
    pub(crate) lock_bytes: usize,
    pub(crate) options: &'a SourceCapsuleOptions,
    pub(crate) facts: &'a [LinkedPackageSourceFact],
    pub(crate) imports: &'a [LinkedPackageImportFact],
    pub(crate) linked_function_ids: &'a [String],
    pub(crate) sources: &'a [super::PackageSource],
    pub(crate) source_set_digest: &'a str,
    pub(crate) link_digest: &'a str,
}

pub(crate) fn render(input: RenderInput<'_>) -> Result<String, Diagnostic> {
    let RenderInput {
        resolution_digest,
        resolution_bytes,
        lock_digest,
        lock_bytes,
        options,
        facts,
        imports,
        linked_function_ids,
        sources,
        source_set_digest,
        link_digest,
    } = input;
    let packages = facts.iter().zip(sources).map(|(fact, source)| bounded_output::budgeted_format(format_args!(
        "{{\"package\":{},\"version\":{},\"subject_digest\":{},\"report_digest\":{},\"interface_digest\":{},\"interface_source_revision\":{},\"source_revision\":{},\"source_digest\":{},\"source_bytes\":{},\"source\":{}}}",
        quote_json(&fact.coordinate.package), quote_json(&fact.coordinate.version),
        quote_json(&fact.subject_digest), quote_json(&fact.report_digest),
        quote_json(&fact.interface_digest), quote_json(&fact.interface_source_revision),
        quote_json(&fact.source_revision), quote_json(&fact.source_digest), fact.source_bytes,
        quote_json(&source.source)
    ))).collect::<Vec<_>>().budgeted_join(",");
    let import_rows = imports.iter().map(|import| bounded_output::budgeted_format(format_args!(
        "{{\"dependent\":{{\"package\":{},\"version\":{}}},\"dependency\":{{\"package\":{},\"version\":{}}},\"kind\":\"function\",\"target\":{},\"alias\":{},\"ordinal\":{}}}",
        quote_json(&import.dependent.package), quote_json(&import.dependent.version),
        quote_json(&import.dependency.package), quote_json(&import.dependency.version),
        quote_json(&import.target), quote_json(&import.alias), import.ordinal
    ))).collect::<Vec<_>>().budgeted_join(",");
    let linked = linked_function_ids
        .iter()
        .map(|id| quote_json(id))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let used_source_bytes = facts
        .iter()
        .try_fold(0usize, |sum, fact| sum.checked_add(fact.source_bytes))
        .ok_or_else(|| super::limit_error("package-source source byte accounting overflowed"))?;
    let render_payload = |used_output_bytes: usize| {
        bounded_output::budgeted_format(format_args!(
        "{{\"schema\":{},\"resolution_digest\":{},\"resolution_bytes\":{},\"lock_digest\":{},\"lock_bytes\":{},\"root\":{},\"packages\":[{}],\"imports\":[{}],\"linked_functions\":[{}],\"source_set_digest\":{},\"link_digest\":{},\"limits\":{{\"min_packages\":{},\"max_packages\":{},\"max_source_bytes\":{},\"max_total_source_bytes\":{},\"max_imports\":{},\"max_render_bytes\":{},\"max_output_bytes\":{},\"requested_max_bytes\":{}}},\"budget\":{{\"used_packages\":{},\"used_source_bytes\":{},\"used_imports\":{},\"used_linked_functions\":{},\"used_output_bytes\":{}}},\"nonclaims\":[\"caller_owned_source_capsule_not_registry_fetch_or_provenance\",\"capsule_sources_are_the_only_executable_code\",\"dependency_metadata_only_checks_source_derived_import_association\",\"report_facts_are_interface_evidence_not_implementation_source\",\"effect_free_scalar_static_link_only\",\"no_external_tools_target_execution_component_model_wasi_or_dynamic_linking\",\"evidence_is_not_authority\"]}}",
        quote_json(SCHEMA), quote_json(resolution_digest), resolution_bytes,
        quote_json(lock_digest), lock_bytes, quote_json(&options.root_package), packages,
        import_rows, linked, quote_json(source_set_digest), quote_json(link_digest),
        MIN_PACKAGES, MAX_PACKAGES, MAX_SOURCE_BYTES, MAX_TOTAL_SOURCE_BYTES, MAX_IMPORTS,
        MAX_RENDER_BYTES, MAX_OUTPUT_BYTES, options.max_bytes, facts.len(), used_source_bytes,
        imports.len(), linked_function_ids.len(), used_output_bytes))
    };
    let mut used_output_bytes = 0usize;
    for _ in 0..8 {
        let payload = render_payload(used_output_bytes);
        let next = bounded_output::budgeted_format(format_args!(
            "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
            quote_json(SCHEMA),
            quote_json(&domain_digest(CAPSULE_DOMAIN, payload.as_bytes())),
            payload.len(),
            payload
        ));
        if next.len() == used_output_bytes {
            return Ok(next);
        }
        used_output_bytes = next.len();
    }
    Err(super::limit_error(
        "package-source capsule output accounting did not converge",
    ))
}

pub(crate) fn validate_submitted(value: &str, maximum: usize) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.len() > maximum
        || value.len() > MAX_OUTPUT_BYTES
        || value.starts_with('\u{feff}')
        || value.ends_with('\n')
        || value.contains('\r')
    {
        return Err(super::wire_error(
            "package-source capsule is outside its wire bound",
        ));
    }
    validate_compact_json(value)?;
    let parsed: serde_json::Value = serde_json::from_str(value)
        .map_err(|_| super::wire_error("package-source capsule is not JSON"))?;
    let object = parsed
        .as_object()
        .ok_or_else(|| super::wire_error("package-source capsule must be an object"))?;
    if object.len() != 4
        || object.get("schema").and_then(|v| v.as_str()) != Some(SCHEMA)
        || !object.contains_key("digest")
        || !object.contains_key("bytes")
        || !object.contains_key("payload")
    {
        return Err(super::wire_error(
            "package-source capsule wrapper shape is invalid",
        ));
    }
    let marker = "\"payload\":";
    let start = value
        .find(marker)
        .ok_or_else(|| super::wire_error("package-source payload is missing"))?
        + marker.len();
    let payload = value
        .get(start..value.len().saturating_sub(1))
        .ok_or_else(|| super::wire_error("package-source payload range is invalid"))?;
    if object.get("bytes").and_then(|v| v.as_u64()) != Some(payload.len() as u64)
        || object.get("digest").and_then(|v| v.as_str())
            != Some(domain_digest(CAPSULE_DOMAIN, payload.as_bytes()).as_str())
    {
        return Err(super::wire_error(
            "package-source capsule wrapper binding is invalid",
        ));
    }
    Ok(())
}

pub(crate) fn wrapper_digest(value: &str) -> Result<String, Diagnostic> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|v| v["digest"].as_str().map(bounded_output::budgeted_clone))
        .ok_or_else(|| super::authentication_error("package-source bound wrapper digest is absent"))
}

pub(crate) fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DOMAIN, source.as_bytes())
}

pub(crate) fn source_set_digest(facts: &[(&crate::package_lock_v2::Coordinate, &str)]) -> String {
    let mut hasher = domain_hasher(SOURCE_SET_DOMAIN);
    hasher.update((facts.len() as u64).to_le_bytes());
    for (coordinate, source) in facts {
        hash_field(&mut hasher, coordinate.package.as_bytes());
        hash_field(&mut hasher, coordinate.version.as_bytes());
        hash_field(&mut hasher, source.as_bytes());
    }
    bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ))
}

pub(crate) fn link_digest(
    source_set: &str,
    root: &str,
    modules: &[crate::workspace_graph::PackageWorkspaceModule],
    imports: &[crate::workspace_graph::PackageWorkspaceImport],
    linked_function_ids: &[String],
) -> String {
    let mut hasher = domain_hasher(LINK_DOMAIN);
    hash_field(&mut hasher, source_set.as_bytes());
    hash_field(&mut hasher, root.as_bytes());
    hasher.update((modules.len() as u64).to_le_bytes());
    for module in modules {
        hash_field(&mut hasher, module.package.as_bytes());
        hash_field(&mut hasher, module.interface.digest.as_bytes());
    }
    hasher.update((imports.len() as u64).to_le_bytes());
    for import in imports {
        hash_field(&mut hasher, import.dependent.as_bytes());
        hash_field(&mut hasher, import.dependency.as_bytes());
        hash_field(&mut hasher, import.target.as_bytes());
        hash_field(&mut hasher, import.alias.as_bytes());
        hasher.update((import.ordinal as u64).to_le_bytes());
    }
    hasher.update((linked_function_ids.len() as u64).to_le_bytes());
    for id in linked_function_ids {
        hash_field(&mut hasher, id.as_bytes());
    }
    bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ))
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = domain_hasher(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ))
}
fn domain_hasher(domain: &[u8]) -> Sha256 {
    let mut h = Sha256::new();
    h.update(domain);
    h
}
fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn validate_compact_json(value: &str) -> Result<(), Diagnostic> {
    let mut parser = CanonicalJsonParser {
        bytes: value.as_bytes(),
        offset: 0,
        depth: 0,
        values: 0,
        objects: 0,
        keys: 0,
    };
    parser.value()?;
    if parser.offset != parser.bytes.len() {
        return Err(super::wire_error(
            "package-source capsule has trailing JSON data",
        ));
    }
    Ok(())
}

struct CanonicalJsonParser<'a> {
    bytes: &'a [u8],
    offset: usize,
    depth: usize,
    values: usize,
    objects: usize,
    keys: usize,
}

impl CanonicalJsonParser<'_> {
    fn value(&mut self) -> Result<(), Diagnostic> {
        self.values = self
            .values
            .checked_add(1)
            .ok_or_else(|| super::wire_error("package-source JSON value count overflowed"))?;
        if self.values > MAX_JSON_VALUES {
            return Err(super::wire_error(
                "package-source JSON value inventory exceeds the closed bound",
            ));
        }
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'\"') => self.string().map(|_| ()),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'0'..=b'9') => self.number(),
            _ => Err(super::wire_error(
                "package-source JSON token is invalid or noncanonical",
            )),
        }
    }

    fn object(&mut self) -> Result<(), Diagnostic> {
        self.enter(b'{')?;
        self.objects = self
            .objects
            .checked_add(1)
            .ok_or_else(|| super::wire_error("package-source JSON object count overflowed"))?;
        if self.objects > MAX_JSON_OBJECTS {
            return Err(super::wire_error(
                "package-source JSON object inventory exceeds the closed bound",
            ));
        }
        let mut keys = BTreeSet::new();
        if self.take(b'}') {
            return self.leave();
        }
        loop {
            let key = self.string()?;
            self.keys = self
                .keys
                .checked_add(1)
                .ok_or_else(|| super::wire_error("package-source JSON key count overflowed"))?;
            if self.keys > MAX_JSON_KEYS {
                return Err(super::wire_error(
                    "package-source JSON key inventory exceeds the closed bound",
                ));
            }
            if !keys.insert(key) {
                return Err(super::wire_error(
                    "package-source JSON contains a duplicate key",
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
        self.expect(b'\"')?;
        let mut escaped = false;
        while let Some(byte) = self.peek() {
            self.offset += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'\"' {
                let slice = std::str::from_utf8(&self.bytes[start..self.offset])
                    .map_err(|_| super::wire_error("package-source JSON is not UTF-8"))?;
                let decoded: String = serde_json::from_str(slice)
                    .map_err(|_| super::wire_error("package-source JSON string is invalid"))?;
                if quote_json(&decoded) != slice {
                    return Err(super::wire_error(
                        "package-source JSON string is not canonically quoted",
                    ));
                }
                return Ok(decoded);
            } else if byte < 0x20 {
                return Err(super::wire_error(
                    "package-source JSON string contains a control byte",
                ));
            }
        }
        Err(super::wire_error(
            "package-source JSON string is unterminated",
        ))
    }

    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        let slice = std::str::from_utf8(&self.bytes[start..self.offset])
            .map_err(|_| super::wire_error("package-source JSON integer is invalid"))?;
        if slice.is_empty() || (slice.len() > 1 && slice.starts_with('0')) {
            return Err(super::wire_error(
                "package-source JSON integer is not canonical",
            ));
        }
        slice
            .parse::<u64>()
            .map(|_| ())
            .map_err(|_| super::wire_error("package-source JSON integer overflows u64"))
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), Diagnostic> {
        if self.bytes.get(self.offset..self.offset + literal.len()) == Some(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            Err(super::wire_error("package-source JSON literal is invalid"))
        }
    }

    fn enter(&mut self, byte: u8) -> Result<(), Diagnostic> {
        self.expect(byte)?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| super::wire_error("package-source JSON depth overflowed"))?;
        if self.depth > MAX_JSON_DEPTH {
            return Err(super::wire_error(
                "package-source JSON depth exceeds the closed bound",
            ));
        }
        Ok(())
    }

    fn leave(&mut self) -> Result<(), Diagnostic> {
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or_else(|| super::wire_error("package-source JSON depth underflowed"))?;
        Ok(())
    }

    fn expect(&mut self, byte: u8) -> Result<(), Diagnostic> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(super::wire_error(
                "package-source JSON punctuation is invalid",
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
