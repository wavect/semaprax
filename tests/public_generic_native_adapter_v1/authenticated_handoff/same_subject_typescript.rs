//! Generated calling-consumer cell for the exact existing matrix subject.
//! The generated package owns framing, artifact admission and lifecycle calls.
use semaprax::{
    public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1,
    public_generic_consumer::{
        rust_calling::{OwnedByteField, RecordShape},
        typescript_calling::generate_typescript_calling_consumer,
    },
    wasm::PublicGenericWasmProviderArtifactV1,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn exact_tsc_version(stdout: &[u8]) -> bool {
    std::str::from_utf8(stdout).is_ok_and(|text| text.trim() == "Version 5.8.3")
}

fn checked_tsc(candidate: &Path) -> Result<PathBuf, String> {
    // Resolve before changing cwd for compilation. An explicit missing or
    // wrong-version tool must not silently fall back to an ambient compiler.
    let resolved = candidate
        .canonicalize()
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    let version = Command::new(&resolved)
        .arg("--version")
        .output()
        .map_err(|error| format!("{}: {error}", resolved.display()))?;
    if !version.status.success() || !exact_tsc_version(&version.stdout) {
        return Err(format!(
            "{} requires exactly TypeScript 5.8.3, got {:?}",
            resolved.display(),
            String::from_utf8_lossy(&version.stdout)
        ));
    }
    Ok(resolved)
}

pub(super) fn locate_tsc() -> Result<PathBuf, String> {
    if let Some(explicit) = env::var_os("SPX_PG_TSC").or_else(|| env::var_os("TSC")) {
        return checked_tsc(Path::new(&explicit));
    }
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            if cfg!(windows) {
                candidates.push(directory.join("tsc.cmd"));
                candidates.push(directory.join("tsc.exe"));
            }
            candidates.push(directory.join("tsc"));
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join("Library/pnpm/tsc"));
        candidates.push(home.join(".local/share/pnpm/tsc"));
    }
    candidates
        .iter()
        .find_map(|candidate| checked_tsc(candidate).ok())
        .ok_or_else(|| "generated consumer requires tsc 5.8.3 (SPX_PG_TSC or TSC)".into())
}

pub(super) fn observe(
    root: &Path,
    endpoint: &AdmittedPublicGenericEndpointV1,
    artifact: &PublicGenericWasmProviderArtifactV1,
    guard: bool,
) -> (u32, Vec<Vec<u8>>) {
    assert!(checked_tsc(&root.join("missing-tsc")).is_err());
    assert!(!exact_tsc_version(b"Version 5.8.30\n"));
    assert!(!exact_tsc_version(b"Version 5.9.0\n"));
    let tsc = locate_tsc().expect("generated consumer requires pinned TypeScript");
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&endpoint.descriptor().input_facts().owned_leaves);
    let output = shape(&endpoint.descriptor().result_facts().owned_leaves);
    assert_eq!(artifact.descriptor_bytes(), endpoint.descriptor_bytes());
    let consumer = generate_typescript_calling_consumer(
        artifact.descriptor_bytes(),
        artifact.binding(),
        &input,
        &output,
    )
    .unwrap();
    let package = root.join("typescript");
    for (name, contents) in consumer.files() {
        let path = package.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    // Only host field identifiers are spelled here. No carrier bytes or host
    // adapter are supplied: Provider.transform uses the generated codec/ABI.
    let names: Vec<_> = input
        .fields
        .iter()
        .map(|field| {
            format!(
                "field_{}",
                field
                    .identity
                    .bytes()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            )
        })
        .collect();
    fs::write(
        package.join("test/subject.json"),
        serde_json::to_vec(&(guard, names, super::PAYLOADS)).unwrap(),
    )
    .unwrap();
    fs::write(
        package.join("test/same-subject.mjs"),
        include_str!("same_subject_typescript.mjs"),
    )
    .unwrap();
    let built = Command::new(&tsc)
        .current_dir(&package)
        .args(["-p", "tsconfig.json"])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "generated tsc: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let executed = Command::new("node")
        .current_dir(&package)
        .args(["test/same-subject.mjs", "../provider.wasm"])
        .output()
        .expect("generated consumer requires Node");
    assert!(
        executed.status.success(),
        "generated Node: {}",
        String::from_utf8_lossy(&executed.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&executed.stderr).trim());
    serde_json::from_slice(&executed.stdout).unwrap()
}
