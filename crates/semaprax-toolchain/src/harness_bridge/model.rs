//! Logical model (endpoint catalog binding) -> `OpenAiResponsesAdapter`.

use std::collections::BTreeMap;
use std::time::Duration;

use semaprax::model_budget_policy::AdapterFactoryRefusal;
use semaprax::model_budget_policy::ProviderAdapterFactory;
use semaprax::provider_adapter_sdk::vendor::OpenAiResponsesAdapter;
use semaprax::provider_adapter_sdk::ProviderAdapter;
use semaprax_harness::diag::{HarnessDiagnostic, HarnessResult};
use semaprax_harness::endpoint::catalog::LogicalModel;
use semaprax_harness::endpoint::Protocol;

use super::transport::{LoopbackTransport, Secret};

/// Everything needed to construct fresh adapters for one logical model.
#[derive(Clone, Debug)]
pub struct ModelBinding {
    pub logical_id: String,
    pub upstream_model: String,
    endpoint_url: String,
    secret: Option<Secret>,
    max_output_tokens: u64,
    idle_timeout: Duration,
}

impl ModelBinding {
    /// Only a `responses` binding is bridged; other protocols are refused
    /// rather than silently downgraded (`SPX-HPL032`). The endpoint must be a
    /// loopback `http://` origin (`SPX-HPL002`).
    pub fn from_logical(
        model: &LogicalModel,
        endpoint_url: &str,
        secret: Option<Secret>,
        max_output_tokens: u64,
    ) -> HarnessResult<Self> {
        if model.protocol != Protocol::Responses {
            return Err(HarnessDiagnostic::new(
                "SPX-HPL032",
                format!(
                    "logical model `{}` is bound to {}; the bridge admits only responses",
                    model.id,
                    model.protocol.as_str()
                ),
            ));
        }
        // Validate the origin eagerly so a bad binding never reaches a factory.
        LoopbackTransport::new(endpoint_url, None)?;
        Ok(Self {
            logical_id: model.id.clone(),
            upstream_model: model.upstream_model.clone(),
            endpoint_url: endpoint_url.to_owned(),
            secret,
            max_output_tokens,
            idle_timeout: Duration::from_secs(120),
        })
    }

    #[must_use]
    pub fn with_idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    pub fn build(&self) -> HarnessResult<OpenAiResponsesAdapter> {
        let transport = LoopbackTransport::new(&self.endpoint_url, self.secret.clone())?
            .with_idle_timeout(self.idle_timeout);
        Ok(OpenAiResponsesAdapter::new(
            self.upstream_model.clone(),
            self.max_output_tokens,
            Box::new(transport),
        ))
    }
}

/// `ProviderAdapterFactory` keyed by logical model id (the plan's slot id).
#[derive(Default)]
pub struct LogicalModelFactory {
    bindings: BTreeMap<String, ModelBinding>,
}

impl LogicalModelFactory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, binding: ModelBinding) {
        self.bindings.insert(binding.logical_id.clone(), binding);
    }
}

impl ProviderAdapterFactory for LogicalModelFactory {
    fn create(
        &mut self,
        provider_id: &str,
    ) -> Result<Box<dyn ProviderAdapter>, AdapterFactoryRefusal> {
        let binding = self.bindings.get(provider_id).ok_or_else(|| {
            AdapterFactoryRefusal(format!(
                "no bridged binding for logical model `{provider_id}`"
            ))
        })?;
        let adapter = binding
            .build()
            .map_err(|error| AdapterFactoryRefusal(error.to_string()))?;
        Ok(Box::new(adapter))
    }
}
