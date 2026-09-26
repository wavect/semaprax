//! Agent Stage Semantic Work v1 on the Core Wasm stage executor.
//!
//! The metered package is the same replay-verified owned-data build, emitted
//! with the scoped semantic meter selected. The generated observer captures
//! the one instance the package facade creates, resets the exported meter
//! globals before every projection call, and writes exactly one
//! `semaprax.agent-wasm-stage-semantic-work.v1` row after each executed call.
//! A refused charge propagates the private fuel status, which the facade
//! rejects as outside its public status range; the observer settles that call
//! as exhaustion only when the module's own sticky exhaustion global is set.
//! Rows are parsed strictly and every executed projection must report the
//! same semantic work, because each one re-runs the same stage call.

use std::cell::RefCell;

use crate::diagnostic::Diagnostic;
use crate::interpreter::retained_call::SemanticWork;

use super::super::semantic_work::StageSemanticProfile;
use super::invariant;
use super::outcome::{decode_node_outcomes, NodeStageRun};

const SEMANTIC_SCHEMA: &str = "semaprax.agent-wasm-stage-semantic-work.v1";
/// Reserved stdout bytes for one semantic row.
pub(super) const MAX_SEMANTIC_ROW_BYTES: usize = 2 * 1_024;

/// One metered dispatch: the admitted profile and the observed work.
pub(super) struct WasmMeter<'p> {
    profile: &'p StageSemanticProfile,
    observed: RefCell<Option<SemanticWork>>,
}

impl<'p> WasmMeter<'p> {
    pub(super) fn new(profile: &'p StageSemanticProfile) -> Self {
        Self {
            profile,
            observed: RefCell::new(None),
        }
    }

    /// The scoped emission selection for this dispatch's package build.
    pub(super) fn metering(&self) -> crate::wasm::WasmSemanticMetering {
        crate::wasm::WasmSemanticMetering {
            fuel_limit: self.profile.fuel_limit(),
            functions: self.profile.ordinals(),
        }
    }

    /// The observed work, required exactly once per metered dispatch.
    pub(super) fn take(&self) -> Result<SemanticWork, Diagnostic> {
        self.observed
            .borrow_mut()
            .take()
            .ok_or_else(|| invariant("wasm_executor.semantic.unobserved"))
    }

    /// Split the metered observer output, decode both row families, and
    /// record the one agreed semantic work.
    pub(super) fn decode(&self, stdout: &str, expected: usize) -> Result<NodeStageRun, Diagnostic> {
        let mut ordinary = String::new();
        let mut rows = Vec::new();
        for line in stdout.lines() {
            let semantic = serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .is_some_and(|row| row["schema"] == SEMANTIC_SCHEMA);
            if semantic {
                rows.push(self.row(line)?);
            } else {
                ordinary.push_str(line);
                ordinary.push('\n');
            }
        }
        let first = rows
            .first()
            .cloned()
            .ok_or_else(|| invariant("wasm_executor.semantic.arity"))?;
        if rows.len() > expected || rows.iter().any(|row| *row != first) {
            return Err(invariant("wasm_executor.semantic.divergent"));
        }
        let run = if first.exhausted {
            if !ordinary.is_empty() || rows.len() != 1 {
                return Err(invariant("wasm_executor.semantic.exhaustion"));
            }
            NodeStageRun::FuelExhausted
        } else {
            let run = decode_node_outcomes(&ordinary, expected)?;
            let executed = match &run {
                NodeStageRun::Returned(values) => values.len(),
                NodeStageRun::LanguageFailure(_) => 1,
                NodeStageRun::FuelExhausted => 0,
            };
            if executed != rows.len() {
                return Err(invariant("wasm_executor.semantic.arity"));
            }
            run
        };
        *self.observed.borrow_mut() = Some(first);
        Ok(run)
    }

    fn row(&self, line: &str) -> Result<SemanticWork, Diagnostic> {
        let bad = || invariant("wasm_executor.semantic.row");
        if line.len() > MAX_SEMANTIC_ROW_BYTES {
            return Err(bad());
        }
        let value: serde_json::Value = serde_json::from_str(line).map_err(|_| bad())?;
        let object = value
            .as_object()
            .filter(|object| object.len() == 5)
            .ok_or_else(bad)?;
        if object.get("schema").and_then(serde_json::Value::as_str) != Some(SEMANTIC_SCHEMA) {
            return Err(bad());
        }
        let decimal = |value: &serde_json::Value| {
            value
                .as_str()
                .and_then(|text| text.parse::<u64>().ok().filter(|n| n.to_string() == text))
                .ok_or_else(bad)
        };
        let flag = |name: &str| match object.get(name).and_then(serde_json::Value::as_u64) {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => Err(bad()),
        };
        let fuel_used = decimal(object.get("fuel").ok_or_else(bad)?)?;
        let exhausted = flag("exhausted")?;
        if flag("overflow")? {
            return Err(invariant("wasm_executor.semantic.event_overflow"));
        }
        let events = object
            .get("events")
            .and_then(serde_json::Value::as_array)
            .filter(|events| events.len() <= crate::wasm::SEMANTIC_EVENT_CAPACITY as usize)
            .ok_or_else(bad)?
            .iter()
            .map(|event| self.profile.decode_event(decimal(event)?))
            .collect::<Result<Vec<_>, _>>()?;
        if fuel_used > self.profile.fuel_limit() {
            return Err(bad());
        }
        Ok(SemanticWork {
            fuel_used,
            fuel_limit: Some(self.profile.fuel_limit()),
            exhausted,
            finalizer_events: Some(events),
        })
    }

    /// The metered observer. It shares the ordinary observer's outcome rows
    /// and status normalization, and adds the capture, reset and one semantic
    /// row per executed call.
    pub(super) fn observe_source(&self, call_thunks: &str) -> String {
        let capacity = crate::wasm::SEMANTIC_EVENT_CAPACITY;
        let used = crate::wasm::FUEL_USED_EXPORT;
        let exhausted = crate::wasm::EXHAUSTED_EXPORT;
        let count = crate::wasm::EVENT_COUNT_EXPORT;
        let overflow = crate::wasm::EVENT_OVERFLOW_EXPORT;
        let prefix = crate::wasm::EVENT_EXPORT_PREFIX;
        format!(
            r#"import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm = new Uint8Array(fs.readFileSync(new URL('./app.wasm', import.meta.url)));
const realInstantiate = WebAssembly.instantiate;
let meter = null;
WebAssembly.instantiate = async (...args) => {{
  const result = await Reflect.apply(realInstantiate, WebAssembly, args);
  if(meter !== null) throw new Error('SEMAPRAX semantic meter capture');
  meter = result.instance.exports;
  return result;
}};
let api;
try {{ api = await instantiate(wasm); }} finally {{ WebAssembly.instantiate = realInstantiate; }}
if(meter === null) throw new Error('SEMAPRAX semantic meter absent');
const global = name => {{
  const value = meter[name];
  if(!(value instanceof WebAssembly.Global)) throw new Error('SEMAPRAX semantic meter global');
  return value;
}};
const used = global('{used}'), exhausted = global('{exhausted}'), count = global('{count}'), overflow = global('{overflow}');
const events = Array.from({{length:{capacity}}}, (_, index) => global('{prefix}' + index));
const reset = () => {{ used.value = 0n; exhausted.value = 0; count.value = 0; overflow.value = 0; for(const event of events) event.value = 0n; }};
const work = () => {{
  const n = count.value;
  if(!Number.isInteger(n) || n < 0 || n > {capacity}) throw new Error('SEMAPRAX semantic event count');
  return {{schema:'{SEMANTIC_SCHEMA}',fuel:String(used.value),exhausted:exhausted.value,overflow:overflow.value,events:events.slice(0, n).map(event => String(event.value))}};
}};
const out = [];
const normalizeFailure = error => {{
  if(error?.semapraxSemantic !== true) throw error;
  let raw = error.status;
  if(raw === undefined) raw = error.domain === 'semaprax.arithmetic.v1' ? error.code : error.domain === 'semaprax.contract.v1' ? error.code + 8 : NaN;
  if(!Number.isInteger(raw) || raw < 1 || raw > 10) throw new Error('SEMAPRAX stage status');
  const domain = raw <= 8 ? 'semaprax.arithmetic.v1' : 'semaprax.contract.v1';
  const code = raw <= 8 ? raw : raw - 8;
  if(error.status !== undefined && error.status !== raw || error.code !== undefined && (error.code !== code || error.domain !== domain)) throw new Error('SEMAPRAX stage status mismatch');
  return {{schema:'semaprax.agent-wasm-stage-outcome.v2',kind:'language_failure',raw_status:raw,status:{{schema:'semaprax.status.v1',domain_id:domain,code,class:raw<=8?'arithmetic':'contract',retryable:false}}}};
}};
const stage = call => {{try {{
  const value = call();
  if(value instanceof Uint8Array) return {{schema:'semaprax.agent-wasm-stage-outcome.v2',kind:'settled_owned_bytes',value:Array.from(value,b=>b.toString(16).padStart(2,'0')).join(''),byte_length:value.byteLength}};
  return {{schema:'semaprax.agent-wasm-stage-outcome.v2',kind:'returned',value:String(value)}};
}} catch(error) {{return normalizeFailure(error)}}}};
const calls = [
{call_thunks}
];
for (const call of calls) {{
  reset();
  let result = null;
  try {{ result = stage(call); }} catch(error) {{ if(exhausted.value !== 1) throw error; }}
  if(result !== null) out.push(result);
  out.push(work());
  // Exhaustion and a checked failure each settle this invocation; no later
  // projection runs against that instance.
  if(result === null || result.kind === 'language_failure') break;
}}
process.stdout.write(out.map(value => JSON.stringify(value) + '\n').join(''));
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "module test.wasm_semantic_rows;\n@id(\"test.wasm_semantic_rows.helper\") fn helper(value: i64) -> i64 { value }\n@id(\"app.main\") fn main() -> i64 { helper(0) }\n";
    const RETURNED: &str =
        r#"{"schema":"semaprax.agent-wasm-stage-outcome.v2","kind":"returned","value":"7"}"#;

    fn row(fuel: &str, exhausted: u8, overflow: u8, events: &str) -> String {
        format!(
            r#"{{"schema":"{SEMANTIC_SCHEMA}","fuel":"{fuel}","exhausted":{exhausted},"overflow":{overflow},"events":[{events}]}}"#
        )
    }

    #[test]
    fn semantic_rows_are_parsed_strictly_and_must_agree() {
        let checked = crate::check(SOURCE, std::path::Path::new("wasm-semantic-rows.spx")).unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let profile = StageSemanticProfile::admit(&program, "app.main", 9).unwrap();
        let event = format!("\"{}\"", 1_u64 << 32 | 4);
        let good = row("3", 0, 0, &event);

        let meter = WasmMeter::new(&profile);
        let run = meter
            .decode(&format!("{RETURNED}\n{good}\n{RETURNED}\n{good}\n"), 2)
            .expect("agreeing rows decode");
        assert!(matches!(run, NodeStageRun::Returned(values) if values.len() == 2));
        let work = meter.take().unwrap();
        assert_eq!((work.fuel_used, work.exhausted), (3, false));
        assert_eq!(work.finalizer_events.unwrap()[0].liveness_flag, 4);
        assert!(meter.take().is_err(), "observed work is taken exactly once");

        let meter = WasmMeter::new(&profile);
        let run = meter
            .decode(&format!("{}\n", row("9", 1, 0, "")), 2)
            .unwrap();
        assert!(matches!(run, NodeStageRun::FuelExhausted));
        assert!(meter.take().unwrap().exhausted);

        for (forged, field) in [
            (format!("{RETURNED}\n"), "semantic.arity"),
            (
                format!("{RETURNED}\n{good}\n{RETURNED}\n"),
                "semantic.arity",
            ),
            (
                format!(
                    "{RETURNED}\n{good}\n{RETURNED}\n{}\n",
                    row("4", 0, 0, &event)
                ),
                "semantic.divergent",
            ),
            (
                format!("{RETURNED}\n{}\n", row("9", 1, 0, "")),
                "semantic.exhaustion",
            ),
            (
                format!("{RETURNED}\n{}\n", row("3", 0, 1, "")),
                "semantic.event_overflow",
            ),
            (
                format!("{RETURNED}\n{}\n", row("03", 0, 0, "")),
                "semantic.row",
            ),
            (
                format!("{RETURNED}\n{}\n", row("10", 0, 0, "")),
                "semantic.row",
            ),
            (
                format!("{RETURNED}\n{}\n", row("3", 2, 0, "")),
                "semantic.row",
            ),
            (
                format!(
                    "{RETURNED}\n{}\n",
                    row("3", 0, 0, &format!("\"{}\"", 99_u64 << 32))
                ),
                "semantic_work.event.function",
            ),
            (
                format!("{RETURNED}\n{}\n", good.replacen('}', r#","extra":1}"#, 1)),
                "semantic.row",
            ),
        ] {
            let meter = WasmMeter::new(&profile);
            let refused = meter
                .decode(&forged, 2)
                .err()
                .unwrap_or_else(|| panic!("forged rows accepted: {forged}"));
            assert!(
                refused.message.contains(field),
                "{forged}: {}",
                refused.message
            );
        }
    }
}
