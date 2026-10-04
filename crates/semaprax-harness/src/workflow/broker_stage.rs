//! Context stage backed by [`crate::context::Broker`]: compiler-verified native
//! facts plus structural items from the selected external `context.repository`
//! provider, fitted under one byte budget. Used only when the resolved profile
//! selects an external provider; otherwise the plain native stage runs.

use super::stages::*;
use crate::context::{Broker, BrokerRequest, HostExternal, SubprocessNative};
use crate::diag::HarnessResult;
use crate::profile::ResolvedLaunch;
use std::path::PathBuf;

/// Provenance label of compiler facts (also what the byte budget protects).
pub const COMPILER_VERIFIED: &str = "compiler-verified";

pub struct BrokerContext {
    id: String,
    native_only: Broker,
    full: Broker,
    calls: u32,
    note: Option<String>,
}

impl BrokerContext {
    /// `launch` is the resolved external provider; `compiler` the service executable.
    pub fn new(
        compiler: PathBuf,
        launch: ResolvedLaunch,
        env: crate::cli::Environment,
        lock_digest: String,
        config_digest: String,
        scope: Vec<String>,
    ) -> HarnessResult<Self> {
        let id = launch.provider_id.clone();
        let native = |c: &PathBuf| {
            Some(Box::new(SubprocessNative::new(c.clone()))
                as Box<dyn crate::context::NativeContextSource>)
        };
        let native_only = Broker::new(native(&compiler), None);
        let mut full = Broker::new(native(&compiler), None);
        full.add_provider(Box::new(HostExternal::new(
            launch,
            env,
            lock_digest,
            config_digest,
            scope,
        )))?;
        Ok(Self {
            id,
            native_only,
            full,
            calls: 0,
            note: None,
        })
    }
}

impl ContextStage for BrokerContext {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn collect(&mut self, req: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        let query = req
            .seed
            .map(str::to_string)
            .unwrap_or_else(|| req.query.clone());
        let mut breq = BrokerRequest::new(&query, req.max_bytes.max(2048));
        breq.symbol = req.seed.map(str::to_string);
        let native_complete = |o: &crate::context::BrokerOutput| {
            !o.native.is_empty() && o.native.iter().all(|n| n.complete)
        };
        let mut out = None;
        let mut external = false;
        match req.external {
            ExternalContext::Never => {
                out = Some(self.native_only.context(&req.project, &breq));
            }
            ExternalContext::WhenNeeded => {
                let first = self.native_only.context(&req.project, &breq);
                if matches!(&first, Ok(o) if native_complete(o)) {
                    out = Some(first);
                }
            }
            ExternalContext::Always => {}
        }
        let out = match out {
            Some(o) => o,
            None => {
                external = true;
                self.calls += 1;
                match self.full.context(&req.project, &breq) {
                    Ok(o) => Ok(o),
                    // The provider failed: compiler facts still flow, marked incomplete.
                    Err(e) => {
                        self.note = Some(format!(
                            "external context provider failed, compiler facts only: {} {}",
                            e.code, e.message
                        ));
                        external = false;
                        self.native_only.context(&req.project, &breq).map(|mut o| {
                            o.native.iter_mut().for_each(|n| n.complete = false);
                            o
                        })
                    }
                }
            }
        };
        let out = out.map_err(StageFailure::Unavailable)?;
        let mut items = Vec::new();
        for (list, native) in [(&out.native, true), (&out.external, false)] {
            for it in list {
                let label = format!("{}:{}-{}", it.path, it.span.start_line, it.span.end_line);
                let mut text = it.text.clone().unwrap_or_else(|| {
                    format!("structural item {} digest {}", it.provider_id, it.digest)
                });
                for e in &it.edges {
                    text.push_str(&format!("\nedge {} -> {}", e.relation, e.target_path));
                }
                items.push(ContextItem {
                    label,
                    provenance: if native {
                        COMPILER_VERIFIED.to_string()
                    } else {
                        // Provider output is a hint, never compiler verification.
                        format!("external:{}", it.provenance.as_str())
                    },
                    text,
                });
            }
        }
        Ok(ContextPacket {
            provider: self.id.clone(),
            items,
            complete: native_complete(&out) || external,
        })
    }

    fn calls(&self) -> u32 {
        self.calls
    }

    fn take_note(&mut self) -> Option<String> {
        self.note.take()
    }
}
