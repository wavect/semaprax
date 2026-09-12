//! Billing/usage reconciliation: importing a provider's invoice/usage
//! record as **untrusted external evidence** and comparing it against a
//! receipt's own local accounting.
//!
//! # What this explicitly does not claim
//!
//! [`ProviderInvoiceRow`] is exactly that — an externally supplied record a
//! deployment received from a provider, imported here with no adapter-side
//! verification of its authenticity. This module never treats it as proof
//! of anything about the call's semantic correctness (that would be
//! "treating provider invoices as proof of semantic correctness," explicitly
//! out of scope for #180), and never widens a budget, authorizes a future
//! call, or repairs a receipt to make it agree — a discrepancy is reported,
//! never silently normalized away. "Tokens" here is a byte-count proxy this
//! module can compute without a real tokenizer
//! (`local_request_bytes + local_response_bytes`), not a claim that this
//! crate implements real token accounting for any provider.

use std::collections::HashSet;

use super::receipt::ModelCallReceipt;

/// One externally supplied provider usage/invoice line, imported as
/// untrusted evidence. Never constructed from anything this crate itself
/// computed — a real caller parses this out of an actual provider invoice
/// or usage export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderInvoiceRow {
    pub provider_call_id: String,
    pub account_id: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_micros: i64,
}

/// The closed reconciliation outcome vocabulary. Every discrepancy variant
/// carries the specific figures or identifiers that disagreed, never just a
/// boolean "mismatch."
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconciliationOutcome {
    /// The invoice row's reported units exactly match this receipt's local
    /// accounting, cite the expected call reference, and belong to the
    /// expected account.
    Reconciled,
    /// The provider reported more usage units than this receipt's local
    /// accounting recorded.
    ProviderOverReported { local_units: u64, provider_units: u64 },
    /// The provider reported fewer usage units than this receipt's local
    /// accounting recorded.
    ProviderUnderReported { local_units: u64, provider_units: u64 },
    /// This exact `provider_call_id` was already reconciled once before —
    /// the same invoice line submitted twice (or a genuine double-bill).
    DuplicateInvoiceRow { provider_call_id: String },
    /// The invoice row's `provider_call_id` does not match this receipt's
    /// own `provider_call_reference` — it is not evidence about this call
    /// at all.
    UnknownCall { provider_call_id: String },
    /// The invoice row's `account_id` is not the account this reconciler
    /// was constructed to reconcile against.
    WrongAccount { expected: String, found: String },
    /// Reconciliation cannot yet run: either the receipt has not settled
    /// (`ModelCallReceipt::terminal_stage`), or no provider record has
    /// arrived yet. Provider usage may legitimately arrive late; this is
    /// the explicit "not yet known" state, never silently treated as
    /// agreement.
    Uncertain { reason: &'static str },
}

/// Reconciles one receipt against provider-reported invoice rows, tracking
/// which `provider_call_id`s have already been consumed so a duplicate row
/// is detected rather than silently re-applied.
#[derive(Debug)]
pub struct BillingReconciler {
    expected_account: String,
    seen_provider_call_ids: HashSet<String>,
}

impl BillingReconciler {
    #[must_use]
    pub fn new(expected_account: impl Into<String>) -> Self {
        Self {
            expected_account: expected_account.into(),
            seen_provider_call_ids: HashSet::new(),
        }
    }

    /// Reconciles `receipt` against `row` (`None` when no provider record
    /// has arrived for this call yet).
    pub fn reconcile(
        &mut self,
        receipt: &ModelCallReceipt,
        row: Option<&ProviderInvoiceRow>,
    ) -> ReconciliationOutcome {
        if !receipt.terminal_stage.is_settled() {
            return ReconciliationOutcome::Uncertain {
                reason: "receipt has not settled yet",
            };
        }
        let Some(row) = row else {
            return ReconciliationOutcome::Uncertain {
                reason: "no provider invoice row observed yet",
            };
        };
        if row.provider_call_id != receipt.provider_call_reference {
            return ReconciliationOutcome::UnknownCall {
                provider_call_id: row.provider_call_id.clone(),
            };
        }
        if !self.seen_provider_call_ids.insert(row.provider_call_id.clone()) {
            return ReconciliationOutcome::DuplicateInvoiceRow {
                provider_call_id: row.provider_call_id.clone(),
            };
        }
        if row.account_id != self.expected_account {
            return ReconciliationOutcome::WrongAccount {
                expected: self.expected_account.clone(),
                found: row.account_id.clone(),
            };
        }

        let local_units = (receipt.local_request_bytes + receipt.local_response_bytes) as u64;
        let provider_units = row.tokens_in + row.tokens_out;
        if provider_units == local_units {
            ReconciliationOutcome::Reconciled
        } else if provider_units > local_units {
            ReconciliationOutcome::ProviderOverReported {
                local_units,
                provider_units,
            }
        } else {
            ReconciliationOutcome::ProviderUnderReported {
                local_units,
                provider_units,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_call_receipt::receipt::tests::sample_receipt;
    use crate::model_call_receipt::receipt::ReceiptStage;

    fn settled_receipt() -> ModelCallReceipt {
        let mut receipt = sample_receipt();
        receipt.terminal_stage = ReceiptStage::Decoded;
        receipt.local_request_bytes = 10;
        receipt.local_response_bytes = 20;
        receipt.provider_call_reference = "call-ref-1".into();
        receipt
    }

    fn row(provider_call_id: &str, account_id: &str, tokens_in: u64, tokens_out: u64) -> ProviderInvoiceRow {
        ProviderInvoiceRow {
            provider_call_id: provider_call_id.into(),
            account_id: account_id.into(),
            tokens_in,
            tokens_out,
            cost_micros: 1234,
        }
    }

    #[test]
    fn exact_match_reconciles() {
        let receipt = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        let outcome = reconciler.reconcile(&receipt, Some(&row("call-ref-1", "acct-1", 18, 12)));
        // local_units = 10 + 20 = 30; provider tokens_in+tokens_out = 30.
        assert_eq!(outcome, ReconciliationOutcome::Reconciled);
    }

    #[test]
    fn provider_over_report_is_detected_with_the_specific_units() {
        let receipt = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        let outcome = reconciler.reconcile(&receipt, Some(&row("call-ref-1", "acct-1", 20, 20)));
        assert_eq!(
            outcome,
            ReconciliationOutcome::ProviderOverReported {
                local_units: 30,
                provider_units: 40
            }
        );
    }

    #[test]
    fn provider_under_report_is_detected_with_the_specific_units() {
        let receipt = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        let outcome = reconciler.reconcile(&receipt, Some(&row("call-ref-1", "acct-1", 5, 5)));
        assert_eq!(
            outcome,
            ReconciliationOutcome::ProviderUnderReported {
                local_units: 30,
                provider_units: 10
            }
        );
    }

    #[test]
    fn a_duplicate_invoice_row_is_rejected_on_its_second_submission() {
        let receipt = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        let first = reconciler.reconcile(&receipt, Some(&row("call-ref-1", "acct-1", 18, 12)));
        assert_eq!(first, ReconciliationOutcome::Reconciled);
        let second = reconciler.reconcile(&receipt, Some(&row("call-ref-1", "acct-1", 18, 12)));
        assert_eq!(
            second,
            ReconciliationOutcome::DuplicateInvoiceRow {
                provider_call_id: "call-ref-1".into()
            }
        );
    }

    #[test]
    fn a_row_naming_a_different_call_is_unknown() {
        let receipt = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        let outcome = reconciler.reconcile(&receipt, Some(&row("some-other-call", "acct-1", 30, 0)));
        assert_eq!(
            outcome,
            ReconciliationOutcome::UnknownCall {
                provider_call_id: "some-other-call".into()
            }
        );
    }

    #[test]
    fn a_row_for_the_wrong_account_is_rejected() {
        let receipt = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        let outcome = reconciler.reconcile(&receipt, Some(&row("call-ref-1", "acct-9-not-ours", 18, 12)));
        assert_eq!(
            outcome,
            ReconciliationOutcome::WrongAccount {
                expected: "acct-1".into(),
                found: "acct-9-not-ours".into()
            }
        );
    }

    #[test]
    fn reconciliation_is_uncertain_before_the_receipt_settles_or_before_a_row_arrives() {
        let mut unsettled = settled_receipt();
        unsettled.terminal_stage = ReceiptStage::Dispatched;
        let mut reconciler = BillingReconciler::new("acct-1");
        assert_eq!(
            reconciler.reconcile(&unsettled, Some(&row("call-ref-1", "acct-1", 18, 12))),
            ReconciliationOutcome::Uncertain {
                reason: "receipt has not settled yet"
            }
        );

        let settled = settled_receipt();
        let mut reconciler = BillingReconciler::new("acct-1");
        assert_eq!(
            reconciler.reconcile(&settled, None),
            ReconciliationOutcome::Uncertain {
                reason: "no provider invoice row observed yet"
            }
        );
    }
}
