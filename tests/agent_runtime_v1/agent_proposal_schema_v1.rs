//! Agent Proposal Schema v1: the proposal grammar derived from verified
//! stable-ID record and variant declarations, and its typed decoder.

use semaprax::agent_definition::compile_agent_definition;
use semaprax::agent_proposal::{
    compile_agent_proposal_schema, verify_agent_proposal_schema_bundle, ProposalValue,
};
use semaprax::agent_runtime::AgentRunStatus;
use semaprax::agent_transcript;

use super::agent_definition_v1::definition;
use super::profile;

const MODULE_PATH: &str = "fixture-agent-proposal.spx";

const RECORD_MODULE: &str = r#"module fixture.agent.proposal;

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.tool")
    tool: string,
    @id("fixture.agent.type.proposal.budget")
    budget: i64,
    @id("fixture.agent.type.proposal.urgent")
    urgent: bool,
    @id("fixture.agent.type.proposal.sequence")
    sequence: usize,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const NESTED_MODULE: &str = r#"module fixture.agent.proposal_nested;

@id("fixture.agent.type.nested")
record Nested {
    @id("fixture.agent.type.nested.value")
    value: i64,
}

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.inner")
    inner: Nested,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const CLASS_MODULE: &str = r#"module fixture.agent.proposal_class;

@id("fixture.agent.type.proposal")
class Proposal {
    @id("fixture.agent.type.proposal.budget")
    budget: i64,

    @id("fixture.agent.type.proposal.value")
    fn value(self: Proposal) -> i64
{
        self.budget
    }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const GENERIC_MODULE: &str = r#"module fixture.agent.proposal_generic;

@id("fixture.agent.type.proposal")
record Proposal<T> {
    @id("fixture.agent.type.proposal.value")
    value: T,
    @id("fixture.agent.type.proposal.budget")
    budget: i64,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const VARIANT_MODULE: &str = r#"module fixture.agent.proposal_variant;

@id("fixture.agent.type.proposal")
variant Proposal {
    @id("fixture.agent.type.proposal.finish")
    Finish {
        @id("fixture.agent.type.proposal.finish.code")
        code: i64,
    },
    @id("fixture.agent.type.proposal.call")
    Call {
        @id("fixture.agent.type.proposal.call.attempts")
        attempts: i64,
        @id("fixture.agent.type.proposal.call.urgent")
        urgent: bool,
    },
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

/// One well-formed digest that is not this grammar's digest.
fn stale_digest(digest: &str) -> String {
    let mut stale = digest.to_owned();
    let last = stale.pop().expect("a digest is never empty");
    stale.push(if last == '0' { '1' } else { '0' });
    stale
}

fn compile_record() -> semaprax::agent_proposal::CompiledAgentProposalSchema {
    compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &definition(&profile())).unwrap()
}

fn record_proposal(
    schema_digest: &str,
    tool: &str,
    budget: &str,
    urgent: bool,
    sequence: &str,
) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{digest}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.proposal.tool\":\"{tool}\",",
            "\"fixture.agent.type.proposal.budget\":\"{budget}\",",
            "\"fixture.agent.type.proposal.urgent\":{urgent},",
            "\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        digest = schema_digest,
        tool = tool,
        budget = budget,
        urgent = urgent,
        sequence = sequence
    )
}

#[test]
fn proposal_grammar_is_derived_from_the_verified_stable_id_record_type() {
    let definition_source = definition(&profile());
    let first = compile_record();
    let second = compile_record();

    assert_eq!(
        first.schema().canonical_json(),
        second.schema().canonical_json()
    );
    assert_eq!(first.schema().digest(), second.schema().digest());
    assert!(first.schema().canonical_json().ends_with('\n'));
    assert!(!first.schema().canonical_json().trim_end().contains('\n'));
    assert_eq!(first.schema().agent_id(), "fixture.agent");
    assert_eq!(
        first.schema().proposal_type_id(),
        "fixture.agent.type.proposal"
    );

    // The grammar refers to the same identities the AgentDefinition and the
    // AgentGraph carry, and to the module's own field identities.
    let compiled_definition = compile_agent_definition(&definition_source).unwrap();
    assert_eq!(
        first.definition_digest(),
        compiled_definition.definition().digest()
    );
    assert_eq!(
        compiled_definition.definition().proposal_type_id(),
        first.schema().proposal_type_id()
    );
    assert!(compiled_definition
        .graph()
        .canonical_json()
        .contains("fixture.agent.type.proposal"));
    for witness in [
        "\"kind\":\"record\"",
        "\"stable_id\":\"fixture.agent.type.proposal.tool\",\"representation\":\"string\",\"max_bytes\":4096",
        "\"stable_id\":\"fixture.agent.type.proposal.budget\",\"representation\":\"i64\",\"minimum\":\"-9223372036854775808\",\"maximum\":\"9223372036854775807\"",
        "\"stable_id\":\"fixture.agent.type.proposal.urgent\",\"representation\":\"bool\"",
        "\"stable_id\":\"fixture.agent.type.proposal.sequence\",\"representation\":\"u64\",\"minimum\":\"0\",\"maximum\":\"18446744073709551615\"",
        "\"exact_integer_encoding\":\"decimal_string\"",
        "no_authorization_value_or_publication_token_from_a_proposal",
    ] {
        assert!(
            first.schema().canonical_json().contains(witness),
            "missing `{witness}` in {}",
            first.schema().canonical_json()
        );
    }
    // Display names are not part of the grammar.
    assert!(!first.schema().canonical_json().contains("Proposal"));
    assert!(!first.schema().canonical_json().contains("\"tool\""));

    assert_eq!(
        first.source_revision(),
        semaprax::graph::revision(&semaprax::check(RECORD_MODULE, MODULE_PATH).unwrap())
    );

    verify_agent_proposal_schema_bundle(
        RECORD_MODULE,
        MODULE_PATH,
        &definition_source,
        first.schema().canonical_json(),
    )
    .unwrap();

    let tampered = first.schema().canonical_json().replacen(
        "\"representation\":\"i64\"",
        "\"representation\":\"u64\"",
        1,
    );
    let error = verify_agent_proposal_schema_bundle(
        RECORD_MODULE,
        MODULE_PATH,
        &definition_source,
        &tampered,
    )
    .err()
    .unwrap();
    assert_eq!(error[0].code, "SPX-G549");

    // The frozen AgentDefinition v1 known answers are unchanged by this slice.
    assert_eq!(
        compiled_definition.definition().digest(),
        "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0"
    );
    assert_eq!(
        compiled_definition.graph().digest(),
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
    );
}

#[test]
fn a_display_rename_preserves_the_grammar_and_a_type_change_invalidates_it() {
    let definition_source = definition(&profile());
    let baseline = compile_record();
    let valid = record_proposal(
        baseline.schema().digest(),
        "fixture.read",
        "7",
        false,
        "9007199254740993",
    );
    baseline.decode(&valid).unwrap();

    let renamed = RECORD_MODULE
        .replace("record Proposal", "record Suggestion")
        .replace("    tool: string", "    instrument: string")
        .replace("    budget: i64", "    allowance: i64");
    let renamed = compile_agent_proposal_schema(&renamed, MODULE_PATH, &definition_source).unwrap();
    assert_eq!(
        renamed.schema().canonical_json(),
        baseline.schema().canonical_json()
    );
    assert_eq!(renamed.schema().digest(), baseline.schema().digest());
    assert_eq!(
        renamed.schema().proposal_type_revision(),
        baseline.schema().proposal_type_revision()
    );
    renamed.decode(&valid).unwrap();

    for changed_module in [
        RECORD_MODULE.replace("    budget: i64", "    budget: u8"),
        RECORD_MODULE.replace(
            "    urgent: bool,\n",
            "    urgent: bool,\n    @id(\"fixture.agent.type.proposal.extra\")\n    extra: i64,\n",
        ),
        RECORD_MODULE.replace(
            "fixture.agent.type.proposal.budget",
            "fixture.agent.type.proposal.spend",
        ),
    ] {
        let changed =
            compile_agent_proposal_schema(&changed_module, MODULE_PATH, &definition_source)
                .unwrap();
        assert_ne!(changed.schema().digest(), baseline.schema().digest());
        assert_ne!(
            changed.schema().proposal_type_revision(),
            baseline.schema().proposal_type_revision()
        );
        let error = changed.decode(&valid).err().unwrap();
        assert_eq!(error[0].code, "SPX-G551");
        assert_eq!(
            error[0].message,
            "AgentProposal invariant failed: proposal_schema_digest"
        );
    }
}

#[test]
fn the_decoder_rejects_every_enumerated_malformed_and_foreign_proposal() {
    let compiled = compile_record();
    let digest = compiled.schema().digest();
    let valid = record_proposal(digest, "fixture.read", "7", false, "9007199254740993");
    let decoded = compiled.decode(&valid).unwrap();
    assert_eq!(decoded.canonical_json(), valid);
    assert_eq!(decoded.case(), None);
    assert_eq!(decoded.fields().len(), 4);
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.tool"),
        Some(&ProposalValue::Text("fixture.read".to_owned()))
    );
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.budget"),
        Some(&ProposalValue::Signed(7))
    );
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.urgent"),
        Some(&ProposalValue::Bool(false))
    );
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.sequence"),
        Some(&ProposalValue::Unsigned(9_007_199_254_740_993))
    );
    assert_eq!(decoded.field("fixture.agent.type.proposal.absent"), None);

    // Noncanonical documents and unknown schema versions.
    for malformed in [
        valid.trim_end().to_owned(),
        valid.replacen(
            "semaprax.agent-proposal.v1",
            "semaprax.agent-proposal.v2",
            1,
        ),
        valid.replacen(
            "{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "{\"agent_id\":\"fixture.agent\",\"schema\":\"semaprax.agent-proposal.v1\",",
            1,
        ),
        // Fields in lexical rather than declaration order.
        format!(
            concat!(
                "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
                "\"proposal_schema_digest\":\"{digest}\",\"value\":{{\"fields\":{{",
                "\"fixture.agent.type.proposal.budget\":\"7\",",
                "\"fixture.agent.type.proposal.tool\":\"fixture.read\",",
                "\"fixture.agent.type.proposal.urgent\":false,",
                "\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n"
            ),
            digest = digest
        ),
        // A variant-shaped body against a record grammar.
        valid.replacen(
            "\"value\":{\"fields\"",
            "\"value\":{\"case\":\"x\",\"fields\"",
            1,
        ),
        String::from("\n"),
        format!("{}\n{}", valid.trim_end(), valid),
    ] {
        let error = compiled.decode(&malformed).err().unwrap();
        assert_eq!(error[0].code, "SPX-G550", "admitted `{malformed}`");
        assert_eq!(
            error[0].message,
            "AgentProposal is not canonical semaprax.agent-proposal.v1 JSON"
        );
    }

    for (document, expected) in [
        // A cross-agent proposal.
        (
            valid.replacen("\"fixture.agent\"", "\"other.agent\"", 1),
            "agent_id",
        ),
        // A stale proposal bound to a superseded grammar.
        (
            valid.replacen(digest, &stale_digest(digest), 1),
            "proposal_schema_digest",
        ),
        // An extra field.
        (
            valid.replacen(
                "\"fixture.agent.type.proposal.tool\"",
                "\"fixture.agent.type.proposal.other\":\"x\",\"fixture.agent.type.proposal.tool\"",
                1,
            ),
            "value.fields.unknown",
        ),
        // A missing field.
        (
            valid.replacen("\"fixture.agent.type.proposal.urgent\":false,", "", 1),
            "value.fields.missing",
        ),
        // A wrong field identity.
        (
            valid.replacen(
                "fixture.agent.type.proposal.urgent",
                "fixture.agent.type.proposal.hurried",
                1,
            ),
            "value.fields.unknown",
        ),
        // Values that do not match their declared representation.
        (
            valid.replacen(
                "\"fixture.agent.type.proposal.budget\":\"7\"",
                "\"fixture.agent.type.proposal.budget\":7",
                1,
            ),
            "value.representation",
        ),
        (
            valid.replacen(
                "\"fixture.agent.type.proposal.urgent\":false",
                "\"fixture.agent.type.proposal.urgent\":\"false\"",
                1,
            ),
            "value.representation",
        ),
        (
            valid.replacen("\"fixture.read\"", &format!("\"{}\"", "x".repeat(4097)), 1),
            "value.string_bytes",
        ),
    ] {
        let error = compiled.decode(&document).err().unwrap();
        assert_eq!(error[0].code, "SPX-G551", "admitted `{document}`");
        assert_eq!(
            error[0].message,
            format!("AgentProposal invariant failed: {expected}")
        );
    }

    // Malformed exact integers.
    for malformed in ["007", "+7", "7.0", "7e0", " 7", "-0", "", "0x7", "７"] {
        let document = record_proposal(digest, "fixture.read", malformed, false, "1");
        let error = compiled.decode(&document).err().unwrap();
        assert_eq!(error[0].code, "SPX-G551", "admitted budget `{malformed}`");
        assert_eq!(
            error[0].message,
            "AgentProposal invariant failed: value.integer"
        );
    }

    // Exact integers outside their declared bounds.
    for (budget, sequence) in [
        ("9223372036854775808", "1"),
        ("-9223372036854775809", "1"),
        ("7", "18446744073709551616"),
        ("7", "-1"),
    ] {
        let document = record_proposal(digest, "fixture.read", budget, false, sequence);
        let error = compiled.decode(&document).err().unwrap();
        assert_eq!(error[0].code, "SPX-G551");
        assert!(
            error[0].message == "AgentProposal invariant failed: value.integer_range"
                || error[0].message == "AgentProposal invariant failed: value.integer",
            "{}",
            error[0].message
        );
    }

    // An oversized document is refused before any parsing work.
    let error = compiled
        .decode(&format!("{}\n", "x".repeat(65_536)))
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G551");
    assert_eq!(
        error[0].message,
        "AgentProposal invariant failed: proposal_bytes"
    );
}

#[test]
fn exact_integers_beyond_the_json_safe_range_survive_the_decimal_string_wire() {
    let compiled = compile_record();
    let digest = compiled.schema().digest();
    for (budget, sequence) in [
        ("9223372036854775807", "18446744073709551615"),
        ("-9223372036854775808", "9007199254740993"),
        ("0", "0"),
    ] {
        let document = record_proposal(digest, "fixture.read", budget, true, sequence);
        let decoded = compiled.decode(&document).unwrap();
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.budget"),
            Some(&ProposalValue::Signed(budget.parse().unwrap()))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.sequence"),
            Some(&ProposalValue::Unsigned(sequence.parse().unwrap()))
        );
        // The wire form never carries an exact integer as a JSON number.
        assert!(decoded.canonical_json().contains(&format!("\"{budget}\"")));
        assert!(decoded
            .canonical_json()
            .contains(&format!("\"{sequence}\"")));
        assert_eq!(decoded.canonical_json(), document);
    }
}

#[test]
fn a_variant_proposal_grammar_admits_one_case_and_rejects_every_other() {
    let definition_source = definition(&profile());
    let compiled =
        compile_agent_proposal_schema(VARIANT_MODULE, MODULE_PATH, &definition_source).unwrap();
    assert!(compiled
        .schema()
        .canonical_json()
        .contains("\"kind\":\"variant\""));
    assert!(compiled.schema().canonical_json().contains(
        "\"stable_id\":\"fixture.agent.type.proposal.call\",\"fields\":[{\"stable_id\":\"fixture.agent.type.proposal.call.attempts\",\"representation\":\"i64\",\"minimum\":\"-9223372036854775808\",\"maximum\":\"9223372036854775807\"},{\"stable_id\":\"fixture.agent.type.proposal.call.urgent\",\"representation\":\"bool\"}]"
    ));
    // The record and the variant grammar of the same role are distinct.
    assert_ne!(
        compiled.schema().digest(),
        compile_record().schema().digest()
    );

    let digest = compiled.schema().digest();
    let call = format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{digest}\",\"value\":{{",
            "\"case\":\"fixture.agent.type.proposal.call\",\"fields\":{{",
            "\"fixture.agent.type.proposal.call.attempts\":\"3\",",
            "\"fixture.agent.type.proposal.call.urgent\":false}}}}}}\n"
        ),
        digest = digest
    );
    let decoded = compiled.decode(&call).unwrap();
    assert_eq!(decoded.case(), Some("fixture.agent.type.proposal.call"));
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.call.attempts"),
        Some(&ProposalValue::Signed(3))
    );
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.call.urgent"),
        Some(&ProposalValue::Bool(false))
    );

    let wrong_case = call.replacen(
        "fixture.agent.type.proposal.call\",",
        "fixture.agent.type.proposal.abort\",",
        1,
    );
    let error = compiled.decode(&wrong_case).err().unwrap();
    assert_eq!(error[0].code, "SPX-G551");
    assert_eq!(
        error[0].message,
        "AgentProposal invariant failed: value.case"
    );

    // Another admitted case's payload does not satisfy this case.
    let mixed = call.replacen(
        "fixture.agent.type.proposal.call.attempts",
        "fixture.agent.type.proposal.finish.code",
        1,
    );
    let error = compiled.decode(&mixed).err().unwrap();
    assert_eq!(error[0].code, "SPX-G551");
    assert_eq!(
        error[0].message,
        "AgentProposal invariant failed: value.fields.unknown"
    );

    // A record-shaped body does not satisfy a variant grammar.
    let error = compiled
        .decode(&call.replacen("\"case\":\"fixture.agent.type.proposal.call\",", "", 1))
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G550");

    let attempts_out_of_range = call.replacen("\"3\"", "\"9223372036854775808\"", 1);
    let error = compiled.decode(&attempts_out_of_range).err().unwrap();
    assert_eq!(
        error[0].message,
        "AgentProposal invariant failed: value.integer_range"
    );
}

#[test]
fn unadmitted_proposal_types_reject_with_explicit_diagnostics() {
    let definition_source = definition(&profile());
    for (module, expected) in [
        (
            RECORD_MODULE.replace(
                "@id(\"fixture.agent.type.proposal\")",
                "@id(\"fixture.agent.type.other\")",
            ),
            "proposal_type.unresolved",
        ),
        (GENERIC_MODULE.to_owned(), "proposal_type.generic"),
        (
            RECORD_MODULE.replace("    budget: i64", "    budget: f64"),
            "proposal_type.field.type",
        ),
        (
            RECORD_MODULE.replace("    tool: string", "    tool: Bytes"),
            "proposal_type.field.type",
        ),
        (
            RECORD_MODULE.replace("    budget: i64", "    budget: char"),
            "proposal_type.field.type",
        ),
        (NESTED_MODULE.to_owned(), "proposal_type.field.type"),
        (CLASS_MODULE.to_owned(), "proposal_type.kind"),
        (
            RECORD_MODULE.replace(
                "    @id(\"fixture.agent.type.proposal.urgent\")\n    urgent: bool,",
                "    urgent: bool,",
            ),
            "proposal_type.field.identity_origin",
        ),
    ] {
        let error = compile_agent_proposal_schema(&module, MODULE_PATH, &definition_source)
            .err()
            .unwrap_or_else(|| panic!("admitted:\n{module}"));
        assert_eq!(error[0].code, "SPX-G548", "admitted:\n{module}");
        assert_eq!(
            error[0].message,
            format!("AgentProposalSchema invariant failed: {expected}"),
            "for:\n{module}"
        );
    }

    // A by-value recursive proposal type never reaches HIR: the resolver
    // rejects it with its own stable diagnostic before a grammar can exist.
    let recursive = RECORD_MODULE.replace("    budget: i64", "    budget: Proposal");
    let error = compile_agent_proposal_schema(&recursive, MODULE_PATH, &definition_source)
        .err()
        .unwrap();
    assert_ne!(error[0].code, "SPX-G548");
    assert!(error.iter().any(|item| item.severity.is_error()));
}

#[test]
fn an_offline_scripted_provider_uses_the_grammar_and_replays_its_evidence() {
    let profile = profile();
    let definition_source = definition(&profile);
    let compiled = compile_record();
    let proposal = record_proposal(
        compiled.schema().digest(),
        "fixture.read",
        "-9223372036854775808",
        true,
        "18446744073709551615",
    );
    let task = format!(
        "{{\"schema\":\"semaprax.agent-runtime-task.v1\",\"nonce\":\"{}\",\"objective\":\"Return one proposal.\",\"context\":[{{\"label\":\"schema\",\"provenance\":\"caller_untrusted\",\"content\":\"derived\"}}]}}\n",
        "1".repeat(64)
    );
    let action = format!(
        "{{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":{}}}\n",
        serde_json::to_string(&proposal).unwrap()
    );
    let transcript = format!(
        "{{\"schema\":\"semaprax.agent-runtime-transcript.v1\",\"policy_epoch\":7,\"provider\":[{{\"disposition\":\"succeeded\",\"response\":{}}}],\"tools\":[]}}\n",
        serde_json::to_string(&action).unwrap()
    );

    let scripted = agent_transcript::run(&definition_source, &task, &transcript).unwrap();
    assert_eq!(scripted.run.status(), AgentRunStatus::Completed);
    let message = scripted.run.final_message().unwrap();

    // The untrusted provider text is decoded only through the derived grammar.
    let decoded = compiled.decode(message).unwrap();
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.budget"),
        Some(&ProposalValue::Signed(i64::MIN))
    );
    assert_eq!(
        decoded.field("fixture.agent.type.proposal.sequence"),
        Some(&ProposalValue::Unsigned(u64::MAX))
    );
    assert_eq!(decoded.canonical_json(), proposal);

    // The same offline transcript independently replays its own evidence.
    agent_transcript::replay(
        &definition_source,
        &task,
        &transcript,
        scripted.run.evidence(),
    )
    .unwrap();
    let error = agent_transcript::replay(
        &definition_source,
        &task,
        &transcript,
        &scripted
            .run
            .evidence()
            .replacen("completed", "cancelled", 1),
    )
    .err()
    .unwrap();
    assert_eq!(error[0].code, "SPX-V222");

    // A proposal produced for this run is not admitted by another agent's
    // grammar, and carries no authority of its own.
    let other_definition = definition_source.replacen("fixture.agent\"", "other.agent\"", 1);
    let other =
        compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &other_definition).unwrap();
    let error = other.decode(message).err().unwrap();
    assert_eq!(error[0].code, "SPX-G551");
    assert_eq!(error[0].message, "AgentProposal invariant failed: agent_id");
}

#[test]
fn the_proposal_surface_mints_no_authorization_and_reaches_no_host() {
    for source in [
        include_str!("../../src/agent_proposal.rs"),
        include_str!("../../src/agent_proposal/shape.rs"),
        include_str!("../../src/agent_proposal/decode.rs"),
    ] {
        for forbidden in [
            "AgentRuntimeAuthority",
            "AgentHost",
            "Agent::new",
            "attempt_provider",
            "invoke_tool",
            "std::net::",
            "Command::new",
            "fs::write",
            "fs::read",
            "File::create",
            "std::env",
        ] {
            assert!(
                !source.contains(forbidden),
                "the proposal surface references `{forbidden}`"
            );
        }
    }
}
