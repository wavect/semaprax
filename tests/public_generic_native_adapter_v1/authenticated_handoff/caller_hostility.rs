//! A subject-bound physical projection of seven frozen hostile-corpus recipes.
//! These are actual generated clients, not the corpus's synthetic three-leaf
//! document or a replacement adapter. Caller heap allocations are not counted
//! as provider allocations. No broader shape or public-support claim follows.
use super::*;
use semaprax::{
    public_generic_abi::{
        carrier::hostile_corpus::{self, CarrierMutation, TicketMutation},
        descriptor::verify::VerifiedPublicGenericDescriptor,
    },
    public_generic_consumer::{
        c_calling, cxx_calling,
        rust_calling::{OwnedByteField, RecordShape},
    },
};
use std::env;

pub(super) struct Case {
    pub(super) id: &'static str,
    pub(super) source: String,
    pub(super) raw: u8,
}

pub(super) fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(
        source.matches(from).count(),
        1,
        "mutation must hit its exact owning boundary"
    );
    source.replacen(from, to, 1)
}

fn declaration(name: &str, bytes: &[u8]) -> String {
    super::super::array(name, bytes).trim_end().to_owned()
}

/// One of the seven frozen recipes, independent of the caller language that
/// projects it; each projection edits only its generated caller's own ticket,
/// cleanup constant or canonical empty-frame template.
pub(super) enum Recipe {
    Generation(Shift),
    ProviderOwned,
    Cleanup(&'static [u8]),
    Empty(Vec<u8>),
}

pub(super) enum Shift {
    Stale,
    Future,
    Zero,
}

pub(super) struct Hostile {
    pub(super) id: &'static str,
    pub(super) recipe: Recipe,
    pub(super) raw: u8,
}

/// Returns the canonical empty input frame and the seven recipes.
pub(super) fn recipes(descriptor: &VerifiedPublicGenericDescriptor) -> (Vec<u8>, Vec<Hostile>) {
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    let empty = plan
        .frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .map(|path| CarrierLeaf::new(path, LeafKind::Bytes, Vec::new()))
                .collect(),
        )
        .encode();
    let mut output = Vec::new();
    for case in hostile_corpus::cases() {
        let (recipe, raw, code) = match (case.ticket, case.mutation) {
            (
                ticket @ (TicketMutation::StaleGeneration
                | TicketMutation::FutureGeneration
                | TicketMutation::ZeroGeneration),
                CarrierMutation::None,
            ) => {
                let shift = match ticket {
                    TicketMutation::StaleGeneration => Shift::Stale,
                    TicketMutation::FutureGeneration => Shift::Future,
                    TicketMutation::ZeroGeneration => Shift::Zero,
                    _ => unreachable!(),
                };
                (Recipe::Generation(shift), 8, "SPX-PG805")
            }
            (TicketMutation::ProviderOwned, CarrierMutation::None) => {
                (Recipe::ProviderOwned, 7, "SPX-PG804")
            }
            (TicketMutation::AlternateCleanupPlan, CarrierMutation::None) => (
                Recipe::Cleanup(hostile_corpus::ALTERNATE_CLEANUP_PLAN_DIGEST.as_bytes()),
                14,
                "SPX-PG803",
            ),
            (
                TicketMutation::None,
                CarrierMutation::RenameLeafPath(1) | CarrierMutation::LeafKindTag(1, 1),
            ) => {
                let mut changed = empty.clone();
                let path = plan.leaf_paths()[1].as_bytes();
                let offset = changed
                    .windows(path.len())
                    .position(|bytes| bytes == path)
                    .unwrap();
                let (raw, code) = if let CarrierMutation::LeafKindTag(_, tag) = case.mutation {
                    changed[offset + path.len()] = tag;
                    (5, "SPX-PG801")
                } else {
                    changed[offset + path.len() - 1] ^= 0x20;
                    (14, "SPX-PG803")
                };
                super::super::remint(&mut changed);
                let error = match parse_bounded(&changed) {
                    Ok(frame) => plan.validate_frame(&frame).unwrap_err(),
                    Err(error) => error,
                };
                assert_eq!(error.code, code);
                // The production encoder rebuilds the payload-bearing frame and
                // remints its digest; the injected metadata is not a fake codec.
                (Recipe::Empty(changed), raw, code)
            }
            _ => continue,
        };
        assert_eq!(case.expected_code(), Some(code));
        output.push(Hostile {
            id: case.id,
            recipe,
            raw,
        });
    }
    assert_eq!(
        output.len(),
        7,
        "five envelope and two logical recipes must execute"
    );
    (empty, output)
}

pub(super) fn cases(descriptor: &VerifiedPublicGenericDescriptor, source: &str) -> Vec<Case> {
    let (empty, hostile) = recipes(descriptor);
    hostile
        .into_iter()
        .map(|case| {
            let changed = match &case.recipe {
                Recipe::Generation(shift) => {
                    let generation = match shift {
                        Shift::Stale => "generation - 1",
                        Shift::Future => "generation + 1",
                        Shift::Zero => "0",
                    };
                    replace_once(
                        source,
                        "(consumer->provider, generation, SPX_PG_AUTH_OWNERSHIP_CALLER,",
                        &format!(
                            "(consumer->provider, {generation}, SPX_PG_AUTH_OWNERSHIP_CALLER,"
                        ),
                    )
                }
                Recipe::ProviderOwned => replace_once(
                    source,
                    "generation, SPX_PG_AUTH_OWNERSHIP_CALLER,",
                    "generation, SPX_PG_AUTH_OWNERSHIP_PROVIDER,",
                ),
                Recipe::Cleanup(alternate) => replace_once(
                    source,
                    &declaration(
                        "spx_pg_ccc_auth_cleanup",
                        descriptor.settlement().digest().as_bytes(),
                    ),
                    &declaration("spx_pg_ccc_auth_cleanup", alternate),
                ),
                Recipe::Empty(changed) => replace_once(
                    source,
                    &declaration("spx_pg_ccc_auth_empty", &empty),
                    &declaration("spx_pg_ccc_auth_empty", changed),
                ),
            };
            Case {
                id: case.id,
                source: changed,
                raw: case.raw,
            }
        })
        .collect()
}

pub(super) fn run(root: &Path, cxx: bool, opt: &str, expected: i32) {
    run_sanitized(root, cxx, opt, expected, false);
}

/// Like [`run`], additionally compiling with Clang AddressSanitizer +
/// UndefinedBehaviorSanitizer (`-fsanitize=address,undefined
/// -fno-omit-frame-pointer -fno-sanitize-recover=all`, matching this
/// harness's own existing `fixture.rs`/`c_calling_consumer.rs` sanitized
/// pattern) and asserting the exact same stable raw exit status: a genuine
/// sanitizer violation aborts with a different (nondeterministic-looking,
/// but never the expected) status, so this is the same pass/fail oracle,
/// now also proving the compiled path memory/UB-clean under the sanitizer
/// runtime. LOCAL evidence on macOS arm64 only: `ASAN_OPTIONS=detect_leaks=1`
/// is unconditionally fatal here ("detect_leaks is not supported on this
/// platform"), so leak detection is not requested; the existing
/// live-allocation counters (`auth_live`/`fixture_live`) remain the
/// leak/settlement oracle, exactly as for the unsanitized runs.
pub(super) fn run_sanitized(root: &Path, cxx: bool, opt: &str, expected: i32, sanitized: bool) {
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let executable = root.join(format!("probe{opt}{}", env::consts::EXE_SUFFIX));
    let assert_compiled = |command: &mut Command| {
        let result = command.output().expect("required C/C++ toolchain");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    let sanitizer_flags: &[&str] = if sanitized {
        &[
            "-fsanitize=address,undefined",
            "-fno-omit-frame-pointer",
            "-fno-sanitize-recover=all",
        ]
    } else {
        &[]
    };
    if cxx {
        let mut objects = Vec::new();
        for source in ["provider.c", c_calling::CONSUMER_SOURCE_FILE_NAME] {
            let object = root.join(format!("{source}{opt}.o"));
            assert_compiled(
                Command::new(&clang)
                    .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror", "-c"])
                    .args(sanitizer_flags)
                    .arg(root.join(source))
                    .arg("-o")
                    .arg(&object),
            );
            objects.push(object);
        }
        assert_compiled(
            Command::new(env::var_os("CLANGXX").unwrap_or_else(|| "clang++".into()))
                .args(["-std=c++17", opt, "-Wall", "-Wextra", "-Werror"])
                .args(sanitizer_flags)
                .arg("-I")
                .arg(root)
                .arg(root.join("driver.cpp"))
                .args(objects)
                .arg("-o")
                .arg(&executable),
        );
    } else {
        assert_compiled(
            Command::new(clang)
                .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror"])
                .args(sanitizer_flags)
                .arg(root.join("provider.c"))
                .arg(root.join("driver.c"))
                .arg("-o")
                .arg(&executable),
        );
    }
    let mut execute = Command::new(executable);
    if sanitized {
        execute
            .env("ASAN_OPTIONS", "halt_on_error=1")
            .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
    }
    let result = execute.output().unwrap();
    assert_eq!(
        result.status.code(),
        Some(expected),
        "{}: {}",
        root.display(),
        String::from_utf8_lossy(&result.stderr)
    );
    if sanitized {
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !stderr.contains("Sanitizer") && !stderr.contains("runtime error:"),
            "{}: unexpected sanitizer report: {stderr}",
            root.display()
        );
    }
    if cxx && expected == 0 {
        assert_eq!(result.stdout, b"cxx-authenticated-caller-settled");
    }
}

/// The identity-v1 hostile corpus AND its own canonical positive control
/// (the `"canonical"`/raw==0 case below runs at mode 1, the real checked
/// call, not only a refusal). `sanitized` is `false` for this module's own
/// test; [`super::sanitizer_evidence`] calls this again with `true` for its
/// own LOCAL macOS arm64 ASan/UBSan selector, rather than duplicating this
/// function's shape/descriptor/provider assembly a second time.
pub(super) fn run_identity_corpus(sanitized: bool) {
    let root = env::temp_dir().join(format!(
        "semaprax-r07-callers-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    for guard in [true, false] {
        let source = SOURCE.replace(
            "requires true",
            if guard {
                "requires true"
            } else {
                "requires false"
            },
        );
        let parsed = semaprax::check(&source, Path::new("r07-caller.spx")).unwrap();
        let revision = semaprax::format::canonical(&parsed);
        let program = semaprax::hir::resolve(&parsed).unwrap();
        let endpoint =
            derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity")
                .unwrap();
        let descriptor = endpoint.descriptor();
        let artifact =
            render_authenticated_identity_provider(&program, &revision, descriptor).unwrap();
        let provider = format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n", include_str!("../allocations.c"), artifact.source());
        let shape = |paths: &[String]| {
            RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
        };
        let input = shape(&descriptor.input_facts().owned_leaves);
        let output = shape(&descriptor.result_facts().owned_leaves);
        let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
        let frame = plan
            .frame_with_leaves(
                plan.leaf_paths()
                    .iter()
                    .zip(PAYLOADS)
                    .map(|(path, payload)| {
                        CarrierLeaf::new(path, LeafKind::Bytes, payload.to_vec())
                    })
                    .collect(),
            )
            .encode();
        for cxx in [false, true] {
            let files = if cxx {
                cxx_calling::generate_authenticated_identity_calling_consumer_v1(
                    descriptor, &artifact,
                )
                .unwrap()
                .files()
                .to_vec()
            } else {
                c_calling::generate_authenticated_identity_calling_consumer_v1(
                    descriptor, &artifact,
                )
                .unwrap()
                .files()
                .to_vec()
            };
            let original = &files
                .iter()
                .find(|(name, _)| name == c_calling::CONSUMER_SOURCE_FILE_NAME)
                .unwrap()
                .1;
            let mut corpus = cases(descriptor, original);
            corpus.insert(
                0,
                Case {
                    id: "canonical",
                    source: original.clone(),
                    raw: 0,
                },
            );
            let legacy = c_calling::generate_c_calling_consumer(
                descriptor.accepted_bytes(),
                artifact.binding(),
                &input,
                &output,
            )
            .unwrap();
            corpus.push(Case {
                id: "old-flat-refusal",
                source: legacy
                    .files()
                    .iter()
                    .find(|(name, _)| name == c_calling::CONSUMER_SOURCE_FILE_NAME)
                    .unwrap()
                    .1
                    .clone(),
                raw: 5,
            });
            let bypass_source = corpus
                .iter()
                .find(|case| case.id == "future_generation_replay")
                .unwrap()
                .source
                .clone();
            corpus.push(Case {
                id: "generation-check-omission-control",
                source: bypass_source,
                raw: 8,
            });
            for case in corpus {
                let directory = root.join(format!("{guard}-{cxx}-{}", case.id));
                fs::create_dir(&directory).unwrap();
                for (name, bytes) in &files {
                    let path = directory.join(name);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, bytes).unwrap();
                }
                fs::write(
                    directory.join(c_calling::CONSUMER_SOURCE_FILE_NAME),
                    case.source,
                )
                .unwrap();
                let bypass = case.id == "generation-check-omission-control";
                let selected_provider = if bypass {
                    replace_once(&provider, "else if (generation!=spx_pg_auth_generation(slot)) status=SPX_PG_STATUS_HANDLE_INVALID;",
                        "else if (generation==UINT64_MAX) status=SPX_PG_STATUS_HANDLE_INVALID;")
                } else {
                    provider.clone()
                };
                fs::write(directory.join("provider.c"), selected_provider).unwrap();
                let mode = if case.id == "old-flat-refusal" {
                    0
                } else if case.raw == 0 {
                    1
                } else {
                    2
                };
                if cxx {
                    super::cxx::write_driver(&directory, &input, &output, mode, guard, case.raw);
                } else {
                    let field = |path: &str| {
                        format!(
                            "field_{}",
                            path.bytes().map(|b| format!("{b:02x}")).collect::<String>()
                        )
                    };
                    let driver = format!("#define MODE {mode}\n#define EXPECT_REFUSAL {}\n#define EXPECT_CALL_STATUS {}\n#define INPUT0 {}\n#define INPUT1 {}\n#define OUTPUT0 {}\n#define OUTPUT1 {}\n{}", case.raw,
                        if guard {0} else {11}, field(&input.fields[0].identity), field(&input.fields[1].identity),
                        field(&output.fields[0].identity), field(&output.fields[1].identity), include_str!("same_subject_c.c"));
                    fs::write(
                        directory.join("driver.c"),
                        driver.replace("/* CANONICAL_FRAME */", &array("expected_frame", &frame)),
                    )
                    .unwrap();
                }
                for opt in ["-O0", "-O2"] {
                    run_sanitized(&directory, cxx, opt, if bypass { 77 } else { 0 }, sanitized);
                    eprintln!(
                        "R07 caller cxx={cxx} requires={guard} {} {opt}: raw={} oracle={}",
                        case.id,
                        case.raw,
                        if bypass { "detected bypass" } else { "passed" }
                    );
                }
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_c_and_cxx_reject_subject_bound_hostile_handoffs() {
    run_identity_corpus(false);
}
