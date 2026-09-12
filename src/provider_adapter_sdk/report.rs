//! `ProviderAdapterConformanceReport v1`: binds adapter identity/version,
//! provider profile, test corpus identity, observed capabilities, and
//! nonclaims into one canonical, deterministic rendering — mirroring
//! [`crate::model_call_receipt::receipt::ModelCallReceipt`]'s "commitments,
//! not authority" discipline for the same reason: a report is evidence a
//! suite ran and what it found, never a support or publication decision on
//! its own. Generating a passing report is not itself a claim that an
//! adapter is supported; that remains a separate, human decision (issue
//! #181's own "A generated artifact is not a support or publication
//! decision").

use sha2::{Digest as _, Sha256};

use crate::diagnostic::quote_json;
use crate::digest_hex::LowerHex;

pub const REPORT_SCHEMA: &str = "semaprax.provider-adapter-conformance-report.v1";
const REPORT_DOMAIN: &[u8] = b"semaprax.provider-adapter-conformance-report.v1\0";

/// The one shared, named test corpus [`super::conformance::run_conformance_suite`]
/// runs. Versioned so a report can be compared against exactly the corpus
/// that produced it; widening the corpus is a new identifier, never a
/// silent reinterpretation of an old report.
pub const TEST_CORPUS_ID: &str = "semaprax.provider-adapter-conformance-corpus.v1";

/// Standing nonclaims every report carries, regardless of outcome. These
/// are never narrowed by a passing case: a report proves what ran, not
/// more.
pub const NONCLAIM_OFFLINE_ONLY: &str =
    "this report reflects only offline fixture/replay adapters driven in-process; no live network call was made and no real provider was contacted";
pub const NONCLAIM_CANCELLATION_BEST_EFFORT: &str =
    "a Cancelled classification proves only that this process requested cancellation, never that a provider stopped processing or billing";
pub const NONCLAIM_USAGE_UNVERIFIED: &str =
    "usage/cost figures reflect only what the adapter itself reported; this suite does not verify them against real provider billing";
pub const NONCLAIM_NOT_A_SUPPORT_DECISION: &str =
    "a fully passing report is local evidence that this suite ran and observed no violation; it is not itself a support, publication, or compatibility decision";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

/// One named case's outcome within a [`ConformanceReport`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportCaseResult {
    pub case_name: &'static str,
    pub passed: bool,
    /// The specific observed reason. Never empty when `passed` is `false`:
    /// every case that can fail names the exact violation, not a generic
    /// "conformance failure".
    pub detail: String,
}

/// One conformance run's complete, deterministic record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceReport {
    pub adapter_identity: String,
    pub adapter_version: String,
    pub provider_profile: String,
    pub test_corpus_id: String,
    /// Canonical, one-line rendering of the adapter's declared
    /// capabilities (see `super::conformance::render_capabilities`) —
    /// captured once at the start of the run, matching `capabilities` must
    /// stay stable across the run.
    pub observed_capabilities: String,
    /// Cases in the exact order they were run. Never sorted or
    /// deduplicated: run order is itself part of what the report attests.
    pub cases: Vec<ReportCaseResult>,
    pub nonclaims: Vec<&'static str>,
}

impl ConformanceReport {
    #[must_use]
    pub fn all_passed(&self) -> bool {
        !self.cases.is_empty() && self.cases.iter().all(|case| case.passed)
    }

    #[must_use]
    pub fn case(&self, name: &str) -> Option<&ReportCaseResult> {
        self.cases.iter().find(|case| case.case_name == name)
    }

    /// The canonical, deterministic rendering: fixed field order,
    /// JSON-escaped strings, cases and nonclaims rendered in the exact
    /// order they were recorded. Two reports with identical field values
    /// render identically; any differing field renders differently.
    #[must_use]
    pub fn render(&self) -> String {
        let mut cases = String::from("[");
        for (index, case) in self.cases.iter().enumerate() {
            if index > 0 {
                cases.push(',');
            }
            cases.push_str(&format!(
                "{{\"case_name\":{},\"passed\":{},\"detail\":{}}}",
                quote_json(case.case_name),
                case.passed,
                quote_json(&case.detail),
            ));
        }
        cases.push(']');

        let mut nonclaims = String::from("[");
        for (index, nonclaim) in self.nonclaims.iter().enumerate() {
            if index > 0 {
                nonclaims.push(',');
            }
            nonclaims.push_str(&quote_json(nonclaim));
        }
        nonclaims.push(']');

        format!(
            "{{\"schema\":{},\"adapter_identity\":{},\"adapter_version\":{},\"provider_profile\":{},\"test_corpus_id\":{},\"observed_capabilities\":{},\"cases\":{},\"nonclaims\":{}}}",
            quote_json(REPORT_SCHEMA),
            quote_json(&self.adapter_identity),
            quote_json(&self.adapter_version),
            quote_json(&self.provider_profile),
            quote_json(&self.test_corpus_id),
            quote_json(&self.observed_capabilities),
            cases,
            nonclaims,
        )
    }

    /// The canonical digest of this exact report. Any single differing
    /// byte in [`Self::render`] changes this.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(REPORT_DOMAIN, self.render().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ConformanceReport {
        ConformanceReport {
            adapter_identity: "adapter-a".into(),
            adapter_version: "1.0.0".into(),
            provider_profile: "fixture".into(),
            test_corpus_id: TEST_CORPUS_ID.into(),
            observed_capabilities: "streaming=true".into(),
            cases: vec![ReportCaseResult {
                case_name: "capability_negotiation",
                passed: true,
                detail: String::new(),
            }],
            nonclaims: vec![NONCLAIM_OFFLINE_ONLY],
        }
    }

    #[test]
    fn identical_reports_render_and_digest_identically() {
        assert_eq!(sample().render(), sample().render());
        assert_eq!(sample().digest(), sample().digest());
    }

    #[test]
    fn a_single_changed_case_result_changes_the_digest() {
        let base = sample();
        let mut mutated = sample();
        mutated.cases[0].passed = false;
        mutated.cases[0].detail = "ADAPTER-DUPLICATE-COMPLETION: a second Completed event arrived".into();
        assert_ne!(base.render(), mutated.render());
        assert_ne!(base.digest(), mutated.digest());
    }

    #[test]
    fn all_passed_is_false_on_an_empty_case_list() {
        let mut report = sample();
        report.cases.clear();
        assert!(!report.all_passed(), "an empty case list proves nothing ran, not success");
    }

    #[test]
    fn all_passed_requires_every_case_to_pass() {
        let mut report = sample();
        report.cases.push(ReportCaseResult {
            case_name: "drive_to_settlement",
            passed: false,
            detail: "ADAPTER-OVERSIZED-RESPONSE: response exceeded the 64-byte bound".into(),
        });
        assert!(!report.all_passed());
    }
}
