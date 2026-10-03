use super::future_bridge::render_local_future_bridge;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn resolved_tool(name: &str, variable: &str) -> PathBuf {
    let requested = std::env::var_os(variable).unwrap_or_else(|| name.into());
    let requested = PathBuf::from(requested);
    let path = if requested.is_absolute() {
        requested
    } else if requested.components().count() > 1 {
        std::env::current_dir().unwrap().join(requested)
    } else {
        let search = std::env::var_os("PATH").expect("test process has PATH");
        std::env::split_paths(&search)
            .map(|directory| directory.join(&requested))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| panic!("{name} not found on PATH"))
    };
    assert!(
        path.is_absolute() && path.is_file(),
        "invalid {name} path: {}",
        path.display()
    );
    let version = Command::new(&path).arg("--version").output().unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(
        version.starts_with(&format!("{name} 1.")),
        "unexpected {name} identity/version: {version}"
    );
    path
}

fn scratch(label: &str) -> std::path::PathBuf {
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-ri09-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn generated_local_future_bridge_executes_and_refuses_send_escape() {
    let rustc = resolved_tool("rustc", "RUSTC");
    let root = scratch("std");
    let source = format!(
        "{}\n{}",
        render_local_future_bridge(),
        include_str!("future_runtime_fixture.rs.txt")
    );
    fs::write(root.join("runtime.rs"), source).unwrap();
    let compiled = Command::new(&rustc)
        .args([
            "--edition=2021",
            "--test",
            "runtime.rs",
            "-o",
            "runtime-tests",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(root.join("runtime-tests")).output().unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert!(String::from_utf8_lossy(&executed.stdout).contains("7 passed"));

    let negative = format!(
        "{}\nfn needs_send<T: Send>(_: T) {{}}\nfn main() {{ let limit = LocalFutureLimit::new(1); let handle = limit.start(async {{ 1usize }}, 8, |_| 8).unwrap(); needs_send(handle); }}\n",
        render_local_future_bridge()
    );
    fs::write(root.join("not_send.rs"), negative).unwrap();
    let refused = Command::new(&rustc)
        .args(["--edition=2021", "not_send.rs", "-o", "should-not-build"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("E0277"));
    fs::remove_dir_all(root).unwrap();
}

/// Run explicitly with a warm, checkout-private `SEMAPRAX_RI09_TARGET_DIR`.
/// This is a real local HTTP request under caller-owned Tokio, not a mocked
/// Future or a claim that Semaprax source can yet await Rust.
#[test]
#[ignore = "requires explicit checkout-private Cargo target for real reqwest/Tokio gate"]
fn generated_local_future_bridge_runs_locked_reqwest_and_cancels_received_request() {
    let cargo = resolved_tool("cargo", "CARGO");
    let target = std::env::var("SEMAPRAX_RI09_TARGET_DIR")
        .expect("set SEMAPRAX_RI09_TARGET_DIR to a warm target under this checkout");
    let target = fs::canonicalize(target).unwrap();
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    assert!(target.starts_with(checkout.join("target")));
    let root = scratch("reqwest");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        include_bytes!("fixtures/ri09-reqwest/Cargo.toml"),
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        include_bytes!("fixtures/ri09-reqwest/Cargo.lock"),
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        format!(
            "{}\n{}",
            render_local_future_bridge(),
            include_str!("future_reqwest_fixture.rs.txt")
        ),
    )
    .unwrap();
    let run = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
