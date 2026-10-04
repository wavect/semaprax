//! Bridges between the private harness host (`semaprax-harness`) and the
//! compiler crate's model/policy SDK. No new protocol or policy logic lives
//! here: plans convert into `ProviderPolicy`, endpoints become a loopback
//! `HostHttpStreamTransport`, and logical models become the existing
//! `OpenAiResponsesAdapter`. See `docs/HARNESS-TOOLCHAIN-V1.md`.

pub mod model;
pub mod policy;
pub mod transport;
