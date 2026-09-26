//! Actual compiler-owned movement bodies through generated C11/C++17 callers.
//! Neither output assembly nor a test label implements the selected endpoint.
#[path = "checked_moves_postconditions.rs"]
mod postconditions;

use super::*;
use semaprax::{
    public_generic_abi::{
        descriptor::verify::VerifiedPublicGenericDescriptor,
        native::authenticated::{
            render_authenticated_moves_provider, AuthenticatedNativeMovesArtifact,
        },
    },
    public_generic_consumer::{
        c_calling, cxx_calling,
        rust_calling::{OwnedByteField, RecordShape},
    },
};

pub(super) const BODY: &str = r"{
    let saved = value;
    if true {
        let left = saved.left;
        let right = saved.right;
        Pair<Bytes> { left: right, right: left }
    } else {
        Pair<Bytes> { left: saved.left, right: saved.right }
    }
}";

fn checked(source: &str) -> (semaprax::hir::ResolvedProgram, String) {
    let parsed = semaprax::check(source, Path::new("checked-moves.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    let again = semaprax::check(&revision, Path::new("checked-moves.spx")).unwrap();
    assert_eq!(revision, semaprax::format::canonical(&again));
    (semaprax::hir::resolve(&parsed).unwrap(), revision)
}

fn admission_controls() {
    // A common admitted identity subject still produces separately bound
    // profiles; moves-v1 is not a silent widening of identity-v1 authority.
    let (program, revision) = checked(SOURCE);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let identity =
        render_authenticated_identity_provider(&program, &revision, endpoint.descriptor()).unwrap();
    let moves =
        render_authenticated_moves_provider(&program, &revision, endpoint.descriptor()).unwrap();
    assert_eq!(identity.descriptor_bytes(), moves.descriptor_bytes());
    assert_ne!(identity.binding().encode(), moves.binding().encode());
    // The allocator calls the ordinary native invariant-abort path on failure;
    // this body must be refused even though the language itself accepts it.
    let source = SOURCE.replace(
        "{ value }",
        "{ Pair<Bytes> { left: bytes_zeroed(1usize), right: bytes_zeroed(2usize) } }",
    );
    let (program, revision) = checked(&source);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let error = render_authenticated_moves_provider(&program, &revision, endpoint.descriptor())
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-B103");
    assert!(error.message.contains("movement body"));
    // Admission examines both branches, including a statically unselected one.
    let source = SOURCE.replace("{ value }", "{ if true { value } else { Pair<Bytes> { left: bytes_zeroed(1usize), right: bytes_zeroed(2usize) } } }");
    let (program, revision) = checked(&source);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    assert_eq!(
        render_authenticated_moves_provider(&program, &revision, endpoint.descriptor())
            .err()
            .unwrap()
            .code,
        "SPX-B103"
    );
}

fn provider(artifact: &AuthenticatedNativeMovesArtifact) -> String {
    format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n", include_str!("../allocations.c"), artifact.source())
}

fn write_driver(
    root: &Path,
    descriptor: &VerifiedPublicGenericDescriptor,
    mode: u8,
    guard: bool,
    swap: bool,
    cxx: bool,
) {
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let field = |path: &str| {
        format!(
            "field_{}",
            path.bytes().map(|b| format!("{b:02x}")).collect::<String>()
        )
    };
    if cxx {
        super::cxx::write_driver(root, &input, &output, mode, guard, 14);
        let path = root.join("driver.cpp");
        let mut driver = fs::read_to_string(&path).unwrap();
        for (index, expected) in if swap {
            ["2, 11, 17, 23", "1, 7, 13"]
        } else {
            ["1, 7, 13", "2, 11, 17, 23"]
        }
        .iter()
        .enumerate()
        {
            let original = if index == 0 {
                "1, 7, 13"
            } else {
                "2, 11, 17, 23"
            };
            let field = field(&output.fields[index].identity);
            let from = format!(
                "assert(to_owned(last.{field}()) == std::vector<std::uint8_t>({{{original}}}));"
            );
            assert_eq!(driver.matches(&from).count(), 1);
            driver = driver.replace(&from, &format!("if (to_owned(last.{field}()) != std::vector<std::uint8_t>({{{expected}}})) return 42;"));
        }
        fs::write(path, driver).unwrap();
    } else {
        let mut driver = format!("#define MODE {mode}\n#define EXPECT_CALL_STATUS {}\n#define INPUT0 {}\n#define INPUT1 {}\n#define OUTPUT0 {}\n#define OUTPUT1 {}\n{}", if guard {0} else {11}, field(&input.fields[0].identity), field(&input.fields[1].identity), field(&output.fields[0].identity), field(&output.fields[1].identity), include_str!("same_subject_c.c"));
        let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
        let frame = plan
            .frame_with_leaves(
                plan.leaf_paths()
                    .iter()
                    .zip(PAYLOADS)
                    .map(|(path, bytes)| CarrierLeaf::new(path, LeafKind::Bytes, bytes.to_vec()))
                    .collect(),
            )
            .encode();
        driver = driver.replace("/* CANONICAL_FRAME */", &array("expected_frame", &frame));
        for (index, expected) in if swap {
            ["second", "first"]
        } else {
            ["first", "second"]
        }
        .iter()
        .enumerate()
        {
            let original = if index == 0 { "first" } else { "second" };
            let from = format!("assert(output.OUTPUT{index}.len == sizeof({original}) && memcmp(output.OUTPUT{index}.data, {original}, sizeof({original})) == 0);");
            assert_eq!(driver.matches(&from).count(), 1);
            driver = driver.replace(&from, &format!("if (output.OUTPUT{index}.len != sizeof({expected}) || memcmp(output.OUTPUT{index}.data, {expected}, sizeof({expected})) != 0) return 42;"));
        }
        fs::write(root.join("driver.c"), driver).unwrap();
    }
}

fn run_callers(
    root: &Path,
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeMovesArtifact,
    swap: bool,
    guard: bool,
) {
    let physical = provider(artifact);
    let call = physical
        .lines()
        .find(|line| line.contains("(&context, &input, &result) != SPX_STATUS_SUCCESS"))
        .unwrap();
    assert_eq!(physical.matches(call).count(), 1);
    let mutant = physical.replace(call, "    result = input;");
    let (canonical, wrong) = empty_frames(descriptor);
    let shape = RecordShape::new(
        descriptor
            .input_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let legacy = c_calling::generate_c_calling_consumer(
        descriptor.accepted_bytes(),
        artifact.binding(),
        &shape,
        &shape,
    )
    .unwrap();
    for cxx in [false, true] {
        let files = if cxx {
            cxx_calling::generate_authenticated_moves_calling_consumer_v1(descriptor, artifact)
                .unwrap()
                .files()
                .to_vec()
        } else {
            c_calling::generate_authenticated_moves_calling_consumer_v1(descriptor, artifact)
                .unwrap()
                .files()
                .to_vec()
        };
        let original = &files
            .iter()
            .find(|(name, _)| name == c_calling::CONSUMER_SOURCE_FILE_NAME)
            .unwrap()
            .1;
        let declaration =
            |bytes: &[u8]| array("spx_pg_ccc_auth_empty", bytes).trim_end().to_owned();
        assert_eq!(original.matches(&declaration(&canonical)).count(), 1);
        let changed = original.replace(&declaration(&canonical), &declaration(&wrong));
        for (label, mode) in [
            ("checked", 1),
            ("wrong-path", 2),
            ("old-flat", 0),
            ("identity-stub", 1),
        ] {
            let omission = label == "identity-stub";
            if omission && !(swap && guard) {
                continue;
            }
            let directory = root.join(format!("{swap}-{guard}-{cxx}-{label}"));
            fs::create_dir(&directory).unwrap();
            for (name, bytes) in &files {
                let path = directory.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, bytes).unwrap();
            }
            let client = match mode {
                0 => {
                    &legacy
                        .files()
                        .iter()
                        .find(|(name, _)| name == c_calling::CONSUMER_SOURCE_FILE_NAME)
                        .unwrap()
                        .1
                }
                2 => &changed,
                _ => original,
            };
            fs::write(directory.join(c_calling::CONSUMER_SOURCE_FILE_NAME), client).unwrap();
            fs::write(
                directory.join("provider.c"),
                if omission { &mutant } else { &physical },
            )
            .unwrap();
            write_driver(&directory, descriptor, mode, guard, swap, cxx);
            for opt in ["-O0", "-O2"] {
                super::caller_hostility::run(&directory, cxx, opt, if omission { 42 } else { 0 });
                eprintln!(
                    "R07 checked moves branch={swap} requires={guard} cxx={cxx} {label} {opt}: {}",
                    if omission {
                        "payload omission detected, exit42"
                    } else {
                        "status/payload/settlement passed"
                    }
                );
            }
        }
    }
}

fn empty_frames(descriptor: &VerifiedPublicGenericDescriptor) -> (Vec<u8>, Vec<u8>) {
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    let empty = |wrong| {
        plan.frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    CarrierLeaf::new(
                        if wrong && i == 0 {
                            "@9:auth.Left"
                        } else {
                            path
                        },
                        LeafKind::Bytes,
                        Vec::new(),
                    )
                })
                .collect(),
        )
        .encode()
    };
    let canonical = empty(false);
    let wrong = empty(true);
    assert_eq!(
        plan.validate_frame(&parse_bounded(&wrong).unwrap())
            .unwrap_err()
            .code,
        "SPX-PG803"
    );
    (canonical, wrong)
}

#[test]
fn generated_c_and_cxx_execute_checked_movement_bodies() {
    admission_controls();
    let root = std::env::temp_dir().join(format!(
        "semaprax-r07-moves-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    for swap in [true, false] {
        for guard in [true, false] {
            let source = SOURCE
                .replace(
                    "{ value }",
                    &BODY.replace("if true", if swap { "if true" } else { "if false" }),
                )
                .replace(
                    "requires true",
                    if guard {
                        "requires true"
                    } else {
                        "requires false"
                    },
                );
            let (program, revision) = checked(&source);
            let endpoint =
                derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity")
                    .unwrap();
            let descriptor = endpoint.descriptor();
            assert_eq!(
                render_authenticated_identity_provider(&program, &revision, descriptor)
                    .err()
                    .unwrap()
                    .code,
                "SPX-B103"
            );
            let artifact =
                render_authenticated_moves_provider(&program, &revision, descriptor).unwrap();
            assert_eq!(
                artifact.source(),
                render_authenticated_moves_provider(&program, &revision, descriptor)
                    .unwrap()
                    .source()
            );
            let (other_program, other_revision) = checked(SOURCE);
            let other = derive_admitted_public_generic_endpoint_v1(
                &other_program,
                &other_revision,
                "auth.identity",
            )
            .unwrap();
            assert_eq!(
                c_calling::generate_authenticated_moves_calling_consumer_v1(
                    other.descriptor(),
                    &artifact
                )
                .unwrap_err()
                .code,
                "SPX-PG803"
            );
            assert_eq!(
                cxx_calling::generate_authenticated_moves_calling_consumer_v1(
                    other.descriptor(),
                    &artifact
                )
                .unwrap_err()
                .code,
                "SPX-PG803"
            );
            run_callers(&root, descriptor, &artifact, swap, guard);
        }
    }
    postconditions::run(&root);
    fs::remove_dir_all(root).unwrap();
}
