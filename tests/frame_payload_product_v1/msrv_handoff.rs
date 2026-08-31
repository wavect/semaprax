//! Explicit producer, not an MSRV consumer run. A trusted provisioner supplies a
//! private, quiescent Unix directory; no hostile-writer or provenance claim is
//! made. Retain everything for a separate compiler-free, offline consumer host.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write as _};

const FILE_LIMIT: u64 = 16 * 1024 * 1024;
const CONSUMER_FILES: [&str; 4] = ["Cargo.toml", "Cargo.lock", "corpus.json", "src/main.rs"];
const SOURCE_FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/frame.spx",
    "src/tests.spx",
];
const ROOTS: [&str; 6] = [
    "before-generated-sdk",
    "after-generated-sdk",
    "before-rust",
    "before-rust-adversarial",
    "after-rust",
    "after-rust-adversarial",
];

fn plain_directory(path: &Path) -> Result<(), &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "directory is missing")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("expected a plain directory");
    }
    Ok(())
}

fn admit(root: &Path) -> Result<PathBuf, &'static str> {
    #[cfg(not(unix))]
    {
        let _ = root;
        Err("frame handoff producer requires Unix private-mode and single-link checks")
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !root.is_absolute() {
            return Err("handoff root must be absolute");
        }
        plain_directory(root)?;
        let canonical = root
            .canonicalize()
            .map_err(|_| "cannot canonicalize handoff root")?;
        if canonical != root {
            return Err("handoff root must already be canonical and unaliased");
        }
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        if canonical.starts_with(checkout) {
            return Err("handoff root must be outside the checkout");
        }
        let metadata = fs::symlink_metadata(root).map_err(|_| "cannot inspect handoff root")?;
        if metadata.permissions().mode() & 0o7777 != 0o700 {
            return Err("handoff root must have private mode 0700");
        }
        let mut entries = fs::read_dir(root).map_err(|_| "cannot read handoff root")?;
        if entries.next().is_some() {
            return Err("handoff root must be empty");
        }
        Ok(canonical)
    }
}

fn names(directory: &Path, expected: &[&str]) {
    plain_directory(directory).unwrap();
    let mut actual = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        assert!(
            actual.len() < expected.len(),
            "unexpected entry in {}",
            directory.display()
        );
        actual.push(entry.unwrap().file_name().into_string().unwrap());
    }
    actual.sort();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{}", directory.display());
}

fn read_files(root: &Path, paths: &[&str]) -> BTreeMap<String, Vec<u8>> {
    paths
        .iter()
        .map(|relative| {
            let path = root.join(relative);
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "{}",
                path.display()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(metadata.nlink(), 1, "{}", path.display());
            }
            assert!(metadata.len() <= FILE_LIMIT, "{}", path.display());
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .unwrap()
                .take(FILE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            assert!(bytes.len() as u64 <= FILE_LIMIT, "{}", path.display());
            assert_eq!(bytes.len() as u64, metadata.len(), "{}", path.display());
            ((*relative).to_owned(), bytes)
        })
        .collect()
}

fn transfer_files(root: &Path, files: &BTreeMap<String, Vec<u8>>) {
    fs::create_dir(root).unwrap();
    for (relative, bytes) in files {
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(root.join(relative))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}

fn sdk_files() -> [&'static str; 7] {
    let archive = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    [
        "Cargo.toml",
        "build.rs",
        "lib.rs",
        "owned_data_ffi.rs",
        "descriptor.json",
        "semaprax.native-rust-owned-data-sdk.json",
        archive,
    ]
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    names(root, &ROOTS);
    let sdk_files = sdk_files();
    let mut result = BTreeMap::new();
    for directory in ROOTS {
        let path = root.join(directory);
        let files = if directory.ends_with("-generated-sdk") {
            names(&path, &sdk_files);
            read_files(&path, &sdk_files)
        } else {
            names(&path, &["Cargo.toml", "Cargo.lock", "corpus.json", "src"]);
            names(&path.join("src"), &["main.rs"]);
            read_files(&path, &CONSUMER_FILES)
        };
        for (file, bytes) in files {
            assert!(result
                .insert(format!("{directory}/{file}"), bytes)
                .is_none());
        }
    }
    assert_eq!(result.len(), 30);
    result
}

#[test]
#[ignore = "requires an explicit empty private Unix SEMAPRAX_FRAME_CONSUMER_HANDOFF, Clang/archiver/Node and current full compiler; does not run consumer Cargo"]
fn provisioned_frame_consumer_handoff_binds_both_revisions() {
    let supplied = std::env::var_os("SEMAPRAX_FRAME_CONSUMER_HANDOFF")
        .expect("SEMAPRAX_FRAME_CONSUMER_HANDOFF must name a provisioned empty private directory");
    // All rejection above the first compiler/source effect is read-only.
    let root = admit(Path::new(&supplied)).unwrap();
    let scratch = temporary("msrv-handoff-producer");
    assert!(
        !scratch.starts_with(&root),
        "producer scratch must be outside handoff"
    );
    let binary = Path::new(full_toolchain::binary());
    fs::create_dir(&scratch).unwrap();
    eprintln!("retained frame producer scratch: {}", scratch.display());
    let mut products = Vec::new();
    let mut sources = Vec::new();
    let mut sdks = Vec::new();
    let mut retained = BTreeMap::new();
    for (label, renamed) in [("before", false), ("after", true)] {
        let project = scratch.join(format!("{label}-project"));
        copy_project(&project, renamed);
        names(&project, &["semaprax.toml", "src"]);
        names(&project.join("src"), &["app.spx", "frame.spx", "tests.spx"]);
        let source = read_files(&project, &SOURCE_FILES);
        let npm = scratch.join(format!("{label}-npm"));
        let sdk = scratch.join(format!("{label}-generated-sdk"));
        build(binary, &project.join("semaprax.toml"), "npm", &npm);
        build(binary, &project.join("semaprax.toml"), "rust", &sdk);
        // This deliberately runs existing interpreter/native/raw-Wasm proof
        // lanes as well as held-subject, provider and exact package binding.
        products.push(subject_binding::verify_product(&project, &npm, &sdk));
        names(&sdk, &sdk_files());
        let sdk_bytes = read_files(&sdk, &sdk_files());
        transfer_files(&root.join(format!("{label}-generated-sdk")), &sdk_bytes);
        for (file, bytes) in &sdk_bytes {
            assert!(retained
                .insert(format!("{label}-generated-sdk/{file}"), bytes.clone())
                .is_none());
        }
        sdks.push((sdk, sdk_bytes));
        sources.push((project, source));
        for (suffix, corpus) in [("rust", CORPUS), ("rust-adversarial", adversarial::CORPUS)] {
            let consumer = root.join(format!("{label}-{suffix}"));
            fs::create_dir(&consumer).unwrap();
            super::frame_consumer::prepare(&consumer, label, corpus);
            for (file, bytes) in read_files(&consumer, &CONSUMER_FILES) {
                assert!(retained
                    .insert(format!("{label}-{suffix}/{file}"), bytes)
                    .is_none());
            }
        }
    }
    subject_binding::verify_display_rename(&products[0], &products[1]);
    for (project, source) in sources {
        names(&project, &["semaprax.toml", "src"]);
        names(&project.join("src"), &["app.spx", "frame.spx", "tests.spx"]);
        assert_eq!(read_files(&project, &SOURCE_FILES), source);
    }
    for (sdk, bytes) in sdks {
        assert_eq!(read_files(&sdk, &sdk_files()), bytes);
    }
    assert_eq!(snapshot(&root), retained);
    for (path, bytes) in retained {
        let digest = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!("sha256:{digest} {path}");
    }
    // No teardown: provisioner retains these exact files for independent
    // read-only handoff and locked/offline Rust 1.85.1 execution elsewhere.
}

#[cfg(unix)]
#[test]
fn handoff_admission_rejects_nonempty_alias_and_nonprivate_roots_without_effects() {
    use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
    let root = temporary("handoff-admission");
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    assert_eq!(admit(&root), Ok(root.clone()));
    assert_eq!(
        admit(Path::new("relative")),
        Err("handoff root must be absolute")
    );
    let alias = root.join("alias");
    symlink(&root, &alias).unwrap();
    assert_eq!(admit(&alias), Err("expected a plain directory"));
    fs::write(root.join("sentinel"), b"foreign bytes").unwrap();
    assert_eq!(admit(&root), Err("handoff root must be empty"));
    assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"foreign bytes");
    assert_eq!(fs::read_link(alias).unwrap(), root);
    names(&root, &["alias", "sentinel"]);
    let nonprivate = temporary("handoff-nonprivate");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&nonprivate)
        .unwrap();
    fs::set_permissions(&nonprivate, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        admit(&nonprivate),
        Err("handoff root must have private mode 0700")
    );
    names(&nonprivate, &[]);
    eprintln!(
        "retained admission fixtures: {}, {}",
        root.display(),
        nonprivate.display()
    );
}
