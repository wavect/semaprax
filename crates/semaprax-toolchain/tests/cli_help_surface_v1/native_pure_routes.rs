use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// Pure CLI routes must not discover neighboring Rust source as permission to
/// execute it. Rich Native Rust index replay has no CLI verb in this profile.
#[test]
fn check_fmt_query_and_graph_leave_hostile_rust_unexecuted() {
    let root = super::empty_working_directory();
    let source = root.join("app.spx");
    let source_bytes = include_bytes!("../../../../examples/calculator.spx");
    fs::write(&source, source_bytes).unwrap();
    let cargo_manifest = root.join("Cargo.toml");
    fs::write(
        &cargo_manifest,
        "[package]\nname = \"hostile-neighbor\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n[dependencies]\nprobe_macro = { path = \"probe_macro\" }\n",
    )
    .unwrap();
    fs::write(
        root.join("build.rs"),
        "fn main() { std::fs::write(\"build-entered\", b\"1\").unwrap(); let _ = std::net::TcpStream::connect(\"127.0.0.1:9\"); }\n",
    )
    .unwrap();
    let macro_dir = root.join("probe_macro/src");
    fs::create_dir_all(&macro_dir).unwrap();
    fs::write(
        root.join("probe_macro/Cargo.toml"),
        "[package]\nname = \"probe_macro\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\nproc-macro = true\n",
    )
    .unwrap();
    fs::write(
        macro_dir.join("lib.rs"),
        "extern crate proc_macro; #[proc_macro] pub fn probe(_: proc_macro::TokenStream) -> proc_macro::TokenStream { std::fs::write(\"macro-entered\", b\"1\").unwrap(); let _ = std::net::TcpStream::connect(\"127.0.0.1:9\"); \"1\".parse().unwrap() }\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub const PROBE: i64 = probe_macro::probe!();\n",
    )
    .unwrap();

    let shim_dir = root.join("shim");
    fs::create_dir(&shim_dir).unwrap();
    let shim = shim_dir.join("cargo");
    fs::write(
        &shim,
        "#!/bin/sh\nprintf entered > \"$SEMAPRAX_NATIVE_SHIM_MARKER\"\nexit 99\n",
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o700)).unwrap();
    let rustc = shim_dir.join("rustc");
    fs::copy(&shim, &rustc).unwrap();
    fs::set_permissions(&rustc, fs::Permissions::from_mode(0o700)).unwrap();
    let marker = root.join("shim-entered");
    let mut paths = vec![shim_dir.clone()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let path = std::env::join_paths(paths).unwrap();

    for arguments in [
        vec!["check", source.to_str().unwrap()],
        vec!["fmt", source.to_str().unwrap(), "--check"],
        vec!["query", source.to_str().unwrap()],
        vec!["graph", source.to_str().unwrap()],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
            .args(&arguments)
            .current_dir(&root)
            .env("PATH", &path)
            .env("CARGO", &shim)
            .env("RUSTC", &rustc)
            .env("SEMAPRAX_NATIVE_SHIM_MARKER", &marker)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} failed: {}",
            arguments[0],
            String::from_utf8_lossy(&output.stderr)
        );
        for denied in [
            marker.clone(),
            root.join("build-entered"),
            root.join("macro-entered"),
        ] {
            assert!(!denied.exists(), "{} entered native code", arguments[0]);
        }
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
    }
    fs::remove_dir_all(root).unwrap();
}
