use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "public_native_rust_owned_data_sdk_v1/consumer_toolchain.rs"]
mod consumer_toolchain;
#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;
#[path = "support/native_rust_target.rs"]
mod native_rust_target;

use semaprax::project::{
    derive_public_api_descriptor, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};

const SOURCE: &str = include_str!("../examples/owned-data-rust/owned_data.spx");
const REVISION: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const SELECTED: [&str; 3] = [
    "frame.payload",
    "frame.payload-maybe",
    "frame.payload-result",
];
const PACKAGE_CRATE_FILE: &str = "semaprax-generated-native-rust-owned-data-sdk-0.1.0.crate";
const PACKAGE_CRATE_DIRECTORY: &str = "semaprax-generated-native-rust-owned-data-sdk-0.1.0";
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[path = "public_native_rust_owned_data_sdk_v1/handle_identity.rs"]
mod handle_identity;

fn configured_tool(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(configured) = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
    {
        #[cfg(windows)]
        if variable == "SEMAPRAX_ARCHIVER" {
            return configured;
        }
        if let Ok(canonical) = configured.canonicalize() {
            return canonical;
        }
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| panic!("{variable} must name an installed absolute tool"))
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-owned-data-sdk-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn program(source: &str) -> semaprax::hir::ResolvedProgram {
    semaprax::hir::resolve(&semaprax::check(source, Path::new("owned_data.spx")).unwrap()).unwrap()
}

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: REVISION,
        workspace_revision: REVISION,
        project_graph_digest: REVISION,
    }
}

fn artifact(source: &str) -> semaprax::codegen::NativeOwnedDataProviderArtifact {
    let program = program(source);
    let selected = SELECTED.map(str::to_owned);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap()
}

fn run(command: &mut Command, label: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn release_preview_command() -> Command {
    let mut command = Command::new("python3");
    command.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/generated-package-release.py"));
    command
}

fn preview_tamper_is_refused(prepared: &Path, payload: &str) {
    let path = prepared.join("payload").join(payload);
    let original = fs::read(&path).unwrap();
    let mut altered = original.clone();
    altered[0] ^= 1;
    fs::write(&path, altered).unwrap();
    let output = release_preview_command()
        .args(["check", "--kind", "rust", "--prepared-dir"])
        .arg(prepared)
        .output()
        .unwrap();
    fs::write(&path, original).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("on-disk file digests disagree"),
        "tampered preview must be refused before package use: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// Test-only archive admission, not a registry installer or a concurrent-writer
// sandbox. Python/tool absence fails this provisioned gate. Reuse the preview
// reader for the held, bounded snapshot; never reread generator output as truth.
const CRATE_GUARD: &str = r#"
import gzip, importlib.util, io, json, os, stat, sys, tarfile, tomllib
from pathlib import Path

MODE, INPUT, OUTPUT, PREFIX, RELEASE = sys.argv[1:]
INPUT, OUTPUT = Path(INPUT), Path(OUTPUT)
FILE_LIMIT = 16 * 1024 * 1024
TOTAL_LIMIT = 32 * 1024 * 1024
ARCHIVE_LIMIT = TOTAL_LIMIT + 1024 * 1024

def require(condition, message):
    if not condition:
        raise ValueError(message)

def regular_bytes(path, limit):
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and before.st_size <= limit, 'regular file bound')
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        require((before.st_dev, before.st_ino) == (opened.st_dev, opened.st_ino), 'file replaced')
        data = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    require(len(data) <= limit and len(data) == opened.st_size, 'file byte bound')
    require((opened.st_size, opened.st_mtime_ns) == (after.st_size, after.st_mtime_ns), 'file changed')
    require((before.st_dev, before.st_ino) == (path.lstat().st_dev, path.lstat().st_ino), 'path replaced')
    return data

def manifest_rule(actual, original):
    source = tomllib.loads(original.decode('utf-8'))
    require(set(source) == {'package', 'lib', 'workspace'} and source['workspace'] == {}, 'source manifest shape')
    require(source['lib'] == {'path': 'lib.rs'}, 'source lib shape')
    require(source['package'] == {
        'name': 'semaprax-generated-native-rust-owned-data-sdk', 'version': '0.1.0',
        'edition': '2021', 'rust-version': '1.85', 'publish': False, 'build': 'build.rs',
    }, 'source package shape')
    value = tomllib.loads(actual.decode('utf-8'))
    # Closed Cargo normalization: remove empty [workspace], optionally add only
    # these false auto-discovery/readme defaults and the derived lib name.
    # Original package/lib values stay exact; no dependencies, target tables,
    # new build path, features, patches, includes, links, or executable targets.
    require(set(value) == {'package', 'lib'}, 'normalized manifest tables')
    package = value['package']
    for key in ('autolib', 'autobins', 'autoexamples', 'autotests', 'autobenches', 'readme'):
        if key in package:
            require(package.pop(key) is False, 'normalized manifest default')
    library = value['lib']
    if 'name' in library:
        require(library.pop('name') == source['package']['name'].replace('-', '_'), 'normalized lib name')
    require(package == source['package'] and library == source['lib'], 'normalized manifest changed')

def payload_rule(files, expected):
    required = set(expected) | {'Cargo.toml.orig'}
    require(set(files) in (required, required | {'Cargo.lock'}), 'archive inventory')
    for name, data in expected.items():
        if name != 'Cargo.toml':
            require(files[name] == data, 'payload mismatch: ' + name)
    require(files['Cargo.toml.orig'] == expected['Cargo.toml'], 'original Cargo manifest mismatch')
    manifest_rule(files['Cargo.toml'], expected['Cargo.toml'])
    if 'Cargo.lock' in files:
        lock = tomllib.loads(files['Cargo.lock'].decode('utf-8'))
        require(set(lock) == {'version', 'package'} and type(lock['version']) is int and lock['version'] in (3, 4), 'lock shape')
        require(lock['package'] == [{'name': 'semaprax-generated-native-rust-owned-data-sdk', 'version': '0.1.0'}], 'lock dependency substitution')

def archive_payload(packed, expected):
    require(len(packed) <= ARCHIVE_LIMIT, 'compressed bound')
    with gzip.GzipFile(fileobj=io.BytesIO(packed)) as stream:
        raw = stream.read(ARCHIVE_LIMIT + 1)
    require(len(raw) <= ARCHIVE_LIMIT, 'expanded bound')
    files, offset, total = {}, 0, 0
    allowed = set(expected) | {'Cargo.toml.orig', 'Cargo.lock'}
    while offset + 512 <= len(raw):
        header = raw[offset:offset + 512]
        if header == bytes(512):
            require(len(raw) - offset >= 1024 and not any(raw[offset:]), 'archive trailer')
            payload_rule(files, expected)
            return files
        require(len(files) < len(allowed), 'member count')
        # Parse fixed tar headers ourselves: no transparent PAX/GNU extension
        # allocation, links, directories, devices, sparse members or repair.
        require(header[156:157] in (b'0', b'\0'), 'regular archive members only')
        require(header[257:265] in (b'ustar\00000', b'ustar  \0'), 'tar header format')
        if header[257:265] == b'ustar\00000':
            require(not any(header[345:500]), 'tar prefix forbidden')
        def octal(field):
            text = field.rstrip(b'\0 ').lstrip(b' ')
            require(text and all(byte in b'01234567' for byte in text), 'tar octal')
            return int(text, 8)
        require(octal(header[148:156]) == sum(header[:148]) + 8 * 32 + sum(header[156:]), 'tar checksum')
        name_field = header[:100].split(b'\0', 1)
        require(len(name_field) == 2 and not any(name_field[1]), 'tar name termination')
        name = name_field[0].decode('utf-8')
        require(name.startswith(PREFIX + '/'), 'archive root')
        leaf = name[len(PREFIX) + 1:]
        require(leaf in allowed and '/' not in leaf and '\\' not in leaf, 'archive member path')
        require(leaf not in files, 'duplicate archive member')
        size = octal(header[124:136])
        require(size <= FILE_LIMIT, 'member size')
        total += size
        require(total <= TOTAL_LIMIT, 'payload total')
        start, end = offset + 512, offset + 512 + size
        padded = (end + 511) // 512 * 512
        require(padded <= len(raw) and not any(raw[end:padded]), 'member extent')
        files[leaf] = raw[start:end]
        offset = padded
    raise ValueError('missing archive trailer')

def verify_directory(root, files):
    require(stat.S_ISDIR(root.lstat().st_mode), 'physical extracted directory')
    with os.scandir(root) as entries:
        names = []
        for entry in entries:
            require(len(names) < len(files), 'extracted member count')
            names.append(entry.name)
    require(set(names) == set(files), 'extracted inventory')
    for name, expected in files.items():
        require(regular_bytes(root / name, FILE_LIMIT) == expected, 'extracted payload mismatch: ' + name)

def pack(files, mutation=None):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode='w', format=tarfile.USTAR_FORMAT) as tar:
        for name, data in files.items():
            info = tarfile.TarInfo(PREFIX + '/' + name)
            info.size = len(data)
            if name == 'build.rs' and mutation == 'path':
                info.name = PREFIX + '/../build.rs'
            if name == 'build.rs' and mutation == 'symlink':
                info.type, info.linkname, info.size = tarfile.SYMTYPE, 'lib.rs', 0
            tar.addfile(info, io.BytesIO(data))
            if name == 'build.rs' and mutation == 'duplicate':
                tar.addfile(info, io.BytesIO(data))
    return gzip.compress(output.getvalue(), mtime=0)

try:
    if MODE == 'snapshot':
        spec = importlib.util.spec_from_file_location('release', RELEASE)
        release = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(release)
        release.assert_no_credential_env(os.environ)
        manifest, recomputed, payload = release._read_prepared(INPUT, 'rust')
        release._validate_prepared_manifest('rust', manifest, recomputed, payload, INPUT)
        print(json.dumps({name: data.hex() for name, data in payload.items()}, sort_keys=True))
    else:
        encoded = sys.stdin.buffer.read(2 * TOTAL_LIMIT + 4097)
        require(len(encoded) <= 2 * TOTAL_LIMIT + 4096, 'snapshot bound')
        expected = {name: bytes.fromhex(data) for name, data in json.loads(encoded).items()}
        packed = regular_bytes(INPUT, ARCHIVE_LIMIT)
        files = archive_payload(packed, expected)
        if MODE == 'mutate':
            marker = str(OUTPUT) + '.build-entered'
            files['build.rs'] = ('fn main(){std::fs::write(' + json.dumps(marker) + ',b"entered").unwrap();}\n').encode()
            with OUTPUT.open('xb') as stream:
                stream.write(pack(files))
        elif MODE == 'extract':
            for mutation in ('path', 'symlink', 'duplicate'):
                try:
                    archive_payload(pack(files, mutation), expected)
                except ValueError:
                    pass
                else:
                    raise AssertionError('archive negative accepted: ' + mutation)
            for suffix in ('\n[dependencies]\nsubstituted = "1"\n', '\n[[bin]]\nname = "substituted"\npath = "build.rs"\n'):
                forged = dict(files)
                forged['Cargo.toml'] += suffix.encode()
                try:
                    payload_rule(forged, expected)
                except ValueError:
                    pass
                else:
                    raise AssertionError('executable manifest mutation accepted')
            # No extraction effects occur until the entire archive is admitted.
            OUTPUT.mkdir()
            for name, data in files.items():
                with (OUTPUT / name).open('xb') as stream:
                    stream.write(data)
            verify_directory(OUTPUT, files)
            print('archive admitted; traversal/symlink/duplicate/dependency/target controls refused')
        elif MODE == 'verify':
            verify_directory(OUTPUT, files)
        else:
            raise ValueError('unknown guard mode')
except Exception as error:
    print('crate admission refused: ' + str(error), file=sys.stderr)
    sys.exit(2)
"#;

fn crate_guard(mode: &str, input: &Path, output: &Path, snapshot: &[u8]) -> Output {
    let mut child = Command::new("python3")
        .args(["-c", CRATE_GUARD, mode])
        .arg(input)
        .arg(output)
        .arg(PACKAGE_CRATE_DIRECTORY)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/generated-package-release.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("provisioned Python with tomllib is required");
    child.stdin.take().unwrap().write_all(snapshot).unwrap();
    child.wait_with_output().unwrap()
}

fn checked_consumer_run(
    archive: &Path,
    extracted: &Path,
    snapshot: &[u8],
    command: &mut Command,
    entries: &mut usize,
) -> Result<Output, Output> {
    let admission = crate_guard("verify", archive, extracted, snapshot);
    if !admission.status.success() {
        return Err(admission);
    }
    *entries += 1;
    Ok(run(command, "run byte-verified safe consumer"))
}

fn locked_version<'a>(lock: &'a str, package: &str) -> &'a str {
    let marker = format!("[[package]]\nname = \"{package}\"\nversion = \"");
    lock.split_once(&marker)
        .unwrap_or_else(|| panic!("lockfile is missing `{package}`"))
        .1
        .split_once('"')
        .unwrap()
        .0
}

#[test]
#[ignore = "isolated into the dedicated native-rust-owned-data-sdk-v1 CI job \
            per ADR 0003 answer 6 so an unrelated shard failure cannot hide \
            its result; run explicitly with --ignored there"]
fn standalone_setup_reuses_the_root_native_builder_toolchain_closure() {
    let root = include_str!("../Cargo.lock");
    let setup = include_str!("../examples/owned-data-rust/Cargo.lock");
    for package in ["cc", "find-msvc-tools"] {
        assert_eq!(
            locked_version(setup, package),
            locked_version(root, package)
        );
    }
}

#[test]
#[ignore = "isolated into the dedicated native-rust-owned-data-sdk-v1 CI job \
            per ADR 0003 answer 6 so an unrelated shard failure cannot hide \
            its result; run explicitly with --ignored there"]
fn provider_uses_compiler_layouts_and_rejects_hostile_handles_at_o0_and_o2() {
    assert!(
        Command::new("clang")
            .arg("--version")
            .output()
            .unwrap()
            .status
            .success(),
        "WP12 requires clang"
    );
    let artifact = artifact(SOURCE);
    assert!(artifact
        .source()
        .contains("typedef uint64_t spx_owned_bytes_handle_v1;"));
    assert!(artifact.source().contains("spx_owned_bytes_len_v1"));
    assert!(artifact.source().contains("spx_owned_bytes_copy_v1"));
    assert!(artifact.source().contains("spx_owned_bytes_drop_v1"));

    let fixture = Fixture::new("provider");
    let probe = r#"
int main(void) {
    uint64_t size = spx_owned_data_context_size_v1();
    void *first_storage = malloc((size_t)size);
    void *second_storage = malloc((size_t)size);
    if (first_storage == NULL || second_storage == NULL) return 10;
    if (spx_owned_data_context_init_v1(first_storage, size) != 0 || spx_owned_data_context_init_v1(second_storage, size) != 0) return 11;
    spx_context_v1 *first = (spx_context_v1 *)first_storage;
    spx_context_v1 *second = (spx_context_v1 *)second_storage;
    uint8_t input[3] = { UINT8_C(0), UINT8_C(42), UINT8_C(255) };
    uint32_t tag = UINT32_MAX; uint64_t handle = UINT64_C(0); int64_t error = INT64_C(0);
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, input, UINT64_C(3), &tag, &handle, &error) != 0 || tag != 0 || handle == 0) return 12;
    uint64_t length = UINT64_MAX;
    if (spx_owned_bytes_len_v1(first, handle, &length) != 0 || length != 3) return 13;
    if (spx_owned_bytes_len_v1(second, handle, &length) != SPX_OWNED_DATA_INVALID_HANDLE) return 14;
    uint8_t output[3] = {0};
    if (spx_owned_bytes_copy_v1(first, handle, output, UINT64_C(2)) != SPX_OWNED_DATA_COPY_FAILURE) return 15;
    if (spx_owned_bytes_copy_v1(first, handle, output, UINT64_C(3)) != 0 || memcmp(input, output, 3) != 0) return 16;
    if (spx_owned_bytes_drop_v1(first, handle) != 0) return 17;
    if (spx_owned_bytes_len_v1(first, handle, &length) != SPX_OWNED_DATA_INVALID_HANDLE) return 18;
    if (spx_owned_bytes_drop_v1(first, handle) != SPX_OWNED_DATA_INVALID_HANDLE) return 19;

    tag = UINT32_MAX; handle = 0;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(first, NULL, 0, &tag, &handle, &error) != 0 || tag != 0 || handle != 0) return 20;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(first, input, 3, &tag, &handle, &error) != 0 || tag != 1 || handle == 0) return 21;
    spx_owned_data_test_fault_v1(first, 1);
    if (spx_owned_bytes_copy_v1(first, handle, output, 3) != SPX_OWNED_DATA_COPY_FAILURE) return 22;
    if (spx_owned_bytes_drop_v1(first, handle) != 0) return 23;

    tag = UINT32_MAX; handle = 0; error = 0;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(first, input, 1, &tag, &handle, &error) != 0 || tag != 1 || handle != 0 || error != -7) return 24;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(first, input, 3, &tag, &handle, &error) != 0 || tag != 0 || handle == 0) return 25;
    spx_owned_data_test_fault_v1(first, 2);
    if (spx_owned_bytes_drop_v1(first, handle) != SPX_OWNED_DATA_SETTLEMENT_FAILURE) return 26;
    if (spx_owned_data_context_drop_v1(first) != SPX_OWNED_DATA_SETTLEMENT_FAILURE) return 27;
    if (spx_owned_bytes_drop_v1(first, handle) != 0) return 28;
    for (uint32_t iteration = 0; iteration < UINT32_C(5000); ++iteration) {
        handle = 0;
        if (spx_owned_data_call_spx_frame_dot_payload_v1(first, input, 3, &tag, &handle, &error) != 0 || handle == 0) return 30;
        if (spx_owned_bytes_drop_v1(first, handle) != 0) return 31;
    }
    tag = UINT32_C(77); handle = UINT64_C(0); error = INT64_C(88);
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, NULL, 1, &tag, &handle, &error) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 32;
    if (tag != UINT32_C(77) || handle != UINT64_C(0) || error != INT64_C(88)) return 33;
    uint64_t aliased[2] = { UINT64_C(0), UINT64_C(0) };
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, input, 3, (uint32_t *)&aliased[0], &aliased[0], (int64_t *)&aliased[1]) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 34;
    if (aliased[0] != UINT64_C(0) || aliased[1] != UINT64_C(0)) return 35;
    if (spx_owned_data_context_drop_v1(first) != 0 || spx_owned_data_context_drop_v1(second) != 0) return 29;
    free(first_storage); free(second_storage); return 0;
}

"#;
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join(format!("provider-{optimization}.c"));
        let executable = fixture.0.join(format!("provider-{optimization}"));
        std::fs::write(&c, format!("{}\n{probe}", artifact.source())).unwrap();
        run(
            Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_OWNED_DATA_TESTING",
                ])
                .arg(&c)
                .arg("-o")
                .arg(&executable),
            "compile hostile provider harness",
        );
        run(
            &mut Command::new(executable),
            "run hostile provider harness",
        );
    }
    #[cfg(target_os = "linux")]
    {
        let c = fixture.0.join("provider-sanitized.c");
        let executable = fixture.0.join("provider-sanitized");
        std::fs::write(&c, format!("{}\n{probe}", artifact.source())).unwrap();
        run(
            Command::new("clang")
                .args([
                    "-std=c11",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_OWNED_DATA_TESTING",
                    "-fsanitize=address,undefined",
                    "-fno-sanitize-recover=all",
                ])
                .arg(&c)
                .arg("-o")
                .arg(&executable),
            "compile ASan/UBSan provider harness",
        );
        run(
            &mut Command::new(executable),
            "run ASan/UBSan provider harness",
        );
    }
}

#[test]
#[ignore = "isolated into the dedicated native-rust-owned-data-sdk-v1 CI job \
            per ADR 0003 answer 6 so an unrelated shard failure cannot hide \
            its result; run explicitly with --ignored there"]
fn borrow_str_rejects_invalid_utf8_before_semantic_execution() {
    assert!(
        Command::new("clang")
            .arg("--version")
            .output()
            .unwrap()
            .status
            .success(),
        "WP12 requires clang"
    );
    let source = r#"module owned.utf8;
@id("utf8.payload") fn payload(input: borrow str, data: borrow Slice<u8>) -> Bytes { bytes_copy(data) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = program(source);
    let selected = vec!["utf8.payload".to_owned()];
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let artifact = semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    let fixture = Fixture::new("utf8");
    let probe = r#"int main(void){uint64_t size=spx_owned_data_context_size_v1();void *storage=malloc((size_t)size);if(storage==NULL)return 1;if(spx_owned_data_context_init_v1(storage,size)!=0)return 2;spx_context_v1 *context=(spx_context_v1*)storage;uint8_t invalid[2]={UINT8_C(0xc0),UINT8_C(0x80)};uint32_t tag=91;uint64_t handle=0;int64_t error=92;if(spx_owned_data_call_spx_utf8_dot_payload_v1(context,invalid,2,NULL,0,&tag,&handle,&error)!=SPX_OWNED_DATA_ADAPTER_FAILURE)return 3;if(tag!=91||handle!=0||error!=92)return 4;if(context->invocation!=0)return 5;if(spx_owned_data_context_drop_v1(context)!=0)return 6;free(storage);return 0;}"#;
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join(format!("utf8-{optimization}.c"));
        let executable = fixture.0.join(format!("utf8-{optimization}"));
        std::fs::write(&c, format!("{}\n{probe}", artifact.source())).unwrap();
        run(
            Command::new("clang")
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&c)
                .arg("-o")
                .arg(&executable),
            "compile UTF-8 provider harness",
        );
        run(&mut Command::new(executable), "run UTF-8 provider harness");
    }
}

#[test]
#[ignore = "isolated into the dedicated native-rust-owned-data-sdk-v1 CI job \
            per ADR 0003 answer 6 so an unrelated shard failure cannot hide \
            its result; run explicitly with --ignored there"]
fn descriptor_replay_is_exact_and_display_rename_preserves_the_provider_api() {
    let original = artifact(SOURCE);
    let renamed_source = SOURCE
        .replacen("fn frame_payload(", "fn renamed_payload(", 1)
        .replacen("fn optional_payload(", "fn renamed_optional(", 1)
        .replacen("fn parse_payload(", "fn renamed_parse(", 1);
    let renamed = artifact(&renamed_source);
    assert_eq!(original.descriptor(), renamed.descriptor());
    for symbol in [
        "spx_owned_data_call_spx_frame_dot_payload_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1",
    ] {
        assert!(original.source().contains(symbol));
        assert!(renamed.source().contains(symbol));
    }

    let program = program(SOURCE);
    let selected = SELECTED.map(str::to_owned);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let mut mutated = descriptor.canonical_bytes();
    mutated[0] = b'[';
    assert!(semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &mutated,
        &descriptor.digest(),
    )
    .is_err());
}

#[test]
#[ignore = "isolated into the dedicated native-rust-owned-data-sdk-v1 CI job \
            per ADR 0003 answer 6 so an unrelated shard failure cannot hide \
            its result; run explicitly with --ignored there"]
fn packaged_safe_package_builds_offline_and_fail_stops_on_unsettled_handles() {
    let consumer_toolchain = consumer_toolchain::ConsumerToolchain::from_environment()
        .expect("validate opt-in packaged consumer toolchain before generation");
    assert!(
        Command::new("clang")
            .arg("--version")
            .output()
            .unwrap()
            .status
            .success(),
        "WP12 requires clang"
    );
    let archiver_candidates: &[&str] = if cfg!(windows) {
        &[]
    } else if cfg!(target_os = "macos") {
        &["/usr/bin/libtool"]
    } else {
        &["/usr/bin/ar", "/bin/ar"]
    };
    let archiver = configured_tool("SEMAPRAX_ARCHIVER", archiver_candidates);
    let clang_path = configured_tool("CLANG", &["/usr/bin/clang"]);
    let fixture = Fixture::new("package");
    let generated = fixture.0.join("generated-sdk");
    let setup_target = native_rust_target::CargoTarget::new();
    let consumer_target = native_rust_target::CargoTarget::new();
    let package_target = native_rust_target::CargoTarget::new();
    let setup_manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/owned-data-rust/Cargo.toml");
    run(
        native_rust_cargo::cargo_command()
            .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
            .arg(&setup_manifest)
            .arg("--")
            .arg(&generated)
            .env("CLANG", &clang_path)
            .env("SEMAPRAX_ARCHIVER", &archiver)
            .env("CARGO_TARGET_DIR", setup_target.path()),
        "publish owned-data SDK",
    );

    let mut inventory = std::fs::read_dir(&generated)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    inventory.sort();
    let archive = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    let mut expected = vec![
        "Cargo.toml",
        "build.rs",
        "descriptor.json",
        "lib.rs",
        "owned_data_ffi.rs",
        "semaprax.native-rust-owned-data-sdk.json",
        archive,
    ];
    expected.sort();
    assert_eq!(inventory, expected);
    let public = std::fs::read_to_string(generated.join("lib.rs")).unwrap();
    let ffi = std::fs::read_to_string(generated.join("owned_data_ffi.rs")).unwrap();
    assert!(public.contains("#![forbid(unsafe_code)]"));
    assert!(!public.contains("unsafe{"));
    assert!(ffi.contains("#![allow(unsafe_code)]"));
    assert!(ffi.contains("PhantomData<Rc<()>"));
    assert!(!ffi.contains("spx_owned_data_test_fault_v1"));
    assert!(!public.contains("Handle"));

    // The compiler-generated directory first crosses the existing preview
    // security boundary. The later Cargo archive and independent consumer use
    // only this checked payload snapshot, never the generator output tree.
    let preview = fixture.0.join("preview");
    run(
        Command::new("git")
            .args(["diff", "HEAD", "--exit-code", "--quiet"])
            .current_dir(env!("CARGO_MANIFEST_DIR")),
        "exact local revision requires clean tracked sources",
    );
    let revision = run(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR")),
        "read local tested revision",
    );
    let revision = String::from_utf8(revision.stdout).unwrap();
    let revision = revision.trim();
    assert_eq!(revision.len(), 40);
    assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
    run(
        release_preview_command()
            .args(["prepare", "--kind", "rust", "--package-dir"])
            .arg(&generated)
            .args([
                "--project-name",
                "owned-data-rust",
                "--project-version",
                "0.1.0",
                "--commit",
                revision,
                "--output",
            ])
            .arg(&preview),
        "prepare genuine owned-data SDK preview",
    );
    let preview_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(preview.join("package-preview-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(preview_manifest["commit"], revision);
    eprintln!("local generated-crate test revision: {revision} (not release provenance)");
    preview_tamper_is_refused(&preview, "descriptor.json");
    run(
        release_preview_command()
            .args(["check", "--kind", "rust", "--prepared-dir"])
            .arg(&preview),
        "verify genuine owned-data SDK preview",
    );
    let snapshot = crate_guard("snapshot", &preview, &preview, &[]);
    assert!(
        snapshot.status.success(),
        "checked payload snapshot: {}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    let snapshot = snapshot.stdout;
    let preview_payload = preview.join("payload");

    // Issue #145 requires an external consumer of the archive a registry
    // would contain, rather than a path dependency into the generated output.
    // `cargo package` is offline and does not publish; the extract below is
    // the only dependency root the fresh consumer can reach.
    run(
        native_rust_cargo::cargo_command()
            .args(["package", "--offline", "--no-verify", "--manifest-path"])
            .arg(preview_payload.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", package_target.path()),
        "package owned-data SDK tarball",
    );
    let packaged_tarball = package_target
        .path()
        .join("package")
        .join(PACKAGE_CRATE_FILE);
    let extracted_root = fixture.0.join("packaged-sdk");
    fs::create_dir(&extracted_root).unwrap();
    let extracted = extracted_root.join(PACKAGE_CRATE_DIRECTORY);
    let malicious_archive = fixture.0.join("substituted.crate");
    let mutated = crate_guard("mutate", &packaged_tarball, &malicious_archive, &snapshot);
    assert!(
        mutated.status.success(),
        "{}",
        String::from_utf8_lossy(&mutated.stderr)
    );
    let refused_root = fixture.0.join("refused-extract");
    let refused = crate_guard("extract", &malicious_archive, &refused_root, &snapshot);
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("payload mismatch: build.rs"));
    assert!(
        !refused_root.exists(),
        "refusal precedes extraction/build entry"
    );
    assert!(!fixture.0.join("substituted.crate.build-entered").exists());
    let admitted = crate_guard("extract", &packaged_tarball, &extracted, &snapshot);
    assert!(
        admitted.status.success(),
        "{}",
        String::from_utf8_lossy(&admitted.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&admitted.stdout).trim());
    let packaged_manifest = fs::read_to_string(extracted.join("Cargo.toml")).unwrap();
    assert!(!packaged_manifest.contains(env!("CARGO_MANIFEST_DIR")));
    for forbidden in [
        "semaprax-native-rust-interop-builder",
        "semaprax-native-rust-interop",
        "semaprax-native-rust-owned-data-package",
        "semaprax-native-rust-interop-platform",
        "semaprax-toolchain",
        "/Users/",
        "/home/",
        r"C:\\Users\\",
    ] {
        assert!(
            !packaged_manifest.contains(forbidden),
            "packaged owned-data SDK manifest leaks `{forbidden}`"
        );
    }
    let consumer = fixture.0.join("packaged-consumer");
    fs::create_dir_all(consumer.join("src")).unwrap();
    let consumer_source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/owned-data-rust/consumer");
    let consumer_marker = fixture.0.join("consumer-entered");
    let source = fs::read_to_string(consumer_source.join("src/main.rs")).unwrap();
    assert_eq!(source.matches("fn main() {").count(), 1);
    fs::write(
        consumer.join("src/main.rs"),
        source.replace(
            "fn main() {",
            &format!("fn main() {{ std::fs::write({consumer_marker:?}, b\"entered\").unwrap();"),
        ),
    )
    .unwrap();
    fs::write(
        consumer.join("Cargo.toml"),
        format!(
            "[package]\nname = \"semaprax-owned-data-rust-packaged-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\n\n[workspace]\n\n[dependencies]\nsemaprax-generated-native-rust-owned-data-sdk = {{ path = {extracted:?} }}\n\n[lints.rust]\nunsafe_code = \"forbid\"\n"
        ),
    )
    .unwrap();
    let mut cargo_entries = 0;
    let refused = checked_consumer_run(
        &malicious_archive,
        &extracted,
        &snapshot,
        consumer_toolchain
            .cargo_command()
            .args(["run", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", consumer_target.path()),
        &mut cargo_entries,
    )
    .expect_err("substituted archive build script must fail before Cargo entry");
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("payload mismatch: build.rs"));
    assert_eq!(cargo_entries, 0);
    assert!(!fixture.0.join("substituted.crate.build-entered").exists());
    assert!(!consumer_marker.exists());
    let build_script = extracted.join("build.rs");
    let original_build = fs::read(&build_script).unwrap();
    let build_marker = fixture.0.join("extracted-build-entered");
    fs::write(
        &build_script,
        format!("fn main() {{ std::fs::write({build_marker:?}, b\"entered\").unwrap(); }}\n"),
    )
    .unwrap();
    let refused = checked_consumer_run(
        &packaged_tarball,
        &extracted,
        &snapshot,
        consumer_toolchain
            .cargo_command()
            .args(["run", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", consumer_target.path()),
        &mut cargo_entries,
    )
    .expect_err("substituted extracted build script must fail before Cargo entry");
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("extracted payload mismatch: build.rs")
    );
    assert_eq!(cargo_entries, 0);
    assert!(!build_marker.exists());
    assert!(!consumer_marker.exists());
    assert!(!consumer.join("Cargo.lock").exists());
    fs::write(&build_script, original_build).unwrap();
    eprintln!(
        "archive/extracted executable substitutions refused: zero Cargo/build/consumer entry"
    );
    let verified = crate_guard("verify", &packaged_tarball, &extracted, &snapshot);
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
    run(
        consumer_toolchain
            .cargo_command()
            .args(["generate-lockfile", "--offline"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", consumer_target.path()),
        "lock safe consumer",
    );
    let lock_before_run = fs::read(consumer.join("Cargo.lock")).unwrap();
    let consumer_output = checked_consumer_run(
        &packaged_tarball,
        &extracted,
        &snapshot,
        consumer_toolchain
            .cargo_command()
            .args(["run", "--locked", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", consumer_target.path()),
        &mut cargo_entries,
    )
    .expect("exact extracted payload must admit the consumer");
    assert_eq!(cargo_entries, 1);
    assert_eq!(fs::read(consumer_marker).unwrap(), b"entered");
    assert_eq!(consumer_output.stdout, b"42\n");
    assert_eq!(
        fs::read(consumer.join("Cargo.lock")).unwrap(),
        lock_before_run
    );

    #[cfg(not(windows))]
    {
        let testing = fixture.0.join("testing-provider.c");
        let object = fixture.0.join("testing-provider.o");
        let archive_path = fixture.0.join("libtesting_provider.a");
        std::fs::write(&testing, artifact(SOURCE).source()).unwrap();
        run(
            Command::new("clang")
                .args([
                    "-std=c11",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_OWNED_DATA_TESTING",
                    "-c",
                ])
                .arg(&testing)
                .arg("-o")
                .arg(&object),
            "compile test-only fault provider",
        );
        if cfg!(target_os = "macos") {
            run(
                Command::new("/usr/bin/libtool")
                    .args(["-static", "-D", "-o"])
                    .arg(&archive_path)
                    .arg(&object),
                "archive test-only provider",
            );
        } else {
            run(
                Command::new(&archiver)
                    .args(["rcsD"])
                    .arg(&archive_path)
                    .arg(&object),
                "archive test-only provider",
            );
        }
        let test_ffi = ffi
            .replace("fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;", "fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;fn spx_owned_data_test_fault_v1(context:*mut RawContext,fault:u32);")
            .replace("\nstruct Guard<'a>", "\nimpl Context{pub(super) fn inject_fault(&mut self,fault:u32){unsafe{spx_owned_data_test_fault_v1(self.raw.as_ptr(),fault)}}}\nstruct Guard<'a>");
        let test_ffi_path = fixture.0.join("testing_ffi.rs");
        std::fs::write(&test_ffi_path, test_ffi).unwrap();
        let harness = fixture.0.join("settlement.rs");
        std::fs::write(&harness, format!("#[path={:?}]mod ffi;fn main(){{let mode=std::env::args().nth(1).unwrap();let mut context=match ffi::Context::new(){{Ok(v)=>v,Err(_)=>std::process::exit(10)}};let result=context.invoke(|context|{{let raw=match context.call_spx_frame_dot_payload(b\"abc\"){{Ok(v)=>v,Err(_)=>std::process::exit(11)}};context.inject_fault(if mode==\"copy\"{{1}}else{{2}});context.copy_and_settle(raw.handle)}});if mode==\"copy\"{{if !matches!(result,Ok(Err(_))){{std::process::exit(12)}}println!(\"copy-settled\")}}else{{println!(\"value-published\")}}}}", test_ffi_path.display().to_string())).unwrap();
        let executable = fixture.0.join("settlement");
        run(
            consumer_toolchain
                .rustc_command()
                .args(["--edition=2021"])
                .arg(&harness)
                .arg("-L")
                .arg(format!("native={}", fixture.0.display()))
                .args(["-l", "static=testing_provider", "-o"])
                .arg(&executable),
            "compile safe settlement subprocess",
        );
        let copy = run(
            Command::new(&executable).arg("copy"),
            "copy failure settles exactly once",
        );
        assert_eq!(copy.stdout, b"copy-settled\n");
        let failed = Command::new(&executable).arg("drop").output().unwrap();
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty(), "value published before fail-stop");
    }
}
