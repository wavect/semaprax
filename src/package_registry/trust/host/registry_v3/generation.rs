use super::super::{fields, number, parse, valid_name, wire, MAX_GENERATION};
use super::*;
use crate::package_registry::trust::{verify_root_rotation, Checkpoint, InstalledRoot};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(super) const SCHEMA_V2: &str = "semaprax.registry-trust-generation.v2";
pub(super) const SCHEMA: &str = "semaprax.registry-trust-generation.v3";
#[derive(Clone)]
pub(super) struct MirrorAnchor {
    pub version: u64,
    pub digest: String,
    pub observed_time: u64,
}
pub(super) struct Generation {
    pub bytes: String,
    pub value: Value,
    pub root: InstalledRoot,
    pub checkpoint: proof::RegistryCheckpoint,
    pub base: Checkpoint,
    pub mirror: Option<MirrorAnchor>,
}
fn checkpoint_base(checkpoint: &proof::RegistryCheckpoint) -> Result<Checkpoint> {
    let outer = parse(&checkpoint.canonical_bytes())?;
    Checkpoint::from_trusted_store_bytes(string(&outer["checkpoint"])?)
}
pub(super) fn decode(bytes: String) -> Result<Generation> {
    crate::package_build::wire::validate_compact_json_keys(
        &bytes,
        MAX_GENERATION,
        "trust generation v2",
    )
    .map_err(|_| refused("invalid generation-v2 encoding"))?;
    let value: Value =
        serde_json::from_str(&bytes).map_err(|_| refused("invalid generation-v2 JSON"))?;
    let schema = string(&value["schema"])?;
    let mirror = match schema {
        SCHEMA_V2 => {
            fields(
                &value,
                &[
                    "schema",
                    "transition",
                    "previous",
                    "root",
                    "checkpoint",
                    "rotation",
                    "time",
                    "cache",
                    "metadata",
                ],
            )?;
            None
        }
        SCHEMA => {
            fields(
                &value,
                &[
                    "schema",
                    "transition",
                    "previous",
                    "root",
                    "checkpoint",
                    "rotation",
                    "time",
                    "cache",
                    "metadata",
                    "mirror",
                ],
            )?;
            if value["mirror"].is_null() {
                None
            } else {
                let anchor = &value["mirror"];
                fields(anchor, &["version", "digest", "observed_time"])?;
                let digest = string(&anchor["digest"])?;
                if !digest.starts_with("sha256:")
                    || digest.len() != 71
                    || !digest[7..]
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(refused("invalid mirror timestamp digest"));
                }
                Some(MirrorAnchor {
                    version: number(&anchor["version"])?,
                    digest: digest.to_owned(),
                    observed_time: number(&anchor["observed_time"])?,
                })
            }
        }
        _ => return Err(refused("unknown generation schema")),
    };
    if wire(&value) != bytes {
        return Err(refused("noncanonical generation encoding"));
    }
    let previous = string(&value["previous"])?;
    if !previous.is_empty() && !valid_name(previous) {
        return Err(refused("invalid predecessor"));
    }
    if !matches!(
        string(&value["transition"])?,
        "bootstrap" | "update" | "migrate-v1"
    ) {
        return Err(refused("unknown transition"));
    }
    let root = InstalledRoot::from_independently_installed_bytes(string(&value["root"])?)?;
    let checkpoint =
        proof::RegistryCheckpoint::from_trusted_store_bytes(string(&value["checkpoint"])?)?;
    let base = checkpoint_base(&checkpoint)?;
    if base.root_digest != root.digest
        || base.root_version != root.version
        || base.registry != root.registry
        || base.observed_time != number(&value["time"])?
    {
        return Err(refused("root/checkpoint/time disagree"));
    }
    // Exact installed-root binding also checks the sealed namespace floor.
    if proof::RegistryCheckpoint::migrate_from_v1(&base, &root)?.canonical_bytes()
        != checkpoint.canonical_bytes()
    {
        return Err(refused("checkpoint publisher bindings disagree"));
    }
    if let Some(anchor) = mirror.as_ref() {
        let timestamp = base
            .roles
            .get("timestamp")
            .ok_or_else(super::super::stale)?;
        if timestamp.version != anchor.version
            || timestamp.digest != anchor.digest
            || anchor.observed_time > base.observed_time
        {
            return Err(refused("mirror timestamp provenance disagrees"));
        }
    }
    Ok(Generation {
        bytes,
        value,
        root,
        checkpoint,
        base,
        mirror,
    })
}
pub(super) fn bootstrap(root: &str, pin: &str, now: u64) -> Result<Generation> {
    let initial = super::super::bootstrap(root, pin, now)?;
    let checkpoint =
        proof::RegistryCheckpoint::migrate_from_v1(&initial.checkpoint, &initial.root)?;
    let mut value = initial.value;
    value["schema"] = json!(SCHEMA_V2);
    value["transition"] = json!("bootstrap");
    value["checkpoint"] = json!(checkpoint.canonical_bytes());
    decode(wire(&value))
}
pub(super) fn from_v1(old: super::super::Generation) -> Result<Generation> {
    let checkpoint = proof::RegistryCheckpoint::migrate_from_v1(&old.checkpoint, &old.root)?;
    Ok(Generation {
        bytes: old.bytes,
        value: old.value,
        root: old.root,
        checkpoint,
        base: old.checkpoint,
        mirror: None,
    })
}
pub(super) fn prepare(
    old: &Generation,
    update: &Update<'_, '_>,
    migration: bool,
) -> Result<Generation> {
    let rotated;
    let root_bytes;
    let root = if let Some(envelope) = update.rotation {
        rotated = verify_root_rotation(&old.root, envelope, update.trusted_time)?;
        root_bytes = wire(&parse(envelope)?["signed"]);
        rotated.root()
    } else {
        root_bytes = string(&old.value["root"])?.to_owned();
        &old.root
    };
    let candidate =
        proof::verify_update(root, &old.checkpoint, update.trusted_time, &update.metadata)?;
    if candidate.prior_checkpoint_digest() != hash(old.checkpoint.canonical_bytes().as_bytes()) {
        return Err(refused("checkpoint exact-byte CAS disagrees"));
    }
    if !migration && string(&old.value["checkpoint"])? != old.checkpoint.canonical_bytes() {
        return Err(refused("stored checkpoint exact-byte CAS disagrees"));
    }
    candidate.check_lock(update.lock, update.subjects)?;
    if update.subjects.is_empty() || update.artifacts.is_empty() {
        return Err(refused(
            "v3 managed selection requires complete lock and artifacts",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut artifacts = Vec::new();
    let mut total = 0usize;
    if update.artifacts.len() > crate::package_registry::MAX_ENTRIES * 3 {
        return Err(refused("too many selected artifacts"));
    }
    for artifact in update.artifacts {
        total = total
            .checked_add(artifact.bytes.len())
            .ok_or_else(|| refused("artifact size overflow"))?;
        if total > MAX_GENERATION / 4
            || !seen.insert((artifact.package, artifact.version, artifact.path))
        {
            return Err(refused("duplicate or oversized artifact selection"));
        }
        let entry = update
            .metadata
            .registry
            .entries()
            .iter()
            .find(|entry| {
                entry.publication().package == artifact.package
                    && entry.publication().version == artifact.version
            })
            .ok_or_else(|| refused("artifact coordinate missing"))?;
        if !update.subjects.contains(&entry.publication().subject_bytes) {
            return Err(refused("artifact outside locked subjects"));
        }
        candidate.check_artifact(
            artifact.package,
            artifact.version,
            artifact.path,
            artifact.bytes,
        )?;
        let encoded = artifact
            .bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        artifacts.push(json!({"package":artifact.package,"version":artifact.version,"path":artifact.path,"hex":encoded}));
    }
    // The managed scalar cache contains a usable core module for every locked
    // coordinate, not merely one arbitrarily chosen manifest row. Additional
    // rows remain optional but must pass their exact admitted manifest binding.
    for entry in update.metadata.registry.entries() {
        let publication = entry.publication();
        if update.subjects.contains(&publication.subject_bytes)
            && !seen.contains(&(
                publication.package.as_str(),
                publication.version.as_str(),
                "module.wasm",
            ))
        {
            return Err(refused(
                "locked coordinate is missing its core Wasm artifact",
            ));
        }
    }
    artifacts.sort_by(|a, b| {
        (
            a["package"].as_str(),
            a["version"].as_str(),
            a["path"].as_str(),
        )
            .cmp(&(
                b["package"].as_str(),
                b["version"].as_str(),
                b["path"].as_str(),
            ))
    });
    let mut publishers = update
        .metadata
        .publishers
        .iter()
        .map(|(role, bytes)| json!({"role":role,"bytes":bytes}))
        .collect::<Vec<_>>();
    publishers.sort_by(|a, b| a["role"].as_str().cmp(&b["role"].as_str()));
    let mut subjects = update.subjects.to_vec();
    subjects.sort();
    let schema = if old.value["schema"] == SCHEMA {
        SCHEMA
    } else {
        SCHEMA_V2
    };
    let mut value = json!({"schema":schema,"transition":if migration {"migrate-v1"}else{"update"},
        "previous":generation_name(old.bytes.as_bytes()),"root":root_bytes,"checkpoint":candidate.checkpoint().canonical_bytes(),
        "rotation":update.rotation,"time":update.trusted_time,
        "metadata":{"timestamp":update.metadata.timestamp,"snapshot":update.metadata.snapshot,"publishers":publishers,
            "registry":update.metadata.registry.envelope()},
        "cache":{"lock":update.lock,"subjects":subjects,"artifacts":artifacts}});
    if schema == SCHEMA {
        // A mirror update validates and attaches its next anchor below. An
        // ordinary update restores the prior anchor only after proving that
        // it did not move the timestamp role.
        value["mirror"] = Value::Null;
    }
    let bytes = wire(&value);
    if bytes.len() > MAX_GENERATION {
        return Err(refused("generation exceeds bound"));
    }
    decode(bytes)
}

/// The ordinary host route may retain a previously authenticated mirror anchor,
/// but it may not advance the timestamp role without the mirror proof route.
/// Otherwise an ambient non-mirror update could silently create a new
/// seven-day freshness window.
pub(super) fn prepare_ordinary(
    old: &Generation,
    update: &Update<'_, '_>,
    migration: bool,
) -> Result<Generation> {
    let generation = prepare(old, update, migration)?;
    if let Some(anchor) = old.mirror.as_ref() {
        let old_timestamp = old
            .base
            .roles
            .get("timestamp")
            .ok_or_else(super::super::stale)?;
        let next_timestamp = generation
            .base
            .roles
            .get("timestamp")
            .ok_or_else(super::super::stale)?;
        if old_timestamp.version != next_timestamp.version
            || old_timestamp.digest != next_timestamp.digest
        {
            return Err(refused(
                "ordinary update cannot advance a mirror-bound timestamp",
            ));
        }
        return attach_mirror(generation, anchor);
    }
    Ok(generation)
}

/// Converts a verified mirror candidate into a durable generation-v3 anchor.
/// The opaque bridge value is accepted only when its ordinary checkpoint and
/// timestamp role are exactly the candidate independently replayed below.
/// Only the held store commits a mirror update, so this shares its host gate.
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(super) fn prepare_mirror(
    old: &Generation,
    update: &Update<'_, '_>,
    checkpoint: &proof::MirrorCheckpoint,
) -> Result<Generation> {
    let generation = prepare(old, update, false)?;
    if checkpoint.registry_checkpoint().canonical_bytes() != generation.checkpoint.canonical_bytes()
    {
        return Err(refused("mirror checkpoint does not bind held update"));
    }
    let (version, digest, observed_time) = checkpoint
        .anchor()
        .ok_or_else(|| refused("mirror update lacks timestamp provenance"))?;
    let timestamp = generation
        .base
        .roles
        .get("timestamp")
        .ok_or_else(super::super::stale)?;
    if timestamp.version != version || timestamp.digest != digest {
        return Err(refused("mirror timestamp provenance disagrees"));
    }
    match old.mirror.as_ref() {
        Some(previous)
            if previous.version == version
                && previous.digest == digest
                && previous.observed_time == observed_time => {}
        Some(previous) if version > previous.version && observed_time == update.trusted_time => {}
        Some(_) => return Err(refused("mirror timestamp anchor regressed or was rebased")),
        None if old.value["transition"] == "bootstrap" && observed_time == update.trusted_time => {}
        None => {
            return Err(refused(
                "mirror anchor requires an original held bootstrap generation",
            ));
        }
    }
    attach_mirror(
        generation,
        &MirrorAnchor {
            version,
            digest: digest.to_owned(),
            observed_time,
        },
    )
}

fn attach_mirror(mut generation: Generation, anchor: &MirrorAnchor) -> Result<Generation> {
    generation.value["schema"] = json!(SCHEMA);
    generation.value["mirror"] = json!({"version":anchor.version,"digest":anchor.digest,"observed_time":anchor.observed_time});
    decode(wire(&generation.value))
}

/// Shared low-level recovery only needs the exact predecessor link. Full chain
/// authentication happens in load_chain first, not in this callback.
pub(super) fn predecessor(bytes: String) -> Result<(String, String)> {
    let value: Value = serde_json::from_str(&bytes).map_err(|_| refused("invalid generation"))?;
    if value["schema"] == SCHEMA || value["schema"] == SCHEMA_V2 {
        let generation = decode(bytes)?;
        let previous = string(&generation.value["previous"])?.to_owned();
        Ok((generation.bytes, previous))
    } else {
        let generation = super::super::decode(bytes)?;
        let previous = string(&generation.value["previous"])?.to_owned();
        Ok((generation.bytes, previous))
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub(super) fn load_chain(
    held: &super::super::Held,
    active: &str,
    pin: &str,
    recovering: bool,
) -> Result<Generation> {
    let inventory = held.inventory()?;
    let mut names = BTreeSet::new();
    let mut nodes = Vec::new();
    let mut next = active.to_owned();
    let mut legacy = None;
    while !next.is_empty() {
        if !valid_name(&next)
            || !names.insert(next.clone())
            || names.len() > super::super::MAX_GENERATIONS
        {
            return Err(refused("invalid generation chain"));
        }
        let bytes = held.read(&next, MAX_GENERATION)?;
        if generation_name(bytes.as_bytes()) != next {
            return Err(refused("generation digest changed"));
        }
        let value: Value =
            serde_json::from_str(&bytes).map_err(|_| refused("invalid generation"))?;
        if value["schema"] != SCHEMA && value["schema"] != SCHEMA_V2 {
            legacy = Some(from_v1(super::super::load_chain(held, &next, pin, true)?)?);
            // Inventory includes the entire authenticated legacy prefix.
            let mut previous = string(&value["previous"])?.to_owned();
            while !previous.is_empty() {
                if !valid_name(&previous)
                    || !names.insert(previous.clone())
                    || names.len() > super::super::MAX_GENERATIONS
                {
                    return Err(refused("invalid legacy prefix"));
                }
                let old = super::super::decode(held.read(&previous, MAX_GENERATION)?)?;
                previous = string(&old.value["previous"])?.to_owned();
            }
            break;
        }
        let generation = decode(bytes)?;
        next = string(&generation.value["previous"])?.to_owned();
        nodes.push(generation);
    }
    if nodes.is_empty() {
        return Err(refused(
            "v3 open requires generation-v2 head; explicit migration required",
        ));
    }
    if let Some(old) = legacy {
        if nodes.last().expect("nonempty").value["transition"] != "migrate-v1" {
            return Err(refused("explicit migration boundary missing"));
        }
        nodes.push(old);
    } else {
        let first = nodes.last().expect("nonempty");
        if first.value["transition"] != "bootstrap"
            || bootstrap(
                string(&first.value["root"])?,
                pin,
                number(&first.value["time"])?,
            )?
            .bytes
                != first.bytes
        {
            return Err(refused("v3 bootstrap provenance disagrees"));
        }
    }
    for pair in nodes.windows(2) {
        let (new, old) = (&pair[0], &pair[1]);
        let expected = if old.value["schema"] == SCHEMA || old.value["schema"] == SCHEMA_V2 {
            "update"
        } else {
            "migrate-v1"
        };
        if new.value["transition"] != expected || new.base.observed_time < old.base.observed_time {
            return Err(refused("invalid protocol transition"));
        }
        if new.root.digest != old.root.digest {
            let rotation = verify_root_rotation(
                &old.root,
                string(&new.value["rotation"])?,
                number(&new.value["time"])?,
            )?;
            if rotation.root().digest != new.root.digest {
                return Err(refused("root rotation differs"));
            }
        } else if !new.value["rotation"].is_null() {
            return Err(refused("unexpected root rotation"));
        }
        // Preserve all high-water stamps, including prior-profile stamps.
        for (role, stamp) in &old.base.roles {
            let next = new.base.roles.get(role).ok_or_else(super::super::stale)?;
            if next.version < stamp.version
                || (next.version == stamp.version && next.digest != stamp.digest)
            {
                return Err(super::super::stale());
            }
        }
        let old_wire = parse(&old.checkpoint.canonical_bytes())?;
        let new_wire = parse(&new.checkpoint.canonical_bytes())?;
        if old_wire["publishers"] != new_wire["publishers"] {
            return Err(super::super::stale());
        }
    }
    for name in names.clone() {
        let complete = format!("c-{name}");
        if held.read(&complete, 66)? != name {
            return Err(uncertain());
        }
        names.insert(complete);
    }
    if !recovering {
        names.insert("ACTIVE".into());
        if names != inventory {
            return Err(uncertain());
        }
    }
    Ok(nodes.remove(0))
}
