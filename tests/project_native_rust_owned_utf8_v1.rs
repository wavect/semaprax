use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn configured_tool(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(configured) = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
    {
        return configured;
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| panic!("{variable} must name an installed absolute tool"))
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-v10-rust-route-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v10\"\nname = \"utf8-route\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.count\", \"utf8.greeting\"]\ntests = [\"utf8.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module utf8.app;\n\n@id(\"utf8.count\")\nfn count() -> i64\n{\n    7\n}\n\n@id(\"utf8.greeting\")\nfn greeting() -> string\n{\n    \"hello\\u{0}世界\"\n}\n\n@id(\"utf8.app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module utf8.tests;\n\n@id(\"utf8.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    Fixture(root.canonicalize().unwrap())
}

#[test]
fn project_v10_rust_route_emits_a_distinct_mixed_safe_string_package() {
    let fixture = fixture();
    let output = fixture.0.join("rust");
    let clang = configured_tool("CLANG", &["/usr/bin/clang"]);
    let archiver = if cfg!(target_os = "macos") {
        configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/libtool"])
    } else {
        configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/ar", "/bin/ar"])
    };
    let built = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "--manifest-path",
            "semaprax.toml",
            "--target",
            "rust",
            "-o",
        ])
        .arg(&output)
        .current_dir(&fixture.0)
        .env("CLANG", clang)
        .env("SEMAPRAX_ARCHIVER", archiver)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert_eq!(
        String::from_utf8(built.stdout).unwrap(),
        format!(
            "built Project v10 Native Rust owned-data package {}\n",
            output.display()
        )
    );

    let descriptor: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("descriptor.json")).unwrap()).unwrap();
    assert_eq!(descriptor["schema"], "semaprax.public-owned-utf8-api.v1");
    assert_eq!(descriptor["exports"][0]["result"], "i64");
    assert_eq!(descriptor["exports"][1]["result"], "owned-utf8");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join("semaprax.native-rust-owned-utf8-sdk.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["schema"], "semaprax.native-rust-owned-utf8-sdk.v1");
    let safe = std::fs::read_to_string(output.join("lib.rs")).unwrap();
    let settle = safe.find("copy_and_settle(raw.handle)").unwrap();
    let validate = safe.find("String::from_utf8(bytes)").unwrap();
    assert!(settle < validate);
    assert!(safe.contains("pub fn spx_utf8_dot_count(&mut self)->Result<i64,CallError>"));
    assert!(safe.contains("pub fn spx_utf8_dot_greeting(&mut self)->Result<String,CallError>"));
}

#[test]
fn rust_profile_rejection_precedes_explicit_parent_creation() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-rust-profile-rejection-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"legacy.add\"]\ntests = [\"legacy.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module legacy.app;\n\n@id(\"legacy.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"legacy.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module legacy.tests;\n\n@id(\"legacy.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    let fixture = Fixture(root.canonicalize().unwrap());
    let output = fixture.0.join("missing-parent/rust");
    let rejected = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "--manifest-path",
            "semaprax.toml",
            "--target",
            "rust",
            "-o",
        ])
        .arg(&output)
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("SPX-J114"));
    assert!(!fixture.0.join("missing-parent").exists());
}
