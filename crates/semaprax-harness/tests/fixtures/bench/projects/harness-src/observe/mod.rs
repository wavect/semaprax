//! Stage attribution over token and attempt observations (HP-15).
//!
//! Metadata-only events (`semaprax.harness-observation.v1`), exact
//! measurement-boundary token counts, observer isolation, transformation
//! lineage attribution and a coverage-honest report. See
//! `docs/HARNESS-OBSERVATION-V1.md`.

pub mod aggregate;
pub mod cli;
pub mod event;
pub mod lineage;
pub mod report;
pub mod sink;
pub mod tokenizer;

pub use cli::cli_report;
pub use event::{
    Availability, CacheState, Cost, Observation, Outcome, Role, Stage, TokenCount, TokenizerId,
    Warmth, OBSERVATION_SCHEMA,
};
pub use report::{build_report, render_text, Report};
pub use sink::{HostTraffic, JsonlFileSink, MemorySink, Observer, ObserverLimits, Sink, SinkError};
pub use tokenizer::{measure, ByteTokenizer, ExternalTokenizer, Measured, Tokenizer};
