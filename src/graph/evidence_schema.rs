//! Frozen evidence routes reject newer execution semantics.
use super::nested_owned::rejected_evidence_schema;
use super::*;

pub(crate) fn reject_while_loop_evidence_schema(schema: &str) -> Result<(), Diagnostic> {
    if matches!(
        schema,
        "semaprax.graph.v34"
            | "semaprax.graph.v35"
            | "semaprax.graph.v36"
            | "semaprax.graph.v37"
            | "semaprax.graph.v38"
            | "semaprax.graph.v39"
            | "semaprax.graph.v40"
            | "semaprax.graph.v41"
            | "semaprax.graph.v42"
            | "semaprax.graph.v43"
            | "semaprax.graph.v44"
            | "semaprax.graph.v45"
            | "semaprax.graph.v46"
            | "semaprax.graph.v65"
            | "semaprax.graph.v66"
            | "semaprax.graph.v67"
            | "semaprax.graph.v68"
            | "semaprax.graph.v69"
            | "semaprax.graph.v70"
            | "semaprax.graph.v71"
            | "semaprax.graph.v72"
            | "semaprax.graph.v73"
            | "semaprax.graph.v74"
    ) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "Graph v34 is outside frozen evidence admission",
        ));
    }
    if let Some(error) = rejected_evidence_schema(schema) {
        Err(error)
    } else if schema == "semaprax.graph.v24" {
        Err(Diagnostic::io(
            "SPX-G410",
            "projected shared-loan programs select `semaprax.graph.v24`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v23" {
        Err(Diagnostic::io(
            "SPX-G410",
            "shared-loan programs select `semaprax.graph.v23`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v22" {
        Err(Diagnostic::io(
            "SPX-G410",
            "owned variant programs select `semaprax.graph.v22`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v21" {
        Err(Diagnostic::io(
            "SPX-G410",
            "ownership-aware match programs select `semaprax.graph.v21`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v20" {
        Err(Diagnostic::io(
            "SPX-G410",
            "dynamic byte-range programs select `semaprax.graph.v20`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v19" {
        Err(Diagnostic::io(
            "SPX-G410",
            "bounded language-command I/O programs select `semaprax.graph.v19`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v18" {
        Err(Diagnostic::io(
            "SPX-G410",
            "bounded-stdout-transcript programs select `semaprax.graph.v18`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v17" {
        Err(Diagnostic::io(
            "SPX-G410",
            "portable-indexed-byte-data programs select `semaprax.graph.v17`, which is outside this evidence flow's admission",
        ))
    } else if let Some(error) = native_import::evidence_refusal(schema) {
        Err(error)
    } else if schema == "semaprax.graph.v15" {
        Err(Diagnostic::io(
            "SPX-G410",
            "while-loop programs select `semaprax.graph.v15`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v16" {
        Err(Diagnostic::io(
            "SPX-G410",
            "refutable-match programs select `semaprax.graph.v16`, which is outside this evidence flow's admission",
        ))
    } else {
        Ok(())
    }
}
