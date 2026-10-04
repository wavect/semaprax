//! LAW-15's saved immutable-list pack uses the actual checked-source library
//! route and an explicitly provisioned pinned Lean process.
use semaprax::proof_export::list_sort::{self, ProofModule};
const CORRECT: &str = include_str!("../../examples/law-packs/collection/sort.spx");
const EMPTY: &str = include_str!("../../examples/law-packs/collection/mutants/empty-output.spx");
const DUPLICATE: &str =
    include_str!("../../examples/law-packs/collection/mutants/duplicate-element.spx");
const REPAIRED: &str = include_str!("../../examples/law-packs/collection/repaired.spx");
const PROOFS: &str = include_str!("../../examples/law-packs/collection/lemmas.json");
struct NoKernel;
impl semaprax::proof_export::LeanKernel for NoKernel {
    fn check(
        &self,
        _: &str,
    ) -> Result<semaprax::proof_export::KernelRun, semaprax::diagnostic::Diagnostic> {
        panic!("source or stale evidence must refuse before external kernel")
    }
}
fn checked(source: &str) -> semaprax::ast::Program {
    let program = semaprax::check(source, "law15-collection.spx").unwrap();
    let canonical = semaprax::format::canonical(&program);
    let replayed = semaprax::check(&canonical, "law15-collection-canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&replayed));
    let hir = semaprax::hir::resolve(&replayed).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let graph = semaprax::graph::to_json(&replayed).unwrap();
    semaprax::graph::verify_json(&replayed, &graph).unwrap();
    assert!(graph.contains("core.list-step.cons.tail"));
    replayed
}
#[test]
fn collection_law_source_refuses_unsupported_recursion_and_proof_assumptions() {
    let program = checked(CORRECT);
    for source in [EMPTY, DUPLICATE, REPAIRED] {
        checked(source);
    }
    let unsupported_boolean = format!(
        "{CORRECT}\n@id(\"law15.unsupported.boolean\") fn invalid(flag: bool, input: List<i64>) -> List<i64> {{ match list_uncons(input) {{ ListStep::Nil {{}} => list_nil(), ListStep::Cons {{ head, tail }} => tail, }} }}\n"
    );
    assert!(
        semaprax::check(&unsupported_boolean, "unsupported-list-parameter.spx")
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T258")
    );
    let proofs: ProofModule = serde_json::from_str(PROOFS).unwrap();
    let nondecreasing = CORRECT.replace("sort(tail)", "sort(input)");
    if let Ok(program) = semaprax::check(&nondecreasing, "nondecreasing-sort.spx") {
        assert_eq!(
            list_sort::prove(&program, &proofs, &NoKernel)
                .unwrap_err()
                .code,
            "SPX-LI015"
        );
    }
    let mut changed = proofs.clone();
    changed.semantics.push_str(".changed");
    assert_eq!(
        list_sort::prove(&program, &changed, &NoKernel)
            .unwrap_err()
            .code,
        "SPX-LI015"
    );
    changed = proofs;
    changed
        .sort_permutation
        .push_str("\naxiom fabricated : False");
    assert_eq!(
        list_sort::prove(&program, &changed, &NoKernel)
            .unwrap_err()
            .code,
        "SPX-LI015"
    );
}

#[test]
#[ignore = "requires explicitly provisioned pinned Lean 4.34.0"]
fn collection_law_pack_pinned_lean_rejects_empty_and_duplicate_outputs_and_repairs() {
    use semaprax::agent_runtime::AgentCancellation;
    use semaprax::proof_export::installed::{HostProfile, InstalledProofTool, Limits, ToolKind};
    let lean = std::env::var("SEMAPRAX_LAW_LEAN").expect("explicit pinned Lean binary");
    let version = std::env::var("SEMAPRAX_LAW_LEAN_VERSION").expect("exact Lean version line");
    let tool = InstalledProofTool::open(
        std::path::Path::new(&lean),
        &std::env::current_dir().unwrap(),
        ToolKind::Lean,
        &version,
        HostProfile::TrustedLocal,
        Limits::default(),
        AgentCancellation::new(),
    )
    .unwrap();
    let program = checked(CORRECT);
    let proofs: ProofModule = serde_json::from_str(PROOFS).unwrap();
    let certificate = list_sort::prove(&program, &proofs, &tool).unwrap();
    assert_eq!(
        certificate.covered_declarations,
        ["law15.collection.insert", "law15.collection.sort"]
    );
    assert_eq!(
        certificate.unsupported_declarations,
        ["law15.collection.main"]
    );
    assert_eq!(certificate.axioms.len(), 5);
    assert!(certificate.axioms.iter().all(|(_, axioms)| axioms
        .iter()
        .all(|name| matches!(name.as_str(), "propext" | "Quot.sound" | "Classical.choice"))));
    assert!(certificate
        .nonclaims
        .iter()
        .any(|claim| claim == "no_runtime_resource_or_lowering_proof"));
    list_sort::verify(&program, &proofs, &certificate, &tool).unwrap();
    for (name, source) in [("empty-output", EMPTY), ("duplicate-element", DUPLICATE)] {
        let mutant = checked(source);
        let error = list_sort::prove(&mutant, &proofs, &tool).unwrap_err();
        eprintln!("{name}: {error:?}");
        let witness = list_sort::refute_multiplicity_on_pair(&mutant, &tool).unwrap();
        assert_eq!(witness.evidence_kind, "bounded_source_counterexample");
        assert_eq!(witness.input, [1, 2]);
        assert_eq!(witness.witness_value, 2);
        assert_eq!(witness.axioms.len(), 2);
        eprintln!(
            "{name}: count differs for value {} on input {:?}; sortedness alone holds",
            witness.witness_value, witness.input
        );
        list_sort::verify(&mutant, &proofs, &certificate, &NoKernel).unwrap_err();
    }
    let repaired = checked(REPAIRED);
    let repaired_certificate = list_sort::prove(&repaired, &proofs, &tool).unwrap();
    assert_eq!(
        certificate, repaired_certificate,
        "body repair retains the reviewed laws exactly"
    );
    let mut changed = proofs.clone();
    changed.sort_multiplicity.push_str("\n  skip");
    list_sort::verify(&program, &changed, &certificate, &NoKernel).unwrap_err();
    let mut forged = certificate.clone();
    forged.profile.push_str(".changed");
    list_sort::verify(&program, &proofs, &forged, &NoKernel).unwrap_err();
    forged = certificate.clone();
    forged.lean_source = forged
        .lean_source
        .replace(".count value = input.count value", ".length = input.length");
    list_sort::verify(&program, &proofs, &forged, &NoKernel).unwrap_err();
}
