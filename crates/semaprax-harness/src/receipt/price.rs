//! Host-configured, versioned price records. A locally calculated figure is an
//! *estimate*, distinct from a provider-reported charge; absent prices or
//! incomplete usage leave it unknown. Prices are micro-units per million tokens.

use super::normalize::Usage;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pricing {
    /// An explicitly known non-billed source (for example a local model). Zero
    /// billing is not a claim of zero compute cost.
    NonBilled,
    Rates {
        input: Option<u64>,
        cache_read: Option<u64>,
        cache_write: Option<u64>,
        cache_write_1h: Option<u64>,
        output: Option<u64>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceRecord {
    /// Operator-chosen record version (for example a date); recorded with every estimate.
    pub version: String,
    pub pricing: Pricing,
}

/// Price records by model-id prefix (longest prefix wins). Empty by default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PriceBook(BTreeMap<String, PriceRecord>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostEstimate {
    /// `None` is unknown.
    pub micros: Option<u64>,
    pub basis: &'static str,
    pub price_version: Option<String>,
}

impl CostEstimate {
    fn unknown(basis: &'static str, version: Option<&str>) -> Self {
        Self {
            micros: None,
            basis,
            price_version: version.map(str::to_string),
        }
    }
    pub fn to_json(&self) -> Value {
        json!({"kind": "estimate", "micros": self.micros.map_or(json!("unknown"), |n| json!(n)),
               "basis": self.basis, "price_version": self.price_version})
    }
}

impl PriceBook {
    pub fn with(mut self, model_prefix: &str, record: PriceRecord) -> Self {
        self.0.insert(model_prefix.into(), record);
        self
    }
    pub fn record_for(&self, model: &str) -> Option<&PriceRecord> {
        self.0
            .iter()
            .filter(|(p, _)| model.starts_with(p.as_str()))
            .max_by_key(|(p, _)| p.len())
            .map(|(_, r)| r)
    }

    /// Estimate the cost of `usage` for `model` with checked arithmetic. Every
    /// category must be known and, when non-zero, priced; otherwise unknown.
    pub fn estimate(&self, model: &str, usage: &Usage) -> CostEstimate {
        let Some(rec) = self.record_for(model) else {
            return CostEstimate::unknown("unpriced_model", None);
        };
        let v = Some(rec.version.as_str());
        let (input, read, write, write_1h, out) = match &rec.pricing {
            Pricing::NonBilled => {
                return CostEstimate {
                    micros: Some(0),
                    basis: "non_billed_source",
                    price_version: Some(rec.version.clone()),
                }
            }
            Pricing::Rates {
                input,
                cache_read,
                cache_write,
                cache_write_1h,
                output,
            } => (*input, *cache_read, *cache_write, *cache_write_1h, *output),
        };
        // Cache-write tokens billed at the 1h tier are priced apart from the rest.
        let (w_1h, w_rest) = match (usage.cache_write, usage.cache_write_1h) {
            (Some(w), Some(h)) if h <= w => (Some(h), Some(w - h)),
            (Some(0), _) => (Some(0), Some(0)),
            // An unsplit write total cannot be priced when a 1h rate exists.
            (Some(w), None) if write_1h.is_none() => (Some(0), Some(w)),
            _ => (None, None),
        };
        let parts = [
            (usage.uncached_input, input),
            (usage.cache_read, read),
            (w_rest, write),
            (w_1h, write_1h),
            (usage.output, out),
        ];
        let mut total: u128 = 0;
        for (count, price) in parts {
            let Some(count) = count else {
                return CostEstimate::unknown("incomplete_usage", v);
            };
            if count == 0 {
                continue;
            }
            let Some(price) = price else {
                return CostEstimate::unknown("missing_price", v);
            };
            // Round up: an estimate never under-reports a priced category.
            let line = (count as u128 * price as u128).div_ceil(1_000_000);
            total = match total.checked_add(line) {
                Some(t) => t,
                None => return CostEstimate::unknown("overflow", v),
            };
        }
        match u64::try_from(total) {
            Ok(n) => CostEstimate {
                micros: Some(n),
                basis: "estimated_from_price_record",
                price_version: Some(rec.version.clone()),
            },
            Err(_) => CostEstimate::unknown("overflow", v),
        }
    }
}
