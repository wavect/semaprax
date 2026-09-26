//! The same seven frozen hostile-corpus recipes, driven through the generated
//! C11/C++17 callers of the private moves-v1 and allocating-v1 profiles. Both
//! share the authenticated prepare entry with identity-v1; this proves neither
//! later profile reaches allocation or its checked endpoint on a substituted
//! generation, ownership, cleanup plan, field path or variant tag. It is
//! native-only, private evidence: no Rust caller exists for these profiles and
//! no Core Wasm, sanitizer, hosted or public-support claim follows.
use super::caller_hostility::{self, replace_once, Case};
use super::*;
use semaprax::public_generic_abi::native::{
    authenticated::{
        render_authenticated_allocating_provider, render_authenticated_moves_provider,
    },
    binding::NativeProviderBindingV1,
};
use semaprax::public_generic_consumer::{
    c_calling, cxx_calling,
    rust_calling::{OwnedByteField, RecordShape},
};

const GENERATION_CHECK: &str =
    "else if (generation!=spx_pg_auth_generation(slot)) status=SPX_PG_STATUS_HANDLE_INVALID;";
const CLEANUP_CHECK: &str = "else if (!spx_pg_bytes_equal(cleanup,cleanup_len,SPX_PG_AUTH_CLEANUP,SPX_PG_AUTH_CLEANUP_LEN)) status=SPX_PG_AUTH_STATUS_REPLAY_MISMATCH;";

/// Every refusal is a stable raw status and a zero-count endpoint/allocation
/// receipt; each omission control removes exactly one provider check and must
/// instead cross into physical work (driver exit 77).
fn run_profile(
    root: &Path,
    label: &str,
    source: &str,
    render: impl Fn(
        &semaprax::hir::ResolvedProgram,
        &str,
        &semaprax::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor,
    ) -> (
        String,
        Vec<(String, String)>,
        Vec<(String, String)>,
        NativeProviderBindingV1,
    ),
) -> usize {
    let parsed = semaprax::check(source, Path::new("r07-profile-hostility.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    let program = semaprax::hir::resolve(&parsed).unwrap();
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity").unwrap();
    let descriptor = endpoint.descriptor();
    // The identity-v1 renderer refuses these bodies, so a pass here cannot be
    // an identity artifact under another name.
    assert_eq!(
        render_authenticated_identity_provider(&program, &revision, descriptor)
            .err()
            .unwrap()
            .code,
        "SPX-B103"
    );
    let (artifact_source, c_files, cxx_files, binding) = render(&program, &revision, descriptor);
    let provider = format!("{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{artifact_source}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n", include_str!("../allocations.c"));
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let legacy = c_calling::generate_c_calling_consumer(
        descriptor.accepted_bytes(),
        &binding,
        &input,
        &output,
    )
    .unwrap();
    let mut processes = 0;
    for cxx in [false, true] {
        let files = if cxx { &cxx_files } else { &c_files };
        let original = &files
            .iter()
            .find(|(name, _)| name == c_calling::CONSUMER_SOURCE_FILE_NAME)
            .unwrap()
            .1;
        let mut corpus = caller_hostility::cases(descriptor, original);
        let find = |corpus: &[Case], id: &str| {
            corpus
                .iter()
                .find(|case| case.id == id)
                .unwrap()
                .source
                .clone()
        };
        let generation_bypass = find(&corpus, "future_generation_replay");
        let cleanup_bypass = find(&corpus, "substituted_cleanup_plan");
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
        corpus.push(Case {
            id: "generation-check-omission-control",
            source: generation_bypass,
            raw: 8,
        });
        corpus.push(Case {
            id: "cleanup-check-omission-control",
            source: cleanup_bypass,
            raw: 14,
        });
        for case in corpus {
            let directory = root.join(format!("{label}-{cxx}-{}", case.id));
            fs::create_dir(&directory).unwrap();
            for (name, bytes) in files {
                let path = directory.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, bytes).unwrap();
            }
            fs::write(
                directory.join(c_calling::CONSUMER_SOURCE_FILE_NAME),
                &case.source,
            )
            .unwrap();
            // Each mutant removes one check but keeps every referenced name
            // observed, so the -Werror build compiles the omission itself.
            let omitted = match case.id {
                "generation-check-omission-control" => Some((
                    GENERATION_CHECK,
                    "else if (generation==UINT64_MAX) status=SPX_PG_STATUS_HANDLE_INVALID;",
                )),
                "cleanup-check-omission-control" => Some((
                    CLEANUP_CHECK,
                    "else if (!spx_pg_bytes_equal(SPX_PG_AUTH_CLEANUP,SPX_PG_AUTH_CLEANUP_LEN,SPX_PG_AUTH_CLEANUP,SPX_PG_AUTH_CLEANUP_LEN)) status=SPX_PG_AUTH_STATUS_REPLAY_MISMATCH;",
                )),
                _ => None,
            };
            let selected = match omitted {
                Some((check, mutant)) => replace_once(&provider, check, mutant),
                None => provider.clone(),
            };
            fs::write(directory.join("provider.c"), selected).unwrap();
            let mode = if case.id == "old-flat-refusal" { 0 } else { 2 };
            if cxx {
                super::cxx::write_driver(&directory, &input, &output, mode, true, case.raw);
            } else {
                let field = |path: &str| {
                    format!(
                        "field_{}",
                        path.bytes().map(|b| format!("{b:02x}")).collect::<String>()
                    )
                };
                let driver = format!(
                    "#define MODE {mode}\n#define EXPECT_REFUSAL {}\n#define EXPECT_CALL_STATUS 0\n#define INPUT0 {}\n#define INPUT1 {}\n#define OUTPUT0 {}\n#define OUTPUT1 {}\n{}",
                    case.raw,
                    field(&input.fields[0].identity),
                    field(&input.fields[1].identity),
                    field(&output.fields[0].identity),
                    field(&output.fields[1].identity),
                    include_str!("same_subject_c.c")
                );
                fs::write(directory.join("driver.c"), driver).unwrap();
            }
            for opt in ["-O0", "-O2"] {
                caller_hostility::run(&directory, cxx, opt, if omitted.is_some() { 77 } else { 0 });
                processes += 1;
                eprintln!(
                    "R07 profile-hostility {label} cxx={cxx} {} {opt}: raw={} oracle={}",
                    case.id,
                    case.raw,
                    if omitted.is_some() {
                        "detected check omission"
                    } else {
                        "refused before allocation/endpoint"
                    }
                );
            }
        }
    }
    processes
}

#[test]
fn moves_and_allocating_profiles_reject_the_hostile_corpus_before_physical_work() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r07-profiles-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let moves = SOURCE.replace("{ value }", super::checked_moves::BODY);
    let mut processes = run_profile(&root, "moves", &moves, |program, revision, descriptor| {
        let artifact = render_authenticated_moves_provider(program, revision, descriptor).unwrap();
        let c = c_calling::generate_authenticated_moves_calling_consumer_v1(descriptor, &artifact)
            .unwrap();
        let cxx =
            cxx_calling::generate_authenticated_moves_calling_consumer_v1(descriptor, &artifact)
                .unwrap();
        (
            artifact.source().to_owned(),
            c.files().to_vec(),
            cxx.files().to_vec(),
            artifact.binding().clone(),
        )
    });
    let allocating = format!(
        "{}\n{}",
        SOURCE.replace("{ value }", super::checked_allocating::BODY),
        super::checked_allocating::HELPERS
    );
    processes += run_profile(
        &root,
        "allocating",
        &allocating,
        |program, revision, descriptor| {
            let artifact =
                render_authenticated_allocating_provider(program, revision, descriptor).unwrap();
            let c = c_calling::generate_authenticated_allocating_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            let cxx = cxx_calling::generate_authenticated_allocating_calling_consumer_v1(
                descriptor, &artifact,
            )
            .unwrap();
            (
                artifact.source().to_owned(),
                c.files().to_vec(),
                cxx.files().to_vec(),
                artifact.binding().clone(),
            )
        },
    );
    // Two profiles x two languages x (7 recipes + legacy flat + 2 omission
    // controls) x O0/O2.
    assert_eq!(processes, 80);
    fs::remove_dir_all(root).unwrap();
}
