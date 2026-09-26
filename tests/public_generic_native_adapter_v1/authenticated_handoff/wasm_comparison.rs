//! The explicitly measured native-vs-Core-Wasm comparison for the frozen
//! hostile recipes and the canonical positive control, on the same checked
//! subjects the identity-v1, moves-v1 and allocating-v1 native profiles admit.
//! Both columns are measured in this test: native by a direct C ABI caller
//! against the rendered authenticated provider (-O0 and -O2), Core Wasm by
//! Node against the compiler-emitted provider through its production exports.
//! Core Wasm has no test-only counters, so its columns are the observable
//! lifecycle facts named in `wasm_comparison.mjs`, not allocator parity. This
//! is private local evidence; no hosted, sanitizer or public claim follows.
use super::caller_hostility::{self, Recipe};
use super::*;
use semaprax::public_generic_abi::native::authenticated::{
    render_authenticated_allocating_provider, render_authenticated_moves_provider,
};
use std::fmt::Write as _;

/// The production Core Wasm export inventory: no authenticated-ticket entry.
const WASM_EXPORTS: [&str; 11] = [
    "memory",
    "spx_pg_v1_scratch_ptr",
    "spx_pg_v1_scratch_reserve",
    "spx_pg_v1_scratch_capacity",
    "spx_pg_v1_open",
    "spx_pg_v1_input_prepare",
    "spx_pg_v1_call",
    "spx_pg_v1_result_export",
    "spx_pg_v1_value_release",
    "spx_pg_v1_result_release",
    "spx_pg_v1_provider_close",
];
/// Recipes whose substituted fact is a native prepare argument. The Core Wasm
/// prepare export takes (provider, frame pointer, frame length) only.
const TICKET_ONLY: [&str; 5] = [
    "stale_generation_replay",
    "future_generation_replay",
    "zero_generation_replay",
    "provider_owned_before_transfer",
    "substituted_cleanup_plan",
];
/// Recipes carried in the logical frame itself, admitted by both targets.
const FRAME: [&str; 2] = ["substituted_leaf_path", "unknown_leaf_kind_tag_one_over"];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn flat(leaves: &[&[u8]]) -> Vec<u8> {
    let mut out = (leaves.len() as u64).to_le_bytes().to_vec();
    for leaf in leaves {
        out.extend((leaf.len() as u64).to_le_bytes());
        out.extend(*leaf);
    }
    out
}

fn stable(target: &str, raw: u64) -> &'static str {
    match (target, raw) {
        // Both targets' raw 5 is the v1 malformed-carrier status.
        ("native" | "wasm", 5) => "SPX-PG801",
        ("native", 7) => "SPX-PG804",
        ("native", 8) => "SPX-PG805",
        ("native", 14) => "SPX-PG803",
        _ => panic!("unmapped {target} raw status {raw}"),
    }
}

fn native_column(root: &Path, source: &str, constants: &str) -> Vec<Vec<String>> {
    let provider = format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{source}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n", include_str!("../allocations.c"));
    fs::write(root.join("provider.c"), provider).unwrap();
    fs::write(
        root.join("driver.c"),
        format!(
            "{}\n{}\n{constants}\n{}",
            semaprax::public_generic_abi::native::template::HEADER_V1,
            semaprax::public_generic_abi::native::authenticated::HEADER,
            include_str!("wasm_comparison_native.c")
        ),
    )
    .unwrap();
    let mut first: Option<Vec<Vec<String>>> = None;
    for opt in ["-O0", "-O2"] {
        let executable = root.join(format!("native{opt}{}", std::env::consts::EXE_SUFFIX));
        let built = Command::new("clang")
            .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror"])
            .arg(root.join("provider.c"))
            .arg(root.join("driver.c"))
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("comparison requires provisioned clang");
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let run = Command::new(executable).output().unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let rows: Vec<Vec<String>> = String::from_utf8(run.stdout)
            .unwrap()
            .lines()
            .map(|line| line.split(' ').map(str::to_owned).collect())
            .collect();
        // The measured native column is identical at both optimisation levels.
        match &first {
            Some(rows0) => assert_eq!(rows0, &rows),
            None => first = Some(rows),
        }
    }
    first.unwrap()
}

fn wasm_column(
    root: &Path,
    wasm: &semaprax::wasm::PublicGenericWasmProviderArtifactV1,
    canonical: &[u8],
    hostile: &[(&str, Vec<u8>)],
) -> Vec<serde_json::Value> {
    fs::write(root.join("provider.wasm"), wasm.wasm()).unwrap();
    fs::write(root.join("descriptor.bin"), wasm.descriptor_bytes()).unwrap();
    fs::write(root.join("binding.bin"), wasm.binding_bytes()).unwrap();
    let cases = serde_json::json!({
        "exports": WASM_EXPORTS,
        "canonical": hex(canonical),
        "hostile": hostile.iter().map(|(id, frame)| serde_json::json!({"id": id, "frame": hex(frame)})).collect::<Vec<_>>(),
    });
    fs::write(root.join("cases.json"), cases.to_string()).unwrap();
    fs::write(root.join("probe.mjs"), include_str!("wasm_comparison.mjs")).unwrap();
    let run = Command::new("node")
        .arg("probe.mjs")
        .current_dir(root)
        .output()
        .expect("comparison requires provisioned Node");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    serde_json::from_slice(&run.stdout).unwrap()
}

/// Measure one subject and return its rendered side-by-side table rows.
fn compare(root: &Path, label: &str, source: &str, expected: [&[u8]; 2]) -> Vec<String> {
    let parsed = semaprax::check(source, Path::new("r07-wasm-comparison.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let descriptor = endpoint.descriptor();
    let (native_source, native_binding) = match label {
        "identity" => {
            let artifact =
                render_authenticated_identity_provider(&program, &revision, descriptor).unwrap();
            (artifact.source().to_owned(), artifact.binding().encode())
        }
        "moves" => {
            let artifact =
                render_authenticated_moves_provider(&program, &revision, descriptor).unwrap();
            (artifact.source().to_owned(), artifact.binding().encode())
        }
        _ => {
            let artifact =
                render_authenticated_allocating_provider(&program, &revision, descriptor).unwrap();
            (artifact.source().to_owned(), artifact.binding().encode())
        }
    };
    let wasm = semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint).unwrap();
    wasm.verify().unwrap();
    // One descriptor, two separately bound target artifacts.
    assert_eq!(wasm.descriptor_bytes(), descriptor.accepted_bytes());
    assert_ne!(wasm.binding_bytes(), native_binding);

    let (_, recipes) = caller_hostility::recipes(descriptor);
    let mut ids: Vec<_> = recipes.iter().map(|case| case.id).collect();
    ids.sort_unstable();
    let mut all = [&TICKET_ONLY[..], &FRAME[..]].concat();
    all.sort_unstable();
    assert_eq!(ids, all, "every frozen recipe is classified exactly once");
    let mut alternate = None;
    let mut frames = Vec::new();
    for case in &recipes {
        match &case.recipe {
            Recipe::Cleanup(bytes) => alternate = Some(*bytes),
            Recipe::Empty(frame) => frames.push((case.id, frame.clone())),
            Recipe::Generation(_) | Recipe::ProviderOwned => {}
        }
    }
    // Present the two frame recipes in the table's order.
    frames.sort_by_key(|(id, _)| FRAME.iter().position(|frame| frame == id).unwrap());
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    let canonical = plan
        .frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .zip(PAYLOADS)
                .map(|(path, payload)| CarrierLeaf::new(path, LeafKind::Bytes, payload.to_vec()))
                .collect(),
        )
        .encode();
    let legacy = flat(&PAYLOADS);
    let mut constants = String::new();
    for (name, bytes) in [
        ("descriptor", descriptor.accepted_bytes()),
        ("binding", &native_binding),
        ("cleanup", descriptor.settlement().digest().as_bytes()),
        ("alternate", alternate.unwrap()),
        ("canonical", &canonical),
        ("wrong_path", &frames[0].1),
        ("wrong_tag", &frames[1].1),
        ("flat", &legacy),
    ] {
        constants.push_str(&array(name, bytes));
    }
    let directory = root.join(label);
    fs::create_dir(&directory).unwrap();
    let native = native_column(&directory, &native_source, &constants);
    let mut hostile = frames.clone();
    hostile.push(("legacy_flat", legacy.clone()));
    let wasm_rows = wasm_column(&directory, &wasm, &canonical, &hostile);

    // Positive control on both targets: exact selected-body leaves.
    let native_canonical = native.iter().find(|row| row[0] == "canonical").unwrap();
    let native_leaves = decode_native(&unhex(&native_canonical[5]));
    assert_eq!(native_leaves, expected);
    let wasm_canonical = wasm_rows.last().unwrap();
    assert_eq!(wasm_canonical["id"], "canonical");
    let wasm_executes = wasm_canonical["status"] == 0;
    if wasm_executes {
        let result_plan =
            CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Result);
        let wasm_result = unhex(wasm_canonical["bytes"].as_str().unwrap());
        let parsed = parse_bounded(&wasm_result).unwrap();
        result_plan.validate_frame(&parsed).unwrap();
        let wasm_leaves: Vec<_> = parsed
            .leaves()
            .iter()
            .map(|leaf| leaf.payload().to_vec())
            .collect();
        assert_eq!(wasm_leaves, native_leaves);
    }

    let mut rows = Vec::new();
    for row in &native {
        let id = row[0].as_str();
        if id == "closed" {
            assert_eq!(row[1..], ["0", "1", "0", "0"], "native close settles all");
            continue;
        }
        let raw: u64 = row[1].parse().unwrap();
        let mut line = format!("{label} | {id} | native raw={raw}");
        if id == "canonical" {
            write!(
                line,
                " entries={} allocated={} live={}",
                row[2], row[3], row[4]
            )
            .unwrap();
        } else {
            write!(
                line,
                " {} entries={} alloc+{} live={}",
                stable("native", raw),
                row[2],
                row[3],
                row[4]
            )
            .unwrap();
        }
        line.push_str(" | ");
        if TICKET_ONLY.contains(&id) {
            let arity = wasm_rows
                .iter()
                .find(|row| row["id"] == "prepare_arity")
                .unwrap();
            write!(line, "wasm no analogue: prepare arity={}", arity["value"]).unwrap();
        } else if id == "canonical" && !wasm_executes {
            // Recorded as measured: no Core Wasm positive control exists here.
            write!(line, "wasm {}", wasm_canonical["status"].as_str().unwrap()).unwrap();
        } else if id == "canonical" {
            write!(
                line,
                "wasm raw={} again={} consumed={} grown={} close={} leaves=native",
                wasm_canonical["status"],
                wasm_canonical["again"],
                wasm_canonical["consumed"],
                wasm_canonical["grown"],
                wasm_canonical["close"]
            )
            .unwrap();
        } else {
            let wasm_row = wasm_rows.iter().find(|row| row["id"] == id).unwrap();
            let raw = wasm_row["status"].as_u64().unwrap();
            // After the refusal the same provider must then settle the
            // canonical call, or (measured) trap exactly as a fresh one does.
            let recovery = if wasm_executes {
                assert_eq!(
                    wasm_row["recovered"], wasm_canonical["bytes"],
                    "{label} {id}"
                );
                "settled".to_owned()
            } else {
                assert_eq!(
                    wasm_row["recovered"], wasm_canonical["status"],
                    "{label} {id}"
                );
                wasm_row["recovered"].as_str().unwrap().to_owned()
            };
            let native_code = stable("native", row[1].parse().unwrap());
            let wasm_code = stable("wasm", raw);
            write!(
                line,
                "wasm raw={raw} {wasm_code} handle={} call={} grown={} repeat={} recovery={recovery} close={} | {}",
                wasm_row["handle"],
                wasm_row["call"],
                wasm_row["grown"],
                wasm_row["repeat"],
                wasm_row["close"],
                if native_code == wasm_code { "same" } else { "DIVERGES" }
            )
            .unwrap();
        }
        rows.push(line);
    }
    rows
}

fn expected(label: &str) -> Vec<String> {
    // Every subject, including the allocating one, now executes on Core Wasm
    // through the provider's own owned-byte runtime.
    let (recovery, close) = ("settled", "0");
    let canonical = "wasm raw=0 again=8 consumed=8 grown=21037056 close=0 leaves=native".to_owned();
    let ticket = |id: &str, raw: u8, code: &str| {
        format!("{label} | {id} | native raw={raw} {code} entries=0 alloc+0 live=0 | wasm no analogue: prepare arity=3")
    };
    // Measured Core Wasm facts, recorded rather than smoothed over:
    // * its input_prepare collapses every non-capacity codec refusal to raw 5,
    //   so a semantic leaf-path substitution is not reported as SPX-PG803;
    // * it performs its one-time private reservation (memory.grow: the 16 MiB
    //   private region plus the owned-byte heap) before carrier admission,
    //   even on a refused first attempt; a repeated refusal grows nothing,
    //   and no handle or dispatch follows either way.
    let frame = |id: &str, raw: u8, code: &str, verdict: &str| {
        format!("{label} | {id} | native raw={raw} {code} entries=0 alloc+0 live=0 | wasm raw=5 SPX-PG801 handle=0 call=8 grown=21037056 repeat=0 recovery={recovery} close={close} | {verdict}")
    };
    vec![
        ticket("stale_generation_replay", 8, "SPX-PG805"),
        ticket("future_generation_replay", 8, "SPX-PG805"),
        ticket("zero_generation_replay", 8, "SPX-PG805"),
        ticket("provider_owned_before_transfer", 7, "SPX-PG804"),
        ticket("substituted_cleanup_plan", 14, "SPX-PG803"),
        frame("substituted_leaf_path", 14, "SPX-PG803", "DIVERGES"),
        frame("unknown_leaf_kind_tag_one_over", 5, "SPX-PG801", "same"),
        frame("legacy_flat", 5, "SPX-PG801", "same"),
        format!("{label} | canonical | native raw=0 entries=1 allocated=1 live=0 | {canonical}"),
    ]
}

#[test]
fn native_and_core_wasm_outcomes_are_measured_side_by_side() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r07-wasm-comparison-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let allocating = format!(
        "{}\n{}",
        SOURCE.replace("{ value }", super::checked_allocating::BODY),
        super::checked_allocating::HELPERS
    );
    let subjects: [(&str, String, [&[u8]; 2]); 3] = [
        ("identity", SOURCE.to_owned(), PAYLOADS),
        (
            "moves",
            SOURCE.replace("{ value }", super::checked_moves::BODY),
            [PAYLOADS[1], PAYLOADS[0]],
        ),
        ("allocating", allocating, [PAYLOADS[0], &[9, 0, 0]]),
    ];
    let mut table = Vec::new();
    for (label, source, leaves) in &subjects {
        let rows = compare(&root, label, source, *leaves);
        assert_eq!(rows, expected(label), "{label} side-by-side outcome table");
        table.extend(rows);
    }
    eprintln!(
        "R07 measured native-vs-Core-Wasm table:\n{}",
        table.join("\n")
    );
    assert_eq!(table.len(), 27);
    fs::remove_dir_all(root).unwrap();
}
