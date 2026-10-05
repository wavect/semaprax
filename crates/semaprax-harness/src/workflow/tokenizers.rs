//! Host-approved tokenizer registry and the bounded per-run count memo
//! (TC-11). A task can only name a tokenizer the host already provisioned and
//! approved; it never supplies code. The memo stores counts only, keyed by the
//! SHA-256 of the exact text, never the text itself.

use crate::observe::Tokenizer;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Mutex;

/// Built-in names the local helper can serve (approved for naming even before
/// they are provisioned).
pub const BUILTIN_TOKENIZER_NAMES: [&str; 2] = ["cl100k_base", "o200k_base"];

/// Counting semantics used when the host does not declare another.
pub const DEFAULT_SEMANTICS: &str = "plain-text-v1";

/// Approval record for one provisioned tokenizer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Approval {
    pub fingerprint: String,
    /// Counting semantics (the cache `options` component).
    pub semantics: String,
}

/// Supplied tokenizers by name; `add*` is the host's approval act.
#[derive(Default)]
pub struct TokenizerSet {
    tokenizers: BTreeMap<String, Box<dyn Tokenizer>>,
    approvals: BTreeMap<String, Approval>,
}

impl TokenizerSet {
    /// Approve and provision a tokenizer under its own name and fingerprint.
    pub fn add(&mut self, t: Box<dyn Tokenizer>) {
        let a = Approval {
            fingerprint: t.fingerprint().to_string(),
            semantics: DEFAULT_SEMANTICS.to_string(),
        };
        self.approvals.insert(t.name().to_string(), a);
        self.tokenizers.insert(t.name().to_string(), t);
    }
    /// Approve only if the artifact fingerprint matches the pinned one.
    pub fn add_pinned(
        &mut self,
        t: Box<dyn Tokenizer>,
        expected_fingerprint: &str,
        semantics: &str,
    ) -> Result<(), String> {
        if t.fingerprint() != expected_fingerprint {
            return Err(format!(
                "tokenizer `{}` fingerprint `{}` does not match the approved `{expected_fingerprint}`",
                t.name(),
                t.fingerprint()
            ));
        }
        let a = Approval {
            fingerprint: expected_fingerprint.into(),
            semantics: semantics.into(),
        };
        self.approvals.insert(t.name().to_string(), a);
        self.tokenizers.insert(t.name().to_string(), t);
        Ok(())
    }
    pub fn get(&self, name: &str) -> Option<&dyn Tokenizer> {
        self.tokenizers.get(name).map(|b| b.as_ref())
    }
    pub fn approval(&self, name: &str) -> Option<&Approval> {
        self.approvals.get(name)
    }
    /// A task may name a built-in or a host-approved tokenizer, nothing else.
    pub fn is_approved_name(&self, name: &str) -> bool {
        BUILTIN_TOKENIZER_NAMES.contains(&name) || self.approvals.contains_key(name)
    }
}

type Key = ([u8; 32], String, String, String);

/// Bounded FIFO memo of exact counts. Holds digests and integers only.
pub struct CountCache {
    inner: Mutex<Inner>,
}

struct Inner {
    cap: usize,
    map: HashMap<Key, u64>,
    order: VecDeque<Key>,
    hits: u64,
    misses: u64,
}

/// Default entry bound (each entry is roughly 100 bytes).
pub const DEFAULT_CACHE_ENTRIES: usize = 1024;

impl Default for CountCache {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CACHE_ENTRIES)
    }
}

impl CountCache {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                cap,
                map: HashMap::new(),
                order: VecDeque::new(),
                hits: 0,
                misses: 0,
            }),
        }
    }
    /// Count through the memo. Failures are returned and never cached.
    pub fn count_with<E>(
        &self,
        name: &str,
        fingerprint: &str,
        options: &str,
        text: &str,
        count: impl FnOnce() -> Result<u64, E>,
    ) -> Result<u64, E> {
        let key: Key = (
            Sha256::digest(text.as_bytes()).into(),
            name.into(),
            fingerprint.into(),
            options.into(),
        );
        {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(n) = g.map.get(&key).copied() {
                g.hits += 1;
                return Ok(n);
            }
            g.misses += 1;
        }
        let n = count()?;
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.cap > 0 && !g.map.contains_key(&key) {
            while g.map.len() >= g.cap {
                match g.order.pop_front() {
                    Some(old) => {
                        g.map.remove(&old);
                    }
                    None => break,
                }
            }
            g.order.push_back(key.clone());
            g.map.insert(key, n);
        }
        Ok(n)
    }
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .map
            .len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// `(hits, misses)`; a hit is local work avoided, not provider tokens.
    pub fn stats(&self) -> (u64, u64) {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (g.hits, g.misses)
    }
    pub fn stats_json(&self) -> Value {
        let (h, m) = self.stats();
        serde_json::json!({"hits": h, "misses": m, "entries": self.len()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::{Destination, ModelPlan};
    use crate::workflow::budget::{BudgetConfig, ModelTokenizerMap};
    use serde_json::json;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Words {
        fp: &'static str,
        calls: Rc<Cell<u32>>,
        fail: bool,
    }
    impl Tokenizer for Words {
        fn name(&self) -> &str {
            "words-x"
        }
        fn fingerprint(&self) -> &str {
            self.fp
        }
        fn count(&self, t: &str) -> usize {
            self.calls.set(self.calls.get() + 1);
            t.split_whitespace().count()
        }
        fn try_count(&self, t: &str) -> crate::diag::HarnessResult<usize> {
            if self.fail {
                return Err(crate::diag::HarnessDiagnostic::new("SPX-HPD081", "boom"));
            }
            Ok(self.count(t))
        }
    }
    fn plan(id: &str) -> ModelPlan {
        ModelPlan {
            id: id.into(),
            destination: Destination::Local,
            structured_output: true,
            tools: false,
            max_context: 1_000_000,
            est_cost_micros: 0,
            est_latency_ms: 1,
            strength_rank: 1,
        }
    }
    fn cfg(fp: &'static str, fail: bool) -> (BudgetConfig, Rc<Cell<u32>>) {
        let calls = Rc::new(Cell::new(0));
        let mut c = BudgetConfig {
            map: ModelTokenizerMap::empty().with("fam-", "words-x"),
            ..BudgetConfig::default()
        };
        c.tokenizers.add(Box::new(Words {
            fp,
            calls: calls.clone(),
            fail,
        }));
        (c, calls)
    }
    fn budget(c: &BudgetConfig) -> crate::workflow::budget::RequestBudget<'_> {
        crate::workflow::budget::RequestBudget {
            policy: Default::default(),
            map: c.map.clone(),
            tokenizers: &c.tokenizers,
            cache: &c.cache,
        }
    }

    #[test]
    fn identical_text_is_counted_once_across_catalog_floor_and_fit() {
        let (c, calls) = cfg("fp1", false);
        let b = budget(&c);
        let catalog = vec![plan("fam-a"), plan("fam-b"), plan("fam-c")];
        let build = |_: &std::collections::BTreeSet<String>| json!({"p": "one two three"});
        b.floor_estimate(&catalog, &[], &build);
        assert_eq!(calls.get(), 1);
        let f = b.fit(&catalog[0], &[], &build);
        assert_eq!(f.count.tokens, Some(3));
        assert_eq!(calls.get(), 1, "fit reuses the floor count");
        let (h, m) = c.cache.stats();
        assert_eq!(m, 1);
        assert!(h >= 3);
    }

    #[test]
    fn changed_text_options_or_fingerprint_miss() {
        let cache = CountCache::default();
        let n = Cell::new(0u32);
        let go = |name: &str, fp: &str, o: &str, t: &str| {
            cache
                .count_with::<()>(name, fp, o, t, || {
                    n.set(n.get() + 1);
                    Ok(7)
                })
                .unwrap()
        };
        go("t", "f", "o", "a");
        go("t", "f", "o", "a");
        assert_eq!(n.get(), 1);
        go("t", "f", "o", "b");
        go("t", "f", "o2", "a");
        go("t", "f2", "o", "a");
        assert_eq!(n.get(), 4);
    }

    #[test]
    fn cache_is_bounded_and_failures_are_not_cached() {
        let cache = CountCache::with_capacity(2);
        for i in 0..10 {
            cache
                .count_with::<()>("t", "f", "o", &format!("x{i}"), || Ok(1))
                .unwrap();
        }
        assert_eq!(cache.len(), 2);
        let r = cache.count_with("t", "f", "o", "bad", || Err("e"));
        assert!(r.is_err());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn unapproved_unavailable_or_failing_tokenizer_stays_unknown_never_zero() {
        let (c, _) = cfg("fp1", true);
        let r = budget(&c).count("fam-a", "x y");
        assert_eq!(r.tokens, None);
        assert_eq!(r.admission_tokens(), 3);
        let (c, _) = cfg("fp1", false);
        let r = budget(&c).count("other-model", "x y");
        assert_eq!(r.tokens, None);
        // Mapped to a name nobody provisioned.
        let c2 = BudgetConfig {
            map: ModelTokenizerMap::empty().with("fam-", "words-x"),
            ..BudgetConfig::default()
        };
        assert_eq!(budget(&c2).count("fam-a", "x y").tokens, None);
        // A task cannot name an unapproved tokenizer, but can name a host-approved one.
        let v = json!({"fam-": "words-x"});
        assert!(ModelTokenizerMap::from_json(&v).is_err());
        assert!(ModelTokenizerMap::from_json_with(&v, &c.tokenizers).is_ok());
        let task = br#"{"schema":"semaprax.harness-task.v2","mode":"change","goal":"g","acceptance":["a"],"tokenizer_map":{"fam-":"words-x"}}"#;
        assert!(crate::workflow::stages::Task::parse(task).is_err());
        let t = crate::workflow::stages::Task::parse_with(task, &c.tokenizers).unwrap();
        assert_eq!(
            t.tokenizer_map.unwrap().tokenizer_for("fam-a"),
            Some("words-x")
        );
        let bad = br#"{"schema":"semaprax.harness-task.v2","mode":"change","goal":"g","acceptance":["a"],"tokenizer_map":{"fam-":"nope"}}"#;
        assert!(crate::workflow::stages::Task::parse_with(bad, &c.tokenizers).is_err());
        let mut set = TokenizerSet::default();
        let calls = Rc::new(Cell::new(0));
        let w = Words {
            fp: "fpX",
            calls,
            fail: false,
        };
        assert!(set.add_pinned(Box::new(w), "fp1", "s").is_err());
    }
}
