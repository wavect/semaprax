//! Explicit tool-selected Miri gate; generated C is deliberately absent.
use super::*;

#[test]
#[ignore = "requires explicitly selected nightly Cargo with Miri and rust-src"]
fn indexed_url_miri_carrier_and_exclusive_loan_corpus() {
    let cargo = std::env::var("SEMAPRAX_MIRI_CARGO").expect("explicit nightly-capable Cargo");
    let toolchain = std::env::var("SEMAPRAX_MIRI_TOOLCHAIN").expect("explicit Miri toolchain");
    assert!(Path::new(&cargo).is_absolute());
    assert!(!toolchain.is_empty() && !toolchain.contains(char::is_whitespace));
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let root = workspace
        .join("target")
        .join(format!("ri06-url-miri-{}", std::process::id()));
    let sysroot = workspace.join("target/ri06-url-miri-sysroot");
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    std::fs::create_dir_all(root.join("src")).unwrap();
    let _cleanup = Cleanup(root.clone());
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"ri06-url-owner\"\nversion=\"0.1.0\"\nedition=\"2021\"\npublish=false\n[workspace]\n[dependencies]\nurl_alias={package=\"url\",version=\"=2.5.8\"}\n").unwrap();
    std::fs::write(
        root.join("Cargo.lock"),
        include_bytes!("../../../semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        format!(
            "{}\n{}\n{}\n{}",
            include_str!("url_project_carrier.rs.txt"),
            include_str!("url_project_exclusive.rs.txt"),
            include_str!("url_project_carrier_corpus.rs.txt"),
            include_str!("url_miri_corpus.rs.txt")
        ),
    )
    .unwrap();
    let command = || {
        let mut command = Command::new(&cargo);
        command
            .arg(format!("+{toolchain}"))
            .arg("miri")
            .current_dir(&root)
            .env_remove("RUSTC")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("CARGO_TARGET_DIR", root.join("build"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_TEST_DEBUG", "0")
            .env("CARGO_NET_OFFLINE", "true")
            .env("MIRI_SYSROOT", &sysroot)
            .env("MIRIFLAGS", "-Zmiri-strict-provenance -Zmiri-seed=1");
        command
    };
    let setup = command().arg("setup").output().unwrap();
    assert!(
        setup.status.success(),
        "Miri sysroot setup: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let run = command()
        .args([
            "test",
            "--offline",
            "--locked",
            "--lib",
            "carrier_and_loan_corpus",
            "--",
            "--exact",
            "--nocapture",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && stdout.contains("1 passed"),
        "Miri carrier/lease corpus:\n{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}
