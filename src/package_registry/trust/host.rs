//! Owner-private local trust state and managed cache. No fetch capability is
//! returned. The caller supplies the independent bootstrap pin and trusted time.
//! Same-principal hostile mutation, filesystem rollback and lying fsync/storage
//! are outside this cooperative local-host boundary.
#![cfg_attr(
    not(any(target_os = "linux", target_os = "android", target_vendor = "apple")),
    allow(
        dead_code,
        reason = "unsupported hosts expose only fail-closed trust-store APIs; held-generation helpers are not invoked"
    )
)]

use super::*;
use std::path::Path;

pub mod registry_v3;

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
mod unix;
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use unix::Held;

const SCHEMA: &str = "semaprax.registry-trust-generation.v1";
const MAX_GENERATION: usize = 64 * 1024 * 1024;
const MAX_GENERATIONS: usize = 64;

fn refused(message: &str) -> Diagnostic {
    error("SPX-PKR626", message)
}
fn uncertain() -> Diagnostic {
    error(
        "SPX-PKR627",
        "registry store effects may exist; explicit exact revalidation recovery is required",
    )
}

/// Exact caller-owned artifact bytes, never a filesystem path to dereference.
pub struct Artifact<'a> {
    pub package: &'a str,
    pub version: &'a str,
    pub path: &'a str,
    pub bytes: &'a [u8],
}

/// One fixed-time update and exact manifest-bound artifacts. An optional lock
/// additionally requires its complete independently admitted subject closure.
pub struct Update<'a> {
    pub metadata: UpdateInputs<'a>,
    pub rotation: Option<&'a str>,
    pub lock: Option<&'a str>,
    pub subjects: &'a [String],
    pub artifacts: &'a [Artifact<'a>],
    pub trusted_time: u64,
}

/// Evidence only. This receipt cannot authorize fetch, open a path or restore a
/// trusted checkpoint. The live store remains the authority for future updates.
#[derive(Debug)]
pub struct CommitReceipt {
    pub generation_digest: String,
    pub checkpoint_digest: String,
}

struct Generation {
    bytes: String,
    value: Value,
    root: InstalledRoot,
    checkpoint: Checkpoint,
}

fn string(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| refused("generation string required"))
}
fn generation_name(bytes: &[u8]) -> String {
    format!("g-{}", &hash(bytes)[7..])
}
fn valid_name(name: &str) -> bool {
    name.len() == 66 && name.starts_with("g-") && hex::<32>(&name[2..]).is_ok()
}
fn decode(bytes: String) -> Result<Generation> {
    crate::package_build::wire::validate_compact_json_keys(
        &bytes,
        MAX_GENERATION,
        "trust generation",
    )
    .map_err(|_| refused("invalid generation encoding"))?;
    let value: Value =
        serde_json::from_str(&bytes).map_err(|_| refused("invalid generation JSON"))?;
    fields(
        &value,
        &[
            "schema",
            "previous",
            "root",
            "checkpoint",
            "rotation",
            "time",
            "cache",
            "metadata",
        ],
    )?;
    if value["schema"] != SCHEMA || wire(&value) != bytes {
        return Err(refused("noncanonical generation"));
    }
    let previous = string(&value["previous"])?;
    if !previous.is_empty() && !valid_name(previous) {
        return Err(refused("invalid predecessor"));
    }
    number(&value["time"])?;
    let root = InstalledRoot::from_independently_installed_bytes(string(&value["root"])?)?;
    let checkpoint = Checkpoint::from_trusted_store_bytes(string(&value["checkpoint"])?)?;
    if checkpoint.root_digest != root.digest
        || checkpoint.root_version != root.version
        || checkpoint.registry != root.registry
    {
        return Err(refused("root and checkpoint disagree"));
    }
    Ok(Generation {
        bytes,
        value,
        root,
        checkpoint,
    })
}

fn bootstrap(root_bytes: &str, pin: &str, now: u64) -> Result<Generation> {
    // The pin's independent provenance is a caller obligation, never inferred.
    if hash(root_bytes.as_bytes()) != pin {
        return Err(refused("independent root pin disagrees"));
    }
    let root = InstalledRoot::from_independently_installed_bytes(root_bytes)?;
    if root.expires <= now {
        return Err(stale());
    }
    let mut checkpoint = Checkpoint::initial(&root);
    checkpoint.observed_time = now;
    decode(wire(
        &json!({"schema":SCHEMA,"previous":"","root":root_bytes,
        "checkpoint":checkpoint.canonical_bytes(),"rotation":null,"time":now,
        "cache":null,"metadata":null}),
    ))
}

fn prepare(previous: &Generation, update: &Update<'_>) -> Result<Generation> {
    let rotated;
    let root_bytes;
    let root = if let Some(envelope) = update.rotation {
        rotated = verify_root_rotation(&previous.root, envelope, update.trusted_time)?;
        root_bytes = wire(&parse(envelope)?["signed"]);
        rotated.root()
    } else {
        root_bytes = string(&previous.value["root"])?.to_owned();
        &previous.root
    };
    let candidate = verify_update(
        root,
        &previous.checkpoint,
        update.trusted_time,
        &update.metadata,
    )?;
    if candidate.prior_checkpoint_digest()
        != hash(string(&previous.value["checkpoint"])?.as_bytes())
    {
        return Err(refused("checkpoint exact-byte CAS disagrees"));
    }
    match update.lock {
        Some(lock) => candidate.check_lock(lock, update.subjects)?,
        None if update.subjects.is_empty() => (),
        None => {
            return Err(refused(
                "subject selection requires a complete semantic lock",
            ))
        }
    }
    let mut seen = BTreeSet::new();
    let mut artifacts = Vec::new();
    let mut total = 0usize;
    if update.artifacts.len() > super::super::MAX_ENTRIES * 3 {
        return Err(refused("too many selected artifacts"));
    }
    for artifact in update.artifacts {
        total = total.checked_add(artifact.bytes.len()).ok_or_else(shape)?;
        if total > MAX_GENERATION / 4
            || !seen.insert((artifact.package, artifact.version, artifact.path))
        {
            return Err(refused("duplicate or oversized artifact selection"));
        }
        let entry = candidate
            .registry
            .entries()
            .iter()
            .find(|entry| {
                entry.publication().package == artifact.package
                    && entry.publication().version == artifact.version
            })
            .ok_or_else(binding)?;
        if update.lock.is_some()
            && !update
                .subjects
                .iter()
                .any(|subject| subject == &entry.publication().subject_bytes)
        {
            return Err(refused("artifact is outside selected locked subjects"));
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
    let publishers = update
        .metadata
        .publishers
        .iter()
        .map(|(role, bytes)| json!({"role":role,"bytes":bytes}))
        .collect::<Vec<_>>();
    let bytes = wire(
        &json!({"schema":SCHEMA,"previous":generation_name(previous.bytes.as_bytes()),
        "root":root_bytes,"checkpoint":candidate.checkpoint().canonical_bytes(),
        "rotation":update.rotation,"time":update.trusted_time,
        "metadata":{"timestamp":update.metadata.timestamp,"snapshot":update.metadata.snapshot,
            "publishers":publishers,"registry":update.metadata.registry_snapshot},
        "cache":{"lock":update.lock,"subjects":update.subjects,"artifacts":artifacts}}),
    );
    if bytes.len() > MAX_GENERATION {
        return Err(refused("generation exceeds bound"));
    }
    decode(bytes)
}

/// Held directory and exclusive cooperative lock. No clone or serialization.
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub struct HeldTrustStore {
    held: Held,
    pin: String,
    active: String,
    generation: Generation,
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl HeldTrustStore {
    /// Bootstrap only in an existing, empty, owner-private directory. Never use
    /// as a recovery fallback. Pin provenance is the embedding host's obligation.
    pub fn install(
        path: &Path,
        root: &str,
        independent_pin: &str,
        trusted_time: u64,
    ) -> Result<Self> {
        let held = Held::open(path)?;
        if !held.inventory()?.is_empty() {
            return Err(refused("bootstrap requires an empty store"));
        }
        let generation = bootstrap(root, independent_pin, trusted_time)?;
        let active = held.publish(None, &generation.bytes)?;
        let store = Self {
            held,
            pin: independent_pin.to_owned(),
            active,
            generation,
        };
        store.recheck()?;
        Ok(store)
    }

    /// Ordinary opening refuses pending work, unreferenced generations and
    /// malformed state. It never invents an initial checkpoint.
    pub fn open(path: &Path, independent_pin: &str) -> Result<Self> {
        let held = Held::open(path)?;
        let active = held.read("ACTIVE", 66)?;
        let generation = load_chain(&held, &active, independent_pin, false)?;
        held.sync()?;
        Ok(Self {
            held,
            pin: independent_pin.to_owned(),
            active,
            generation,
        })
    }

    fn recheck(&self) -> Result<()> {
        if self.held.read("ACTIVE", 66)? != self.active
            || load_chain(&self.held, &self.active, &self.pin, false)?.bytes
                != self.generation.bytes
        {
            return Err(refused("held store changed before commit"));
        }
        Ok(())
    }

    /// Replays the update under the held lock; commits trust and managed cache
    /// together. Returns evidence only, not fetch permission or a reusable path.
    pub fn commit_update(&mut self, update: &Update<'_>) -> Result<CommitReceipt> {
        self.recheck()?;
        let generation = prepare(&self.generation, update)?;
        self.recheck()?;
        let active = self.held.publish(Some(&self.active), &generation.bytes)?;
        self.active = active;
        self.generation = generation;
        self.recheck().map_err(|_| uncertain())?;
        Ok(self.receipt())
    }

    fn receipt(&self) -> CommitReceipt {
        CommitReceipt {
            generation_digest: hash(self.generation.bytes.as_bytes()),
            checkpoint_digest: hash(
                string(&self.generation.value["checkpoint"])
                    .expect("decoded")
                    .as_bytes(),
            ),
        }
    }

    /// Exact retry after ambiguous effects. Supplies the independently retained
    /// predecessor digest and original immutable request, not a pending-file
    /// instruction. Revalidates freshness at recovery time as well as replaying
    /// the original fixed-time bytes. Partial stage writes are never repaired.
    pub fn recover_update(
        path: &Path,
        independent_pin: &str,
        predecessor_digest: &str,
        update: &Update<'_>,
        recovery_time: u64,
    ) -> Result<Self> {
        let held = Held::open(path)?;
        let predecessor = format!(
            "g-{}",
            predecessor_digest
                .strip_prefix("sha256:")
                .ok_or_else(shape)?
        );
        let previous = load_chain(&held, &predecessor, independent_pin, true)?;
        if recovery_time < update.trusted_time {
            return Err(stale());
        }
        let current_root;
        let root = if let Some(rotation) = update.rotation {
            current_root = verify_root_rotation(&previous.root, rotation, recovery_time)?;
            current_root.root()
        } else {
            &previous.root
        };
        verify_update(root, &previous.checkpoint, recovery_time, &update.metadata)?;
        let generation = prepare(&previous, update)?;
        let active = held.recover(Some(&predecessor), &generation.bytes)?;
        let store = Self {
            held,
            pin: independent_pin.to_owned(),
            active,
            generation,
        };
        store.recheck()?;
        Ok(store)
    }

    /// Explicit interrupted-bootstrap retry; never called by ordinary open.
    pub fn recover_install(
        path: &Path,
        root: &str,
        independent_pin: &str,
        original_time: u64,
        recovery_time: u64,
    ) -> Result<Self> {
        let held = Held::open(path)?;
        if recovery_time < original_time {
            return Err(stale());
        }
        bootstrap(root, independent_pin, recovery_time)?;
        let generation = bootstrap(root, independent_pin, original_time)?;
        let active = held.recover(None, &generation.bytes)?;
        let store = Self {
            held,
            pin: independent_pin.to_owned(),
            active,
            generation,
        };
        store.recheck()?;
        Ok(store)
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn load_chain(held: &Held, active: &str, pin: &str, recovering: bool) -> Result<Generation> {
    let inventory = held.inventory()?;
    let mut names = BTreeSet::new();
    let mut generations = Vec::new();
    let mut next = active.to_owned();
    while !next.is_empty() {
        if !valid_name(&next) || !names.insert(next.clone()) || names.len() > MAX_GENERATIONS {
            return Err(refused("invalid or over-capacity generation chain"));
        }
        let bytes = held.read(&next, MAX_GENERATION)?;
        if generation_name(bytes.as_bytes()) != next {
            return Err(refused("generation digest changed"));
        }
        let generation = decode(bytes)?;
        next = string(&generation.value["previous"])?.to_owned();
        generations.push(generation);
    }
    let first = generations
        .last()
        .ok_or_else(|| refused("missing initial generation"))?;
    if first.root.digest != pin
        || !first.value["rotation"].is_null()
        || !first.value["cache"].is_null()
        || !first.value["metadata"].is_null()
        || bootstrap(
            string(&first.value["root"])?,
            pin,
            number(&first.value["time"])?,
        )?
        .bytes
            != first.bytes
    {
        return Err(refused("bootstrap provenance disagrees"));
    }
    for pair in generations.windows(2) {
        let (new, old) = (&pair[0], &pair[1]);
        if new.checkpoint.observed_time < old.checkpoint.observed_time {
            return Err(stale());
        }
        if new.root.digest != old.root.digest {
            let rotated = verify_root_rotation(
                &old.root,
                string(&new.value["rotation"])?,
                number(&new.value["time"])?,
            )?;
            if rotated.root().digest != new.root.digest {
                return Err(stale());
            }
        } else if !new.value["rotation"].is_null() {
            return Err(refused("unexpected root rotation"));
        }
        for (role, stamp) in &old.checkpoint.roles {
            let new_stamp = new.checkpoint.roles.get(role).ok_or_else(stale)?;
            if new_stamp.version < stamp.version
                || (new_stamp.version == stamp.version && new_stamp.digest != stamp.digest)
            {
                return Err(stale());
            }
        }
    }
    for name in names.clone() {
        let completed = format!("c-{name}");
        if held.read(&completed, 66)? != name {
            return Err(uncertain());
        }
        names.insert(completed);
    }
    if !recovering {
        names.insert("ACTIVE".to_owned());
        if names != inventory {
            return Err(uncertain());
        }
    }
    Ok(generations.remove(0))
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
pub struct HeldTrustStore;
#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
impl HeldTrustStore {
    pub fn open(_path: &Path, _independent_pin: &str) -> Result<Self> {
        Err(refused(
            "durable registry trust store is unsupported on this host",
        ))
    }
}

#[cfg(all(
    test,
    any(target_os = "linux", target_os = "android", target_vendor = "apple")
))]
mod tests;
