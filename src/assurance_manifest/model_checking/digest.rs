//! Model identity: a stable digest over one model's declared descriptor and
//! the exact [`Bounds`] an exploration ran under.
//!
//! This binds `model_checked` manifest records to "which model, at which
//! declared shape, under which bounds" the same way
//! [`crate::assurance_manifest`]'s own source/payload digests bind a
//! manifest to exact source bytes: same domain-separated SHA-256 technique,
//! same length-prefixing discipline as
//! [`super::super::obligation::obligation_id`] so no field boundary can
//! alias another.
//!
//! **Exact scope**: this digest covers the model's *declared* identity
//! (name, version, the invariant and terminal-state labels it asserts) plus
//! the bounds, not the Rust bytecode of `enabled_events`/`apply`/
//! `safety_invariant` themselves. A change to those functions that is not
//! matched by a `version` bump is not detected by this digest; the model
//! authors are responsible for bumping `version` whenever transition or
//! invariant behavior changes, exactly as every other versioned wire schema
//! in this repository is. See
//! [`docs/BOUNDED-MODEL-CHECKING-V1.md`](../../../docs/BOUNDED-MODEL-CHECKING-V1.md)
//! "Model identity and drift" for the full statement of this boundary.

use sha2::{Digest as _, Sha256};

use super::engine::Bounds;

const MODEL_DIGEST_DOMAIN: &[u8] = b"semaprax.assurance-manifest.model-checking.model.v1\0";

/// The declared identity of one concrete [`super::engine::TransitionSystem`]
/// projection: a name and version this crate's authors bump by hand
/// whenever the model's states, events, transitions, or invariants change,
/// plus the exact invariant and terminal-state labels it currently asserts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelDescriptor {
    pub name: &'static str,
    pub version: &'static str,
    pub invariants: &'static [&'static str],
    pub terminal_states: &'static [&'static str],
}

fn write_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Deterministically render `descriptor` and `bounds` into one
/// `sha256:<hex>` digest string. Byte-identical for byte-identical inputs
/// on every platform and Rust toolchain this repository supports (SHA-256
/// over a fixed length-prefixed byte sequence has no platform-dependent
/// step); see the module test for the exact reproducibility claim.
#[must_use]
pub fn model_digest(descriptor: &ModelDescriptor, bounds: Bounds) -> String {
    let mut hasher = Sha256::new();
    hasher.update(MODEL_DIGEST_DOMAIN);
    write_len_prefixed(&mut hasher, descriptor.name.as_bytes());
    write_len_prefixed(&mut hasher, descriptor.version.as_bytes());
    hasher.update((descriptor.invariants.len() as u64).to_le_bytes());
    for invariant in descriptor.invariants {
        write_len_prefixed(&mut hasher, invariant.as_bytes());
    }
    hasher.update((descriptor.terminal_states.len() as u64).to_le_bytes());
    for terminal in descriptor.terminal_states {
        write_len_prefixed(&mut hasher, terminal.as_bytes());
    }
    hasher.update((bounds.max_states as u64).to_le_bytes());
    hasher.update((bounds.max_depth as u64).to_le_bytes());
    hasher.update((bounds.max_transitions as u64).to_le_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESCRIPTOR: ModelDescriptor = ModelDescriptor {
        name: "test.model",
        version: "v1",
        invariants: &["inv_a", "inv_b"],
        terminal_states: &["Done"],
    };
    const BOUNDS: Bounds = Bounds {
        max_states: 10,
        max_depth: 5,
        max_transitions: 20,
    };

    #[test]
    fn digest_is_deterministic_across_repeated_calls() {
        let first = model_digest(&DESCRIPTOR, BOUNDS);
        let second = model_digest(&DESCRIPTOR, BOUNDS);
        assert_eq!(first, second);
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), "sha256:".len() + 64);
    }

    #[test]
    fn digest_changes_with_bounds() {
        let widened = Bounds {
            max_states: 11,
            ..BOUNDS
        };
        assert_ne!(
            model_digest(&DESCRIPTOR, BOUNDS),
            model_digest(&DESCRIPTOR, widened)
        );
    }

    #[test]
    fn digest_changes_with_version() {
        let bumped = ModelDescriptor {
            version: "v2",
            ..DESCRIPTOR
        };
        assert_ne!(
            model_digest(&DESCRIPTOR, BOUNDS),
            model_digest(&bumped, BOUNDS)
        );
    }

    #[test]
    fn digest_changes_with_invariant_set() {
        let extra = ModelDescriptor {
            invariants: &["inv_a", "inv_b", "inv_c"],
            ..DESCRIPTOR
        };
        assert_ne!(
            model_digest(&DESCRIPTOR, BOUNDS),
            model_digest(&extra, BOUNDS)
        );
    }

    #[test]
    fn length_prefixing_prevents_field_boundary_aliasing() {
        // Without length-prefixing, name="ab"+version="c" would hash the same
        // as name="a"+version="bc".
        let first = ModelDescriptor {
            name: "ab",
            version: "c",
            invariants: &[],
            terminal_states: &[],
        };
        let second = ModelDescriptor {
            name: "a",
            version: "bc",
            invariants: &[],
            terminal_states: &[],
        };
        assert_ne!(model_digest(&first, BOUNDS), model_digest(&second, BOUNDS));
    }
}
