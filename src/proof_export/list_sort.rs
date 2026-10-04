//! LAW-15 collection laws over source-authenticated immutable List<i64> bodies.
//! Pinned Lean checks structural totality, sortedness and exact multiplicity.
//! The mathematical model does not establish runtime resources or lowering.
use super::{kernel_report, LeanKernel};
use crate::{ast::Program, diagnostic::Diagnostic};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
mod source;

pub const PROFILE: &str = "semaprax.collection-sort-i64.v1";
pub const SEMANTICS: &str = "semaprax.checked-i64.immutable-list.v1";
pub const PROOF_SCHEMA: &str = "semaprax.collection-sort-proof-module.v1";
pub const CERTIFICATE_SCHEMA: &str = "semaprax.collection-sort-certificate.v1";
const NAMESPACE: &str = "SemapraxLaw15Collection";
const THEOREMS: [(&str, &str, &str); 5] = [
    (
        "insert_permutation",
        "(value : Int) (input : List Int)",
        "(insert value input).Perm (value :: input)",
    ),
    (
        "insert_sorted",
        "(value : Int) (input : List Int) (ordered : input.Pairwise (· ≤ ·))",
        "(insert value input).Pairwise (· ≤ ·)",
    ),
    (
        "sort_sorted",
        "(input : List Int)",
        "(sort input).Pairwise (· ≤ ·)",
    ),
    (
        "sort_permutation",
        "(input : List Int)",
        "(sort input).Perm input",
    ),
    (
        "sort_multiplicity",
        "(input : List Int) (value : Int)",
        "(sort input).count value = input.count value",
    ),
];
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofModule {
    pub schema: String,
    pub semantics: String,
    pub insert_permutation: String,
    pub insert_sorted: String,
    pub sort_permutation: String,
    pub sort_sorted: String,
    pub sort_multiplicity: String,
}
impl ProofModule {
    fn bodies(&self) -> [&str; 5] {
        [
            &self.insert_permutation,
            &self.insert_sorted,
            &self.sort_sorted,
            &self.sort_permutation,
            &self.sort_multiplicity,
        ]
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certificate {
    pub schema: String,
    pub profile: String,
    pub source_sha256: String,
    pub definitions_sha256: String,
    pub proof_module_sha256: String,
    pub lean_source: String,
    pub proof_module: ProofModule,
    pub covered_declarations: Vec<String>,
    pub unsupported_declarations: Vec<String>,
    pub axioms: Vec<(String, Vec<String>)>,
    pub nonclaims: Vec<String>,
}
fn refused(reason: &str) -> Diagnostic {
    Diagnostic::io("SPX-LI015", format!("collection law refused: {reason}"))
}
fn digest(bytes: &[u8]) -> String {
    let mut output = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("String formatting");
    }
    output
}
fn admissible(body: &str) -> bool {
    let forbidden = [
        "sorry",
        "admit",
        "axiom",
        "unsafe",
        "run_tac",
        "run_term_elab_m",
        "theorem",
        "def",
        "opaque",
        "constant",
        "instance",
        "import",
        "namespace",
        "macro",
        "syntax",
        "set_option",
        "elab",
    ];
    !body.is_empty()
        && body.len() <= 4096
        && body.lines().count() <= 64
        && !body.contains('#')
        && !body.chars().any(|c| c.is_control() && c != '\n')
        && !body
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|word| forbidden.contains(&word.to_ascii_lowercase().as_str()))
}
fn render(program: &Program, proofs: &ProofModule) -> Result<(String, String), Diagnostic> {
    if proofs.schema != PROOF_SCHEMA
        || proofs.semantics != SEMANTICS
        || proofs.bodies().iter().any(|body| !admissible(body))
    {
        return Err(refused(
            "proof version, semantics or tactic outside admitted profile",
        ));
    }
    let definitions = source::definitions(program)?;
    let mut text = definitions.clone();
    for ((name, params, statement), proof) in THEOREMS.iter().zip(proofs.bodies()) {
        text.push_str(&format!("theorem {name} {params} : {statement} := by\n"));
        for line in proof.lines() {
            text.push_str("  ");
            text.push_str(line);
            text.push('\n');
        }
        text.push('\n');
    }
    for (name, _, _) in THEOREMS {
        text.push_str(&format!("#print axioms {name}\n"));
    }
    text.push_str(&format!("end {NAMESPACE}\n"));
    Ok((text, definitions))
}
pub fn prove(
    program: &Program,
    proofs: &ProofModule,
    kernel: &impl LeanKernel,
) -> Result<Certificate, Diagnostic> {
    let (lean_source, definitions) = render(program, proofs)?;
    let run = kernel.check(&lean_source)?;
    let names = THEOREMS
        .iter()
        .map(|(name, _, _)| format!("{NAMESPACE}.{name}"))
        .collect::<Vec<_>>();
    let kernel_report::KernelVerdict::Checked { axioms } =
        kernel_report::parse(&names, &run.toolchain, &run.output)
    else {
        return Err(refused(
            "pinned kernel did not confirm every fixed collection law",
        ));
    };
    let covered_declarations = vec![
        "law15.collection.insert".into(),
        "law15.collection.sort".into(),
    ];
    let mut unsupported_declarations: Vec<String> = program
        .functions
        .iter()
        .filter(|f| !covered_declarations.contains(&f.stable_id))
        .map(|f| f.stable_id.clone())
        .collect();
    unsupported_declarations.extend(program.types.iter().map(|item| item.stable_id.clone()));
    unsupported_declarations.extend(program.interfaces.iter().map(|item| item.stable_id.clone()));
    unsupported_declarations.extend(program.protocols.iter().map(|item| item.stable_id.clone()));
    unsupported_declarations.extend(
        program
            .implementations
            .iter()
            .map(|item| item.stable_id.clone()),
    );
    unsupported_declarations.extend(
        program
            .session_protocols
            .iter()
            .map(|item| item.stable_id.clone()),
    );
    unsupported_declarations.extend(program.agents.iter().map(|item| item.stable_id.clone()));
    unsupported_declarations.sort();
    Ok(Certificate {
        schema: CERTIFICATE_SCHEMA.into(),
        profile: PROFILE.into(),
        source_sha256: digest(crate::format::canonical(program).as_bytes()),
        definitions_sha256: digest(definitions.as_bytes()),
        proof_module_sha256: digest(&serde_json::to_vec(proofs).expect("proof module JSON")),
        lean_source,
        proof_module: proofs.clone(),
        covered_declarations,
        unsupported_declarations,
        axioms,
        nonclaims: vec![
            "no_runtime_resource_or_lowering_proof".into(),
            "no_public_list_abi".into(),
            "finite_mathematical_lists_over_exact_signed_elements".into(),
            "runtime_capacity_depth_and_allocation_remain_checked".into(),
        ],
    })
}
/// Stale source, profile or independently held proof/assumption bytes fail
/// before any external kernel call. A successful replay invokes the kernel.
pub fn verify(
    program: &Program,
    current_proofs: &ProofModule,
    certificate: &Certificate,
    kernel: &impl LeanKernel,
) -> Result<(), Diagnostic> {
    if certificate.schema != CERTIFICATE_SCHEMA
        || certificate.profile != PROFILE
        || &certificate.proof_module != current_proofs
        || certificate.source_sha256 != digest(crate::format::canonical(program).as_bytes())
        || certificate.proof_module_sha256
            != digest(&serde_json::to_vec(current_proofs).expect("proof module JSON"))
    {
        return Err(refused(
            "source, law version or current assumption/proof module drift",
        ));
    }
    let (text, definitions) = render(program, current_proofs)?;
    if text != certificate.lean_source
        || digest(definitions.as_bytes()) != certificate.definitions_sha256
    {
        return Err(refused("translated definition or theorem drift"));
    }
    if &prove(program, current_proofs, kernel)? != certificate {
        return Err(refused("kernel, coverage or report drift"));
    }
    Ok(())
}

/// A bounded, kernel-checked counterexample to multiplicity on the fixed pair
/// [1, 2]. This is distinct from a failed proof search and grants no positive
/// universal law evidence. Sortedness alone is checked as a weak-law control.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counterexample {
    pub profile: String,
    pub evidence_kind: String,
    pub source_sha256: String,
    pub definitions_sha256: String,
    pub input: [i64; 2],
    pub witness_value: i64,
    pub axioms: Vec<(String, Vec<String>)>,
    pub nonclaim: String,
}
pub fn refute_multiplicity_on_pair(
    program: &Program,
    kernel: &impl LeanKernel,
) -> Result<Counterexample, Diagnostic> {
    let definitions = source::definitions(program)?;
    let mut text = definitions.clone();
    text.push_str("theorem weak_sorted_control : (sort [1, 2]).Pairwise (· ≤ ·) := by decide\n");
    text.push_str("theorem multiplicity_counterexample : (sort [1, 2]).count 2 ≠ ([1, 2] : List Int).count 2 := by decide\n");
    text.push_str("#print axioms weak_sorted_control\n#print axioms multiplicity_counterexample\nend SemapraxLaw15Collection\n");
    let run = kernel.check(&text)?;
    let names = ["weak_sorted_control", "multiplicity_counterexample"]
        .map(|name| format!("{NAMESPACE}.{name}"));
    let kernel_report::KernelVerdict::Checked { axioms } =
        kernel_report::parse(&names, &run.toolchain, &run.output)
    else {
        return Err(refused(
            "kernel did not check the concrete multiplicity counterexample",
        ));
    };
    Ok(Counterexample {
        profile: PROFILE.into(),
        evidence_kind: "bounded_source_counterexample".into(),
        source_sha256: digest(crate::format::canonical(program).as_bytes()),
        definitions_sha256: digest(definitions.as_bytes()),
        input: [1, 2],
        witness_value: 2,
        axioms,
        nonclaim: "no_positive_universal_law_or_runtime_lowering_evidence".into(),
    })
}
