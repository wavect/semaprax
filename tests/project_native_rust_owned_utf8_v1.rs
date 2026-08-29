use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

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
        "schema = \"semaprax.project.v10\"\nname = \"utf8-route\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\"]\nweb_exports = [\"utf8.count\", \"utf8.greeting\"]\ntests = [\"utf8.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module utf8.app;\n@id(\"utf8.count\") fn count() -> i64 { 7 }\n@id(\"utf8.greeting\") fn greeting() -> string { \"hello\\0世界\" }\n@id(\"utf8.tests\") fn tests() -> i64 { 0 }\n@id(\"utf8.app.main\") fn main() -> i64 { 0 }\n",
    )
    .unwrap();
    Fixture(root.canonicalize().unwrap())
}

#[test]
fn project_v10_rust_route_emits_a_distinct_mixed_safe_string_package() {
    let fixture = fixture();
    let output = fixture.0.join("rust");
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
