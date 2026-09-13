//! #111 source-feedback bridge for explicit OpenCode transport.
//!
//! `CompiledIterativeLifecycle::run_live` remains the sole source proposal
//! decoder and retry owner. This bridge serializes its actual checked context
//! with the lifecycle's existing canonical retained-value encoding, then asks
//! the configured host transport for raw proposal text.

use semaprax::agent_lifecycle::canonical_retained_value_json;
use semaprax::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use semaprax::diagnostic::{quote_json, Diagnostic};
use semaprax::live_invocation::{
    ModelInvocationOutcome, ModelInvocationRequest, ModelInvokeCapability,
};

use super::accounting::{
    OpenCodeAccountingRefusal, OpenCodeSourceAccounting, OpenCodeSourceAttemptReceipt,
};
use super::{OpenCodeGrammar, OpenCodeModelHandler, OpenCodeRunner};

const SOURCE_CONTEXT_SCHEMA: &str = "semaprax.opencode-source-proposal-context.v1";
const MAX_CONTEXT_BYTES: usize = 65_536;

#[path = "source_checkpoint.rs"]
mod source_checkpoint;

/// Uses the actual #111 state, observation and feedback values as canonical,
/// read-only provider context. It holds the explicit model capability; source
/// text and a model response cannot create that authority.
pub struct OpenCodeProposalSource<'a, R> {
    handler: &'a mut OpenCodeModelHandler<R>,
    capability: &'a ModelInvokeCapability,
    deployment_binding: String,
    grammar: OpenCodeGrammar,
    max_response_bytes: usize,
    accounting: OpenCodeSourceAccounting<'a>,
}

impl<'a, R> OpenCodeProposalSource<'a, R> {
    // Preserve the existing public constructor diagnostic type.
    #[allow(clippy::result_large_err)]
    pub fn new(
        handler: &'a mut OpenCodeModelHandler<R>,
        capability: &'a ModelInvokeCapability,
        deployment_binding: String,
        grammar: OpenCodeGrammar,
        max_response_bytes: usize,
        accounting: OpenCodeSourceAccounting<'a>,
    ) -> Result<Self, Diagnostic> {
        if max_response_bytes == 0 {
            return Err(Diagnostic::io(
                "SPX-I239",
                "OpenCode source response limit must be positive",
            ));
        }
        Ok(Self {
            handler,
            capability,
            deployment_binding,
            grammar,
            max_response_bytes,
            accounting,
        })
    }

    /// Redacted host-only attempt observations. This in-memory slice is not
    /// a durable journal and cannot support recovery or migration.
    #[must_use]
    pub fn receipts(&self) -> &[OpenCodeSourceAttemptReceipt] {
        self.accounting.receipts()
    }

    fn prepare_request(
        &self,
        context: &ProposalRequest<'_>,
    ) -> Result<(String, ModelInvocationRequest), Box<Diagnostic>> {
        if context.proposal_schema_digest != self.grammar.digest {
            return Err(Diagnostic::io(
                "SPX-I239",
                "OpenCode grammar does not bind the source proposal schema",
            )
            .into());
        }
        let encoded_context = context_bytes(context)?;
        let prompt = format!(
            "SEMAPRAX source proposal v1\nsource_context={}\ndeployment={}\nproposal_schema_digest={}\ncanonical_agent_proposal_schema={}\nReturn one canonical semaprax.agent-proposal.v1 document for the supplied proposal schema. End the document with exactly one literal LF (U+000A); a missing LF is rejected by the compiler.\n",
            String::from_utf8(encoded_context).expect("canonical source context is UTF-8"), self.deployment_binding,
            self.grammar.digest, self.grammar.canonical_schema,
        );
        if prompt.len() > super::MAX_PROMPT_BYTES {
            return Err(Diagnostic::io(
                "SPX-I239",
                "OpenCode source prompt exceeds its byte limit",
            )
            .into());
        }
        let turn = u32::try_from(context.turn).map_err(|_| {
            Diagnostic::io(
                "SPX-I239",
                "OpenCode source turn exceeds the accounting bound",
            )
        })?;
        Ok((
            prompt.clone(),
            ModelInvocationRequest {
                turn,
                task: context.task.objective.clone(),
                observation: prompt.into_bytes(),
                proposal_grammar_digest: self.grammar.digest.clone(),
                deployment_binding: self.deployment_binding.clone(),
                max_response_bytes: self.max_response_bytes,
                effective_budget: self.accounting.reservation_units(),
            },
        ))
    }
}

fn context_bytes(request: &ProposalRequest<'_>) -> Result<Vec<u8>, Box<Diagnostic>> {
    let previous_effect = request
        .previous_effect
        .map(|bytes| quote_json(&hex(bytes)))
        .unwrap_or_else(|| "null".into());
    let previous_rejection = request
        .previous_rejection
        .map(quote_json)
        .unwrap_or_else(|| "null".into());
    let body = format!(
        "{{\"schema\":{},\"turn\":{},\"attempt\":{},\"source_revision\":{},\"proposal_schema_digest\":{},\"task_objective\":{},\"task_budget\":{},\"state\":{},\"observation\":{},\"previous_effect\":{},\"previous_rejection\":{},\"remaining_iterations\":{}}}",
        quote_json(SOURCE_CONTEXT_SCHEMA), request.turn, request.attempt,
        quote_json(request.source_revision), quote_json(request.proposal_schema_digest),
        quote_json(&hex(&request.task.objective)), request.task.budget,
        canonical_retained_value_json(request.state), canonical_retained_value_json(request.observation),
        previous_effect, previous_rejection, request.remaining_iterations,
    );
    if body.len() > MAX_CONTEXT_BYTES {
        return Err(
            Diagnostic::io("SPX-I239", "OpenCode source context exceeds its byte limit").into(),
        );
    }
    Ok(body.into_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl<R: OpenCodeRunner> ProposalSource for OpenCodeProposalSource<'_, R> {
    fn check_deadline(&self) -> Result<(), Vec<Diagnostic>> {
        self.accounting
            .check_deadline()
            .map_err(|refusal| vec![accounting_diagnostic(refusal)])
    }

    fn propose(&mut self, context: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        let (prompt, request) = self
            .prepare_request(&context)
            .map_err(|error| vec![*error])?;
        if self.handler.runner.cancelled(&self.handler.config) {
            return Err(vec![Diagnostic::io(
                "SPX-I239",
                "OpenCode source invocation was cancelled before dispatch",
            )]);
        }
        self.accounting
            .reserve(request, context.attempt, prompt.len())
            .map_err(|refusal| vec![accounting_diagnostic(refusal)])?;
        let _capability_reason = self.capability.reason();
        let outcome = self.handler.invoke_prompt(&prompt, self.max_response_bytes);
        let reported_usage = self
            .handler
            .last_receipt
            .as_ref()
            .and_then(|receipt| receipt.usage.clone());
        self.accounting
            .finish(&outcome, reported_usage)
            .map_err(|refusal| vec![accounting_diagnostic(refusal)])?;
        match outcome {
            ModelInvocationOutcome::Settled(bytes) => String::from_utf8(bytes).map_err(|_| {
                vec![Diagnostic::io(
                    "SPX-I239",
                    "OpenCode settled non-UTF-8 proposal bytes",
                )]
            }),
            ModelInvocationOutcome::Failed {
                failure,
                attempted_bytes,
            } => Err(vec![Diagnostic::io("SPX-I239", format!("OpenCode source transport failed: {}; provider category: {:?}; attempted response bytes: {attempted_bytes}", failure.as_str(), self.handler.last_provider_failure))]),
        }
    }
}

fn accounting_diagnostic(refusal: OpenCodeAccountingRefusal) -> Diagnostic {
    let reason = match refusal {
        OpenCodeAccountingRefusal::BudgetExhausted => "budget_exhausted",
        OpenCodeAccountingRefusal::DeadlineExceeded => "deadline_exceeded",
        OpenCodeAccountingRefusal::InvalidBudget => "invalid_budget",
        OpenCodeAccountingRefusal::PolicyRefused => "budget_policy_refused",
        OpenCodeAccountingRefusal::AttemptCapacity => "attempt_capacity",
        OpenCodeAccountingRefusal::PendingAttempt
        | OpenCodeAccountingRefusal::UnreservedAttempt => "accounting_state",
    };
    Diagnostic::io(
        "SPX-I239",
        format!("OpenCode source accounting refused: {reason}"),
    )
}

impl super::OpenCodeGrammar {
    /// Carries the canonical Agent Proposal Schema v1 guidance for #111; it
    /// intentionally does not label it an InteractionValue envelope.
    pub fn from_proposal(
        schema: &semaprax::agent_proposal::CompiledAgentProposalSchema,
    ) -> Result<Self, String> {
        let canonical_schema = schema.schema().canonical_json().to_owned();
        if canonical_schema.len() > MAX_CONTEXT_BYTES {
            return Err("OpenCode proposal grammar exceeds its host byte budget".into());
        }
        Ok(Self {
            digest: schema.schema().digest().to_owned(),
            canonical_schema,
            provider_schema: String::new(),
        })
    }
}
