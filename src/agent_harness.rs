//! Replayable AgentDefinition-to-payment execution composition.
//!
//! The compiler binds one admitted AgentDefinition/AgentGraph pair to one
//! canonical Economic Agent Policy. Execution still receives model/tool and
//! journal/chain/approval/custody authority only through separate injected
//! hosts. Model output remains untrusted Payment Intent data.

use sha2::{Digest, Sha256};

use crate::agent_definition::{
    compile_agent_definition, verify_agent_graph_bundle, CompiledAgentDefinition,
};
use crate::agent_runtime::{Agent, AgentCancellation, AgentHost, AgentRun};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::economic_agent::{
    economic_agent_policy_digest, EconomicAgent, EconomicAgentHost, EconomicRun,
};

const GRAPH_SCHEMA: &str = "semaprax.agent-payment-graph.v1";
const GRAPH_DOMAIN: &[u8] = b"semaprax.agent-payment-graph.digest.v1\0";
const MAX_GRAPH_BYTES: usize = 65_536;

const NONCLAIMS: [&str; 8] = [
    "no_model_output_payment_authority",
    "no_model_self_approval_or_policy_expansion",
    "no_builtin_provider_network_journal_approval_custody_or_broadcast_authority",
    "no_seed_private_key_credential_or_signing_material_input",
    "no_mainnet_authority",
    "no_exactly_once_signing_broadcast_or_payment",
    "no_language_level_transition_execution",
    "runtime_v1_and_economic_agent_v1_remain_the_execution_kernels",
];

/// Compiler-owned graph binding an AgentDefinition to an Economic Agent Policy.
pub struct AgentPaymentGraph {
    source: String,
    digest: String,
}

impl AgentPaymentGraph {
    /// Returns canonical compact JSON with exactly one terminal LF.
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated graph digest.
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Immutable compilation product for the complete runtime/payment bridge.
pub struct CompiledAgentPaymentGraph {
    agent: CompiledAgentDefinition,
    economic_policy: String,
    economic_policy_digest: String,
    graph: AgentPaymentGraph,
}

impl CompiledAgentPaymentGraph {
    /// Returns the admitted language-native agent compilation product.
    pub fn agent(&self) -> &CompiledAgentDefinition {
        &self.agent
    }

    /// Returns the exact canonical policy bytes bound by the payment graph.
    pub fn economic_policy(&self) -> &str {
        &self.economic_policy
    }

    /// Returns the Economic Agent Policy digest bound by the payment graph.
    pub fn economic_policy_digest(&self) -> &str {
        &self.economic_policy_digest
    }

    /// Returns the compiler-owned payment graph.
    pub fn graph(&self) -> &AgentPaymentGraph {
        &self.graph
    }

    /// Instantiates both execution kernels with disjoint caller-owned hosts.
    pub fn instantiate<AH: AgentHost, EH: EconomicAgentHost>(
        &self,
        agent_host: AH,
        economic_host: EH,
        cancellation: AgentCancellation,
    ) -> Result<AgentPaymentHarness<AH, EH>, Vec<Diagnostic>> {
        let runtime = self.agent.instantiate(agent_host, cancellation.clone())?;
        let economic = EconomicAgent::new(&self.economic_policy, economic_host, cancellation)?;
        Ok(AgentPaymentHarness {
            agent_definition_digest: self.agent.definition().digest().to_owned(),
            agent_graph_digest: self.agent.graph().digest().to_owned(),
            payment_graph_digest: self.graph.digest().to_owned(),
            runtime,
            economic,
        })
    }
}

/// One live runtime/payment composition over separately injected authorities.
pub struct AgentPaymentHarness<AH: AgentHost, EH: EconomicAgentHost> {
    agent_definition_digest: String,
    agent_graph_digest: String,
    payment_graph_digest: String,
    runtime: Agent<AH>,
    economic: EconomicAgent<EH>,
}

impl<AH: AgentHost, EH: EconomicAgentHost> AgentPaymentHarness<AH, EH> {
    /// Runs the model/tool state machine, then admits its completed final
    /// message as a Payment Intent and executes the separately authorized
    /// Economic Agent state machine.
    pub fn run_payment(&mut self, task: &str) -> Result<AgentPaymentRun, Vec<Diagnostic>> {
        let agent = self.runtime.run(task)?;
        let economic = self.economic.execute(&agent)?;
        Ok(AgentPaymentRun {
            agent_definition_digest: self.agent_definition_digest.clone(),
            agent_graph_digest: self.agent_graph_digest.clone(),
            payment_graph_digest: self.payment_graph_digest.clone(),
            agent,
            economic,
        })
    }
}

/// Evidence-bearing result from both chained state machines.
pub struct AgentPaymentRun {
    agent_definition_digest: String,
    agent_graph_digest: String,
    payment_graph_digest: String,
    agent: AgentRun,
    economic: EconomicRun,
}

impl AgentPaymentRun {
    pub fn agent_definition_digest(&self) -> &str {
        &self.agent_definition_digest
    }

    pub fn agent_graph_digest(&self) -> &str {
        &self.agent_graph_digest
    }

    pub fn payment_graph_digest(&self) -> &str {
        &self.payment_graph_digest
    }

    pub fn agent_run(&self) -> &AgentRun {
        &self.agent
    }

    pub fn economic_run(&self) -> &EconomicRun {
        &self.economic
    }
}

/// Compiles and binds one canonical AgentDefinition and Economic Policy.
pub fn compile_agent_payment_graph(
    agent_definition: &str,
    economic_policy: &str,
) -> Result<CompiledAgentPaymentGraph, Vec<Diagnostic>> {
    let agent = compile_agent_definition(agent_definition)?;
    let economic_policy_digest = economic_agent_policy_digest(economic_policy)?;
    let graph_source = render_graph(&agent, &economic_policy_digest);
    if graph_source.len() > MAX_GRAPH_BYTES {
        return Err(vec![graph_mismatch()]);
    }
    let graph = AgentPaymentGraph {
        digest: digest(GRAPH_DOMAIN, graph_source.as_bytes()),
        source: graph_source,
    };
    Ok(CompiledAgentPaymentGraph {
        agent,
        economic_policy: economic_policy.to_owned(),
        economic_policy_digest,
        graph,
    })
}

/// Independently recompiles every source-owned input and exact-compares both
/// compiler-owned graph projections.
pub fn verify_agent_payment_graph_bundle(
    agent_definition: &str,
    economic_policy: &str,
    agent_graph: &str,
    payment_graph: &str,
) -> Result<(), Vec<Diagnostic>> {
    if payment_graph.len() > MAX_GRAPH_BYTES {
        return Err(vec![graph_mismatch()]);
    }
    let compiled = compile_agent_payment_graph(agent_definition, economic_policy)?;
    verify_agent_graph_bundle(
        agent_definition,
        compiled.agent().runtime_v1_profile(),
        agent_graph,
    )?;
    if compiled.graph().canonical_json().as_bytes() != payment_graph.as_bytes() {
        return Err(vec![graph_mismatch()]);
    }
    Ok(())
}

fn render_graph(agent: &CompiledAgentDefinition, policy_digest: &str) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_definition_digest\":{},\"agent_graph_digest\":{},\"economic_policy_digest\":{},",
        quote_json(GRAPH_SCHEMA),
        quote_json(agent.definition().digest()),
        quote_json(agent.graph().digest()),
        quote_json(policy_digest),
    );
    output.push_str(
        "\"proposal_schema\":\"semaprax.economic-agent-payment-intent.v1\",\"flow\":[\"agent_runtime_run\",\"completed_final_message\",\"payment_intent_admission\",\"policy_reservation\",\"chain_snapshot\",\"simulation\",\"separate_approval\",\"custody_sign\",\"broadcast_once\",\"reconciliation\"],",
    );
    output.push_str(
        "\"authority_boundary\":{\"model_output\":\"untrusted_data\",\"payment_policy\":\"source_bound\",\"approval\":\"injected\",\"custody\":\"injected_opaque\",\"chain_and_broadcast\":\"injected_test_network_only\"},\"evidence_chain\":{\"economic_evidence_binds_agent_evidence\":true,\"independent_replay_required\":true},\"nonclaims\":[",
    );
    for (index, item) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&quote_json(item));
    }
    output.push_str("]}\n");
    output
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn graph_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G505",
        "Agent Payment Graph is not the exact replay of its AgentDefinition and Economic Policy",
    )
}
