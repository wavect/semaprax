//! Generated Rust callers for the private moves-v1 and allocating-v1 profiles,
//! built offline as standalone packages and linked to the actual rendered
//! native provider. They reuse the identity-v1 Rust caller generator; only the
//! profile name, package/library names and bound artifact differ. Private,
//! native-only evidence: no Core Wasm, sanitizer, hosted or public claim.
use super::caller_hostility::{self, replace_once, Recipe, Shift};
use super::rust::{cargo, compile_provider, write_consumer};
use super::*;
use semaprax::public_generic_abi::{
    descriptor::verify::VerifiedPublicGenericDescriptor,
    native::{
        authenticated::{
            render_authenticated_allocating_provider, render_authenticated_moves_provider,
            ALLOCATING_PROFILE, MOVES_PROFILE, PROFILE,
        },
        binding::NativeProviderBindingV1,
    },
};
use semaprax::public_generic_consumer::rust_calling::{
    generate_authenticated_allocating_calling_consumer_v1,
    generate_authenticated_identity_calling_consumer_v1,
    generate_authenticated_moves_calling_consumer_v1, generate_rust_calling_consumer,
    CallingConsumer, OwnedByteField, RecordShape,
};
use sha2::{Digest as _, Sha256};
use std::{env, path::PathBuf};

const GENERATION_CHECK: &str =
    "else if (generation!=spx_pg_auth_generation(slot)) status=SPX_PG_STATUS_HANDLE_INVALID;";
const CLEANUP_CHECK: &str = "else if (!spx_pg_bytes_equal(cleanup,cleanup_len,SPX_PG_AUTH_CLEANUP,SPX_PG_AUTH_CLEANUP_LEN)) status=SPX_PG_AUTH_STATUS_REPLAY_MISMATCH;";
const TICKET: &str = "                generation,\n                0, // caller-owned ticket";

struct Profile {
    label: &'static str,
    library: &'static str,
    expected: [&'static str; 2],
    pin: &'static str,
}

fn digest(consumer: &CallingConsumer) -> String {
    let mut hash = Sha256::new();
    for (name, contents) in consumer.files() {
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update((contents.len() as u64).to_le_bytes());
        hash.update(contents.as_bytes());
    }
    format!("{:x}", semaprax::digest_hex::LowerHex(hash.finalize()))
}

fn file<'a>(consumer: &'a CallingConsumer, name: &str) -> &'a str {
    &consumer
        .files()
        .iter()
        .find(|(file, _)| file == name)
        .unwrap()
        .1
}

fn checked(source: &str) -> (semaprax::hir::ResolvedProgram, String) {
    let parsed = semaprax::check(source, Path::new("r07-profile-rust.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    (semaprax::hir::resolve(&parsed).unwrap(), revision)
}

/// On one subject all three profiles admit, the moves-v1 package is the
/// identity-v1 package except for its profile name, package names and the
/// separately bound provider artifact; allocating-v1 likewise.
fn reuse_controls() {
    let (program, revision) = checked(SOURCE);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let descriptor = endpoint.descriptor();
    let identity = render_authenticated_identity_provider(&program, &revision, descriptor).unwrap();
    let moves = render_authenticated_moves_provider(&program, &revision, descriptor).unwrap();
    let allocating =
        render_authenticated_allocating_provider(&program, &revision, descriptor).unwrap();
    let base = generate_authenticated_identity_calling_consumer_v1(descriptor, &identity).unwrap();
    for (profile, package, generated, binding) in [
        (
            MOVES_PROFILE,
            "moves-",
            generate_authenticated_moves_calling_consumer_v1(descriptor, &moves).unwrap(),
            moves.binding(),
        ),
        (
            ALLOCATING_PROFILE,
            "allocating-",
            generate_authenticated_allocating_calling_consumer_v1(descriptor, &allocating).unwrap(),
            allocating.binding(),
        ),
    ] {
        assert_ne!(binding.encode(), identity.binding().encode());
        assert_eq!(base.files().len(), generated.files().len());
        for ((name, original), (other_name, contents)) in base.files().iter().zip(generated.files())
        {
            assert_eq!(name, other_name);
            match name.as_str() {
                "Cargo.toml" => assert_eq!(
                    contents,
                    &original
                        .replace(
                            "authenticated-rust",
                            &format!("authenticated-{package}rust")
                        )
                        .replace(
                            "authenticated_rust",
                            &format!("authenticated_{}rust", package.replace('-', "_"))
                        )
                ),
                "src/lib.rs" => assert_eq!(contents, &original.replace(PROFILE, profile)),
                "src/descriptor.rs" => assert_ne!(contents, original),
                _ => assert_eq!(contents, original, "{name} must be shared"),
            }
        }
    }
}

fn project(
    directory: &Path,
    descriptor: &VerifiedPublicGenericDescriptor,
    empty: &[u8],
    recipe: &Recipe,
) {
    let edit = |name: &str, from: &str, to: &str| {
        let path = directory.join(name);
        let source = fs::read_to_string(&path).unwrap();
        fs::write(path, replace_once(&source, from, to)).unwrap();
    };
    match recipe {
        Recipe::Generation(shift) => {
            let generation = match shift {
                Shift::Stale => "generation.wrapping_sub(1)",
                Shift::Future => "generation.wrapping_add(1)",
                Shift::Zero => "0",
            };
            edit(
                "src/provider.rs",
                TICKET,
                &TICKET.replace("generation,", &format!("{generation},")),
            );
        }
        Recipe::ProviderOwned => edit("src/provider.rs", TICKET, &TICKET.replace("0, //", "1, //")),
        Recipe::Cleanup(alternate) => edit(
            "src/authenticated.rs",
            &format!(
                "pub(crate) const CLEANUP: &[u8] = &{:?};",
                descriptor.settlement().digest().as_bytes()
            ),
            &format!("pub(crate) const CLEANUP: &[u8] = &{alternate:?};"),
        ),
        Recipe::Empty(changed) => edit(
            "src/authenticated.rs",
            &format!("const EMPTY: &[u8] = &{empty:?};"),
            &format!("const EMPTY: &[u8] = &{changed:?};"),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_case(
    root: &Path,
    target: &Path,
    profile: &Profile,
    id: &str,
    consumer: &CallingConsumer,
    library: &str,
    provider: &str,
    input: &RecordShape,
    output: &RecordShape,
    mode: u8,
    raw: u8,
    edit: impl Fn(&Path),
    expected_exit: i32,
) -> usize {
    let directory = root.join(format!("{}-{id}", profile.label));
    write_consumer(&directory, consumer);
    edit(&directory);
    fs::write(directory.join("provider.c"), provider).unwrap();
    let field = |path: &str| {
        format!(
            "field_{}",
            path.bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    };
    let mut driver = include_str!("profile_rust_driver.rs.txt").to_owned();
    for (token, value) in [
        ("@LIBRARY@", library.to_owned()),
        ("@MODE@", mode.to_string()),
        ("@RAW@", raw.to_string()),
        ("@INPUT0@", field(&input.fields[0].identity)),
        ("@INPUT1@", field(&input.fields[1].identity)),
        ("@OUTPUT0@", field(&output.fields[0].identity)),
        ("@OUTPUT1@", field(&output.fields[1].identity)),
        ("@EXPECT0@", profile.expected[0].to_owned()),
        ("@EXPECT1@", profile.expected[1].to_owned()),
    ] {
        driver = driver.replace(token, &value);
    }
    fs::create_dir_all(directory.join("src/bin")).unwrap();
    fs::write(directory.join("src/bin/profile.rs"), driver).unwrap();
    let locked = cargo(&directory, target)
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap();
    assert!(
        locked.status.success(),
        "offline consumer lock: {}",
        String::from_utf8_lossy(&locked.stderr)
    );
    let mut processes = 0;
    for opt in ["-O0", "-O2"] {
        let provider_dir = compile_provider(&directory, opt);
        let run = cargo(&directory, target)
            .env("SPX_PG_PROVIDER_LIB_DIR", provider_dir)
            .env("SPX_PG_PROVIDER_LIB_NAME", "spx_pg_reference_provider")
            .args([
                "run",
                "--locked",
                "--offline",
                "--quiet",
                "--bin",
                "profile",
            ])
            .output()
            .unwrap();
        assert_eq!(
            run.status.code(),
            Some(expected_exit),
            "Rust {} {id} {opt}: {}",
            profile.label,
            String::from_utf8_lossy(&run.stderr)
        );
        if expected_exit == 0 {
            assert_eq!(run.stdout, b"rust-authenticated-profile-settled\n");
        }
        processes += 1;
        eprintln!(
            "R07 generated Rust {} {id} {opt}: raw={raw} exit={expected_exit}",
            profile.label
        );
    }
    processes
}

fn run_profile(
    root: &Path,
    target: &Path,
    profile: &Profile,
    source: &str,
    render: impl Fn(
        &semaprax::hir::ResolvedProgram,
        &str,
        &VerifiedPublicGenericDescriptor,
    ) -> (String, CallingConsumer, NativeProviderBindingV1),
) -> usize {
    let (program, revision) = checked(source);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let descriptor = endpoint.descriptor();
    assert_eq!(
        render_authenticated_identity_provider(&program, &revision, descriptor)
            .err()
            .unwrap()
            .code,
        "SPX-B103"
    );
    let (artifact_source, consumer, binding) = render(&program, &revision, descriptor);
    assert!(file(&consumer, "Cargo.toml").contains(profile.library));
    let provider = format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{artifact_source}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n", include_str!("../allocations.c"));
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let legacy =
        generate_rust_calling_consumer(descriptor.accepted_bytes(), &binding, &input, &output)
            .unwrap();
    // Each omission mutant removes one provider check but keeps its names
    // referenced, so the -Werror build compiles the omission itself.
    let generation_omitted = replace_once(
        &provider,
        GENERATION_CHECK,
        "else if (generation==UINT64_MAX) status=SPX_PG_STATUS_HANDLE_INVALID;",
    );
    let cleanup_omitted = replace_once(&provider, CLEANUP_CHECK, "else if (!spx_pg_bytes_equal(SPX_PG_AUTH_CLEANUP,SPX_PG_AUTH_CLEANUP_LEN,SPX_PG_AUTH_CLEANUP,SPX_PG_AUTH_CLEANUP_LEN)) status=SPX_PG_AUTH_STATUS_REPLAY_MISMATCH;");
    let call = provider
        .lines()
        .find(|line| line.contains("(&context, &input, &result) != SPX_STATUS_SUCCESS"))
        .unwrap();
    let call_omitted = if call.ends_with('{') {
        replace_once(&provider, &format!("{call}\n        status = SPX_PG_STATUS_CONTRACT_FAILURE;\n        goto settle_arena;\n    }}"), "    result = input;")
    } else {
        replace_once(&provider, call, "    result = input;")
    };
    let (empty, recipes) = caller_hostility::recipes(descriptor);
    let (future, cleanup) = {
        let find = |id: &str| recipes.iter().find(|case| case.id == id).unwrap();
        (
            find("future_generation_replay"),
            find("substituted_cleanup_plan"),
        )
    };
    let lib = profile.library;
    let none = |_: &Path| {};
    let mut processes = run_case(
        root,
        target,
        profile,
        "canonical",
        &consumer,
        lib,
        &provider,
        &input,
        &output,
        1,
        0,
        none,
        0,
    );
    for case in &recipes {
        processes += run_case(
            root,
            target,
            profile,
            case.id,
            &consumer,
            lib,
            &provider,
            &input,
            &output,
            2,
            case.raw,
            |directory| project(directory, descriptor, &empty, &case.recipe),
            0,
        );
    }
    processes += run_case(
        root,
        target,
        profile,
        "old-flat-refusal",
        &legacy,
        "spx_pg_rust_calling_consumer",
        &provider,
        &input,
        &output,
        2,
        5,
        none,
        0,
    );
    processes += run_case(
        root,
        target,
        profile,
        "generation-check-omission-control",
        &consumer,
        lib,
        &generation_omitted,
        &input,
        &output,
        2,
        future.raw,
        |directory| project(directory, descriptor, &empty, &future.recipe),
        77,
    );
    processes += run_case(
        root,
        target,
        profile,
        "cleanup-check-omission-control",
        &consumer,
        lib,
        &cleanup_omitted,
        &input,
        &output,
        2,
        cleanup.raw,
        |directory| project(directory, descriptor, &empty, &cleanup.recipe),
        77,
    );
    processes += run_case(
        root,
        target,
        profile,
        "checked-call-omission-control",
        &consumer,
        lib,
        &call_omitted,
        &input,
        &output,
        1,
        0,
        none,
        42,
    );
    // The byte pin is checked after the physical runs, so a provider mutant
    // (which also moves the bound artifact digest) is caught physically first.
    assert_eq!(
        digest(&consumer),
        profile.pin,
        "{} generated Rust",
        profile.label
    );
    processes
}

#[test]
fn generated_rust_moves_and_allocating_callers_admit_before_physical_handoff() {
    reuse_controls();
    let root = env::temp_dir().join(format!(
        "semaprax-r07-profile-rust-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let target = env::var_os("CARGO_TARGET_DIR")
        .map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/agent-private"),
            PathBuf::from,
        )
        .join("generated-rust")
        .join("r07-profiles");
    let moves = Profile {
        label: "moves",
        library: "spx_pg_private_authenticated_moves_rust_v1",
        // The selected branch swaps the distinct, non-palindromic leaves.
        expected: ["[2, 11, 17, 23]", "[1, 7, 13]"],
        pin: "6cc020f4dec6f0c1fac0c21aba03bbbd826faed7cb981e8dd5e105e70bf6b575",
    };
    let mut processes = run_profile(
        &root,
        &target,
        &moves,
        &SOURCE.replace("{ value }", super::checked_moves::BODY),
        |program, revision, descriptor| {
            let artifact =
                render_authenticated_moves_provider(program, revision, descriptor).unwrap();
            let consumer =
                generate_authenticated_moves_calling_consumer_v1(descriptor, &artifact).unwrap();
            assert_eq!(
                consumer,
                generate_authenticated_moves_calling_consumer_v1(descriptor, &artifact).unwrap()
            );
            let (other_program, other_revision) = checked(SOURCE);
            let other = derive_admitted_public_generic_endpoint_v1(
                &other_program,
                &other_revision,
                "auth.identity",
            )
            .unwrap();
            assert_eq!(
                generate_authenticated_moves_calling_consumer_v1(other.descriptor(), &artifact)
                    .unwrap_err()
                    .code,
                "SPX-PG803"
            );
            (
                artifact.source().to_owned(),
                consumer,
                artifact.binding().clone(),
            )
        },
    );
    let allocating = Profile {
        label: "allocating",
        library: "spx_pg_private_authenticated_allocating_rust_v1",
        // Left is a fresh copy; right is remade by the allocating callee.
        expected: ["[1, 7, 13]", "[9, 0, 0]"],
        pin: "6e0cd7418859e50db7af4daeb3855195ab92212f87a511b832258413cf062eed",
    };
    processes += run_profile(
        &root,
        &target,
        &allocating,
        &format!(
            "{}\n{}",
            SOURCE.replace("{ value }", super::checked_allocating::BODY),
            super::checked_allocating::HELPERS
        ),
        |program, revision, descriptor| {
            let artifact =
                render_authenticated_allocating_provider(program, revision, descriptor).unwrap();
            let consumer =
                generate_authenticated_allocating_calling_consumer_v1(descriptor, &artifact)
                    .unwrap();
            // A descriptor from another subject cannot bind this artifact.
            let (other_program, other_revision) = checked(SOURCE);
            let other = derive_admitted_public_generic_endpoint_v1(
                &other_program,
                &other_revision,
                "auth.identity",
            )
            .unwrap();
            assert_eq!(
                generate_authenticated_allocating_calling_consumer_v1(
                    other.descriptor(),
                    &artifact
                )
                .unwrap_err()
                .code,
                "SPX-PG803"
            );
            (
                artifact.source().to_owned(),
                consumer,
                artifact.binding().clone(),
            )
        },
    );
    // Two profiles x (canonical + 7 recipes + legacy flat + 3 omission
    // controls) x O0/O2.
    assert_eq!(processes, 48);
    fs::remove_dir_all(root).unwrap();
}
