use std::collections::BTreeMap;

use serde_json::Value;

use crate::bounded_output;
use crate::diagnostic::quote_json;
use crate::package_lock_v2;

macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }

#[derive(Clone)]
pub(super) struct Authenticated {
    pub(super) coordinate: package_lock_v2::Coordinate,
    pub(super) report_digest: String,
    pub(super) report_bytes: usize,
    pub(super) lock_digest: String,
    pub(super) lock_bytes: usize,
    pub(super) subjects_digest: String,
    pub(super) subjects_bytes: usize,
    pub(super) report: Report,
    pub(super) context: Value,
    pub(super) lock_targets: BTreeMap<String, String>,
}

#[derive(Clone)]
pub(super) struct Report {
    pub(super) exports: BTreeMap<String, Value>,
    pub(super) types: BTreeMap<String, Value>,
    pub(super) targets: BTreeMap<String, String>,
    pub(super) unproven: bool,
    pub(super) call_contract: bool,
    pub(super) imported_resource: bool,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Finding {
    pub(super) classification: &'static str,
    pub(super) axis: &'static str,
    pub(super) subject: String,
    pub(super) before: String,
    pub(super) after: String,
    pub(super) reason: &'static str,
}

impl Finding {
    pub(super) fn render(&self) -> String {
        bf!("{{\"classification\":{},\"axis\":{},\"subject\":{},\"before\":{},\"after\":{},\"reason\":{}}}",quote_json(self.classification),quote_json(self.axis),quote_json(&self.subject),quote_json(&self.before),quote_json(&self.after),quote_json(self.reason))
    }
}

pub(super) fn render_input(value: &Authenticated) -> String {
    bf!("{{\"package\":{},\"version\":{},\"report_digest\":{},\"report_bytes\":{},\"lock_digest\":{},\"lock_bytes\":{},\"subjects_digest\":{},\"subjects_bytes\":{}}}",quote_json(&value.coordinate.package),quote_json(&value.coordinate.version),quote_json(&value.report_digest),value.report_bytes,quote_json(&value.lock_digest),value.lock_bytes,quote_json(&value.subjects_digest),value.subjects_bytes)
}
