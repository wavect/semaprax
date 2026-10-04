//! Bounded routing-decision cache. Entries are keyed by provider/model/
//! checkpoint plus feature, catalog and policy digests; a hit is still
//! revalidated against the live admissible set by the caller.

use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub provider_id: String,
    pub model_id: String,
    pub checkpoint: String,
    pub features: String,
    pub catalog: String,
    pub policy: String,
}

#[derive(Debug)]
pub struct DecisionCache {
    cap: usize,
    map: BTreeMap<CacheKey, String>,
    order: VecDeque<CacheKey>,
    hits: u64,
}

impl DecisionCache {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            map: BTreeMap::new(),
            order: VecDeque::new(),
            hits: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<String> {
        let v = self.map.get(key).cloned();
        if v.is_some() {
            self.hits += 1;
        }
        v
    }

    /// Insert, evicting the oldest entry beyond the cap (FIFO).
    pub fn put(&mut self, key: CacheKey, choice: String) {
        if self.cap == 0 {
            return;
        }
        if self.map.insert(key.clone(), choice).is_none() {
            self.order.push_back(key);
        }
        while self.map.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
}
