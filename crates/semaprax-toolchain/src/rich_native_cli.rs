//! Read-only private CLI inspection of explicitly selected native authority.

use crate::rich_cargo_execution::{
    validate_selected_native_target, validate_selected_tools, CargoExecutionError,
};
use crate::rich_native_host::NativeHostRefusal;
use semaprax_native_rust_interop::{NativeBuildPolicy, NativeEffectContract, TrustedNativeProfile};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_IDENTITY_FILE_BYTES: u64 = 1_048_576;
const MAX_TOOL_FILE_BYTES: u64 = 134_217_728;
const USAGE: &str = "native-authority-check requires <plan-file> <crate-file> <cargo> <rustc> <target> <strict|sandbox|trusted> <opaque|audited-assertion> <build|dispatch> [--effect name]... [--grant name]... [--require name]... [--json]\n";

struct Options<'a> {
    plan: PathBuf,
    native_crate: PathBuf,
    cargo: PathBuf,
    rustc: PathBuf,
    target: &'a str,
    policy: NativeBuildPolicy,
    audited: bool,
    check: &'a str,
    effects: Vec<&'a str>,
    grants: Vec<&'a str>,
    required: Vec<&'a str>,
    json: bool,
}

fn parse(arguments: &[String]) -> Result<Options<'_>, (String, u8)> {
    if arguments.len() < 8 {
        return Err((USAGE.into(), 2));
    }
    let policy = match arguments[5].as_str() {
        "strict" => NativeBuildPolicy::StrictDenyExecution,
        "sandbox" => NativeBuildPolicy::EnforcedSandbox,
        "trusted" => NativeBuildPolicy::TrustedHost,
        _ => return Err((USAGE.into(), 2)),
    };
    let audited = match arguments[6].as_str() {
        "opaque" => false,
        "audited-assertion" => true,
        _ => return Err((USAGE.into(), 2)),
    };
    let check = arguments[7].as_str();
    if !matches!(check, "build" | "dispatch") {
        return Err((USAGE.into(), 2));
    }
    let mut options = Options {
        plan: PathBuf::from(&arguments[0]),
        native_crate: PathBuf::from(&arguments[1]),
        cargo: PathBuf::from(&arguments[2]),
        rustc: PathBuf::from(&arguments[3]),
        target: &arguments[4],
        policy,
        audited,
        check,
        effects: Vec::new(),
        grants: Vec::new(),
        required: Vec::new(),
        json: false,
    };
    let mut cursor = 8;
    while cursor < arguments.len() {
        match arguments[cursor].as_str() {
            "--json" if !options.json => {
                options.json = true;
                cursor += 1;
            }
            "--effect" | "--grant" | "--require" if cursor + 1 < arguments.len() => {
                let value = arguments[cursor + 1].as_str();
                if value.is_empty() || value.starts_with("--") {
                    return Err((USAGE.into(), 2));
                }
                match arguments[cursor].as_str() {
                    "--effect" => options.effects.push(value),
                    "--grant" => options.grants.push(value),
                    _ => options.required.push(value),
                }
                cursor += 2;
            }
            _ => return Err((USAGE.into(), 2)),
        }
    }
    if !audited && !options.effects.is_empty() {
        return Err((USAGE.into(), 2));
    }
    Ok(options)
}

fn identity(path: &Path) -> Result<Vec<u8>, NativeHostRefusal> {
    if !path.is_absolute() {
        return Err(NativeHostRefusal::Cargo(CargoExecutionError::InvalidInput));
    }
    let metadata = fs::metadata(path)
        .map_err(|_| NativeHostRefusal::Cargo(CargoExecutionError::InvalidInput))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_IDENTITY_FILE_BYTES {
        return Err(NativeHostRefusal::Cargo(CargoExecutionError::InvalidInput));
    }
    let bytes =
        fs::read(path).map_err(|_| NativeHostRefusal::Cargo(CargoExecutionError::InvalidInput))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_IDENTITY_FILE_BYTES {
        return Err(NativeHostRefusal::Cargo(CargoExecutionError::InvalidInput));
    }
    Ok(bytes)
}

fn tool_identity(cargo: &Path, rustc: &Path) -> Result<[u8; 32], NativeHostRefusal> {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.native-tool-pair.v1\0");
    for path in [cargo, rustc] {
        let metadata = fs::metadata(path)
            .map_err(|_| NativeHostRefusal::Cargo(CargoExecutionError::MissingTool))?;
        if !metadata.is_file() || metadata.len() > MAX_TOOL_FILE_BYTES {
            return Err(NativeHostRefusal::Cargo(CargoExecutionError::MissingTool));
        }
        hasher.update(metadata.len().to_be_bytes());
        let mut file = File::open(path)
            .map_err(|_| NativeHostRefusal::Cargo(CargoExecutionError::MissingTool))?;
        let mut read = 0u64;
        let mut buffer = [0u8; 8192];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|_| NativeHostRefusal::Cargo(CargoExecutionError::MissingTool))?;
            if count == 0 {
                break;
            }
            read += count as u64;
            if read > MAX_TOOL_FILE_BYTES {
                return Err(NativeHostRefusal::Cargo(CargoExecutionError::MissingTool));
            }
            hasher.update(&buffer[..count]);
        }
        if read != metadata.len() {
            return Err(NativeHostRefusal::Cargo(CargoExecutionError::MissingTool));
        }
    }
    Ok(hasher.finalize().into())
}

/// Inspect selected local bytes and policy without invoking Cargo or native code.
pub fn run(arguments: &[String]) -> Result<String, (String, u8)> {
    let options = parse(arguments)?;
    let result = inspect(&options);
    result.map_err(|error| error.render(options.json))
}

fn inspect(options: &Options<'_>) -> Result<String, NativeHostRefusal> {
    validate_selected_tools(&options.cargo, &options.rustc).map_err(NativeHostRefusal::Cargo)?;
    validate_selected_native_target(options.target).map_err(NativeHostRefusal::Cargo)?;
    let plan = identity(&options.plan)?;
    let native_crate = identity(&options.native_crate)?;
    let tools = tool_identity(&options.cargo, &options.rustc)?;
    let contract = if options.audited {
        NativeEffectContract::Audited(&options.effects)
    } else {
        NativeEffectContract::Opaque
    };
    let profile =
        TrustedNativeProfile::admit(&plan, &native_crate, &tools, contract, options.policy)
            .map_err(NativeHostRefusal::Trust)?;
    match options.check {
        "build" => {
            profile
                .authorize_build(&plan, &native_crate, &tools)
                .map_err(NativeHostRefusal::Trust)?;
        }
        "dispatch" => {
            let grant = profile
                .grant(&options.grants)
                .map_err(NativeHostRefusal::Trust)?;
            profile
                .dispatch(&grant, &options.required, || ())
                .map_err(NativeHostRefusal::Dispatch)?;
        }
        _ => unreachable!("parser limits the check"),
    }
    let contract = if options.audited {
        "maintainer-audited assertion; Rust behavior is not compiler verified"
    } else {
        "opaque native behavior"
    };
    if options.json {
        Ok(format!(
            "{}\n",
            serde_json::json!({
                "status": "inspected",
                "check": options.check,
                "effect_contract": contract,
                "build_code": options.policy.disclosure(),
                "process_execution": false,
                "exposed_roots": [],
                "os_confinement_enforced": false,
            })
        ))
    } else {
        Ok(format!(
            "native authority inspected; {contract}; {}; no process executed or roots exposed; OS confinement is not enforced\n",
            options.policy.disclosure()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> (PathBuf, Vec<String>) {
        let root = std::env::temp_dir().join(format!(
            "semaprax-native-authority-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let plan = root.join("plan");
        let native_crate = root.join("crate");
        let cargo = root.join("cargo");
        let rustc = root.join("rustc");
        for (path, bytes) in [
            (&plan, b"binding plan".as_slice()),
            (&native_crate, b"prepared crate"),
            (&cargo, b"cargo image"),
            (&rustc, b"rustc image"),
        ] {
            fs::write(path, bytes).unwrap();
        }
        let target = if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
            "aarch64-apple-darwin"
        } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
            "x86_64-apple-darwin"
        } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
            "aarch64-unknown-linux-gnu"
        } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
            "x86_64-unknown-linux-gnu"
        } else {
            "x86_64-pc-windows-msvc"
        };
        let args = [
            plan.display().to_string(),
            native_crate.display().to_string(),
            cargo.display().to_string(),
            rustc.display().to_string(),
            target.into(),
            "trusted".into(),
            "opaque".into(),
            "build".into(),
        ]
        .to_vec();
        (root, args)
    }

    #[test]
    fn actual_file_and_profile_checks_render_all_five_refusals() {
        let (root, args) = fixture();
        let mut selected = args.clone();
        selected[4] = "unsupported-target".into();
        assert!(run(&selected).unwrap_err().0.contains("SPX-B122"));
        selected = args.clone();
        selected[2] = root.join("missing-cargo").display().to_string();
        assert!(run(&selected).unwrap_err().0.contains("SPX-B125"));
        selected = args.clone();
        selected[5] = "strict".into();
        assert!(run(&selected).unwrap_err().0.contains("SPX-B126"));
        selected[5] = "sandbox".into();
        assert!(run(&selected).unwrap_err().0.contains("SPX-B127"));
        selected = args.clone();
        selected[6] = "audited-assertion".into();
        selected[7] = "dispatch".into();
        selected.extend(["--effect", "host.file", "--require", "host.file"].map(str::to_owned));
        assert!(run(&selected).unwrap_err().0.contains("SPX-B129"));
        selected[6] = "opaque".into();
        selected.truncate(8);
        assert!(run(&selected).unwrap_err().0.contains("SPX-B126"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trusted_read_only_inspection_discloses_without_execution() {
        let (root, mut args) = fixture();
        args.push("--json".into());
        let value: serde_json::Value = serde_json::from_str(&run(&args).unwrap()).unwrap();
        assert_eq!(value["status"], "inspected");
        assert_eq!(value["process_execution"], false);
        assert_eq!(value["os_confinement_enforced"], false);
        assert_eq!(value["exposed_roots"], serde_json::json!([]));
        assert!(value["build_code"]
            .as_str()
            .unwrap()
            .contains("trusted host"));
        fs::remove_dir_all(root).unwrap();
    }
}
