//! Opt-in local inventory; never retains build trees or Cargo caches.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
};
const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARTIFACTS: usize = 2048;
thread_local! {static ACTIVE:RefCell<Option<Inventory>>=const {RefCell::new(None)};}
struct Inventory {
    output: PathBuf,
    root: PathBuf,
    commands: Vec<Value>,
    artifacts: BTreeMap<String, Value>,
    subjects: Vec<Value>,
    tools: BTreeMap<String, Value>,
    status: &'static str,
    matrix: Option<String>,
    receipts: Vec<Value>,
    provenance: Value,
}
pub(super) struct Capture;
impl Capture {
    pub(super) fn start(root: &Path) -> Self {
        ACTIVE.with(|slot| {
            *slot.borrow_mut() = std::env::var_os("SPX_PG_MATRIX_EVIDENCE").map(|base| {
                let output = PathBuf::from(base).join(root.file_name().unwrap());
                fs::create_dir_all(output.parent().unwrap()).unwrap();
                fs::create_dir(&output).expect("evidence run directory must be new");
                Inventory {
                    output,
                    root: root.to_owned(),
                    commands: Vec::new(),
                    artifacts: BTreeMap::new(),
                    subjects: Vec::new(),
                    tools: BTreeMap::new(),
                    status: "incomplete",
                    matrix: None,
                    receipts: Vec::new(),
                    provenance: provenance(),
                }
            });
        });
        Self
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let was_panicking = std::thread::panicking();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ACTIVE.with(|slot| {
                if let Some(state) = slot.borrow_mut().as_mut() {
                    collect(
                        &state.root.clone(),
                        &state.root.clone(),
                        &mut state.artifacts,
                    )
                    .unwrap();
                    if was_panicking {
                        state.status = "failed";
                    } else {
                        state.status = "engine_execution_complete";
                    }
                    write(state);
                }
            })
        }));
        if let Err(error) = result {
            if was_panicking {
                eprintln!("matrix evidence capture also failed during runner unwind");
            } else {
                std::panic::resume_unwind(error);
            }
        }
    }
}
fn hash_file(path: &Path) -> (u64, String) {
    assert!(
        fs::symlink_metadata(path).unwrap().file_type().is_file(),
        "artifact inventory requires a regular file and rejects symlinks"
    );
    let mut file = fs::File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut length = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        length += read as u64;
        digest.update(&buffer[..read]);
    }
    (length, format!("sha256:{:x}", digest.finalize()))
}
fn collect(root: &Path, path: &Path, files: &mut BTreeMap<String, Value>) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        assert!(!kind.is_symlink(), "artifact inventory rejects symlinks");
        if kind.is_dir() {
            collect(root, &entry.path(), files)?;
        } else if kind.is_file() {
            assert!(files.len() < MAX_ARTIFACTS, "artifact inventory bound");
            let (length, hash) = hash_file(&entry.path());
            files.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                json!({"bytes":length,"sha256":hash}),
            );
        }
    }
    Ok(())
}
fn write(state: &Inventory) {
    let document = json!({"schema":"semaprax.public-generic.settlement-matrix-inventory.v1","corpus":super::CORPUS_VERSION_V2,"status":state.status,"subject_inventory":state.subjects,"artifacts":state.artifacts,"commands":state.commands,"tools":state.tools,"matrix":state.matrix,"receipts":state.receipts,"provenance":state.provenance,"scope":"local proof-only; generated assets inventoried by exact content hash; Cargo cache excluded; no hosted or publication claim"});
    let bytes = serde_json::to_vec_pretty(&document).unwrap();
    assert!(
        bytes.len() <= MAX_MANIFEST_BYTES,
        "compact evidence manifest bound"
    );
    fs::write(state.output.join("inventory.json"), bytes).unwrap();
}
pub(super) fn subject(
    name: &str,
    source: &str,
    descriptor: &[u8],
    native_binding: &[u8],
    wasm_binding: &[u8],
) {
    ACTIVE.with(|slot| {if let Some(state)=slot.borrow_mut().as_mut(){state.subjects.push(json!({"subject":name,"canonical_source":source,"descriptor_hex":hex(descriptor),"native_binding_hex":hex(native_binding),"wasm_binding_hex":hex(wasm_binding)}));}});
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub(super) fn effective_cwd(command: &Command) -> std::io::Result<PathBuf> {
    let inherited = std::env::current_dir()?;
    command
        .get_current_dir()
        .map_or(inherited.clone(), |cwd| {
            if cwd.is_absolute() {
                cwd.to_owned()
            } else {
                inherited.join(cwd)
            }
        })
        .canonicalize()
}
fn version_command(command: &Command, effective_cwd: Option<&Path>) -> Command {
    let mut version = Command::new(command.get_program());
    version.arg("--version");
    if let Some(cwd) = effective_cwd {
        version.current_dir(cwd);
    }
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            version.env(key, value);
        } else {
            version.env_remove(key);
        }
    }
    version
}
pub(super) fn command(
    command: &Command,
    label: &str,
    role: Option<&str>,
    cwd: &std::io::Result<PathBuf>,
    result: &std::io::Result<Output>,
) {
    ACTIVE.with(|slot| {if let Some(state)=slot.borrow_mut().as_mut(){
        let program=command.get_program().to_string_lossy().into_owned();
        let args=command.get_args().map(|arg|arg.to_string_lossy().into_owned()).collect::<Vec<_>>();
        let observation=match result {Ok(output)=>json!({"code":output.status.code(),"success":output.status.success(),"status":output.status.to_string(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}),Err(error)=>json!({"spawn_error":error.to_string()})};
        state.commands.push(json!({"label":label,"tool_role":role,"program":program,"args":args,"cwd":cwd.as_ref().ok().map(|p|p.to_string_lossy().into_owned()),"cwd_error":cwd.as_ref().err().map(|error|error.to_string()),"configured_cwd":command.get_current_dir().map(|p|p.to_string_lossy().into_owned()),"env":command.get_envs().map(|(k,v)|(k.to_string_lossy().into_owned(),v.map(|v|v.to_string_lossy().into_owned()))).collect::<BTreeMap<_,_>>(),"result":observation}));
        if let Some(role)=role {
            let identity=format!("{role}:{program}");
            if !state.tools.contains_key(&identity) {
                let version=version_command(command,cwd.as_ref().ok().map(PathBuf::as_path)).output();
                let observed=match version {Ok(output)=>json!({"code":output.status.code(),"success":output.status.success(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}),Err(error)=>json!({"spawn_error":error.to_string()})};
                state.tools.insert(identity,json!({"role":role,"program":program,"args":["--version"],"cwd":cwd.as_ref().ok().map(|p|p.to_string_lossy().into_owned()),"result":observed}));
            }
        }
    }});
}
pub(super) fn external_artifact(name: &str, path: &Path) {
    ACTIVE.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            let (bytes, hash) = hash_file(path);
            state
                .artifacts
                .insert(name.into(), json!({"bytes":bytes,"sha256":hash}));
        }
    });
}
pub(super) fn matrix(matrix: &str) {
    ACTIVE.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.matrix = Some(matrix.into());
            state.status = "matrix_asserted";
            write(state);
        }
    });
}

#[test]
fn inventory_is_exact_and_does_not_follow_symlinks_or_keep_builds() {
    let root =
        std::env::temp_dir().join(format!("spx-matrix-inventory-test-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("artifact"), b"exact bytes").unwrap();
    let mut files = BTreeMap::new();
    collect(&root, &root, &mut files).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files["artifact"]["bytes"], 11);
    assert_eq!(
        files["artifact"]["sha256"],
        format!("sha256:{:x}", Sha256::digest(b"exact bytes"))
    );
    fs::write(root.join("artifact"), b"other bytes").unwrap();
    let mut changed = BTreeMap::new();
    collect(&root, &root, &mut changed).unwrap();
    assert_ne!(files, changed);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("artifact"), root.join("alias")).unwrap();
        let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            collect(&root, &root, &mut BTreeMap::new())
        }));
        assert!(rejected.is_err(), "symlink artifact must fail closed");
    }
    fs::remove_dir_all(root).unwrap();
}

fn provenance() -> Value {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--version", "--verbose"])
        .output()
        .unwrap();
    json!({"git_commit":git(&["rev-parse","HEAD"]).trim(),"git_status":git(&["status","--porcelain"]),"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"rustc_status":rustc.status.to_string(),"rustc":String::from_utf8_lossy(&rustc.stdout),"rustc_stderr":String::from_utf8_lossy(&rustc.stderr)})
}
pub(super) fn receipts(engine: &str, receipts: &[super::Observation]) {
    ACTIVE.with(|slot|{if let Some(state)=slot.borrow_mut().as_mut(){
        for receipt in receipts {state.receipts.push(json!({"engine":engine,"case":receipt.label,"primary":receipt.primary,"secondary":receipt.secondary,"dispatch":receipt.dispatch,"live":receipt.live,"peak":receipt.peak,"release_order":receipt.order,"leaves":receipt.leaves.as_ref().map(|(l,r)|(hex(l),hex(r))),"notes":receipt.note}));}
    }});
}

#[test]
fn provenance_records_effective_cwd_and_explicit_alias_roles_with_failed_versions() {
    let inherited = Command::new("unused-provider");
    let cwd = effective_cwd(&inherited).unwrap();
    assert_eq!(
        cwd,
        std::env::current_dir().unwrap().canonicalize().unwrap()
    );
    let mut relative = Command::new("unused-provider");
    relative.current_dir(".");
    let relative_cwd = effective_cwd(&relative).unwrap();
    assert_eq!(relative_cwd, cwd.clone());
    let root = std::env::temp_dir().join(format!("spx-matrix-provenance-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    // The tool alias is deliberately named cc, not clang; an explicit role
    // must preserve a failed version query even for wrappers/aliases.
    let mut alias = Command::new(root.join("cc"));
    alias
        .env("MATRIX_TEST_OVERRIDE", "exact")
        .env_remove("MATRIX_TEST_REMOVED");
    let captured_cwd = effective_cwd(&alias);
    let version = version_command(&alias, captured_cwd.as_ref().ok().map(PathBuf::as_path));
    assert_eq!(version.get_program(), alias.get_program());
    assert_eq!(version.get_current_dir(), Some(cwd.as_path()));
    assert_eq!(
        version.get_envs().collect::<Vec<_>>(),
        alias.get_envs().collect::<Vec<_>>()
    );
    let state = Inventory {
        output: root.join("unused"),
        root: root.clone(),
        commands: Vec::new(),
        artifacts: BTreeMap::new(),
        subjects: Vec::new(),
        tools: BTreeMap::new(),
        status: "incomplete",
        matrix: None,
        receipts: Vec::new(),
        provenance: json!({"test":true}),
    };
    ACTIVE.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(state);
    });
    command(
        &alias,
        "required compiler",
        Some("c11-compiler"),
        &captured_cwd,
        &Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing alias",
        )),
    );
    ACTIVE.with(|slot| {
        let state = slot.borrow();
        let state = state.as_ref().unwrap();
        assert_eq!(state.commands[0]["cwd"], cwd.to_string_lossy().as_ref());
        assert_eq!(state.tools.len(), 1);
        let version = state.tools.values().next().unwrap();
        assert_eq!(version["role"], "c11-compiler");
        assert!(version["result"]["spawn_error"].is_string());
    });
    #[cfg(unix)]
    {
        fs::write(root.join("real"), b"binary").unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("binary-link")).unwrap();
        let refused = std::panic::catch_unwind(|| {
            external_artifact("executed-rust", &root.join("binary-link"))
        });
        assert!(refused.is_err());
        ACTIVE.with(|slot| assert!(slot.borrow().as_ref().unwrap().artifacts.is_empty()));
    }
    ACTIVE.with(|slot| {
        *slot.borrow_mut() = None;
    });
    fs::remove_dir_all(root).unwrap();
}
