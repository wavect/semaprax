//! Explicit #112 OpenCode source-feedback embedding example.
//!
//! No argument performs the same one-turn checked source lifecycle with a
//! fixed offline proposal. `--live --opencode ABS --scratch ABS` replaces only
//! that proposal source with the free configured OpenCode profile.
use std::path::PathBuf;
use std::time::Duration;

use semaprax::agent_lifecycle::iterative::{
    compile_agent_lifecycle_v2,
    driver::{ProposalRequest, ProposalSource},
    IterativeBudget, IterativeStatus,
};
use semaprax::agent_lifecycle::{AgentReadOperation, AuthorizedRequest, LifecycleTask};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::diagnostic::Diagnostic;
use semaprax::live_invocation::ModelInvokeCapability;
use semaprax_toolchain::opencode_host::source::OpenCodeProposalSource;
use semaprax_toolchain::opencode_host::{
    OpenCodeGrammar, OpenCodeHostConfig, OpenCodeModelHandler, OpenCodeRunner,
    OpenCodeRunnerFailure, ProcessOpenCodeRunner,
};

#[path = "fixtures/opencode_source_fixture.rs"]
mod fixtures;
use fixtures::{DEFINITION, SOURCE};

struct Read;
impl AgentReadOperation for Read {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        Some(b"observed".to_vec())
    }
}

struct OfflineProposal {
    proposal: String,
}
impl ProposalSource for OfflineProposal {
    fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        Ok(self.proposal.clone())
    }
}

fn proposal(digest: &str) -> String {
    format!("{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":{:?},\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"1\",\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n", digest)
}

// The smoke owns this explicit archive and permits only one provider run.
struct ArchivedRunner {
    directory: PathBuf,
    called: bool,
}
impl OpenCodeRunner for ArchivedRunner {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        if self.called {
            return Err(OpenCodeRunnerFailure::Refused);
        }
        self.called = true;
        std::fs::write(self.directory.join("prompt.txt"), prompt)
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        let bytes = ProcessOpenCodeRunner.run(config, prompt)?;
        std::fs::write(self.directory.join("events.jsonl"), &bytes)
            .map_err(|_| OpenCodeRunnerFailure::Provider)?;
        Ok(bytes)
    }
    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        session: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let bytes = ProcessOpenCodeRunner.export(config, session)?;
        std::fs::write(self.directory.join("session.json"), &bytes)
            .map_err(|_| OpenCodeRunnerFailure::Provider)?;
        Ok(bytes)
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let compiled = compile_agent_lifecycle_v2(
        SOURCE,
        "opencode-live-smoke.spx",
        DEFINITION,
        "fixture.agent.type.step",
    )
    .map_err(|e| format!("{e:?}"))?;
    let grammar =
        OpenCodeGrammar::from_proposal(compiled.proposal_schema()).map_err(|e| e.to_string())?;
    let task = LifecycleTask {
        objective: format!(
            "Reply with exactly this canonical proposal, without markdown or explanation: {}",
            proposal(compiled.proposal_schema().schema().digest())
        )
        .into_bytes(),
        budget: 1,
    };
    let mut read = Read;
    let cancellation = AgentCancellation::new();
    let budget = IterativeBudget {
        max_iterations: 1,
        ..IterativeBudget::default()
    };
    if args.is_empty() {
        let mut source = OfflineProposal {
            proposal: proposal(compiled.proposal_schema().schema().digest()),
        };
        let run = compiled
            .run_live(&task, &mut source, &mut read, budget, &cancellation)
            .map_err(|e| format!("{e:?}"))?;
        println!("offline status={:?}", run.status());
        return if run.status() == IterativeStatus::Complete {
            Ok(())
        } else {
            Err("offline lifecycle did not complete".into())
        };
    }
    if args.len() != 7
        || args[0] != "--live"
        || args[1] != "--opencode"
        || args[3] != "--scratch"
        || args[5] != "--evidence"
    {
        return Err(
            "usage: --live --opencode ABSOLUTE_EXECUTABLE --scratch EMPTY_ABSOLUTE_DIR --evidence NEW_ABSOLUTE_DIR".into(),
        );
    }
    let config = OpenCodeHostConfig::new(
        PathBuf::from(&args[2]),
        PathBuf::from(&args[4]),
        Duration::from_secs(30),
        grammar.clone(),
    )
    .map_err(|e| e.to_string())?;
    let directory = PathBuf::from(&args[6]);
    if !directory.is_absolute() {
        return Err("evidence path must be absolute".into());
    }
    std::fs::create_dir(&directory).map_err(|_| "evidence directory must be new".to_string())?;
    let mut handler = OpenCodeModelHandler::new(
        config,
        ArchivedRunner {
            directory,
            called: false,
        },
    );
    let capability = ModelInvokeCapability::grant("opencode_live_smoke explicit --live");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "opencode-live-smoke.v1".into(),
        grammar,
        65_536,
    )
    .map_err(|e| format!("{e:?}"))?;
    let run = compiled
        .run_live(&task, &mut source, &mut read, budget, &cancellation)
        .map_err(|e| format!("{e:?}"))?;
    drop(source);
    println!("live status={:?}", run.status());
    if let Some(receipt) = &handler.last_receipt {
        println!(
            "provider=opencode model={} session={} reported_total_tokens={:?}",
            semaprax_toolchain::opencode_host::OPENCODE_MODEL,
            receipt.session_id,
            receipt.usage_total
        );
    }
    if run.status() == IterativeStatus::Complete {
        Ok(())
    } else {
        Err("live lifecycle did not complete".into())
    }
}
