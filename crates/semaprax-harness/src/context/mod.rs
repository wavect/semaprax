//! Native-first context broker with revision-safe caches (HP-05).
//!
//! The compiler supplies mandatory `.spx` facts; one external repository
//! provider (or explicitly federated disjoint scopes) supplies surrounding
//! discovery as untrusted hints. Diagnostics are `SPX-HPE001..`: 001 mandatory
//! native facts exceed the budget, 002 no compiler, 010 snapshot, 020-023
//! compiler facts, 030-032 external provider, 040-041 provider selection and
//! federation, 050 provenance promotion refused, 060-061 index report mismatch.

pub mod broker;
pub mod budget;
pub mod cache;
pub mod cli;
pub mod external;
pub mod identity;
pub mod index_adoption;
pub mod item;
pub mod native;

pub use broker::{Broker, BrokerOutput, BrokerRequest};
pub use cache::{CacheConfig, CacheKey, ResultCache};
pub use external::{
    ExternalQuery, ExternalResponse, ExternalSource, HostExternal, ProviderIdentity,
};
pub use item::{ContextItem, Link, Tier};
pub use native::{NativeContextSource, NativeFacts, NativeQuery, SubprocessNative};

pub fn cli_context(args: &[String], env: &crate::cli::Environment) -> crate::cli::Outcome {
    cli::run(args, env)
}
