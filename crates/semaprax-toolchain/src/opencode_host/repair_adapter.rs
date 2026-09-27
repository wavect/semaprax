//! Provider-Adapter SDK projection of the explicit OpenCode process host.
//!
//! The repair CLI uses the same `OpenCodeModelHandler` and injected
//! `OpenCodeRunner` boundary as the ordinary source-live route.  This small
//! projection exists because Direct Runtime v2 consumes a streaming
//! `ProviderAdapter`, whereas the older source-live bridge consumes a
//! `ProposalSource`.  It neither parses proposals nor selects an endpoint.

use std::collections::VecDeque;
use std::path::Path;

use semaprax::digest_hex::LowerHex;
use semaprax::live_invocation::{ModelFailure, ModelInvocationOutcome};
use semaprax::provider_adapter_sdk::{
    AdapterCapabilities, AdapterEvent, AdapterInvocationCapability, AdapterModelIdentity,
    AdapterPoll, AdapterRefusal, AdapterRequest, AdapterSettlement, AdapterUsage,
    CancellationSemantics, EndpointPolicy, ProviderAdapter, StructuredOutputMode,
    TokenAccountingSource,
};
use sha2::{Digest, Sha256};

use super::{OpenCodeHostConfig, OpenCodeModelHandler, OpenCodeRunner, OPENCODE_MODEL};

const ADAPTER_IDENTITY: &str = "opencode-repair-adapter";
const ADAPTER_VERSION: &str = "1.0.0";
const PROVIDER_PROFILE: &str = "opencode-free";
const MAX_REQUEST_BYTES: usize = 65_536;
const MAX_RESPONSE_BYTES: usize = 65_536;

/// The host-selected identity used by the repair runtime binding.  It is an
/// identity commitment only; the CLI operands still supply the executable,
/// scratch directory and per-run invocation capability.
#[must_use]
pub fn source_model_identity(
    executable: &Path,
    scratch: &Path,
) -> semaprax::agent_runtime_v2::SourceModelAdapterIdentity {
    semaprax::agent_runtime_v2::SourceModelAdapterIdentity {
        provider_id: "opencode".into(),
        model_id: OPENCODE_MODEL.into(),
        adapter_identity: host_adapter_identity(executable, scratch),
        adapter_version: ADAPTER_VERSION.into(),
        provider_profile: PROVIDER_PROFILE.into(),
    }
}

pub(crate) fn source_model_identity_for_config(
    config: &OpenCodeHostConfig,
) -> semaprax::agent_runtime_v2::SourceModelAdapterIdentity {
    semaprax::agent_runtime_v2::SourceModelAdapterIdentity {
        provider_id: "opencode".into(),
        model_id: OPENCODE_MODEL.into(),
        adapter_identity: host_adapter_identity_with_binding(
            &config.executable,
            &config.sandbox,
            &config.executable_binding,
        ),
        adapter_version: ADAPTER_VERSION.into(),
        provider_profile: PROVIDER_PROFILE.into(),
    }
}

fn host_adapter_identity(executable: &Path, scratch: &Path) -> String {
    let executable_binding =
        super::executable_binding(executable).unwrap_or_else(|| "unreadable-executable".to_owned());
    host_adapter_identity_with_binding(executable, scratch, &executable_binding)
}

fn host_adapter_identity_with_binding(
    executable: &Path,
    scratch: &Path,
    executable_binding: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.opencode-repair-host-binding.v1\0");
    hasher.update(executable.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(executable_binding.as_bytes());
    hasher.update(b"\0");
    hasher.update(scratch.to_string_lossy().as_bytes());
    format!(
        "{ADAPTER_IDENTITY}:sha256:{:x}",
        LowerHex(hasher.finalize())
    )
}

fn capabilities(adapter_identity: String) -> AdapterCapabilities {
    AdapterCapabilities {
        adapter_identity,
        adapter_version: ADAPTER_VERSION.into(),
        provider_profile: PROVIDER_PROFILE.into(),
        structured_output_modes: vec![StructuredOutputMode::RawText],
        supports_streaming: true,
        token_accounting_source: TokenAccountingSource::ProviderReported,
        cancellation_semantics: CancellationSemantics::BestEffortRequestStop,
        retryable_failure_classes: Vec::new(),
        endpoint_policy: EndpointPolicy::HostInjected,
        max_request_bytes: MAX_REQUEST_BYTES,
        max_response_bytes: MAX_RESPONSE_BYTES,
        max_context_tokens: 16_384,
        max_output_tokens: 16_384,
    }
}

/// One fresh SDK adapter over one fresh OpenCode model handler.  `start` is
/// intentionally synchronous because the existing process host already owns
/// bounded nonblocking capture and verifies the OpenCode export receipt before
/// it returns raw response text.  The result is then exposed in the SDK's
/// canonical delta/completed/settled sequence.
pub struct OpenCodeRepairAdapter<R> {
    handler: OpenCodeModelHandler<R>,
    capabilities: AdapterCapabilities,
    identity: AdapterModelIdentity,
    polls: VecDeque<AdapterPoll>,
    terminal: Option<AdapterPoll>,
    started: bool,
}

impl<R> OpenCodeRepairAdapter<R> {
    #[must_use]
    pub fn new(config: OpenCodeHostConfig, runner: R) -> Self {
        let adapter_identity = source_model_identity_for_config(&config).adapter_identity;
        Self::new_with_adapter_identity(config, runner, adapter_identity)
    }

    /// Construct an adapter whose advertised identity is already bound by a
    /// narrower host capability. The caller remains responsible for deriving
    /// that identity from the ordinary OpenCode host identity; this method
    /// does not parse or grant a capability itself.
    #[must_use]
    pub(crate) fn new_with_adapter_identity(
        config: OpenCodeHostConfig,
        runner: R,
        adapter_identity: String,
    ) -> Self {
        let capabilities = capabilities(adapter_identity);
        Self {
            handler: OpenCodeModelHandler::new(config, runner),
            capabilities,
            identity: AdapterModelIdentity {
                provider_id: "opencode".into(),
                model_id: OPENCODE_MODEL.into(),
                capabilities: vec!["raw_text".into(), "streaming".into()],
            },
            polls: VecDeque::new(),
            terminal: None,
            started: false,
        }
    }

    fn set_terminal(&mut self, terminal: AdapterPoll) {
        self.terminal = Some(terminal.clone());
        self.polls.push_back(terminal);
    }
}

impl<R: OpenCodeRunner> ProviderAdapter for OpenCodeRepairAdapter<R> {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    fn model_identity(&self) -> Option<&AdapterModelIdentity> {
        Some(&self.identity)
    }

    fn start(
        &mut self,
        _capability: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        if self.started {
            return Err(AdapterRefusal(
                "OpenCode repair adapter was already started".into(),
            ));
        }
        if request.request_bytes.is_empty()
            || request.request_bytes.len() > MAX_REQUEST_BYTES
            || request.max_response_bytes == 0
            || request.max_response_bytes > MAX_RESPONSE_BYTES
        {
            return Err(AdapterRefusal(
                "OpenCode repair request exceeds bounds".into(),
            ));
        }
        let prompt = std::str::from_utf8(&request.request_bytes)
            .map_err(|_| AdapterRefusal("OpenCode repair request is not UTF-8".into()))?;
        self.started = true;
        match self
            .handler
            .invoke_prompt(prompt, request.max_response_bytes)
        {
            ModelInvocationOutcome::Settled(response_bytes) => {
                let usage = self
                    .handler
                    .last_receipt
                    .as_ref()
                    .and_then(|receipt| receipt.usage.as_ref())
                    .map(|usage| AdapterUsage {
                        tokens_in: usage.input,
                        tokens_out: usage.output,
                        cost_micros: None,
                    })
                    .unwrap_or(AdapterUsage {
                        tokens_in: None,
                        tokens_out: None,
                        cost_micros: None,
                    });
                self.polls.push_back(AdapterPoll::Event(AdapterEvent::Delta(
                    response_bytes.clone(),
                )));
                self.polls
                    .push_back(AdapterPoll::Event(AdapterEvent::Completed));
                self.set_terminal(AdapterPoll::Settled(AdapterSettlement {
                    response_bytes,
                    usage,
                }));
            }
            ModelInvocationOutcome::Failed {
                failure,
                attempted_bytes,
            } => self.set_terminal(AdapterPoll::Failed {
                failure,
                attempted_bytes,
            }),
        }
        Ok(())
    }

    fn poll(&mut self) -> AdapterPoll {
        self.polls
            .pop_front()
            .or_else(|| self.terminal.clone())
            .unwrap_or(AdapterPoll::Pending)
    }

    fn cancel(&mut self, _reason: &str) {
        self.handler.config.cancellation.cancel();
        if self.terminal.is_none() {
            self.set_terminal(AdapterPoll::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use semaprax::provider_adapter_sdk::{
        AdapterInvocationCapability, AdapterPoll, AdapterRequest, ProviderAdapter,
    };

    use super::*;
    use crate::opencode_host::{OpenCodeGrammar, OpenCodeRunnerFailure};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct RefusingRunner(Rc<Cell<u32>>);

    impl OpenCodeRunner for RefusingRunner {
        fn run(
            &mut self,
            _: &OpenCodeHostConfig,
            _: &str,
        ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
            self.0.set(self.0.get() + 1);
            Err(OpenCodeRunnerFailure::Refused)
        }

        fn export(
            &mut self,
            _: &OpenCodeHostConfig,
            _: &str,
        ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
            unreachable!("a refused run never exports")
        }
    }

    fn config() -> (OpenCodeHostConfig, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "semaprax-opencode-repair-adapter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let config = OpenCodeHostConfig::new(
            super::super::fixture_executable(),
            root.clone(),
            Duration::from_secs(1),
            OpenCodeGrammar {
                digest: "fixture-grammar".into(),
                canonical_schema: "{}".into(),
                provider_schema: String::new(),
            },
        )
        .unwrap();
        (config, root)
    }

    #[test]
    fn repair_adapter_uses_the_injected_opencode_runner_and_normalizes_refusal() {
        let (config, root) = config();
        let calls = Rc::new(Cell::new(0));
        let mut adapter = OpenCodeRepairAdapter::new(config, RefusingRunner(Rc::clone(&calls)));
        assert_eq!(
            adapter.model_identity().unwrap().model_id,
            OPENCODE_MODEL,
            "the Adapter SDK selection is the fixed OpenCode profile"
        );
        adapter
            .start(
                &AdapterInvocationCapability::grant("test OpenCode repair authority"),
                &AdapterRequest {
                    request_bytes: b"{}".to_vec(),
                    max_response_bytes: 64,
                },
            )
            .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(
            adapter.poll(),
            AdapterPoll::Failed {
                failure: ModelFailure::Refused,
                attempted_bytes: 0,
            }
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_repair_adapter_request_refuses_without_runner_dispatch() {
        let (config, root) = config();
        let calls = Rc::new(Cell::new(0));
        let mut adapter = OpenCodeRepairAdapter::new(config, RefusingRunner(Rc::clone(&calls)));
        assert!(adapter
            .start(
                &AdapterInvocationCapability::grant("test OpenCode repair authority"),
                &AdapterRequest {
                    request_bytes: vec![0xff],
                    max_response_bytes: 64,
                },
            )
            .is_err());
        assert_eq!(calls.get(), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn repair_adapter_identity_binds_the_selected_host_paths() {
        let (config, root) = config();
        let canonical_root = root.canonicalize().unwrap();
        let executable = config.executable.clone();
        let expected = source_model_identity(&executable, &canonical_root).adapter_identity;
        let adapter = OpenCodeRepairAdapter::new(config, RefusingRunner(Rc::new(Cell::new(0))));
        assert_eq!(adapter.capabilities().adapter_identity, expected);
        assert_ne!(
            source_model_identity(Path::new("/usr/bin/false"), &canonical_root).adapter_identity,
            expected
        );
        assert_ne!(
            source_model_identity(&executable, &root.join("other")).adapter_identity,
            expected
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
