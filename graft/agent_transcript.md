---
covers: []
---
# agent_transcript.rs

- SCHEMA_V1 · constant · L22-L22 — pub const SCHEMA_V1: &str = "semaprax.agent-runtime-transcript.v1";
- MAX_TRANSCRIPT_BYTES · constant · L24-L24 — pub const MAX_TRANSCRIPT_BYTES: usize = 4 * 1024 * 1024;
- MAX_ENTRIES · constant · L25-L25 — const MAX_ENTRIES: usize = 256;
- ProviderEntry · struct · L29-L33 — pub struct ProviderEntry
- Transcript · struct · L37-L42 — pub struct Transcript
- malformed · function · L44-L46 — fn malformed(message: impl Into<String>) -> Diagnostic
- parse_transcript · function · L50-L171 — pub fn parse_transcript(source: &str) -> Result<Transcript, Diagnostic>
- Probe · struct · L174-L176 — struct Probe
- policy_epoch · function · L179-L181 — fn policy_epoch(&self) -> u64
- elapsed_ms · function · L183-L185 — fn elapsed_ms(&self) -> u64
- TranscriptHost · struct · L191-L195 — pub struct TranscriptHost
- new · function · L199-L205 — pub fn new(transcript: Transcript) -> Self
- policy_epoch · function · L209-L211 — fn policy_epoch(&self) -> u64
- elapsed_ms · function · L213-L215 — fn elapsed_ms(&self) -> u64
- boundary_probe · function · L217-L221 — fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe>
- tokenize · function · L223-L225 — fn tokenize(&mut self, _: &str, request: &str) -> Option<u64>
- attempt_provider · function · L227-L258 — fn attempt_provider(
- invoke_tool · function · L260-L269 — fn invoke_tool(&mut self, _: &str, _: &str, _: &str, sink: &mut AgentToolResultSink) -> bool
- status_text · function · L274-L284 — pub fn status_text(status: AgentRunStatus) -> &'static str
- ScriptedRun · struct · L287-L290 — pub struct ScriptedRun
- run · function · L294-L311 — pub fn run(
- run_receipt · function · L315-L327 — pub fn run_receipt(scripted: &ScriptedRun) -> String
- replay · function · L331-L353 — pub fn replay(
