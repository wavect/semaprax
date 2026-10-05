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

/// The rounding contract shared by estimates and spend reservations (MN-02):
/// each priced usage category is rounded up to whole micro-units on its own,
/// then the categories are summed. A reservation that covers usage must
/// therefore cover the per-category rounding, not a single combined ceiling.
/// `u64 * u64` always fits a `u128`; callers sum with checked arithmetic.
pub fn price_line(count: u64, price_per_million: u64) -> u128 {
    (count as u128 * price_per_million as u128).div_ceil(1_000_000)
}

pub const PRICE_BOOK_SCHEMA: &str = "semaprax.harness-price-book.v1";

impl PriceBook {
    /// Parse a versioned price book:
    /// `{"schema": "semaprax.harness-price-book.v1", "records": [{"model_prefix", "version",
    /// "pricing": "non_billed" | {"input", "cache_read", "cache_write", "cache_write_1h", "output"}}]}`
    /// with prices in micro-units per million tokens. Closed shape; a missing
    /// price stays unknown.
    pub fn from_json(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or("price book must be an object")?;
        if o.get("schema").and_then(Value::as_str) != Some(PRICE_BOOK_SCHEMA) {
            return Err(format!("price book schema must be `{PRICE_BOOK_SCHEMA}`"));
        }
        if let Some(k) = o
            .keys()
            .find(|k| !matches!(k.as_str(), "schema" | "records"))
        {
            return Err(format!("unknown price book member `{k}`"));
        }
        let recs = o
            .get("records")
            .and_then(Value::as_array)
            .filter(|a| a.len() <= 256)
            .ok_or("`records` must be an array of at most 256")?;
        let mut book = Self::default();
        for r in recs {
            let r = r.as_object().ok_or("a record must be an object")?;
            if let Some(k) = r
                .keys()
                .find(|k| !matches!(k.as_str(), "model_prefix" | "version" | "pricing"))
            {
                return Err(format!("unknown record member `{k}`"));
            }
            let text = |k: &str| {
                r.get(k)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty() && s.len() <= 128)
                    .map(str::to_string)
                    .ok_or(format!("record `{k}` must be a short non-empty string"))
            };
            let prefix = text("model_prefix")?;
            let version = text("version")?;
            let pricing = match r.get("pricing") {
                Some(Value::String(s)) if s == "non_billed" => Pricing::NonBilled,
                Some(Value::Object(p)) => {
                    if let Some(k) = p.keys().find(|k| {
                        !matches!(
                            k.as_str(),
                            "input" | "cache_read" | "cache_write" | "cache_write_1h" | "output"
                        )
                    }) {
                        return Err(format!("unknown price member `{k}`"));
                    }
                    let n = |k: &str| -> Result<Option<u64>, String> {
                        match p.get(k) {
                            None => Ok(None),
                            Some(x) => x
                                .as_u64()
                                .map(Some)
                                .ok_or(format!("price `{k}` must be a non-negative integer")),
                        }
                    };
                    Pricing::Rates {
                        input: n("input")?,
                        cache_read: n("cache_read")?,
                        cache_write: n("cache_write")?,
                        cache_write_1h: n("cache_write_1h")?,
                        output: n("output")?,
                    }
                }
                _ => return Err("`pricing` must be `non_billed` or an object of prices".into()),
            };
            book = book.with(&prefix, PriceRecord { version, pricing });
        }
        Ok(book)
    }

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
            let line = price_line(count, price);
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
