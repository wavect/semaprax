//! Exercise the real public selectors in the existing model/effect corpus.
//! The explicitly optimized native leg remains a separate private comparison.
use super::*;
use crate::agent_lifecycle::authorization::{target_protocol::TargetHostHandler, StageBackend};
use crate::agent_lifecycle::iterative::driver::ProposalSource;

pub(super) fn held_wasm() -> WasmTargetHost {
    std::env::var_os("SEMAPRAX_TEST_WASM_STAGE_NODE")
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain(
            [
                "/usr/bin/node",
                "/usr/local/bin/node",
                "/opt/homebrew/bin/node",
            ]
            .map(std::path::PathBuf::from),
        )
        .find_map(|path| WasmTargetHost::open(path).ok())
        .expect("public target corpus requires an explicitly held Node runtime")
}
impl CompiledTypedEffects {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::agent_lifecycle) fn run_public_target_fixture(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TargetHostHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        backend: StageBackend<'_>,
    ) -> Result<TargetEffectRun, Vec<Diagnostic>> {
        let native;
        let wasm;
        let selected = match backend {
            StageBackend::Interpreter => TargetStageBackend::Interpreter,
            StageBackend::Native { host } => {
                native = NativeTargetHost::open(host.compiler_path()).map_err(|e| vec![e])?;
                assert_eq!(native.identity(), host.identity());
                TargetStageBackend::Native(&native)
            }
            StageBackend::Wasm { source } => {
                assert_eq!(self.target_source.as_deref(), Some(source));
                wasm = held_wasm();
                TargetStageBackend::CoreWasmHeld(&wasm)
            }
            StageBackend::WasmHeld { host, source } => {
                assert_eq!(self.target_source.as_deref(), Some(source));
                wasm = held_wasm();
                assert_eq!(wasm.identity(), host.identity());
                TargetStageBackend::CoreWasmHeld(&wasm)
            }
            StageBackend::NativeAtOptimization { .. } => {
                return self.run_target_live_on(
                    task,
                    source,
                    handler,
                    stages,
                    effects,
                    cancellation,
                    backend,
                );
            }
            StageBackend::Metered { .. } => {
                panic!("public selector fixture cannot accept a private wrapper")
            }
        };
        self.run_target_live_with_backend(
            task,
            source,
            handler,
            stages,
            effects,
            cancellation,
            selected,
        )
    }
}
