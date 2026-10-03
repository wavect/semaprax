//! The monomorphic iterator tail is the source carrier for LAW-08's list lane.
const REVERSE: &str = include_str!("../fixtures/law08-structural-list.spx");

#[test]
fn wrong_reverse_bodies_keep_this_length_but_cannot_inherit_order_or_involution() {
    use semaprax::proof_export::list_induction::{self, ProofModule};
    let proofs: ProofModule = serde_json::from_str(include_str!("../../proofs/law08/list-lemmas.json")).unwrap();
    for (replacement, observed) in [
        ("vec_push<i64>(reverse(rest), 0)", "7"),
        ("append(vec_push<i64>(vec_with_capacity<i64>(8192usize), item), rest)", "8"),
    ] {
        let source = REVERSE.replace("vec_push<i64>(reverse(rest), item)", replacement);
        let program = semaprax::check(&source, "wrong-reverse-law08.spx").unwrap();
        let path = std::env::temp_dir().join(format!("semaprax-law08-wrong-{}.spx", observed));
        std::fs::write(&path, &source).unwrap();
        let options = semaprax::interpreter::InterpreterOptions::new(
            65_536,
            semaprax::interpreter::DEFAULT_MAX_STEPS,
        ).unwrap();
        let result = semaprax::interpreter::interpret(&path, "app.main", &[], &options).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], observed);
        let _ = std::fs::remove_file(path);
        // A successful three-element execution is only a bounded control.
        // The source authenticator refuses to relabel either as the exact
        // unbounded reverse definition and fixed order law.
        let error = list_induction::prove(&program, &proofs, &NoKernel).unwrap_err();
        assert_eq!(error.code, "SPX-LI001");
    }
}

struct NoKernel;
impl semaprax::proof_export::LeanKernel for NoKernel {
    fn check(&self, _: &str) -> Result<semaprax::proof_export::KernelRun, semaprax::diagnostic::Diagnostic> {
        panic!("unsupported source must refuse before kernel authority")
    }
}

#[test]
fn monomorphic_owned_iterator_tail_can_return_sequence_from_match() {
    let program = semaprax::check(REVERSE, "structural-list.spx").unwrap();
    let canonical = semaprax::format::canonical(&program);
    let reparsed = semaprax::check(&canonical, "structural-list-canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let resolved = semaprax::hir::resolve(&reparsed).unwrap();
    semaprax::hir::validate(&resolved).unwrap();
    let graph = semaprax::graph::to_json(&reparsed).unwrap();
    semaprax::graph::verify_json(&reparsed, &graph).unwrap();
    let path = std::env::temp_dir().join(format!(
        "semaprax-law08-structural-list-{}.spx",
        std::process::id()
    ));
    std::fs::write(&path, &canonical).unwrap();
    let options = semaprax::interpreter::InterpreterOptions::new(
        65_536,
        semaprax::interpreter::DEFAULT_MAX_STEPS,
    )
    .unwrap();
    let outcome = semaprax::interpreter::interpret(&path, "app.main", &[], &options).unwrap();
    let result: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
    assert_eq!(result["payload"]["outcome"]["kind"], "returned");
    assert_eq!(result["payload"]["outcome"]["value"], "9");
    let _ = std::fs::remove_file(path);
    let c = semaprax::codegen::emit_c(&reparsed).unwrap();
    assert_eq!(c, semaprax::codegen::emit_c(&reparsed).unwrap());
    let wasm = semaprax::wasm::emit_module(&reparsed).unwrap();
    assert_eq!(wasm, semaprax::wasm::emit_module(&reparsed).unwrap());
}

#[test]
#[ignore = "requires explicitly provisioned pinned Lean 4.34.0 executable"]
fn pinned_lean_replays_source_bound_unbounded_list_laws() {
    use semaprax::agent_runtime::AgentCancellation;
    use semaprax::proof_export::installed::{HostProfile, InstalledProofTool, Limits, ToolKind};
    use semaprax::proof_export::list_induction::{self, ProofModule};
    let program = semaprax::check(REVERSE, "structural-list-law08.spx").unwrap();
    let proofs: ProofModule =
        serde_json::from_str(include_str!("../../proofs/law08/list-lemmas.json")).unwrap();
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
    let certificate = list_induction::prove(&program, &proofs, &tool).unwrap();
    assert_eq!(certificate.coverage.len(), program.functions.len());
    assert_eq!(
        certificate
            .coverage
            .iter()
            .filter(|row| row.outcome == "exported_direct_tail")
            .count(),
        2
    );
    assert_eq!(certificate.axioms.len(), 5);
    assert!(certificate
        .axioms
        .iter()
        .all(|(_, axioms)| axioms.iter().all(|axiom| matches!(
            axiom.as_str(),
            "propext" | "Quot.sound" | "Classical.choice"
        ))));
    list_induction::verify(&program, &certificate, &tool).unwrap();
    list_induction::verify_against_module(&program, &proofs, &certificate, &tool).unwrap();
    let mut stale_current_module = proofs.clone();
    stale_current_module.append_eq.push_str("\n  simp");
    let stale = list_induction::verify_against_module(
        &program, &stale_current_module, &certificate, &NoKernel,
    ).unwrap_err();
    assert_eq!(stale.code, "SPX-LI001");

    let mut wrong_association = certificate.clone();
    wrong_association.theorem_law_ids[0].1 = "list.reverse".into();
    assert!(list_induction::verify(&program, &wrong_association, &tool).is_err());
    let mut swapped_definition = certificate.clone();
    swapped_definition.definitions_sha256 = "sha256:swapped".into();
    assert!(list_induction::verify(&program, &swapped_definition, &tool).is_err());
    let mut swapped_exported_body = certificate.clone();
    swapped_exported_body.lean_source = swapped_exported_body
        .lean_source
        .replace("(reverse rest) ++ [item]", "item :: reverse rest");
    assert!(list_induction::verify(&program, &swapped_exported_body, &tool).is_err());
    let mut changed_lemma_assumption = certificate.clone();
    changed_lemma_assumption
        .proof_module
        .append_eq
        .push_str("\n  simp");
    assert!(list_induction::verify(&program, &changed_lemma_assumption, &tool).is_err());
    let mut changed_element_semantics = certificate.clone();
    changed_element_semantics.profile = "semaprax.list-induction-u8.v1".into();
    assert!(list_induction::verify(&program, &changed_element_semantics, &tool).is_err());
    let wrong_source = REVERSE.replace(
        "vec_push<i64>(reverse(rest), item)",
        "vec_push<i64>(reverse(rest), item + 1)",
    );
    let wrong_program = semaprax::check(&wrong_source, "wrong-reverse.spx").unwrap();
    assert!(list_induction::verify(&wrong_program, &certificate, &tool).is_err());
    let nondecreasing = REVERSE.replace("reverse(rest)", "reverse(input)");
    if let Ok(program) = semaprax::check(&nondecreasing, "nondecreasing.spx") {
        assert!(list_induction::prove(&program, &proofs, &tool).is_err());
    }
    let mut extra_axiom = proofs.clone();
    extra_axiom
        .reverse_length
        .push_str("\naxiom invented : False");
    assert!(list_induction::prove(&program, &extra_axiom, &tool).is_err());
    let mut cyclic = proofs;
    cyclic.append_eq = "exact append_empty left".into();
    assert!(list_induction::prove(&program, &cyclic, &tool).is_err());
    cyclic.append_eq = "decide".into(); // finite testing cannot prove this universal theorem.
    assert!(list_induction::prove(&program, &cyclic, &tool).is_err());
}

#[test]
#[ignore = "requires explicitly provisioned C11 compiler"]
fn recursive_list_match_executes_in_native_c_at_o0_and_o2() {
    use std::process::Command;
    let clang = std::env::var("CLANG").expect("explicit C11 compiler");
    let program = semaprax::check(REVERSE, "structural-list-native.spx").unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let symbol = "app.main"
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(64)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(1000), entries, UINT32_C(64), NULL, NULL, NULL)) return 10;
    int64_t out = 0;
    if (spx_decl_{symbol}(&context, &out) != SPX_STATUS_SUCCESS || out != INT64_C(9)) return 11;
    return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let stem = format!(
            "semaprax-law08-native-{}-{}",
            std::process::id(),
            &optimization[2..]
        );
        let c = std::env::temp_dir().join(format!("{stem}.c"));
        let executable = std::env::temp_dir().join(stem);
        std::fs::write(&c, format!("{generated}\n{probe}")).unwrap();
        let built = Command::new(&clang)
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "C11 {optimization}: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let result = Command::new(&executable).output().unwrap();
        assert!(
            result.status.success(),
            "C11 {optimization}: {:?} {}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}

#[test]
#[ignore = "requires explicitly provisioned Node.js Core Wasm runtime"]
fn recursive_list_match_executes_in_core_wasm_with_owned_vec_host() {
    use std::process::Command;
    let node = std::env::var("SEMAPRAX_LAW_NODE").expect("explicit Node.js binary");
    let program = semaprax::check(REVERSE, "structural-list-wasm.spx").unwrap();
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let path = std::env::temp_dir().join(format!(
        "semaprax-law08-wasm-{}.wasm",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let script = r#"
import { readFileSync } from 'node:fs';
const entries = new Map();
let next = 1n;
const key = value => {
  if (typeof value !== 'bigint' || value === 0n) throw Error('invalid vector handle');
  return value.toString();
};
const read = (handle, tag) => {
  const entry = entries.get(key(handle));
  if (!entry || entry.tag !== tag) throw Error('stale or mistyped vector');
  return entry;
};
const alloc = (tag, capacity, values = []) => {
  const handle = next++;
  entries.set(key(handle), { tag, capacity, values });
  return handle;
};
const env = {
  spx_add: (a, b) => a + b,
  spx_sub: (a, b) => a - b,
  spx_mul: (a, b) => a * b,
  spx_div: (a, b) => a / b,
  spx_rem: (a, b) => a % b,
  spx_neg: a => -a,
  spx_contract_fail: code => { throw Error(`unexpected contract ${code}`); },
  spx_vec_with_capacity: (tag, capacity) => {
    const n = Number(capacity);
    return Number.isSafeInteger(n) && n >= 0 && n <= 8192 ? alloc(tag, n) : 0n;
  },
  spx_vec_push: (source, tag, value) => {
    const old = read(source, tag);
    if (old.values.length >= old.capacity) return 0n;
    const values = [...old.values, value];
    entries.delete(key(source));
    return alloc(tag, old.capacity, values);
  },
  spx_vec_len: (source, tag) => BigInt(read(source, tag).values.length),
  spx_vec_capacity: (source, tag) => BigInt(read(source, tag).capacity),
  spx_vec_get: (source, tag, index) => {
    const old = read(source, tag);
    const n = Number(index);
    if (!Number.isSafeInteger(n) || n < 0 || n >= old.values.length) throw Error('vector index');
    return old.values[n];
  },
  spx_vec_drop: source => {
    if (!entries.delete(key(source))) throw Error('double vector drop');
  },
};
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[1]), { env });
const observed = instance.exports.semaprax_main();
if (observed !== 9n || entries.size !== 0) {
  throw Error(`Core Wasm list result ${observed}, live vectors ${entries.size}`);
}
"#;
    let run = Command::new(node)
        .args(["--input-type=module", "--eval", script])
        .arg(&path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(path);
    assert!(
        run.status.success(),
        "Core Wasm stdout={} stderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
