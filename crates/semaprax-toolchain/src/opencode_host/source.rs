//! #111 source-feedback bridge for explicit OpenCode transport.
//!
//! `CompiledIterativeLifecycle::run_live` remains the sole source proposal
//! decoder and retry owner. This bridge serializes its actual checked context
//! with the lifecycle's existing canonical retained-value encoding, then asks
//! the configured host transport for raw proposal text.

use semaprax::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use semaprax::agent_lifecycle::canonical_retained_value_json;
use semaprax::diagnostic::{quote_json, Diagnostic};
use semaprax::live_invocation::{ModelFailure, ModelInvocationOutcome, ModelInvokeCapability};

use super::{OpenCodeGrammar, OpenCodeModelHandler, OpenCodeRunner};

const SOURCE_CONTEXT_SCHEMA: &str = "semaprax.opencode-source-proposal-context.v1";
const MAX_CONTEXT_BYTES: usize = 65_536;

/// Uses the actual #111 state, observation and feedback values as canonical,
/// read-only provider context. It holds the explicit model capability; source
/// text and a model response cannot create that authority.
pub struct OpenCodeProposalSource<'a, R> {
    handler: &'a mut OpenCodeModelHandler<R>,
    capability: &'a ModelInvokeCapability,
    deployment_binding: String,
    grammar: OpenCodeGrammar,
    max_response_bytes: usize,
}

impl<'a, R> OpenCodeProposalSource<'a, R> {
    pub fn new(
        handler: &'a mut OpenCodeModelHandler<R>,
        capability: &'a ModelInvokeCapability,
        deployment_binding: String,
        grammar: OpenCodeGrammar,
        max_response_bytes: usize,
    ) -> Result<Self, Diagnostic> {
        if max_response_bytes == 0 {
            return Err(Diagnostic::io("SPX-I239", "OpenCode source response limit must be positive"));
        }
        Ok(Self { handler, capability, deployment_binding, grammar, max_response_bytes })
    }
}

fn context_bytes(request: &ProposalRequest<'_>) -> Result<Vec<u8>, Diagnostic> {
    let previous_effect = request.previous_effect.map(|bytes| quote_json(&hex(bytes))).unwrap_or_else(|| "null".into());
    let previous_rejection = request.previous_rejection.map(quote_json).unwrap_or_else(|| "null".into());
    let body = format!(
        "{{\"schema\":{},\"turn\":{},\"attempt\":{},\"source_revision\":{},\"proposal_schema_digest\":{},\"task_objective\":{},\"task_budget\":{},\"state\":{},\"observation\":{},\"previous_effect\":{},\"previous_rejection\":{},\"remaining_iterations\":{}}}",
        quote_json(SOURCE_CONTEXT_SCHEMA), request.turn, request.attempt,
        quote_json(request.source_revision), quote_json(request.proposal_schema_digest),
        quote_json(&hex(&request.task.objective)), request.task.budget,
        canonical_retained_value_json(request.state), canonical_retained_value_json(request.observation),
        previous_effect, previous_rejection, request.remaining_iterations,
    );
    if body.len() > MAX_CONTEXT_BYTES { return Err(Diagnostic::io("SPX-I239", "OpenCode source context exceeds its byte limit")); }
    Ok(body.into_bytes())
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|byte| format!("{byte:02x}")).collect() }

fn transport_error(failure: ModelFailure, attempted_bytes: usize) -> Diagnostic {
    Diagnostic::io("SPX-I239", format!("OpenCode source transport failed: {}; attempted response bytes: {attempted_bytes}", failure.as_str()))
}

impl<R: OpenCodeRunner> ProposalSource for OpenCodeProposalSource<'_, R> {
    fn propose(&mut self, context: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        if context.proposal_schema_digest != self.grammar.digest {
            return Err(vec![Diagnostic::io("SPX-I239", "OpenCode grammar does not bind the source proposal schema")]);
        }
        let context = context_bytes(&context).map_err(|error| vec![error])?;
        let prompt = format!(
            "SEMAPRAX source proposal v1\nsource_context={}\ndeployment={}\nproposal_schema_digest={}\ncanonical_agent_proposal_schema={}\nReturn one canonical semaprax.agent-proposal.v1 document for the supplied proposal schema.\n",
            String::from_utf8(context).expect("canonical source context is UTF-8"), self.deployment_binding,
            self.grammar.digest, self.grammar.canonical_schema,
        );
        let _capability_reason = self.capability.reason();
        match self.handler.invoke_prompt(&prompt, self.max_response_bytes) {
            ModelInvocationOutcome::Settled(bytes) => String::from_utf8(bytes).map_err(|_| vec![Diagnostic::io("SPX-I239", "OpenCode settled non-UTF-8 proposal bytes")]),
            ModelInvocationOutcome::Failed { failure, attempted_bytes } => Err(vec![transport_error(failure, attempted_bytes)]),
        }
    }
}

impl super::OpenCodeGrammar {
    /// Carries the canonical Agent Proposal Schema v1 guidance for #111; it
    /// intentionally does not label it an InteractionValue envelope.
    pub fn from_proposal(schema: &semaprax::agent_proposal::CompiledAgentProposalSchema) -> Result<Self, String> {
        let canonical_schema = schema.schema().canonical_json().to_owned();
        if canonical_schema.len() > MAX_CONTEXT_BYTES { return Err("OpenCode proposal grammar exceeds its host byte budget".into()); }
        Ok(Self { digest: schema.schema().digest().to_owned(), canonical_schema, provider_schema: String::new() })
    }
}
