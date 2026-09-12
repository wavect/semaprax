//! Hostile-input evidence for the macOS Seatbelt confinement primitive.
//!
//! Every escape test below is paired with an unconfined baseline run of the
//! identical fixture argument. That baseline is not decoration: without it, a
//! "denied" result could just as easily mean the OS refused for an unrelated
//! reason (a missing directory, a permissions bit, no route to a host) as
//! that Seatbelt caught anything. The pairing is what makes the confined
//! result a specific, attributable outcome instead of merely "an error".
use super::*;
use std::process::Command;
use std::sync::OnceLock;

static FIXTURE: OnceLock<PathBuf> = OnceLock::new();

fn fixture_binary() -> &'static Path {
    FIXTURE.get_or_init(|| {
        let root = fresh_dir("fixture-build");
        let source = root.join("fixture.rs");
        std::fs::write(&source, include_str!("fixture.rs")).unwrap();
        let binary = root.join("fixture");
        let output = Command::new("rustc")
            .args(["--edition=2021", "-O"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        binary
    })
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "semaprax-doctor-confinement-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    dir
}

#[test]
fn healthy_confined_process_settles_completed_and_writes_only_inside_scratch() {
    let scratch = fresh_dir("healthy-scratch");
    let confined = confined_spawn(
        fixture_binary(),
        &[OsStr::new("healthy"), scratch.as_os_str()],
        &scratch,
    )
    .unwrap();
    let settled = settle(confined, Duration::from_secs(5));
    assert_eq!(settled.status, Settlement::Completed);
    assert!(String::from_utf8_lossy(&settled.stdout).contains("healthy-ok"));
    assert_eq!(
        std::fs::read(scratch.join("ok.txt")).unwrap(),
        b"confined-ok"
    );
}

/// The deliberate confinement escape this contract requires: a confined
/// process attempts a write outside its scratch root, and the primitive
/// must catch it with the specific `EPERM` Seatbelt denial, not merely fail.
#[test]
fn deliberate_confinement_escape_write_outside_scratch_is_denied_with_eperm() {
    let scratch = fresh_dir("escape-write-scratch");
    let outside_root = fresh_dir("escape-write-outside");
    let outside = outside_root.join("escaped.txt");

    let baseline = Command::new(fixture_binary())
        .args(["escape-write", outside.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&baseline.stdout).contains("write-permitted"),
        "unconfined baseline must succeed, or a confined denial proves nothing: {}",
        String::from_utf8_lossy(&baseline.stdout)
    );
    std::fs::remove_file(&outside).unwrap();

    let confined = confined_spawn(
        fixture_binary(),
        &[OsStr::new("escape-write"), outside.as_os_str()],
        &scratch,
    )
    .unwrap();
    let settled = settle(confined, Duration::from_secs(5));
    assert_eq!(settled.status, Settlement::Completed);
    let stdout = String::from_utf8_lossy(&settled.stdout).into_owned();
    assert!(
        stdout.contains(&format!("write-denied:{}", libc::EPERM)),
        "expected the specific EPERM Seatbelt denial, got: {stdout}"
    );
    assert!(!outside.exists());
}

/// The second required deliberate escape: an outbound network connect from
/// inside confinement. The unconfined baseline gets `ECONNREFUSED` (the
/// kernel attempted the connect); confinement must instead produce `EPERM`
/// (the kernel refused to attempt it at all) -- two different, specific,
/// named outcomes, not "an error" in both cases.
#[test]
fn deliberate_confinement_escape_network_connect_is_denied_with_eperm() {
    let scratch = fresh_dir("escape-network-scratch");

    let baseline = Command::new(fixture_binary())
        .arg("escape-network")
        .output()
        .unwrap();
    let baseline_stdout = String::from_utf8_lossy(&baseline.stdout).into_owned();
    assert!(
        baseline_stdout.contains("connect-denied:")
            && !baseline_stdout.contains("connect-permitted"),
        "expected an unconfined ECONNREFUSED-style baseline (nothing listens on the fixed \
         loopback port), got: {baseline_stdout}"
    );
    assert!(
        !baseline_stdout.contains(&format!("connect-denied:{}", libc::EPERM)),
        "baseline must fail for a network reason, not already read as EPERM: {baseline_stdout}"
    );

    let confined =
        confined_spawn(fixture_binary(), &[OsStr::new("escape-network")], &scratch).unwrap();
    let settled = settle(confined, Duration::from_secs(5));
    assert_eq!(settled.status, Settlement::Completed);
    let stdout = String::from_utf8_lossy(&settled.stdout).into_owned();
    assert!(
        stdout.contains(&format!("connect-denied:{}", libc::EPERM)),
        "expected the specific EPERM Seatbelt denial, got: {stdout}"
    );
}

/// A scratch root containing Seatbelt string-literal metacharacters must not
/// be able to close the profile's `(subpath "...")` literal early and widen
/// write authority. This is the hostile-input case for `escape_seatbelt_string`.
#[test]
fn scratch_root_containing_seatbelt_metacharacters_is_escaped_not_injected() {
    let scratch = fresh_dir("inject\"-attempt)paren");
    let outside_root = fresh_dir("inject-outside");
    let outside = outside_root.join("escaped.txt");

    let confined_inside = confined_spawn(
        fixture_binary(),
        &[OsStr::new("healthy"), scratch.as_os_str()],
        &scratch,
    )
    .unwrap();
    let settled_inside = settle(confined_inside, Duration::from_secs(5));
    assert_eq!(settled_inside.status, Settlement::Completed);
    assert!(scratch.join("ok.txt").exists());

    let confined_outside = confined_spawn(
        fixture_binary(),
        &[OsStr::new("escape-write"), outside.as_os_str()],
        &scratch,
    )
    .unwrap();
    let settled_outside = settle(confined_outside, Duration::from_secs(5));
    assert_eq!(settled_outside.status, Settlement::Completed);
    let stdout = String::from_utf8_lossy(&settled_outside.stdout).into_owned();
    assert!(
        stdout.contains(&format!("write-denied:{}", libc::EPERM)),
        "an unescaped quote in the scratch path must not widen write authority: {stdout}"
    );
    assert!(!outside.exists());
}

#[test]
fn confined_process_exiting_nonzero_settles_failed_with_exact_exit_code() {
    let scratch = fresh_dir("fail-scratch");
    let confined = confined_spawn(fixture_binary(), &[OsStr::new("fail")], &scratch).unwrap();
    let settled = settle(confined, Duration::from_secs(5));
    assert_eq!(
        settled.status,
        Settlement::Failed(FailureReason::ExitCode(9))
    );
}

#[test]
fn confined_process_exceeding_deadline_settles_cancelled_not_completed_or_failed() {
    let scratch = fresh_dir("cancel-scratch");
    let confined = confined_spawn(fixture_binary(), &[OsStr::new("sleep")], &scratch).unwrap();
    let settled = settle(confined, Duration::from_millis(300));
    assert_eq!(settled.status, Settlement::Cancelled);
    assert!(!String::from_utf8_lossy(&settled.stdout).contains("should-not-print"));
}

/// The settlement analog of the Linux contract's empty-cgroup proof: a
/// descendant the primary confined process left behind, past the primary's
/// own exit, must be caught as `Uncertain`, never silently folded into
/// `Completed`.
#[test]
fn a_descendant_left_behind_in_the_confined_group_settles_uncertain_not_completed() {
    let scratch = fresh_dir("leak-scratch");
    let confined =
        confined_spawn(fixture_binary(), &[OsStr::new("leak-descendant")], &scratch).unwrap();
    let settled = settle(confined, Duration::from_secs(5));
    assert_eq!(
        settled.status,
        Settlement::Uncertain(UncertainReason::GroupStillPresent)
    );
    assert!(String::from_utf8_lossy(&settled.stdout).contains("leaked"));
    // Let the detached `sleep 2` finish so the test process leaves no
    // lingering descendant of its own behind.
    std::thread::sleep(Duration::from_secs(3));
}

#[test]
fn absent_confinement_launcher_fails_closed_never_silently_unconfined() {
    let scratch = fresh_dir("absent-launcher-scratch");
    let result = confined_spawn_via(
        Path::new("/nonexistent/semaprax-sandbox-exec-fixture"),
        fixture_binary(),
        &[OsStr::new("healthy"), scratch.as_os_str()],
        &scratch,
    );
    assert_eq!(result.err(), Some(ConfinementError::Unsupported));
    assert!(
        std::fs::read_dir(&scratch).unwrap().next().is_none(),
        "no write must occur when confinement cannot be established"
    );
}

#[test]
fn relative_executable_or_scratch_root_is_rejected_before_any_spawn() {
    let scratch = fresh_dir("relative-scratch");
    assert_eq!(
        confined_spawn(Path::new("relative-exe"), &[], &scratch).err(),
        Some(ConfinementError::Invalid)
    );
    assert_eq!(
        confined_spawn(fixture_binary(), &[], Path::new("relative-scratch")).err(),
        Some(ConfinementError::Invalid)
    );
}

#[test]
fn sticky_settlement_preserves_first_terminal_status_over_later_cleanup_attempts() {
    let mut state = StickySettlement::default();
    state.select(Settlement::Failed(FailureReason::ExitCode(7)));
    state.select(Settlement::Completed);
    state.select(Settlement::Cancelled);
    state.select(Settlement::Uncertain(UncertainReason::GroupStillPresent));
    assert_eq!(
        state.resolve(),
        Some(Settlement::Failed(FailureReason::ExitCode(7)))
    );
}

#[test]
fn sticky_settlement_lets_a_pending_completed_be_overridden_by_a_later_terminal_status() {
    let mut state = StickySettlement::default();
    state.select(Settlement::Completed);
    state.select(Settlement::Uncertain(UncertainReason::GroupStillPresent));
    assert_eq!(
        state.resolve(),
        Some(Settlement::Uncertain(UncertainReason::GroupStillPresent))
    );
}

#[test]
fn sticky_settlement_with_no_selection_resolves_to_none() {
    assert_eq!(StickySettlement::default().resolve(), None);
}
