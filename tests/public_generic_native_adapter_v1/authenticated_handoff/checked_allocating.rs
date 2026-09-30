//! Real generated C/C++ callers, checked allocating callees, and physical leases.
use super::*;
use semaprax::{
    public_generic_abi::{
        descriptor::verify::VerifiedPublicGenericDescriptor,
        native::authenticated::{
            render_authenticated_allocating_provider, render_authenticated_moves_provider,
            AuthenticatedNativeAllocatingArtifact,
        },
    },
    public_generic_consumer::{c_calling, cxx_calling},
};
use sha2::{Digest as _, Sha256};

pub(super) const HELPERS: &str = r#"
@id("auth.offset")
fn offset(base: usize) -> usize { base }
@id("auth.remake")
fn remake(value: own Bytes, index: usize) -> Bytes {
    let unused_copy = bytes_copy(bytes_as_slice(value));
    let fresh = bytes_set(bytes_zeroed(3usize), index, 9u8);
    fresh
}
"#;
pub(super) const BODY: &str = r"{
    let left = value.left;
    let right = value.right;
    let copied = bytes_copy(bytes_as_slice(left));
    let fresh = remake(right, offset(0usize));
    Pair<Bytes> { left: copied, right: fresh }
}";
const OVERSIZE: &str = "{ Pair<Bytes> { left: value.left, right: bytes_zeroed(65537usize) } }";

#[derive(Clone, Copy)]
struct Case {
    label: &'static str,
    raw: u8,
    issued: usize,
    peak: usize,
    mode: u8,
    live: usize,
    payload_hooks: usize,
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "exact observation/mutation boundary: {from}"
    );
    source.replacen(from, to, 1)
}

fn checked(source: &str) -> (semaprax::hir::ResolvedProgram, String) {
    let parsed = semaprax::check(source, Path::new("checked-allocating.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    assert_eq!(
        revision,
        semaprax::format::canonical(
            &semaprax::check(&revision, Path::new("checked-allocating.spx")).unwrap()
        )
    );
    let graph = semaprax::graph::to_json(&parsed).unwrap();
    assert_eq!(graph, semaprax::graph::to_json(&parsed).unwrap());
    assert!(graph.contains("auth.identity") && graph.contains("core.bytes.drop"));
    if source.contains("fn remake") {
        assert!(graph.contains("auth.remake") && graph.contains("core.bytes.copy"));
        assert!(graph.contains("core.bytes.zeroed") && graph.contains("core.bytes.set"));
    }
    (semaprax::hir::resolve(&parsed).unwrap(), revision)
}

fn frozen_runtime() {
    // Independently extracted from the pre-profile runtime at 69e0b65b.
    // The concatenation is the exact unchanged default emitter order.
    let source = include_str!("../../../src/codegen/native_byte_data.rs");
    let mut runtime = String::new();
    for name in ["PREFIX", "ALLOCATORS", "OPERATIONS", "DROP"] {
        let start = format!("const BYTE_DATA_{name}_C: &str = r#\"");
        runtime.push_str(
            source
                .split_once(&start)
                .unwrap()
                .1
                .split_once("\"#;")
                .unwrap()
                .0,
        );
    }
    assert_eq!(runtime.len(), 7_489);
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(runtime.as_bytes()))
        ),
        "f5f05852a39e264dac30ce5d5c37809b35cf6faf18ef610aea84ae92641f334f"
    );
}

fn observed_provider(artifact: &AuthenticatedNativeAllocatingArtifact) -> String {
    let mut source = artifact.source().to_owned();
    assert!(!source.contains("spx_bytes_copy("));
    assert!(!source.contains("spx_bytes_zeroed("));
    assert_eq!(source.matches("free(value->ptr)").count(), 0);
    source = replace_once(&source, "    arena->live += 1;",
        "    arena->live += 1;\n    ++observed_issued;\n    if (arena->live > observed_peak) observed_peak = arena->live;");
    source = replace_once(
        &source,
        "        arena->live -= 1;",
        "        arena->live -= 1;\n        ++observed_drops;",
    );
    source = replace_once(&source, "    context.target_state = NULL;",
        "    observed_live = arena.live;\n    ++observed_settled;\n    context.target_state = NULL;");
    format!(
        "{}\n{}\n{source}\n{}\n",
        include_str!("../allocations.c"),
        include_str!("checked_allocating_observe.h"),
        include_str!("checked_allocating_observe.c")
    )
}

fn write_driver(root: &Path, descriptor: &VerifiedPublicGenericDescriptor, case: Case, cxx: bool) {
    let mut driver = if cxx {
        include_str!("checked_allocating.cpp")
    } else {
        include_str!("checked_allocating.c")
    }
    .to_owned();
    let field = |path: &str| {
        format!(
            "field_{}",
            path.bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    };
    for (token, value) in [
        ("@INPUT0@", field(&descriptor.input_facts().owned_leaves[0])),
        ("@INPUT1@", field(&descriptor.input_facts().owned_leaves[1])),
        (
            "@OUTPUT0@",
            field(&descriptor.result_facts().owned_leaves[0]),
        ),
        (
            "@OUTPUT1@",
            field(&descriptor.result_facts().owned_leaves[1]),
        ),
        ("@RAW@", case.raw.to_string()),
        ("@ISSUED@", case.issued.to_string()),
        ("@PEAK@", case.peak.to_string()),
        ("@LIVE@", case.live.to_string()),
        ("@MODE@", case.mode.to_string()),
        ("@HOOKS@", case.payload_hooks.to_string()),
    ] {
        driver = driver.replace(token, &value);
    }
    fs::write(
        root.join(if cxx { "driver.cpp" } else { "driver.c" }),
        driver,
    )
    .unwrap();
}

fn write_consumer(
    directory: &Path,
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeAllocatingArtifact,
    cxx: bool,
) {
    let files = if cxx {
        cxx_calling::generate_authenticated_allocating_calling_consumer_v1(descriptor, artifact)
            .unwrap()
            .files()
            .to_vec()
    } else {
        c_calling::generate_authenticated_allocating_calling_consumer_v1(descriptor, artifact)
            .unwrap()
            .files()
            .to_vec()
    };
    for (name, bytes) in files {
        let path = directory.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    if cxx {
        // Observe the wrapper's actual private C carriers just after
        // its unchanged generated call; do not replace that call.
        let path = directory.join(cxx_calling::WRAPPER_HEADER_FILE_NAME);
        let header = fs::read_to_string(&path).unwrap();
        let call = "    const auto status = ::spx_pg_consumer_transform_with_settlement(raw_, &c_input, &c_output, &report);";
        let mut after =
            format!("{call}\n    assert(::spx_pg_consumer_test_live_handles(raw_) == 0);\n");
        for leaf in &descriptor.input_facts().owned_leaves {
            let field = leaf
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            after.push_str(&format!(
                "    assert(!c_input.field_{field}.data && !c_input.field_{field}.len);\n"
            ));
        }
        for leaf in &descriptor.result_facts().owned_leaves {
            let field = leaf
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            after.push_str(&format!("    if (report.native_status != 0) assert(!c_output.field_{field}.data && !c_output.field_{field}.len);\n"));
        }
        fs::write(
            path,
            format!(
                "#include <cassert>\n{}",
                replace_once(&header, call, &after)
            ),
        )
        .unwrap();
    }
}

fn omission_controls(
    directory: &Path,
    descriptor: &VerifiedPublicGenericDescriptor,
    physical: &str,
) {
    let call = physical
        .lines()
        .find(|line| line.contains("(&context, &input, &result) != SPX_STATUS_SUCCESS"))
        .unwrap();
    let call_block = format!("{call}\n        status = SPX_PG_STATUS_CONTRACT_FAILURE;\n        goto settle_arena;\n    }}");
    let omitted = replace_once(physical, &call_block, "    result = input;");
    fs::write(directory.join("provider.c"), omitted).unwrap();
    caller_hostility::run(directory, false, "-O0", 42);
    eprintln!("R07 allocating selected-call omission: exact payload oracle exit42");
    fs::write(directory.join("provider.c"), physical).unwrap();
    write_driver(
        directory,
        descriptor,
        Case {
            label: "wrong-ordinal",
            raw: 10,
            issued: 0,
            peak: 0,
            live: 0,
            payload_hooks: 0,
            mode: 2,
        },
        false,
    );
    caller_hostility::run(directory, false, "-O0", 78);
    eprintln!("R07 allocating wrong allocation ordinal: pre-endpoint oracle exit78");
}

fn run_subject(root: &Path, source: &str, cases: &[Case]) {
    let (program, revision) = checked(source);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let descriptor = endpoint.descriptor();
    assert_eq!(
        render_authenticated_moves_provider(&program, &revision, descriptor)
            .err()
            .unwrap()
            .code,
        "SPX-B103"
    );
    let artifact =
        render_authenticated_allocating_provider(&program, &revision, descriptor).unwrap();
    assert_eq!(
        artifact.source(),
        render_authenticated_allocating_provider(&program, &revision, descriptor)
            .unwrap()
            .source()
    );
    let physical = observed_provider(&artifact);
    for case in cases {
        for cxx in [false, true] {
            let directory = root.join(format!("{}-{cxx}", case.label));
            fs::create_dir(&directory).unwrap();
            write_consumer(&directory, descriptor, &artifact, cxx);
            let mut provider = physical.clone();
            if case.live != 0 {
                let line = provider
                    .lines()
                    .rfind(|line| line.starts_with("    spx_bytes_drop(&result."))
                    .unwrap()
                    .to_owned();
                provider = replace_once(
                    &provider,
                    &line,
                    "    /* Negative: omit one canonical result drop. */",
                );
            }
            fs::write(directory.join("provider.c"), &provider).unwrap();
            write_driver(&directory, descriptor, *case, cxx);
            for opt in ["-O0", "-O2"] {
                caller_hostility::run(&directory, cxx, opt, 0);
                eprintln!("R07 allocating {} cxx={cxx} {opt}: raw{}, issued{}, drops{}, peak{}, live{}, payload-hooks{}, two transforms/close0",
                    case.label, case.raw, case.issued, case.issued - case.live, case.peak, case.live, case.payload_hooks);
            }
            if case.label == "success" && !cxx {
                omission_controls(&directory, descriptor, &physical);
            }
        }
    }
}

#[test]
fn generated_c_and_cxx_settle_reserved_allocating_bodies() {
    frozen_runtime();
    admission_controls();
    let root = std::env::temp_dir().join(format!(
        "semaprax-r07-allocating-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let source = format!("{}\n{HELPERS}", SOURCE.replace("{ value }", BODY));
    let success = Case {
        label: "success",
        raw: 0,
        issued: 5,
        peak: 5,
        mode: 0,
        live: 0,
        payload_hooks: 2,
    };
    run_subject(
        &root,
        &source,
        &[
            success,
            Case {
                label: "reservation-refusal",
                raw: 10,
                issued: 0,
                peak: 0,
                mode: 1,
                live: 0,
                payload_hooks: 0,
            },
            Case {
                label: "drop-omission",
                raw: 11,
                live: 1,
                ..success
            },
        ],
    );
    run_subject(
        &root,
        &source.replace("offset(0usize)", "offset(3usize)"),
        &[Case {
            label: "callee-status",
            raw: 11,
            payload_hooks: 0,
            ..success
        }],
    );
    run_subject(
        &root,
        &source.replace("requires true", "requires true\n    ensures false"),
        &[Case {
            label: "postcondition",
            raw: 11,
            payload_hooks: 0,
            ..success
        }],
    );
    run_subject(
        &root,
        &source.replace("requires true", "requires false"),
        &[Case {
            label: "precondition",
            raw: 11,
            issued: 2,
            peak: 2,
            payload_hooks: 0,
            ..success
        }],
    );
    let source = SOURCE.replace("{ value }", OVERSIZE);
    let oversize = Case {
        label: "oversize",
        raw: 6,
        issued: 3,
        peak: 3,
        mode: 0,
        live: 0,
        payload_hooks: 0,
    };
    run_subject(
        &root,
        &source,
        &[
            oversize,
            Case {
                label: "oversize-sticky-drop-omission",
                live: 1,
                ..oversize
            },
        ],
    );
}

fn admission_controls() {
    let dynamic = SOURCE.replace(
        "{ value }",
        "{ let count = 1usize; Pair<Bytes> { left: bytes_zeroed(count), right: value.right } }",
    );
    let errors = semaprax::check(&dynamic, Path::new("dynamic-capacity.spx")).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-T271"));
    // Other allocator families remain a generation-time refusal, including
    // those in a reached callee or statically unselected branch.
    for body in [
        "{ let text = \"not arena Bytes\"; value }",
        "{ if true { value } else { let text = \"not arena Bytes\"; value } }",
    ] {
        let (program, revision) = checked(&SOURCE.replace("{ value }", body));
        let endpoint =
            derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity")
                .unwrap();
        assert_eq!(
            render_authenticated_allocating_provider(&program, &revision, endpoint.descriptor())
                .err()
                .unwrap()
                .code,
            "SPX-B103"
        );
    }
    // The existing cumulative site bound is checked, not silently truncated
    // to fit a smaller reservation. This is a language diagnostic before HIR
    // admission or any physical artifact operation.
    let allocations: String = (0..33)
        .map(|i| format!("let unused{i} = bytes_zeroed(1usize);"))
        .collect();
    let source = SOURCE.replace("{ value }", &format!("{{ {allocations} value }}"));
    let errors = semaprax::check(&source, Path::new("over-capacity.spx")).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-T267"));
}
