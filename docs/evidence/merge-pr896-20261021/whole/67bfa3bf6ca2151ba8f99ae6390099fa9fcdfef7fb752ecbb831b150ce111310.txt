//! Parsed historical JSON DATA only. Every caller retains its fresh reads and
//! complete replay; no validation result or selected source row is retained.
use serde_json::Value;
use std::sync::Mutex;

pub(super) struct Data {
    entry: Option<(String, Value)>,
}
impl Data {
    pub(super) const fn new() -> Self {
        Self { entry: None }
    }
    pub(super) fn parse(&mut self, raw: &str) -> Result<Value, serde_json::Error> {
        // Preserve the original parser even outside the retention bound. The
        // owning loaders, not this data optimization, enforce admission caps.
        if raw.len() > super::SOURCE_CAP {
            return serde_json::from_str(raw);
        }
        if let Some((bytes, value)) = &self.entry {
            if bytes == raw {
                return Ok(value.clone());
            }
        }
        let value: Value = serde_json::from_str(raw)?;
        self.entry = Some((raw.to_owned(), value.clone()));
        Ok(value)
    }
}

pub(super) fn parse(data: &Mutex<Data>, raw: &str) -> Result<Value, serde_json::Error> {
    // Poisoning cannot introduce a new source refusal or substitute old DATA.
    // On that path use the original parser without retaining anything.
    match data.lock() {
        Ok(mut data) => data.parse(raw),
        Err(_) => serde_json::from_str(raw),
    }
}

// Sixteen literal parse sites, each retaining at most one bounded input/object.
// Evaluate the original expression exactly once, before taking the DATA lock;
// read_view/held (and nested overlay replay) never execute while it is held.
macro_rules! historical_json {
    ($raw:expr) => {{
        static DATA: std::sync::Mutex<historical_json_data::Data> =
            std::sync::Mutex::new(historical_json_data::Data::new());
        historical_json_data::parse(&DATA, $raw)
    }};
}

#[cfg(test)]
#[path = "main_e177_historical_data_tests.rs"]
mod tests;
