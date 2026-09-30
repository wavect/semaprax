//! Agent Stage Semantic Work v1 on the native C11 stage executor.
//!
//! The artifact is the same whole-module `emit_hir_c` projection with the
//! semantic meter enabled, driven by the same boundary wrapper. After the
//! existing result line and settlement receipt, the wrapper prints exactly
//! one `SEMANTIC-WORK v1` receipt: charged units, the sticky exhaustion and
//! event-overflow flags, the selected status (or `none`), and every performed
//! canonical plan finalizer in execution order. The receipt is parsed
//! strictly; an omitted, extra, malformed or contradictory row fails closed.

use crate::codegen::{
    NativeSemanticMetering, CALL_DEPTH_STATUS_DOMAIN, SEMANTIC_EVENT_CAPACITY,
    SEMANTIC_FUEL_STATUS_DOMAIN,
};
use crate::interpreter::retained_call::SemanticWork;

use super::super::semantic_work::{compiler_status, StageSemanticProfile};
use super::*;

const SEMANTIC_RECEIPT: &str = "SEMANTIC-WORK v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run_metered(
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    host: &NativeStageHost,
    optimization: &str,
    profile: &StageSemanticProfile,
    cancellation: Option<&crate::agent_runtime::AgentCancellation>,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    if !(1..=1_000_000).contains(&max_steps) {
        return Err(invariant("native_executor.max_steps"));
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == prepared.function_id())
        .ok_or_else(|| invariant("native_executor.entry.absent"))?;
    if entry.params.len() != arguments.len() || arguments.len() != prepared.parameter_count() {
        return Err(invariant("native_executor.argument.arity"));
    }
    let (body, borrowed_count) = render_driver(program, entry, arguments)?;
    let body = body
        .strip_suffix("    return 0;\n")
        .ok_or_else(|| invariant("native_executor.semantic.driver"))?;
    let body = format!("{body}{}    return 0;\n", receipt_printer(&entry.id));
    let metering = NativeSemanticMetering {
        fuel_limit: profile.fuel_limit(),
        functions: profile.ordinals(),
    };
    let generated = crate::codegen::emit_hir_c_semantically_metered(program, &metering)
        .map_err(|_| invariant("native_executor.codegen"))?;
    let root = ProbeDirectory::create()?;
    let outcome = compile_and_run(&generated, &body, &root, host, optimization, cancellation);
    root.cleanup();
    let stdout = outcome?;
    let (boundary, semantic) = stdout
        .strip_suffix('\n')
        .and_then(|text| text.rsplit_once('\n'))
        .ok_or_else(|| invariant("native_executor.semantic.receipt"))?;
    let receipt = parse_receipt(semantic, &entry.id, profile)?;
    if boundary.starts_with("STATUS_FAILURE\n") {
        let settlement = boundary
            .strip_prefix("STATUS_FAILURE\n")
            .filter(|rest| !rest.contains('\n'))
            .ok_or_else(|| invariant("native_executor.decode.settlement"))?;
        check_receipt(settlement, &entry.id, borrowed_count, 0)?;
        let (domain, code) = receipt
            .status
            .as_ref()
            .ok_or_else(|| invariant("native_executor.semantic.status"))?;
        let outcome = if receipt.work.exhausted {
            if domain != SEMANTIC_FUEL_STATUS_DOMAIN || *code != 1 {
                return Err(invariant("native_executor.semantic.exhaustion"));
            }
            RetainedCallOutcome::FuelExhausted
        } else if domain == CALL_DEPTH_STATUS_DOMAIN && *code == 1 {
            // The same unconditional call-depth admission every native
            // function's prologue enforces (`SPX_MAX_CALL_DEPTH`), refused
            // before this call was ever selected for metering; not a
            // language-visible status.
            RetainedCallOutcome::CallDepthExceeded
        } else {
            RetainedCallOutcome::LanguageFailure(compiler_status(domain, *code)?)
        };
        return Ok(RetainedCallEvaluation {
            function_id: entry.id.clone(),
            outcome,
            cleanup_events: Vec::new(),
            steps_used: 0,
            max_steps,
            failure: None,
            semantic_work: Some(receipt.work),
        });
    }
    if receipt.status.is_some() || receipt.work.exhausted {
        return Err(invariant("native_executor.semantic.status"));
    }
    let result_declaration = nominal_declaration(&entry.return_type)?.clone();
    let mut evaluation = decode(
        entry.id.clone(),
        &result_declaration,
        &format!("{boundary}\n"),
        max_steps,
        borrowed_count,
    )?;
    evaluation.semantic_work = Some(receipt.work);
    Ok(evaluation)
}

/// C statements printing the one semantic receipt row. `domain_id` strings
/// are compiler- or executor-owned identifiers without whitespace.
fn receipt_printer(function: &DeclarationId) -> String {
    format!(
        "    {{\n\
         \x20       const struct spx_normalized_status *spx_semantic_status = spx_native_exec_token == SPX_STATUS_SUCCESS ? NULL : spx_status_resolve(&spx_ctx, spx_native_exec_token);\n\
         \x20       printf(\"{SEMANTIC_RECEIPT} {hex} %llu %u %u \", (unsigned long long)spx_semantic_fuel_used, spx_semantic_fuel_exhausted ? 1u : 0u, spx_semantic_event_overflow ? 1u : 0u);\n\
         \x20       if (spx_native_exec_token == SPX_STATUS_SUCCESS) printf(\"none 0\");\n\
         \x20       else if (spx_semantic_status == NULL) printf(\"unresolved 0\");\n\
         \x20       else printf(\"%s %u\", spx_semantic_status->domain_id, (unsigned)spx_semantic_status->code);\n\
         \x20       printf(\" %u\", (unsigned)spx_semantic_event_count);\n\
         \x20       for (uint32_t spx_i = 0; spx_i < spx_semantic_event_count; ++spx_i) printf(\" %llu\", (unsigned long long)spx_semantic_events[spx_i]);\n\
         \x20       printf(\"\\n\");\n\
         \x20   }}\n",
        hex = hex_payload(function),
    )
}

struct SemanticReceipt {
    work: SemanticWork,
    status: Option<(String, u32)>,
}

fn parse_receipt(
    line: &str,
    function: &DeclarationId,
    profile: &StageSemanticProfile,
) -> Result<SemanticReceipt, Diagnostic> {
    let bad = || invariant("native_executor.semantic.receipt");
    let rest = line
        .strip_prefix(SEMANTIC_RECEIPT)
        .and_then(|rest| rest.strip_prefix(' '))
        .ok_or_else(bad)?;
    let mut fields = rest.split(' ');
    let mut next = || fields.next().ok_or_else(bad);
    if next()? != hex_payload(function) {
        return Err(bad());
    }
    let fuel_used = canonical_u64(next()?)?;
    let exhausted = flag(next()?)?;
    if flag(next()?)? {
        return Err(invariant("native_executor.semantic.event_overflow"));
    }
    let domain = next()?.to_owned();
    let code = u32::try_from(canonical_u64(next()?)?).map_err(|_| bad())?;
    let status = match (domain.as_str(), code) {
        ("none", 0) => None,
        ("none" | "unresolved", _) => return Err(bad()),
        _ => Some((domain, code)),
    };
    let count = canonical_u64(next()?)?;
    if count > u64::from(SEMANTIC_EVENT_CAPACITY) {
        return Err(bad());
    }
    let mut events = Vec::new();
    for _ in 0..count {
        events.push(profile.decode_event(canonical_u64(next()?)?)?);
    }
    if fields.next().is_some() || fuel_used > profile.fuel_limit() {
        return Err(bad());
    }
    Ok(SemanticReceipt {
        work: SemanticWork {
            fuel_used,
            fuel_limit: Some(profile.fuel_limit()),
            exhausted,
            finalizer_events: Some(events),
        },
        status,
    })
}

fn canonical_u64(token: &str) -> Result<u64, Diagnostic> {
    let value = token
        .parse::<u64>()
        .map_err(|_| invariant("native_executor.semantic.receipt"))?;
    if value.to_string() != token {
        return Err(invariant("native_executor.semantic.receipt"));
    }
    Ok(value)
}

fn flag(token: &str) -> Result<bool, Diagnostic> {
    match token {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(invariant("native_executor.semantic.receipt")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "module test.native_semantic_receipt;\n@id(\"test.native_semantic_receipt.helper\") fn helper(value: i64) -> i64 { value }\n@id(\"app.main\") fn main() -> i64 { helper(0) }\n";

    fn profile() -> (hir::ResolvedProgram, StageSemanticProfile) {
        let checked = crate::check(SOURCE, Path::new("native-semantic-receipt.spx")).unwrap();
        let program = hir::resolve(&checked).unwrap();
        let profile = StageSemanticProfile::admit(&program, "app.main", 9).unwrap();
        (program, profile)
    }

    #[test]
    fn semantic_receipt_is_parsed_strictly() {
        let (program, profile) = profile();
        let entry = &program.functions.last().unwrap().id;
        let hex = hex_payload(entry);
        let accepted = parse_receipt(
            &format!(
                "{SEMANTIC_RECEIPT} {hex} 3 0 0 none 0 2 {} 5",
                1_u64 << 32 | 7
            ),
            entry,
            &profile,
        )
        .expect("an exact receipt parses");
        assert_eq!(accepted.work.fuel_used, 3);
        assert_eq!(accepted.work.fuel_limit, Some(9));
        assert!(!accepted.work.exhausted && accepted.status.is_none());
        let events = accepted.work.finalizer_events.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].liveness_flag, 7);
        assert_eq!(events[1].liveness_flag, 5);
        let exhausted = parse_receipt(
            &format!("{SEMANTIC_RECEIPT} {hex} 9 1 0 {SEMANTIC_FUEL_STATUS_DOMAIN} 1 0"),
            entry,
            &profile,
        )
        .expect("an exhaustion receipt parses");
        assert!(exhausted.work.exhausted);
        assert_eq!(
            exhausted.status,
            Some((SEMANTIC_FUEL_STATUS_DOMAIN.to_owned(), 1))
        );
        for (forged, field) in [
            (
                format!("{SEMANTIC_RECEIPT} 00 3 0 0 none 0 0"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 03 0 0 none 0 0"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 10 0 0 none 0 0"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 2 0 none 0 0"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 0 1 none 0 0"),
                "semantic.event_overflow",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 0 0 none 4 0"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 0 0 unresolved 0 0"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 0 0 none 0 1"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 0 0 none 0 0 7"),
                "semantic.receipt",
            ),
            (
                format!("{SEMANTIC_RECEIPT} {hex} 3 0 0 none 0 1 {}", 99_u64 << 32),
                "semantic_work.event.function",
            ),
            (
                format!("SEMANTIC-WORK v2 {hex} 3 0 0 none 0 0"),
                "semantic.receipt",
            ),
            (String::new(), "semantic.receipt"),
        ] {
            let refused = parse_receipt(&forged, entry, &profile)
                .err()
                .unwrap_or_else(|| panic!("forged receipt accepted: {forged}"));
            assert!(
                refused.message.contains(field),
                "{forged}: {}",
                refused.message
            );
        }
    }
}
