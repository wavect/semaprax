use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(export_name: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-v8-rust-route-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    for directory in ["a", "t", "z"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v8\"\nname = \"route\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"route.app\"\nsources = [\"a/core.spx\", \"t/tests.spx\", \"z/app.spx\"]\nweb_exports = [\"route.payload\", \"route.size\", \"route.valid\"]\ntests = [\"route.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("a/core.spx"),
        format!(
            "module route.core;\n\n@id(\"route.decision\")\nrecord Decision {{ @id(\"route.decision.allowed\") allowed: bool, }}\n\n@id(\"route.valid\")\nfn {export_name}(value: i64) -> bool {{\n    let decision = Decision {{ allowed: value >= 0, }};\n    decision.allowed\n}}\n\n@id(\"route.payload\")\nfn payload(input: borrow Slice<u8>) -> Bytes {{ bytes_copy(input) }}\n\n@id(\"route.size\")\nfn size() -> usize {{ 7usize }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("t/tests.spx"),
        format!(
            "module route.tests;\nuse function @id(\"route.valid\") from route.core as {export_name};\n\n@id(\"route.tests.main\")\nfn main() -> i64 {{ if {export_name}(1) {{ 0 }} else {{ 1 }} }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("z/app.spx"),
        format!(
            "module route.app;\nuse function @id(\"route.valid\") from route.core as {export_name};\n\n@id(\"route.app.main\")\nfn main() -> i64 {{ if {export_name}(1) {{ 0 }} else {{ 1 }} }}\n"
        ),
    )
    .unwrap();
    Fixture(root.canonicalize().unwrap())
}

fn run(root: &Path, output: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "--manifest-path",
            "semaprax.toml",
            "--target",
            "rust",
            "-o",
            output,
        ])
        .current_dir(root)
        .output()
        .unwrap()
}

fn inventory(path: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn project_v8_rust_cli_normalizes_relative_output_replays_and_never_clobbers() {
    let fixture = fixture("valid");
    let result = run(&fixture.0, "dist/rust");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8(result.stderr).unwrap()
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        format!(
            "built Project v8 Native Rust owned-data package {}\n",
            fixture.0.join("dist/rust").display()
        )
    );
    let package = fixture.0.join("dist/rust");
    let files = inventory(&package);
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
        archive,
        "owned_data_ffi.rs",
        "semaprax.native-rust-owned-data-sdk.json",
    ];
    expected.sort();
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
    let descriptor: serde_json::Value =
        serde_json::from_slice(files.get("descriptor.json").unwrap()).unwrap();
    assert_eq!(descriptor["exports"][1]["result"], "usize");
    assert_eq!(
        descriptor["exports"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["rust_method_name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "spx_route_dot_payload",
            "spx_route_dot_size",
            "spx_route_dot_valid",
        ]
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        files
            .get("semaprax.native-rust-owned-data-sdk.json")
            .unwrap(),
    )
    .unwrap();
    assert!(!manifest["nonclaims"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "no_project_v8_activation"));
    let lib = String::from_utf8(files.get("lib.rs").unwrap().clone()).unwrap();
    assert!(lib.contains("pub fn spx_route_dot_size(&mut self)->Result<u64,CallError>"));

    let before = inventory(&package);
    let second = run(&fixture.0, "dist/rust");
    assert!(!second.status.success());
    assert_eq!(inventory(&package), before);
}

#[test]
fn display_rename_preserves_the_stable_rust_method_identity() {
    let original = fixture("valid");
    let renamed = fixture("renamed_valid");
    for (fixture, output) in [(&original, "original"), (&renamed, "renamed")] {
        let result = run(&fixture.0, output);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8(result.stderr).unwrap()
        );
    }
    let method = |fixture: &Fixture, output: &str| {
        let descriptor: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixture.0.join(output).join("descriptor.json")).unwrap(),
        )
        .unwrap();
        descriptor["exports"][2]["rust_method_name"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(method(&original, "original"), "spx_route_dot_valid");
    assert_eq!(method(&renamed, "renamed"), "spx_route_dot_valid");
}
