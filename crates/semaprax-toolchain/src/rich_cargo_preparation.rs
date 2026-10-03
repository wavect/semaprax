//! Pure Cargo closure recording for the rich Native Rust interop lane.
//!
//! Cargo itself resolves metadata. This module only authenticates the resulting
//! selected package graph and binds its exact build inputs into deterministic
//! preparation bytes. It intentionally contains no subprocess API: acquisition
//! and authorized `--locked --offline` execution are separate toolchain steps.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write;

pub const RICH_CARGO_PREPARATION_SCHEMA: &str = "semaprax.native-rust-rich-cargo-preparation.v1";
pub const RICH_CARGO_PREPARATION_DOMAIN: &str =
    "semaprax.native-rust-rich-cargo-preparation.digest.v1\0";
pub const MAX_PREPARATION_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CARGO_PACKAGES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CargoPreparationError {
    /// SPX-B121: malformed or non-canonical recorded preparation input.
    Malformed,
    /// SPX-B122: an unavailable source mode or custom target was requested.
    Unsupported,
    /// SPX-B123: Cargo's selected graph disagrees with the supplied source facts.
    Disagreement,
    /// SPX-B124: a bounded preparation input exceeds its admitted capacity.
    Capacity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LockedCargoSource {
    Registry {
        package_id: String,
        checksum: String,
    },
    Local {
        package_id: String,
        tree_digest: String,
    },
}

impl LockedCargoSource {
    pub fn package_id(&self) -> &str {
        match self {
            Self::Registry { package_id, .. } | Self::Local { package_id, .. } => package_id,
        }
    }

    fn render(&self, out: &mut String) {
        match self {
            Self::Registry {
                package_id,
                checksum,
            } => {
                write!(
                    out,
                    "{{\"id\":\"{}\",\"kind\":\"registry\",\"checksum\":\"{}\"}}",
                    escape(package_id),
                    escape(checksum)
                )
                .expect("writing a String cannot fail");
            }
            Self::Local {
                package_id,
                tree_digest,
            } => {
                write!(
                    out,
                    "{{\"id\":\"{}\",\"kind\":\"local\",\"tree_digest\":\"{}\"}}",
                    escape(package_id),
                    escape(tree_digest)
                )
                .expect("writing a String cannot fail");
            }
        }
    }
}

/// The caller must collect these bytes under its own held-file authority.
/// `prepare` neither opens paths nor consults ambient Cargo configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoPreparationInput {
    pub binding_plan: Vec<u8>,
    pub descriptor: Vec<u8>,
    pub cargo_metadata: Vec<u8>,
    pub cargo_lock: Vec<u8>,
    pub cargo_config: Vec<u8>,
    pub toolchain_identity: Vec<u8>,
    pub target_spec_identity: Vec<u8>,
    /// Exact host/target distinction used by Cargo's build graph.
    pub host_target_identity: Vec<u8>,
    /// Declared build-script inputs, after the execution authority rejects
    /// undeclared host reads.
    pub build_script_inputs: Vec<u8>,
    /// Declared proc-macro inputs, after the execution authority rejects
    /// undeclared host reads.
    pub proc_macro_inputs: Vec<u8>,
    /// Native compiler, linker, and archive tool identities and inputs.
    pub native_toolchain_inputs: Vec<u8>,
    pub generator_revision: String,
    pub target: String,
    pub panic_strategy: String,
    pub profile: String,
    pub selected_features: Vec<String>,
    pub sources: Vec<LockedCargoSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCargoClosure {
    bytes: Vec<u8>,
    digest: String,
}

/// Caller-owned cache of canonical prepared closures. It holds no paths,
/// process authority, acquired package data, or generated artifacts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PreparedCargoClosureCache {
    entries: BTreeMap<String, PreparedCargoClosure>,
}

/// Caller-owned generated-artifact admission cache. Entries are bound to the
/// complete canonical closure, never merely a package or target name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PreparedCargoArtifactCache {
    entries: BTreeMap<String, PreparedCargoArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCargoArtifact {
    closure_digest: String,
    bytes: Vec<u8>,
    digest: String,
}

impl PreparedCargoClosure {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Recompute the domain-separated digest and require canonical preparation
    /// bytes. This is deliberately independent from `prepare`'s input checks.
    pub fn replay(bytes: &[u8], expected_digest: &str) -> Result<Self, CargoPreparationError> {
        if bytes.len() > MAX_PREPARATION_INPUT_BYTES
            || !bytes.ends_with(b"\n")
            || bytes.iter().any(|byte| *byte == b'\r')
            || std::str::from_utf8(bytes).is_err()
        {
            return Err(CargoPreparationError::Malformed);
        }
        let digest = domain_digest(bytes);
        if !valid_digest(expected_digest) || digest != expected_digest {
            return Err(CargoPreparationError::Disagreement);
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            digest,
        })
    }
}

impl PreparedCargoClosureCache {
    pub fn get(&self, digest: &str) -> Option<&PreparedCargoClosure> {
        self.entries.get(digest)
    }

    /// Insert a newly prepared closure, or return an existing byte-identical
    /// closure under its digest. A digest collision fails closed.
    pub fn reuse_or_insert(
        &mut self,
        closure: PreparedCargoClosure,
    ) -> Result<(PreparedCargoClosure, bool), CargoPreparationError> {
        if let Some(existing) = self.entries.get(closure.digest()) {
            if existing.bytes() != closure.bytes() {
                return Err(CargoPreparationError::Disagreement);
            }
            return Ok((existing.clone(), true));
        }
        self.entries
            .insert(closure.digest().to_owned(), closure.clone());
        Ok((closure, false))
    }
}

impl PreparedCargoArtifact {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn closure_digest(&self) -> &str {
        &self.closure_digest
    }
}

impl PreparedCargoArtifactCache {
    pub fn get(&self, closure: &PreparedCargoClosure) -> Option<&PreparedCargoArtifact> {
        self.entries.get(closure.digest())
    }

    /// Admit bytes once for this exact closure. An attempt to replace bytes for
    /// the same closure fails closed and leaves the admitted entry unchanged.
    pub fn reuse_or_publish(
        &mut self,
        closure: &PreparedCargoClosure,
        bytes: Vec<u8>,
    ) -> Result<(PreparedCargoArtifact, bool), CargoPreparationError> {
        if let Some(existing) = self.entries.get(closure.digest()) {
            if existing.bytes != bytes || existing.closure_digest != closure.digest() {
                return Err(CargoPreparationError::Disagreement);
            }
            return Ok((existing.clone(), true));
        }
        let artifact = PreparedCargoArtifact {
            closure_digest: closure.digest().to_owned(),
            digest: sha256(&bytes),
            bytes,
        };
        self.entries
            .insert(closure.digest().to_owned(), artifact.clone());
        Ok((artifact, false))
    }
}

pub fn prepare_cargo_closure(
    input: CargoPreparationInput,
) -> Result<PreparedCargoClosure, CargoPreparationError> {
    validate_input(&input)?;
    let packages = selected_packages(&input.cargo_metadata)?;
    validate_sources(&packages, &input.sources)?;

    let mut bytes = String::with_capacity(4096);
    write!(
        bytes,
        "{{\"schema\":\"{RICH_CARGO_PREPARATION_SCHEMA}\",\"binding_plan_digest\":\"{}\",\"descriptor_digest\":\"{}\",\"cargo_metadata_digest\":\"{}\",\"cargo_lock_digest\":\"{}\",\"cargo_config_digest\":\"{}\",\"toolchain_digest\":\"{}\",\"target_spec_digest\":\"{}\",\"host_target_digest\":\"{}\",\"build_script_inputs_digest\":\"{}\",\"proc_macro_inputs_digest\":\"{}\",\"native_toolchain_inputs_digest\":\"{}\",\"generator_revision\":\"{}\",\"target\":\"{}\",\"panic_strategy\":\"{}\",\"profile\":\"{}\",\"features\":[",
        sha256(&input.binding_plan),
        sha256(&input.descriptor),
        sha256(&input.cargo_metadata),
        sha256(&input.cargo_lock),
        sha256(&input.cargo_config),
        sha256(&input.toolchain_identity),
        sha256(&input.target_spec_identity),
        sha256(&input.host_target_identity),
        sha256(&input.build_script_inputs),
        sha256(&input.proc_macro_inputs),
        sha256(&input.native_toolchain_inputs),
        escape(&input.generator_revision),
        escape(&input.target),
        escape(&input.panic_strategy),
        escape(&input.profile),
    )
    .expect("writing a String cannot fail");
    render_strings(&mut bytes, &input.selected_features);
    bytes.push_str("],\"packages\":[");
    for (index, (package, _)) in packages.iter().enumerate() {
        if index > 0 {
            bytes.push(',');
        }
        write!(bytes, "\"{}\"", escape(package)).expect("writing a String cannot fail");
    }
    bytes.push_str("],\"sources\":[");
    for (index, source) in input.sources.iter().enumerate() {
        if index > 0 {
            bytes.push(',');
        }
        source.render(&mut bytes);
    }
    bytes.push_str("]}\n");
    let digest = domain_digest(bytes.as_bytes());
    Ok(PreparedCargoClosure {
        bytes: bytes.into_bytes(),
        digest,
    })
}

/// Prepare the pure canonical record and reuse a caller-owned held closure
/// when every bound input produces the same digest and bytes.
pub fn prepare_cargo_closure_cached(
    input: CargoPreparationInput,
    cache: &mut PreparedCargoClosureCache,
) -> Result<(PreparedCargoClosure, bool), CargoPreparationError> {
    cache.reuse_or_insert(prepare_cargo_closure(input)?)
}

fn validate_input(input: &CargoPreparationInput) -> Result<(), CargoPreparationError> {
    let byte_inputs = [
        &input.binding_plan,
        &input.descriptor,
        &input.cargo_metadata,
        &input.cargo_lock,
        &input.cargo_config,
        &input.toolchain_identity,
        &input.target_spec_identity,
        &input.host_target_identity,
        &input.build_script_inputs,
        &input.proc_macro_inputs,
        &input.native_toolchain_inputs,
    ];
    if byte_inputs.iter().any(|value| value.is_empty())
        || byte_inputs
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()))
            .ok_or(CargoPreparationError::Capacity)?
            > MAX_PREPARATION_INPUT_BYTES
    {
        return Err(CargoPreparationError::Capacity);
    }
    if !valid_atom(&input.generator_revision)
        || !valid_target(&input.target)
        || input.target.ends_with(".json")
        || !matches!(input.panic_strategy.as_str(), "unwind" | "abort")
        || !valid_atom(&input.profile)
        || !strictly_sorted(&input.selected_features)
        || input
            .selected_features
            .iter()
            .any(|feature| !valid_atom(feature))
        || input.sources.len() > MAX_CARGO_PACKAGES
        || !strictly_sorted_by(&input.sources, LockedCargoSource::package_id)
        || input.sources.iter().any(|source| match source {
            LockedCargoSource::Registry {
                package_id,
                checksum,
            } => !valid_atom(package_id) || !valid_digest(checksum),
            LockedCargoSource::Local {
                package_id,
                tree_digest,
            } => !valid_atom(package_id) || !valid_digest(tree_digest),
        })
    {
        return Err(CargoPreparationError::Malformed);
    }
    Ok(())
}

fn selected_packages(metadata: &[u8]) -> Result<Vec<(String, bool)>, CargoPreparationError> {
    let value: serde_json::Value =
        serde_json::from_slice(metadata).map_err(|_| CargoPreparationError::Malformed)?;
    let root = value.as_object().ok_or(CargoPreparationError::Malformed)?;
    let packages = root
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or(CargoPreparationError::Malformed)?;
    let resolve = root
        .get("resolve")
        .and_then(serde_json::Value::as_object)
        .ok_or(CargoPreparationError::Malformed)?;
    let nodes = resolve
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or(CargoPreparationError::Malformed)?;
    if packages.is_empty() || packages.len() > MAX_CARGO_PACKAGES || nodes.is_empty() {
        return Err(CargoPreparationError::Capacity);
    }
    let mut source_by_id = std::collections::BTreeMap::new();
    for package in packages {
        let package = package
            .as_object()
            .ok_or(CargoPreparationError::Malformed)?;
        let id = package
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| valid_atom(id))
            .ok_or(CargoPreparationError::Malformed)?;
        let source = package.get("source");
        let registry = match source.and_then(serde_json::Value::as_str) {
            Some("registry+https://github.com/rust-lang/crates.io-index") => true,
            None | Some("") => false,
            Some(_) => return Err(CargoPreparationError::Unsupported),
        };
        if source_by_id.insert(id.to_owned(), registry).is_some() {
            return Err(CargoPreparationError::Malformed);
        }
    }
    let mut selected = Vec::with_capacity(nodes.len());
    for node in nodes {
        let node = node.as_object().ok_or(CargoPreparationError::Malformed)?;
        let id = node
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| source_by_id.contains_key(*id))
            .ok_or(CargoPreparationError::Disagreement)?;
        if node
            .get("features")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|features| {
                features
                    .iter()
                    .any(|feature| feature.as_str().is_none_or(|value| !valid_atom(value)))
            })
        {
            return Err(CargoPreparationError::Malformed);
        }
        selected.push((
            id.to_owned(),
            *source_by_id.get(id).expect("selected id was checked"),
        ));
    }
    selected.sort_by(|left, right| left.0.cmp(&right.0));
    if selected.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(CargoPreparationError::Malformed);
    }
    Ok(selected)
}

fn validate_sources(
    packages: &[(String, bool)],
    sources: &[LockedCargoSource],
) -> Result<(), CargoPreparationError> {
    if packages.len() != sources.len()
        || packages
            .iter()
            .zip(sources)
            .any(|((package, registry), source)| {
                package != source.package_id()
                    || *registry != matches!(source, LockedCargoSource::Registry { .. })
            })
    {
        return Err(CargoPreparationError::Disagreement);
    }
    Ok(())
}

fn render_strings(out: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write!(out, "\"{}\"", escape(value)).expect("writing a String cannot fail");
    }
}

fn sha256(bytes: &[u8]) -> String {
    digest_string(Sha256::digest(bytes).as_ref())
}

fn domain_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(RICH_CARGO_PREPARATION_DOMAIN.as_bytes());
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest_string(digest.as_ref())
}

fn digest_string(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(7 + bytes.len() * 2);
    value.push_str("sha256:");
    for byte in bytes {
        write!(value, "{byte:02x}").expect("writing a String cannot fail");
    }
    value
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_atom(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'_' | b'-' | b'.' | b'+' | b'/' | b'@' | b':' | b'#' | b'='
                )
        })
}

fn valid_target(value: &str) -> bool {
    value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn strictly_sorted(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn strictly_sorted_by<T>(values: &[T], value: impl Fn(&T) -> &str) -> bool {
    values
        .windows(2)
        .all(|pair| value(&pair[0]) < value(&pair[1]))
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(value: &str) -> String {
        sha256(value.as_bytes())
    }

    fn input() -> CargoPreparationInput {
        CargoPreparationInput {
            binding_plan: b"binding-plan".to_vec(),
            descriptor: b"descriptor".to_vec(),
            cargo_metadata: br#"{"packages":[{"id":"path+file:///workspace/app#app@0.1.0","source":null},{"id":"registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0","source":"registry+https://github.com/rust-lang/crates.io-index"}],"resolve":{"nodes":[{"id":"path+file:///workspace/app#app@0.1.0","features":["serde"]},{"id":"registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0","features":["derive"]}]}}"#.to_vec(),
            cargo_lock: b"lock".to_vec(),
            cargo_config: b"source".to_vec(),
            toolchain_identity: b"cargo rustc".to_vec(),
            target_spec_identity: b"aarch64-apple-darwin".to_vec(),
            host_target_identity: b"host=aarch64-apple-darwin;target=aarch64-apple-darwin"
                .to_vec(),
            build_script_inputs: b"build-script-inputs".to_vec(),
            proc_macro_inputs: b"proc-macro-inputs".to_vec(),
            native_toolchain_inputs: b"rustc+cc+ar".to_vec(),
            generator_revision: "sha256:generator".into(),
            target: "aarch64-apple-darwin".into(),
            panic_strategy: "unwind".into(),
            profile: "release".into(),
            selected_features: vec!["derive".into(), "std".into()],
            sources: vec![
                LockedCargoSource::Local {
                    package_id: "path+file:///workspace/app#app@0.1.0".into(),
                    tree_digest: digest("app"),
                },
                LockedCargoSource::Registry {
                    package_id: "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0".into(),
                    checksum: digest("serde"),
                },
            ],
        }
    }

    #[test]
    fn records_a_deterministic_locked_closure_without_process_authority() {
        let first = prepare_cargo_closure(input()).unwrap();
        let second = prepare_cargo_closure(input()).unwrap();
        assert_eq!(first, second);
        let record: serde_json::Value = serde_json::from_slice(first.bytes()).unwrap();
        assert_eq!(
            record.get("schema").and_then(serde_json::Value::as_str),
            Some(RICH_CARGO_PREPARATION_SCHEMA)
        );
        assert_eq!(
            PreparedCargoClosure::replay(first.bytes(), first.digest()).unwrap(),
            first
        );
    }

    #[test]
    fn caller_owned_cache_reuses_only_an_identical_prepared_closure() {
        let mut cache = PreparedCargoClosureCache::default();
        let (first, reused) = prepare_cargo_closure_cached(input(), &mut cache).unwrap();
        assert!(!reused);
        let (second, reused) = prepare_cargo_closure_cached(input(), &mut cache).unwrap();
        assert!(reused);
        assert_eq!(first, second);
        assert_eq!(cache.get(first.digest()), Some(&first));

        let mut changed = input();
        changed.cargo_lock = b"changed-lock".to_vec();
        let (changed, reused) = prepare_cargo_closure_cached(changed, &mut cache).unwrap();
        assert!(!reused);
        assert_ne!(changed, first);
    }

    #[test]
    fn rejects_custom_sources_and_source_fact_disagreement() {
        let mut custom = input();
        custom.cargo_metadata = String::from_utf8(custom.cargo_metadata)
            .unwrap()
            .replace(
                "registry+https://github.com/rust-lang/crates.io-index",
                "git+https://example.invalid/repo",
            )
            .into_bytes();
        assert_eq!(
            prepare_cargo_closure(custom),
            Err(CargoPreparationError::Unsupported)
        );

        let mut missing = input();
        missing.sources.pop();
        assert_eq!(
            prepare_cargo_closure(missing),
            Err(CargoPreparationError::Disagreement)
        );
    }

    #[test]
    fn changes_to_recorded_build_inputs_invalidate_the_closure() {
        let baseline = prepare_cargo_closure(input()).unwrap();
        let mutations: [fn(&mut CargoPreparationInput); 8] = [
            |input: &mut CargoPreparationInput| input.cargo_lock = b"changed-lock".to_vec(),
            |input: &mut CargoPreparationInput| input.cargo_config = b"changed-config".to_vec(),
            |input: &mut CargoPreparationInput| {
                input.toolchain_identity = b"changed-rustc".to_vec()
            },
            |input: &mut CargoPreparationInput| {
                input.target_spec_identity = b"changed-target".to_vec()
            },
            |input: &mut CargoPreparationInput| {
                input.host_target_identity = b"changed-host-target".to_vec()
            },
            |input: &mut CargoPreparationInput| {
                input.build_script_inputs = b"changed-build-script".to_vec()
            },
            |input: &mut CargoPreparationInput| {
                input.proc_macro_inputs = b"changed-proc-macro".to_vec()
            },
            |input: &mut CargoPreparationInput| {
                input.native_toolchain_inputs = b"changed-native-tools".to_vec()
            },
        ];
        for mutate in mutations {
            let mut changed = input();
            mutate(&mut changed);
            assert_ne!(prepare_cargo_closure(changed).unwrap(), baseline);
        }

        let mut local_edit = input();
        let LockedCargoSource::Local { tree_digest, .. } = &mut local_edit.sources[0] else {
            unreachable!("fixture's first source is local");
        };
        *tree_digest = digest("edited-local-source");
        assert_ne!(prepare_cargo_closure(local_edit).unwrap(), baseline);
    }
}
