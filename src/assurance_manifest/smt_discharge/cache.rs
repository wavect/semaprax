//! A complete semantic/tool/policy digest key for one discharge attempt,
//! and a minimal in-memory cache keyed by it.
//!
//! See [`docs/SMT-DISCHARGE-V1.md`](../../../docs/SMT-DISCHARGE-V1.md)
//! "Cache key" for why every field listed in [`CacheKeyInput`] must appear
//! in the digest, and why this tranche ships only the key computation and
//! an in-memory cache rather than a persisted, cross-run store: a
//! persisted cache is a durable artifact with its own retention,
//! eviction, and multi-process-safety concerns that deserve their own
//! hostile-input review, out of scope here (see "Explicitly deferred").

use std::collections::HashMap;
use std::time::Duration;

use sha2::{Digest as _, Sha256};

const CACHE_KEY_DOMAIN: &[u8] = b"semaprax.smt-discharge.cache-key.v1\0";

/// Every input that must be part of one discharge's cache identity. Two
/// calls differing in *any* field must produce different keys; this is
/// the property `tests.rs`'s hostile-substitution cases exercise.
#[derive(Clone, Debug)]
pub struct CacheKeyInput<'a> {
    /// The exact obligation this discharge attempt targets, e.g.
    /// `"app.mod.check:ensure:0"`; ties the key to one declaration and one
    /// contract clause, not merely to a function.
    pub obligation_locator: &'a str,
    /// The exact rendered SMT-LIB2 script text: already a complete,
    /// deterministic function of the source's resolved HIR-equivalent
    /// shape (parameter types, contract clauses, body) as
    /// [`super::translate`] emits it, so a source edit that changes
    /// meaning always changes this string.
    pub script: &'a str,
    pub solver_identity: &'a str,
    pub solver_version: &'a str,
    pub timeout: Duration,
    /// Ids of every assumption this discharge attempt is permitted to
    /// lean on. Empty for the pure bounded-subset discharge this tranche
    /// implements, but part of the key regardless so a future producer
    /// that does pass assumptions cannot collide with one that does not.
    pub assumption_ids: &'a [String],
    /// The target/profile policy this result is scoped to (see the
    /// spec's "Runtime-guard fallback policy"); `"none"` when no policy
    /// selection applies yet.
    pub target_policy: &'a str,
}

fn write_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Compute the complete cache key for one discharge attempt. Every field of
/// [`CacheKeyInput`] is length-prefixed before hashing so no naive
/// concatenation of two differently-split inputs can collide (the same
/// technique [`super::super::obligation_id`] uses).
#[must_use]
pub fn cache_key(input: &CacheKeyInput<'_>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_KEY_DOMAIN);
    write_len_prefixed(&mut hasher, input.obligation_locator.as_bytes());
    write_len_prefixed(&mut hasher, input.script.as_bytes());
    write_len_prefixed(&mut hasher, input.solver_identity.as_bytes());
    write_len_prefixed(&mut hasher, input.solver_version.as_bytes());
    write_len_prefixed(&mut hasher, &input.timeout.as_nanos().to_le_bytes());
    write_len_prefixed(
        &mut hasher,
        &(input.assumption_ids.len() as u64).to_le_bytes(),
    );
    for id in input.assumption_ids {
        write_len_prefixed(&mut hasher, id.as_bytes());
    }
    write_len_prefixed(&mut hasher, input.target_policy.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// A minimal process-local cache. Deliberately not persisted to disk or
/// shared across processes in this tranche; see the module doc.
#[derive(Default)]
pub struct DischargeCache<V> {
    entries: HashMap<String, V>,
}

impl<V: Clone> DischargeCache<V> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<V> {
        self.entries.get(key).cloned()
    }

    pub fn insert(&mut self, key: String, value: V) {
        self.entries.insert(key, value);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input() -> CacheKeyInput<'static> {
        CacheKeyInput {
            obligation_locator: "app.mod.check:ensure:0",
            script: "(assert true)",
            solver_identity: "z3",
            solver_version: "4.13.0",
            timeout: Duration::from_millis(2000),
            assumption_ids: &[],
            target_policy: "none",
        }
    }

    #[test]
    fn identical_inputs_produce_identical_keys() {
        assert_eq!(cache_key(&base_input()), cache_key(&base_input()));
    }

    #[test]
    fn a_source_edit_that_changes_the_rendered_script_changes_the_key() {
        let mut changed = base_input();
        changed.script = "(assert false)";
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn a_different_obligation_locator_changes_the_key() {
        let mut changed = base_input();
        changed.obligation_locator = "app.mod.check:ensure:1";
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn a_different_solver_identity_changes_the_key() {
        let mut changed = base_input();
        changed.solver_identity = "cvc5";
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn a_different_solver_version_changes_the_key() {
        let mut changed = base_input();
        changed.solver_version = "4.12.0";
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn a_different_timeout_changes_the_key() {
        let mut changed = base_input();
        changed.timeout = Duration::from_millis(3000);
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn a_different_assumption_set_changes_the_key() {
        let ids = vec!["assume-1".to_owned()];
        let mut changed = base_input();
        changed.assumption_ids = &ids;
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn a_different_target_policy_changes_the_key() {
        let mut changed = base_input();
        changed.target_policy = "native_only";
        assert_ne!(cache_key(&base_input()), cache_key(&changed));
    }

    #[test]
    fn boundary_aliasing_across_adjacent_fields_does_not_collide() {
        // Without length-prefixing, obligation_locator="ab"+script="c" would
        // hash identically to obligation_locator="a"+script="bc".
        let mut first = base_input();
        first.obligation_locator = "ab";
        first.script = "c";
        let mut second = base_input();
        second.obligation_locator = "a";
        second.script = "bc";
        assert_ne!(cache_key(&first), cache_key(&second));
    }

    #[test]
    fn hostile_cache_substitution_a_stale_proof_cannot_survive_any_single_field_change() {
        // Simulates the attack the issue names: an attacker (or a stale
        // cache entry) tries to reuse a cached "proved" result after the
        // source, solver, timeout, assumptions, or target changed. Every
        // one of those must be rejected by a key mismatch, i.e. a cache
        // miss, never a silent hit.
        let mut cache: DischargeCache<&'static str> = DischargeCache::new();
        let original = base_input();
        cache.insert(cache_key(&original), "proved");

        let mutations: Vec<CacheKeyInput<'static>> = vec![
            {
                let mut m = base_input();
                m.script = "(assert (not true))";
                m
            },
            {
                let mut m = base_input();
                m.solver_identity = "cvc5";
                m
            },
            {
                let mut m = base_input();
                m.solver_version = "0.0.0";
                m
            },
            {
                let mut m = base_input();
                m.timeout = Duration::from_millis(1);
                m
            },
            {
                let mut m = base_input();
                m.target_policy = "wasm_only";
                m
            },
        ];
        for mutation in mutations {
            assert_eq!(
                cache.get(&cache_key(&mutation)),
                None,
                "a mutated input must never hit the original cache entry"
            );
        }
        // The exact original input still hits.
        assert_eq!(cache.get(&cache_key(&original)), Some("proved"));
    }

    #[test]
    fn cache_reuse_skips_a_repeated_lookup() {
        let mut cache: DischargeCache<u32> = DischargeCache::new();
        let key = cache_key(&base_input());
        assert!(cache.is_empty());
        cache.insert(key.clone(), 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&key), Some(1));
    }
}
