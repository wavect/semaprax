//! Replay-only admission for compiler-resolved Rust API metadata.
//!
//! This crate deliberately does not invoke Cargo, rustc, rustdoc, or a macro
//! expander.  A separately installed, pinned extractor may emit the compact
//! envelope accepted here.  Stable consumers only replay a prepared envelope;
//! they never silently download or select a nightly compiler.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt::Write;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const RUST_API_INDEX_SCHEMA: &str = "semaprax.rust-api-index.v1";
pub const RUSTDOC_EXTRACTOR_SCHEMA: &str = "semaprax.rustdoc-extractor.v1";
pub const MAX_INDEX_BYTES: usize = 1_048_576;
pub const MAX_ITEMS: usize = 512;
pub const MAX_PATH_BYTES: usize = 512;
pub const MAX_SIGNATURE_BYTES: usize = 4_096;
pub const MAX_TYPE_DEPTH: usize = 32;
pub const EXTRACTOR_SETUP_INSTRUCTION: &str = "Install the project-pinned nightly rustdoc JSON extractor, or provide an index prepared for the selected stable target and features.";

const INDEX_DIGEST_DOMAIN: &[u8] = b"semaprax.rust-api-index.digest.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustApiIndex {
    canonical: String,
    digest: String,
    package: PackageIdentity,
    target: String,
    feature_digest: String,
    extractor: ExtractorIdentity,
    items: Vec<ApiItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
    pub source_sha256: String,
    pub renamed_from: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractorIdentity {
    pub mode: ExtractorMode,
    pub executable_sha256: String,
    pub rustc_version: String,
    pub rustdoc_format: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractorMode {
    Prepared,
    NightlyRustdocJson,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiItem {
    pub path: String,
    pub kind: ItemKind,
    pub receiver: Receiver,
    pub signature: String,
    pub type_depth: usize,
    pub support: Support,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Function,
    InherentMethod,
    TraitMethod,
    AssociatedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Receiver {
    None,
    Shared,
    Mutable,
    Owned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Support {
    Supported,
    Rejected { reason: RejectionReason },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    Private,
    SealedTrait,
    OpaqueReturn,
    UnsupportedGeneric,
    UnsupportedSignature,
    ExpansionLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexError {
    Malformed,
    SetupRequired,
    IdentityMismatch,
    ItemUnavailable,
}

impl RustApiIndex {
    /// Replays an exact prepared index.  It accepts no extractor output and
    /// therefore remains usable on a stable-only installation.
    pub fn replay(bytes: &[u8]) -> Result<Self, IndexError> {
        validate_input(bytes)?;
        let value: Value = serde_json::from_slice(bytes).map_err(|_| IndexError::Malformed)?;
        let root = exact_object(&value, 6)?;
        if string(root, "schema")? != RUST_API_INDEX_SCHEMA {
            return Err(IndexError::Malformed);
        }
        let package = parse_package(required(root, "package")?)?;
        let target = bounded(string(root, "target")?, MAX_PATH_BYTES)?;
        let feature_digest = digest(string(root, "feature_digest")?)?.to_owned();
        let extractor = parse_extractor(required(root, "extractor")?)?;
        let limits = exact_object(required(root, "limits")?, 3)?;
        if number(limits, "max_items")? != MAX_ITEMS
            || number(limits, "max_type_depth")? != MAX_TYPE_DEPTH
            || number(limits, "max_index_bytes")? != MAX_INDEX_BYTES
        {
            return Err(IndexError::Malformed);
        }
        let rows = required(root, "items")?
            .as_array()
            .filter(|rows| rows.len() <= MAX_ITEMS)
            .ok_or(IndexError::Malformed)?;
        let mut paths = BTreeSet::new();
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let item = parse_item(row)?;
            if !paths.insert(item.path.clone()) {
                return Err(IndexError::Malformed);
            }
            items.push(item);
        }
        if !items
            .windows(2)
            .all(|pair| pair[0].path.as_bytes() < pair[1].path.as_bytes())
        {
            return Err(IndexError::Malformed);
        }
        let index = Self {
            canonical: render(&package, &target, &feature_digest, &extractor, &items),
            digest: String::new(),
            package,
            target,
            feature_digest,
            extractor,
            items,
        };
        if index.canonical.as_bytes() != bytes {
            return Err(IndexError::Malformed);
        }
        let digest = index_digest(bytes);
        Ok(Self { digest, ..index })
    }

    /// Admits output from the explicit extractor protocol.  This does not run
    /// the extractor.  A missing extractor is a setup error, never a fallback.
    pub fn admit_extractor_output(bytes: &[u8]) -> Result<Self, IndexError> {
        validate_input(bytes)?;
        let value: Value = serde_json::from_slice(bytes).map_err(|_| IndexError::Malformed)?;
        let root = exact_object(&value, 2)?;
        if string(root, "schema")? != RUSTDOC_EXTRACTOR_SCHEMA {
            return Err(IndexError::SetupRequired);
        }
        let index = required(root, "index")?;
        let rendered = serde_json::to_string(index).map_err(|_| IndexError::Malformed)?;
        let mut canonical = rendered.into_bytes();
        canonical.push(b'\n');
        let admitted = Self::replay(&canonical)?;
        if admitted.extractor.mode != ExtractorMode::NightlyRustdocJson {
            return Err(IndexError::Malformed);
        }
        Ok(admitted)
    }

    pub fn canonical_json(&self) -> &str {
        &self.canonical
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn package(&self) -> &PackageIdentity {
        &self.package
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn feature_digest(&self) -> &str {
        &self.feature_digest
    }
    pub fn extractor(&self) -> &ExtractorIdentity {
        &self.extractor
    }
    pub fn items(&self) -> &[ApiItem] {
        &self.items
    }

    /// Rejects a prepared index when the selected stable target/features or
    /// package source have drifted, before any foreign call can be prepared.
    pub fn require_identity(
        &self,
        source_sha256: &str,
        target: &str,
        feature_digest: &str,
    ) -> Result<(), IndexError> {
        if self.package.source_sha256 != source_sha256
            || self.target != target
            || self.feature_digest != feature_digest
        {
            return Err(IndexError::IdentityMismatch);
        }
        Ok(())
    }

    /// Checks the package as well as target, enabled features, and source
    /// digest. Callers should do this before validating or preparing wrappers.
    pub fn require_package_identity(
        &self,
        name: &str,
        version: &str,
        source_sha256: &str,
        target: &str,
        feature_digest: &str,
    ) -> Result<(), IndexError> {
        if self.package.name != name || self.package.version != version {
            return Err(IndexError::IdentityMismatch);
        }
        self.require_identity(source_sha256, target, feature_digest)
    }

    /// Returns selected records only when every path exists and is supported.
    /// This is discovery admission; the selected stable compiler must still
    /// validate generated signatures and calls before foreign execution.
    pub fn select_supported<'a>(&'a self, paths: &[&str]) -> Result<Vec<&'a ApiItem>, IndexError> {
        if paths.is_empty() || paths.len() > MAX_ITEMS {
            return Err(IndexError::ItemUnavailable);
        }
        let mut selected = Vec::with_capacity(paths.len());
        let mut previous: Option<&str> = None;
        for path in paths {
            if previous.is_some_and(|value| value.as_bytes() >= path.as_bytes()) {
                return Err(IndexError::ItemUnavailable);
            }
            previous = Some(path);
            let item = self
                .items
                .binary_search_by(|item| item.path.as_str().cmp(path))
                .ok()
                .map(|index| &self.items[index])
                .filter(|item| item.support == Support::Supported)
                .ok_or(IndexError::ItemUnavailable)?;
            selected.push(item);
        }
        Ok(selected)
    }
}

fn parse_package(value: &Value) -> Result<PackageIdentity, IndexError> {
    let object = exact_object(value, 4)?;
    let renamed = match required(object, "renamed_from")? {
        Value::Null => None,
        Value::String(value) => Some(bounded(value, MAX_PATH_BYTES)?),
        _ => return Err(IndexError::Malformed),
    };
    Ok(PackageIdentity {
        name: bounded(string(object, "name")?, MAX_PATH_BYTES)?,
        version: bounded(string(object, "version")?, 128)?,
        source_sha256: digest(string(object, "source_sha256")?)?.to_owned(),
        renamed_from: renamed,
    })
}

fn parse_extractor(value: &Value) -> Result<ExtractorIdentity, IndexError> {
    let object = exact_object(value, 4)?;
    let mode = match string(object, "mode")? {
        "prepared" => ExtractorMode::Prepared,
        "nightly-rustdoc-json" => ExtractorMode::NightlyRustdocJson,
        _ => return Err(IndexError::Malformed),
    };
    if mode == ExtractorMode::Prepared
        && !string(object, "rustdoc_format")?.starts_with("prepared:")
    {
        return Err(IndexError::Malformed);
    }
    Ok(ExtractorIdentity {
        mode,
        executable_sha256: digest(string(object, "executable_sha256")?)?.to_owned(),
        rustc_version: bounded(string(object, "rustc_version")?, 256)?,
        rustdoc_format: bounded(string(object, "rustdoc_format")?, 256)?,
    })
}

fn parse_item(value: &Value) -> Result<ApiItem, IndexError> {
    let object = exact_object(value, 6)?;
    let kind = match string(object, "kind")? {
        "function" => ItemKind::Function,
        "inherent_method" => ItemKind::InherentMethod,
        "trait_method" => ItemKind::TraitMethod,
        "associated_type" => ItemKind::AssociatedType,
        _ => return Err(IndexError::Malformed),
    };
    let receiver = match string(object, "receiver")? {
        "none" => Receiver::None,
        "shared" => Receiver::Shared,
        "mutable" => Receiver::Mutable,
        "owned" => Receiver::Owned,
        _ => return Err(IndexError::Malformed),
    };
    let type_depth = number(object, "type_depth")?;
    if type_depth > MAX_TYPE_DEPTH {
        return Err(IndexError::Malformed);
    }
    let support = match string(object, "support")? {
        "supported" if required(object, "reason")? == &Value::Null => Support::Supported,
        "rejected" => Support::Rejected {
            reason: parse_reason(string(object, "reason")?)?,
        },
        _ => return Err(IndexError::Malformed),
    };
    Ok(ApiItem {
        path: bounded(string(object, "path")?, MAX_PATH_BYTES)?,
        kind,
        receiver,
        signature: bounded(string(object, "signature")?, MAX_SIGNATURE_BYTES)?,
        type_depth,
        support,
    })
}

fn parse_reason(value: &str) -> Result<RejectionReason, IndexError> {
    match value {
        "private" => Ok(RejectionReason::Private),
        "sealed_trait" => Ok(RejectionReason::SealedTrait),
        "opaque_return" => Ok(RejectionReason::OpaqueReturn),
        "unsupported_generic" => Ok(RejectionReason::UnsupportedGeneric),
        "unsupported_signature" => Ok(RejectionReason::UnsupportedSignature),
        "expansion_limit" => Ok(RejectionReason::ExpansionLimit),
        _ => Err(IndexError::Malformed),
    }
}

fn validate_input(bytes: &[u8]) -> Result<(), IndexError> {
    if bytes.is_empty()
        || bytes.len() > MAX_INDEX_BYTES
        || !bytes.ends_with(b"\n")
        || bytes.contains(&0)
    {
        return Err(IndexError::Malformed);
    }
    Ok(())
}

fn exact_object(value: &Value, fields: usize) -> Result<&Map<String, Value>, IndexError> {
    value
        .as_object()
        .filter(|object| object.len() == fields)
        .ok_or(IndexError::Malformed)
}
fn required<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value, IndexError> {
    object.get(name).ok_or(IndexError::Malformed)
}
fn string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, IndexError> {
    required(object, name)?
        .as_str()
        .ok_or(IndexError::Malformed)
}
fn number(object: &Map<String, Value>, name: &str) -> Result<usize, IndexError> {
    required(object, name)?
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(IndexError::Malformed)
}
fn bounded(value: &str, maximum: usize) -> Result<String, IndexError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        Err(IndexError::Malformed)
    } else {
        Ok(value.to_owned())
    }
}
fn digest(value: &str) -> Result<&str, IndexError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value)
    } else {
        Err(IndexError::Malformed)
    }
}

fn index_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(INDEX_DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn render(
    package: &PackageIdentity,
    target: &str,
    feature_digest: &str,
    extractor: &ExtractorIdentity,
    items: &[ApiItem],
) -> String {
    let renamed = package
        .renamed_from
        .as_deref()
        .map(json)
        .unwrap_or_else(|| "null".to_owned());
    let mut output = format!("{{\"schema\":\"{RUST_API_INDEX_SCHEMA}\",\"package\":{{\"name\":{},\"version\":{},\"source_sha256\":{},\"renamed_from\":{renamed}}},\"target\":{},\"feature_digest\":{},\"extractor\":{{\"mode\":{},\"executable_sha256\":{},\"rustc_version\":{},\"rustdoc_format\":{}}},\"items\":[", json(&package.name), json(&package.version), json(&package.source_sha256), json(target), json(feature_digest), json(mode_name(extractor.mode)), json(&extractor.executable_sha256), json(&extractor.rustc_version), json(&extractor.rustdoc_format));
    for (index, item) in items.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let (support, reason) = match item.support {
            Support::Supported => ("supported", "null".to_owned()),
            Support::Rejected { reason } => ("rejected", json(reason_name(reason))),
        };
        write!(output, "{{\"path\":{},\"kind\":{},\"receiver\":{},\"signature\":{},\"type_depth\":{},\"support\":{},\"reason\":{reason}}}", json(&item.path), json(kind_name(item.kind)), json(receiver_name(item.receiver)), json(&item.signature), item.type_depth, json(support)).unwrap();
    }
    output.push_str(&format!("],\"limits\":{{\"max_items\":{MAX_ITEMS},\"max_type_depth\":{MAX_TYPE_DEPTH},\"max_index_bytes\":{MAX_INDEX_BYTES}}}}}\n"));
    output
}

fn json(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization")
}
fn mode_name(value: ExtractorMode) -> &'static str {
    match value {
        ExtractorMode::Prepared => "prepared",
        ExtractorMode::NightlyRustdocJson => "nightly-rustdoc-json",
    }
}
fn kind_name(value: ItemKind) -> &'static str {
    match value {
        ItemKind::Function => "function",
        ItemKind::InherentMethod => "inherent_method",
        ItemKind::TraitMethod => "trait_method",
        ItemKind::AssociatedType => "associated_type",
    }
}
fn receiver_name(value: Receiver) -> &'static str {
    match value {
        Receiver::None => "none",
        Receiver::Shared => "shared",
        Receiver::Mutable => "mutable",
        Receiver::Owned => "owned",
    }
}
fn reason_name(value: RejectionReason) -> &'static str {
    match value {
        RejectionReason::Private => "private",
        RejectionReason::SealedTrait => "sealed_trait",
        RejectionReason::OpaqueReturn => "opaque_return",
        RejectionReason::UnsupportedGeneric => "unsupported_generic",
        RejectionReason::UnsupportedSignature => "unsupported_signature",
        RejectionReason::ExpansionLimit => "expansion_limit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &[u8] = include_bytes!("../fixtures/protocol-envelope-example.json");

    #[test]
    fn prepared_protocol_example_replays_deterministically_and_binds_identity() {
        let first = RustApiIndex::replay(EXAMPLE).unwrap();
        let second = RustApiIndex::replay(EXAMPLE).unwrap();
        assert_eq!(first.canonical_json().as_bytes(), EXAMPLE);
        assert_eq!(first.digest(), second.digest());
        assert!(first
            .items()
            .iter()
            .any(|item| item.path == "fixture_api::Example::simple"
                && item.support == Support::Supported));
        assert!(first.items().iter().any(|item| matches!(
            item.support,
            Support::Rejected {
                reason: RejectionReason::UnsupportedGeneric
            }
        )));
        first
            .require_identity(
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "x86_64-unknown-linux-gnu",
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
            .unwrap();
        first
            .require_package_identity(
                "fixture_api",
                "0.0.0",
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "x86_64-unknown-linux-gnu",
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
            .unwrap();
        assert_eq!(
            first
                .select_supported(&["fixture_api::Example::simple"])
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            first.select_supported(&["fixture_api::Example::generic"]),
            Err(IndexError::ItemUnavailable)
        );
        assert_eq!(
            first.select_supported(&[
                "fixture_api::Example::simple",
                "fixture_api::Example::opaque"
            ]),
            Err(IndexError::ItemUnavailable)
        );
    }

    #[test]
    fn malformed_truncated_and_drifted_indexes_refuse_before_use() {
        assert_eq!(
            RustApiIndex::replay(&EXAMPLE[..EXAMPLE.len() - 1]),
            Err(IndexError::Malformed)
        );
        let index = RustApiIndex::replay(EXAMPLE).unwrap();
        assert_eq!(
            index.require_identity(
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "x86_64-unknown-linux-gnu",
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            ),
            Err(IndexError::IdentityMismatch)
        );
        assert_eq!(
            RustApiIndex::admit_extractor_output(b"{\"schema\":\"missing\",\"index\":{}}\n"),
            Err(IndexError::SetupRequired)
        );
        assert!(!EXTRACTOR_SETUP_INSTRUCTION.is_empty());
    }

    #[test]
    fn extractor_output_requires_explicit_nightly_mode() {
        let index = std::str::from_utf8(EXAMPLE).unwrap().trim_end();
        let wrapped = format!("{{\"schema\":\"{RUSTDOC_EXTRACTOR_SCHEMA}\",\"index\":{index}}}\n");
        assert_eq!(
            RustApiIndex::admit_extractor_output(wrapped.as_bytes()),
            Err(IndexError::Malformed)
        );
    }

    #[test]
    fn item_and_depth_limits_fail_closed() {
        let mut bytes = EXAMPLE.to_vec();
        let needle = b"\"type_depth\":1";
        let offset = bytes
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap();
        bytes.splice(
            offset..offset + needle.len(),
            b"\"type_depth\":33".iter().copied(),
        );
        assert_eq!(RustApiIndex::replay(&bytes), Err(IndexError::Malformed));
        let uppercase_digest = std::str::from_utf8(EXAMPLE)
            .unwrap()
            .replace("aaaaaaaa", "AAAAAAAA");
        assert_eq!(
            RustApiIndex::replay(uppercase_digest.as_bytes()),
            Err(IndexError::Malformed)
        );
    }

    #[test]
    fn item_count_and_document_byte_limits_fail_closed() {
        let mut value: Value = serde_json::from_slice(EXAMPLE).unwrap();
        let items = value.get_mut("items").unwrap().as_array_mut().unwrap();
        let template = items[0].clone();
        items.resize(MAX_ITEMS + 1, template);
        let mut too_many = serde_json::to_vec(&value).unwrap();
        too_many.push(b'\n');
        assert_eq!(RustApiIndex::replay(&too_many), Err(IndexError::Malformed));

        let oversized = vec![b' '; MAX_INDEX_BYTES + 1];
        assert_eq!(RustApiIndex::replay(&oversized), Err(IndexError::Malformed));
    }
}
