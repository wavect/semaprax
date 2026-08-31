//! Explicit real-unpacked-product evidence, not release provenance or installation.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "release_archive_product_v1/admission.rs"]
mod admission;
#[path = "release_archive_product_v1/calculator.rs"]
mod calculator;
#[path = "release_archive_product_v1/command.rs"]
mod command;
#[path = "release_archive_product_v1/daemon.rs"]
mod daemon;
#[path = "release_archive_product_v1/owned_frame.rs"]
mod owned_frame;

use admission::Release;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn outside_release(label: &str, release: &Release) -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            !parent.starts_with(&release.root),
            "fixture parent must be outside archive"
        );
        Self::new(label)
    }
    fn new(label: &str) -> Self {
        assert!(label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        assert!(
            !parent.starts_with(&checkout),
            "fixture parent must be outside checkout"
        );
        let root = parent.join(format!(
            "semaprax-archive-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        eprintln!("retained archive-product fixture: {}", root.display());
        Self { root }
    }
}

#[test]
#[ignore = "requires an actual unpacked native release in absolute SEMAPRAX_RELEASE_ROOT and its exact SEMAPRAX_RELEASE_COMMIT label"]
fn provisioned_archive_cli_and_daemon_work_outside_checkout() {
    let release = Release::admit();
    let fixture = Fixture::outside_release("onboarding", &release);
    release.verify_versions(&fixture.root);
    let calculator = calculator::run(&release, &fixture.root);
    daemon::run(&release, &calculator, &fixture.root);
    release.assert_unchanged();
}

#[test]
#[ignore = "requires a real unpacked native release, exact commit label and absolute NODE/CLANG/SEMAPRAX_ARCHIVER/CARGO with cached offline Rust dependencies"]
fn provisioned_archive_owned_frame_consumers_work_outside_checkout() {
    let release = Release::admit();
    let fixture = Fixture::outside_release("owned-frame", &release);
    release.verify_versions(&fixture.root);
    let frame = fixture.root.join("frame");
    std::fs::create_dir(&frame).unwrap();
    owned_frame::run(&release.cli, &frame);
    release.assert_unchanged();
}
