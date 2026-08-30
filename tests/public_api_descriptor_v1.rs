use semaprax::hir::{self, OwnershipMode, ResolvedType};
use semaprax::project::{
    derive_public_api_descriptor, replay_public_api_descriptor, ProjectManifest,
    PublicApiParameterType, PublicApiResultType, PublicApiSubject,
    MAX_PUBLIC_API_CLOSURE_FUNCTIONS, MAX_PUBLIC_API_EXPORTS, MAX_PUBLIC_API_PARAMETERS,
    PUBLIC_OWNED_DATA_API_SCHEMA, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};
use sha2::{Digest, Sha256};

#[path = "public_api_descriptor_v1/parameter_identity.rs"]
mod parameter_identity;

const PROJECT_REVISION: &str =
    "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const WORKSPACE_REVISION: &str =
    "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const GRAPH_DIGEST: &str =
    "sha256:3333333333333333333333333333333333333333333333333333333333333333";

const ADMITTED: &str = r#"module public.api;

@id("api.bool")
fn bool_value(value: bool) -> bool { value }

@id("api.bytes")
fn bytes_value(value: borrow Slice<u8>) -> Bytes { bytes_copy(value) }

@id("api.i64")
fn integer_value(value: i64) -> i64 { value }

@id("api.mixed")
fn mixed_value(number: i64, flag: bool, text: borrow str, data: borrow Slice<u8>) -> usize {
    if flag { byte_len(data) } else { byte_len(str_as_bytes(text)) + if number == 0 { 0usize } else { 1usize } }
}

@id("api.option")
fn option_value(value: borrow Slice<u8>) -> Option<Bytes> {
    Option<Bytes>::Some { value: bytes_copy(value) }
}

@id("api.result")
fn result_value(value: borrow Slice<u8>) -> Result<Bytes, i64> {
    Result<Bytes, i64>::Ok { value: bytes_copy(value) }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: PROJECT_REVISION,
        workspace_revision: WORKSPACE_REVISION,
        project_graph_digest: GRAPH_DIGEST,
    }
}

fn resolve(source: &str) -> hir::ResolvedProgram {
    let checked = semaprax::check(source, "public-api.spx").unwrap();
    hir::resolve(&checked).unwrap()
}

fn selected() -> Vec<String> {
    [
        "api.bool",
        "api.bytes",
        "api.i64",
        "api.mixed",
        "api.option",
        "api.result",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn remint_descriptor_digest(bytes: &[u8]) -> String {
    remint_descriptor_digest_with_domain(b"semaprax.public-owned-data-api.digest.v1\0", bytes)
}

fn remint_descriptor_digest_with_domain(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("sha256:{hex}")
}

#[test]
fn canonical_descriptor_covers_every_admitted_type_and_replays() {
    let program = resolve(ADMITTED);
    let descriptor = derive_public_api_descriptor(&program, &selected(), subject()).unwrap();
    assert_eq!(descriptor.schema(), PUBLIC_OWNED_DATA_API_SCHEMA);
    assert_eq!(
        descriptor.project_schema(),
        PUBLIC_OWNED_DATA_PROJECT_SCHEMA
    );
    assert_eq!(descriptor.exports().len(), 6);
    assert_eq!(
        descriptor
            .exports()
            .iter()
            .map(|export| export.result())
            .collect::<Vec<_>>(),
        [
            PublicApiResultType::Bool,
            PublicApiResultType::OwnedBytes,
            PublicApiResultType::I64,
            PublicApiResultType::Usize,
            PublicApiResultType::OptionOwnedBytes,
            PublicApiResultType::ResultOwnedBytesI64,
        ]
    );
    let mixed = &descriptor.exports()[3];
    assert_eq!(mixed.typescript_name(), "api.mixed");
    assert_eq!(mixed.rust_method_name(), "spx_api_dot_mixed");
    assert_eq!(
        mixed
            .parameters()
            .iter()
            .map(|parameter| parameter.ty())
            .collect::<Vec<_>>(),
        [
            PublicApiParameterType::I64,
            PublicApiParameterType::Bool,
            PublicApiParameterType::BorrowStr,
            PublicApiParameterType::BorrowSliceU8,
        ]
    );
    assert_eq!(mixed.parameters()[2].source_name(), "text");
    assert!(!mixed.parameters()[2].stable_id().as_str().is_empty());

    let bytes = descriptor.canonical_bytes();
    let digest = descriptor.digest();
    assert_eq!(bytes, descriptor.canonical_bytes());
    assert_eq!(digest, descriptor.digest());
    assert!(digest.starts_with("sha256:"));
    let replayed =
        replay_public_api_descriptor(&program, &selected(), subject(), &bytes, &digest).unwrap();
    assert_eq!(replayed, descriptor);
}

#[test]
fn descriptor_rejects_every_non_profile_signature_family_and_forged_hir() {
    let source = r#"module public.rejected;
@id("bad.array") fn array(value: [u8; 1]) -> i64 { 0 }
@id("bad.bytes") fn bytes(value: own Bytes) -> i64 { 0 }
@id("bad.char") fn character(value: char) -> i64 { 0 }
@id("bad.f32") fn float32(value: f32) -> i64 { 0 }
@id("bad.f64") fn float64(value: f64) -> i64 { 0 }
@id("bad.i32") fn integer32(value: i32) -> i64 { 0 }
@id("bad.string") fn string_value(value: string) -> i64 { 0 }
@id("bad.u8") fn byte(value: u8) -> i64 { 0 }
@id("bad.usize") fn size(value: usize) -> i64 { 0 }
@id("bad.option-error") fn option_error() -> Option<i64> { Option<i64>::None {} }
@id("bad.result-error") fn result_error() -> Result<Bytes, bool> { Result<Bytes, bool>::Err { error: false } }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = resolve(source);
    for id in [
        "bad.array",
        "bad.bytes",
        "bad.char",
        "bad.f32",
        "bad.f64",
        "bad.i32",
        "bad.string",
        "bad.u8",
        "bad.usize",
        "bad.option-error",
        "bad.result-error",
    ] {
        let error =
            derive_public_api_descriptor(&program, &[id.to_owned()], subject()).unwrap_err();
        assert_eq!(
            error.code, "SPX-J113",
            "unexpected rejection for {id}: {error}"
        );
    }

    let baseline = resolve(ADMITTED);
    for ty in [
        ResolvedType::Unit,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::ArrayU8(0),
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
    ] {
        let mut forged = baseline.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "api.i64")
            .unwrap();
        function.params[0].ty = ty;
        assert!(derive_public_api_descriptor(&forged, &["api.i64".to_owned()], subject()).is_err());
    }
    for ownership in [
        OwnershipMode::Own,
        OwnershipMode::Borrow,
        OwnershipMode::Shared,
    ] {
        let mut forged = baseline.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "api.i64")
            .unwrap();
        function.params[0].ownership = ownership;
        assert!(derive_public_api_descriptor(&forged, &["api.i64".to_owned()], subject()).is_err());
    }
}

#[test]
fn selection_identity_closure_and_parameter_limits_fail_closed() {
    let program = resolve(ADMITTED);
    assert!(derive_public_api_descriptor(&program, &[], subject()).is_err());
    assert!(derive_public_api_descriptor(
        &program,
        &["api.bytes".to_owned(), "api.bool".to_owned()],
        subject()
    )
    .is_err());
    assert!(derive_public_api_descriptor(
        &program,
        &["api.bool".to_owned(), "api.bool".to_owned()],
        subject()
    )
    .is_err());
    assert!(derive_public_api_descriptor(&program, &["missing.id".to_owned()], subject()).is_err());
    assert!(derive_public_api_descriptor(&program, &["app.main".to_owned()], subject()).is_err());

    let automatic = resolve(
        "module auto.id; fn hidden() -> i64 { 1 } @id(\"app.main\") fn main() -> i64 { 0 }",
    );
    let automatic_id = automatic
        .functions
        .iter()
        .find(|function| function.name == "hidden")
        .unwrap()
        .id
        .as_str()
        .to_owned();
    assert!(derive_public_api_descriptor(&automatic, &[automatic_id], subject()).is_err());

    let cycle = resolve("module cycle.api; @id(\"cycle.a\") fn a() -> i64 { b() } @id(\"cycle.b\") fn b() -> i64 { a() } @id(\"app.main\") fn main() -> i64 { 0 }");
    assert!(derive_public_api_descriptor(&cycle, &["cycle.a".to_owned()], subject()).is_err());
    let contract = resolve("module contract.api; @id(\"contract.value\") fn value() -> i64 requires true { 1 } @id(\"app.main\") fn main() -> i64 { 0 }");
    assert!(
        derive_public_api_descriptor(&contract, &["contract.value".to_owned()], subject()).is_err()
    );

    let imported = resolve(
        r#"module import.api;
@id("import.host")
interface Host permits {} {
    @id("import.host.echo")
    import rust fn echo(value: i64) -> unit effects {} failure infallible;
}
@id("import.value")
fn value(input: i64) -> i64 {
    let acknowledged = echo(input);
    input
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    assert!(
        derive_public_api_descriptor(&imported, &["import.value".to_owned()], subject()).is_err()
    );

    let effectful = resolve(
        r#"module effect.api;
permit { host.echo }
@id("effect.host")
interface Host permits { host.echo } {
    @id("effect.host.echo")
    import rust fn echo(value: i64) -> i64
        effects { host.echo }
        failure status "host.echo.v1";
}
@id("effect.value")
fn value(input: i64) -> i64 uses { host.echo } { echo(input) }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    assert!(
        derive_public_api_descriptor(&effectful, &["effect.value".to_owned()], subject()).is_err()
    );

    let eight = (0..MAX_PUBLIC_API_PARAMETERS)
        .map(|index| format!("p{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let exact = resolve(&format!(
        "module params.exact; @id(\"params.exact\") fn exact({eight}) -> i64 {{ 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    ));
    assert_eq!(
        derive_public_api_descriptor(&exact, &["params.exact".to_owned()], subject())
            .unwrap()
            .exports()[0]
            .parameters()
            .len(),
        MAX_PUBLIC_API_PARAMETERS
    );
    let nine = (0..=MAX_PUBLIC_API_PARAMETERS)
        .map(|index| format!("p{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let over = resolve(&format!(
        "module params.over; @id(\"params.over\") fn over({nine}) -> i64 {{ 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    ));
    assert!(derive_public_api_descriptor(&over, &["params.over".to_owned()], subject()).is_err());
}

#[test]
fn one_and_maximum_exports_are_bounded_and_deterministic() {
    let one = resolve("module exports.one; @id(\"export.00\") fn value() -> i64 { 0 } @id(\"app.main\") fn main() -> i64 { 0 }");
    let one_descriptor =
        derive_public_api_descriptor(&one, &["export.00".to_owned()], subject()).unwrap();
    assert_eq!(one_descriptor.exports().len(), 1);
    assert_eq!(
        String::from_utf8(one_descriptor.canonical_bytes()).unwrap(),
        concat!(
            "{\"schema\":\"semaprax.public-owned-data-api.v1\",",
            "\"project_schema\":\"semaprax.project.v8\",",
            "\"project_revision\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",",
            "\"workspace_revision\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",",
            "\"project_graph_digest\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",",
            "\"exports\":[{\"stable_id\":\"export.00\",\"typescript_name\":\"export.00\",",
            "\"rust_method_name\":\"spx_export_dot_00\",\"parameters\":[],\"result\":\"i64\"}],",
            "\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,",
            "\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,",
            "\"max_descriptor_bytes\":1048576}}\n"
        )
    );
    assert_eq!(
        one_descriptor.digest(),
        "sha256:4bd0b1e44a1b572731f77259ce1115c8ca408bb409128e1265b63367a47d92f0"
    );
    let mut source = String::from("module exports.maximum;\n");
    let mut ids = Vec::new();
    for index in 0..MAX_PUBLIC_API_EXPORTS {
        let id = format!("export.{index:02}");
        source.push_str(&format!(
            "@id(\"{id}\") fn value_{index}() -> i64 {{ {index} }}\n"
        ));
        ids.push(id);
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    let maximum = resolve(&source);
    let descriptor = derive_public_api_descriptor(&maximum, &ids, subject()).unwrap();
    assert_eq!(descriptor.exports().len(), MAX_PUBLIC_API_EXPORTS);
    ids.push("export.32".to_owned());
    assert!(derive_public_api_descriptor(&maximum, &ids, subject()).is_err());
}

#[test]
fn exact_and_over_limit_closures_are_distinguished() {
    fn chain(functions: usize) -> (hir::ResolvedProgram, Vec<String>) {
        let mut source = String::from("module closure.bound;\n");
        for index in 0..functions {
            let body = if index + 1 == functions {
                "0".to_owned()
            } else {
                format!("f_{}()", index + 1)
            };
            source.push_str(&format!(
                "@id(\"closure.{index:03}\") fn f_{index}() -> i64 {{ {body} }}\n"
            ));
        }
        source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
        (resolve(&source), vec!["closure.000".to_owned()])
    }

    // The linked executable inventory includes the mandatory unselected main
    // function. Therefore 255 selected closure functions + main is the exact
    // 256-function Project-v8 inventory boundary.
    let (exact, selected) = chain(MAX_PUBLIC_API_CLOSURE_FUNCTIONS - 1);
    assert!(derive_public_api_descriptor(&exact, &selected, subject()).is_ok());
    let (over, selected) = chain(MAX_PUBLIC_API_CLOSURE_FUNCTIONS);
    assert!(derive_public_api_descriptor(&over, &selected, subject()).is_err());
}

#[test]
fn every_submitted_byte_and_structural_mutation_fails_replay() {
    let program = resolve(ADMITTED);
    let descriptor = derive_public_api_descriptor(&program, &selected(), subject()).unwrap();
    let bytes = descriptor.canonical_bytes();
    let digest = descriptor.digest();
    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 1;
        assert!(
            replay_public_api_descriptor(&program, &selected(), subject(), &mutated, &digest)
                .is_err()
        );
    }
    for index in [0, bytes.len() / 2, bytes.len() - 1] {
        let mut deleted = bytes.clone();
        deleted.remove(index);
        assert!(
            replay_public_api_descriptor(&program, &selected(), subject(), &deleted, &digest)
                .is_err()
        );
        let mut inserted = bytes.clone();
        inserted.insert(index, b' ');
        assert!(
            replay_public_api_descriptor(&program, &selected(), subject(), &inserted, &digest)
                .is_err()
        );
    }
    for length in [0, 1, bytes.len() / 2, bytes.len() - 1] {
        assert!(replay_public_api_descriptor(
            &program,
            &selected(),
            subject(),
            &bytes[..length],
            &digest
        )
        .is_err());
    }

    let canonical = String::from_utf8(bytes.clone()).unwrap();
    let exports_start = canonical.find("\"exports\":[").unwrap() + "\"exports\":[".len();
    let second_row = canonical[exports_start..]
        .find("},{\"stable_id\":\"api.bytes\"")
        .unwrap()
        + exports_start;
    let mut missing_scalar = canonical.clone();
    missing_scalar.replace_range(exports_start..second_row + 2, "");
    let missing_scalar = missing_scalar.into_bytes();
    assert!(replay_public_api_descriptor(
        &program,
        &selected(),
        subject(),
        &missing_scalar,
        &remint_descriptor_digest(&missing_scalar),
    )
    .is_err());

    let scalar_start = canonical.find("{\"stable_id\":\"api.i64\"").unwrap();
    let scalar_end = canonical[scalar_start..]
        .find("},{\"stable_id\":\"api.mixed\"")
        .unwrap()
        + scalar_start;
    let surplus_row = canonical[scalar_start..scalar_end]
        .replace("api.i64", "api.scalar-surplus")
        .replace("spx_api_dot_i64", "spx_api_dot_scalar_hyphen_surplus");
    let exports_end = canonical.find("],\"limits\":").unwrap();
    let mut surplus_scalar = canonical.clone();
    surplus_scalar.insert_str(exports_end, &format!(",{surplus_row}"));
    let surplus_scalar = surplus_scalar.into_bytes();
    assert!(replay_public_api_descriptor(
        &program,
        &selected(),
        subject(),
        &surplus_scalar,
        &remint_descriptor_digest(&surplus_scalar),
    )
    .is_err());

    let foreign_revision = String::from_utf8(bytes.clone())
        .unwrap()
        .replace(
            PROJECT_REVISION,
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .into_bytes();
    let reminted = remint_descriptor_digest(&foreign_revision);
    assert!(replay_public_api_descriptor(
        &program,
        &selected(),
        subject(),
        &foreign_revision,
        &reminted
    )
    .is_err());
}

#[test]
fn foreign_subjects_and_display_rename_are_distinguished() {
    let before = resolve(ADMITTED);
    let after = resolve(&ADMITTED.replace("fn integer_value", "fn renamed_integer_value"));
    let before_descriptor = derive_public_api_descriptor(&before, &selected(), subject()).unwrap();
    let after_descriptor = derive_public_api_descriptor(&after, &selected(), subject()).unwrap();
    assert_eq!(
        before_descriptor.canonical_bytes(),
        after_descriptor.canonical_bytes()
    );
    assert_eq!(before_descriptor.digest(), after_descriptor.digest());

    let bytes = before_descriptor.canonical_bytes();
    let digest = before_descriptor.digest();
    let foreign_revision = PublicApiSubject {
        project_revision: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ..subject()
    };
    assert!(
        replay_public_api_descriptor(&before, &selected(), foreign_revision, &bytes, &digest)
            .is_err()
    );
    let foreign_graph = PublicApiSubject {
        project_graph_digest:
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ..subject()
    };
    assert!(
        replay_public_api_descriptor(&before, &selected(), foreign_graph, &bytes, &digest).is_err()
    );
}

#[test]
fn legacy_project_v1_through_v7_canonical_manifest_bytes_are_unchanged() {
    let manifests = [
        "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v2\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v3\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v4\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ncapabilities = [\"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v5\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v2\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v6\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"language-command-io.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v7\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"line-command-io.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
    ];
    for manifest in manifests {
        assert_eq!(
            ProjectManifest::parse(manifest)
                .unwrap()
                .to_canonical_toml(),
            manifest
        );
    }
}

#[test]
fn project_v8_is_activated_only_by_manifest_and_profile_admission() {
    let manifest = include_str!("../src/project/manifest.rs");
    let profile = include_str!("../src/project/profile.rs");
    let npm = include_str!("../src/project/npm.rs");
    let native_sdk = include_str!("../src/project/native_sdk.rs");
    let wasm = include_str!("../src/wasm.rs");

    assert!(manifest.contains(PUBLIC_OWNED_DATA_PROJECT_SCHEMA));
    assert!(profile.contains("PROJECT_PROFILE_OWNED_DATA_API_V1"));
    assert!(profile.contains("owned-data-api.v1"));
    for target_orchestrator in [npm, native_sdk, wasm] {
        assert!(!target_orchestrator.contains(PUBLIC_OWNED_DATA_PROJECT_SCHEMA));
        assert!(!target_orchestrator.contains(PUBLIC_OWNED_DATA_API_SCHEMA));
    }
}
