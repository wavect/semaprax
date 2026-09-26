//! Durable continuation contract v1: positive runs, a crash after every
//! journal record, and hostile answers/journals/directories.

use super::*;
use crate::resumable_effects::core::{CleanupHandler, EffectHandler};
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

mod control;

// --- fault injection consulted by `journal::Journal::append` ---------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FaultAction {
    Proceed,
    CrashBefore,
    CrashAfter,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Fault {
    at: usize,
    after: bool,
    seen: usize,
}

impl Fault {
    pub(super) fn before_append(&mut self) -> FaultAction {
        let index = self.seen;
        self.seen += 1;
        match (index == self.at, self.after) {
            (true, false) => FaultAction::CrashBefore,
            (true, true) => FaultAction::CrashAfter,
            _ => FaultAction::Proceed,
        }
    }
}

thread_local! {
    static ARMED: RefCell<Option<Fault>> = const { RefCell::new(None) };
}

pub(super) fn armed_fault() -> Option<Fault> {
    ARMED.with(|armed| armed.borrow_mut().take())
}

fn arm(at: usize, after: bool) {
    ARMED.with(|armed| *armed.borrow_mut() = Some(Fault { at, after, seen: 0 }));
}

// --- fixtures ----------------------------------------------------------------

const SOURCE: &str = r#"
module test.durable_continuation;
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> i64 {
    let first = yield seed + 1;
    let second = yield first * 2;
    let third = yield second + seed;
    first + second + third
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Started, 3 x (Yielded, Dispatched, Answered), Completed, CleanupStarted,
/// CleanupSettled.
const FULL_RUN_RECORDS: usize = 13;
const EXPECTED: i64 = 30 + 600 + 6020;
const STEPS: usize = 100_000;

fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(source, "durable-continuation.spx").unwrap()).unwrap()
}

fn key() -> SourceCheckpointKey {
    SourceCheckpointKey::new([0x42; 32])
}

fn policy() -> CapabilityPolicy {
    CapabilityPolicy::new(vec!["app.ask".into()]).unwrap()
}

fn args() -> Vec<ArgumentValue> {
    vec![ArgumentValue::Int(2)]
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(mode: u32) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "spx-continuation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        Self(path)
    }

    fn dir(&self) -> JournalDirectory {
        JournalDirectory::open(&self.0).unwrap()
    }

    fn journal(&self, invocation: &str) -> PathBuf {
        self.0.join(journal::journal_name(invocation))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Host {
    /// `(site, request)` per physical call. The site is derived from the
    /// fixture's fixed request sequence, never from the host's own count.
    calls: Vec<(u32, ArgumentValue)>,
}

const REQUESTS: [i64; 3] = [3, 60, 602];

impl EffectHandler<ArgumentValue, ArgumentValue> for Host {
    fn dispatch(&mut self, request: &ArgumentValue) -> Result<ArgumentValue, String> {
        let ArgumentValue::Int(value) = request else {
            return Err("unexpected request".into());
        };
        let site = REQUESTS
            .iter()
            .position(|expected| expected == value)
            .expect("fixture request") as u32;
        self.calls.push((site, request.clone()));
        Ok(ArgumentValue::Int(value * 10))
    }
}

#[derive(Default)]
struct Cleanup(usize);

impl CleanupHandler<DurableOutcome> for Cleanup {
    fn run(&mut self, _: &DurableOutcome) -> Result<(), String> {
        self.0 += 1;
        Ok(())
    }
}

fn start<'a>(
    scratch: &Scratch,
    key: &'a SourceCheckpointKey,
    program: &'a ResolvedProgram,
    invocation: &str,
) -> Result<DurableInvocation<'a>, ContinuationError> {
    DurableInvocation::start(
        &scratch.dir(),
        key,
        program,
        "app.ask",
        &args(),
        invocation,
        7,
        STEPS,
    )
}

fn recover<'a>(
    scratch: &Scratch,
    key: &'a SourceCheckpointKey,
    program: &'a ResolvedProgram,
    invocation: &str,
    torn: TornTailPolicy,
) -> Result<DurableInvocation<'a>, ContinuationError> {
    DurableInvocation::recover(
        &scratch.dir(),
        key,
        program,
        "app.ask",
        &args(),
        invocation,
        7,
        STEPS,
        torn,
    )
}

fn settled_value(status: &ContinuationStatus) -> ArgumentValue {
    match status {
        ContinuationStatus::Settled {
            outcome: DurableOutcome::Completed(value),
            cleanup: CleanupSettlement::Completed | CleanupSettlement::HostConfirmed,
        } => value.clone(),
        other => panic!("not settled: {other:?}"),
    }
}

fn line_count(path: &Path) -> usize {
    fs::read(path)
        .unwrap()
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
}

// --- positive ----------------------------------------------------------------

#[test]
fn multi_yield_run_completes_once_per_site_and_publishes_after_cleanup() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let mut invocation = start(&scratch, &key, &program, "inv-positive").unwrap();
    let ContinuationStatus::AwaitingDispatch(first) = invocation.status() else {
        panic!("start did not suspend");
    };
    assert_eq!(first.site, 0);
    assert_eq!(first.request, ArgumentValue::Int(3));
    assert_eq!(first.invocation_id, "inv-positive");
    let (mut host, mut cleanup) = (Host::default(), Cleanup::default());
    let status = invocation
        .drive(&policy(), &mut host, &mut cleanup)
        .unwrap();
    assert_eq!(settled_value(&status), ArgumentValue::Int(EXPECTED));
    assert_eq!(
        host.calls,
        [
            (0, ArgumentValue::Int(3)),
            (1, ArgumentValue::Int(60)),
            (2, ArgumentValue::Int(602)),
        ]
    );
    assert_eq!(cleanup.0, 1);
    assert_eq!(
        line_count(&scratch.journal("inv-positive")),
        FULL_RUN_RECORDS
    );
    let mode = fs::metadata(scratch.journal("inv-positive"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);

    // A settled journal recovers as settled and dispatches nothing more.
    // The live instance holds the single-writer lock until it is dropped.
    drop(invocation);
    let mut recovered = recover(
        &scratch,
        &key,
        &program,
        "inv-positive",
        TornTailPolicy::Refuse,
    )
    .unwrap();
    let mut idle = Host::default();
    let status = recovered.drive(&policy(), &mut idle, &mut cleanup).unwrap();
    assert_eq!(settled_value(&status), ArgumentValue::Int(EXPECTED));
    assert!(idle.calls.is_empty());
    assert_eq!(cleanup.0, 1);
    // The same invocation cannot be started twice.
    assert!(matches!(
        start(&scratch, &key, &program, "inv-positive"),
        Err(ContinuationError::AlreadyStarted)
    ));
}

#[test]
fn explicit_request_answer_exchange_and_sticky_cleanup_failure() {
    struct FailingCleanup(usize);
    impl CleanupHandler<DurableOutcome> for FailingCleanup {
        fn run(&mut self, _: &DurableOutcome) -> Result<(), String> {
            self.0 += 1;
            Err("cleanup failed".into())
        }
    }
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let mut invocation = start(&scratch, &key, &program, "inv-explicit").unwrap();
    for _ in 0..3 {
        let request = invocation.dispatch(&policy()).unwrap();
        let ArgumentValue::Int(value) = request.request else {
            panic!()
        };
        invocation
            .answer(
                &policy(),
                &request.bind_answer(ArgumentValue::Int(value * 10)),
            )
            .unwrap();
    }
    // The result is durable but unpublished until cleanup settles.
    let pending = invocation.status();
    assert_eq!(
        pending,
        ContinuationStatus::CleanupPending { failure: None }
    );
    assert!(!format!("{pending:?}").contains(&EXPECTED.to_string()));
    let mut cleanup = FailingCleanup(0);
    invocation.settle(&mut cleanup).unwrap();
    assert_eq!(
        invocation.status(),
        ContinuationStatus::Settled {
            outcome: DurableOutcome::Completed(ArgumentValue::Int(EXPECTED)),
            cleanup: CleanupSettlement::Failed,
        }
    );
    assert!(matches!(
        invocation.settle(&mut cleanup),
        Err(ContinuationError::NotAwaitingCleanup)
    ));
    assert_eq!(cleanup.0, 1);
}

#[test]
fn host_failure_is_sticky_and_answers_after_it_refuse() {
    struct Failing;
    impl EffectHandler<ArgumentValue, ArgumentValue> for Failing {
        fn dispatch(&mut self, _: &ArgumentValue) -> Result<ArgumentValue, String> {
            Err("host failed".into())
        }
    }
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let mut invocation = start(&scratch, &key, &program, "inv-fail").unwrap();
    let ContinuationStatus::AwaitingDispatch(request) = invocation.status() else {
        panic!()
    };
    let status = invocation
        .drive(&policy(), &mut Failing, &mut Cleanup::default())
        .unwrap();
    assert!(matches!(
        status,
        ContinuationStatus::Settled {
            outcome: DurableOutcome::Failed(DurableFailure::HandlerFailed),
            ..
        }
    ));
    assert!(matches!(
        invocation.answer(&policy(), &request.bind_answer(ArgumentValue::Int(1))),
        Err(ContinuationError::AlreadySettled)
    ));
}

// --- crash after every record ------------------------------------------------

/// Crash immediately before or after each of the thirteen appends, recover
/// from the observed tail and finish. Every site is dispatched at most once;
/// cleanup runs at most once; in-doubt windows are surfaced, never repeated.
#[test]
fn crash_at_every_record_recovers_without_repeat_dispatch_or_cleanup() {
    let (key, program) = (key(), program(SOURCE));
    let (mut cases, mut answers_in_doubt, mut cleanups_in_doubt) = (0, 0, 0);
    for at in 0..FULL_RUN_RECORDS {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-crash-{at}-{after}");
            let (mut host, mut cleanup) = (Host::default(), Cleanup::default());
            arm(at, after);
            let crashed = start(&scratch, &key, &program, &invocation)
                .and_then(|mut live| live.drive(&policy(), &mut host, &mut cleanup));
            assert!(
                matches!(crashed, Err(ContinuationError::Storage)),
                "fault {at}/{after} did not fire: {crashed:?}"
            );
            let _ = armed_fault();
            // `create` precedes the first append, so the file always exists;
            // a crash before `Started` leaves it empty and recovery starts it.
            let acknowledged = line_count(&scratch.journal(&invocation));
            assert_eq!(acknowledged, at + usize::from(after));
            let mut recovered = recover(
                &scratch,
                &key,
                &program,
                &invocation,
                TornTailPolicy::Refuse,
            )
            .unwrap();
            let mut status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
            if let ContinuationStatus::AwaitingAnswer { request, in_doubt } = status.clone() {
                // Dispatched is durable but its answer is not: never redispatch.
                assert!(in_doubt);
                answers_in_doubt += 1;
                let dispatched_before = host.calls.len();
                let ArgumentValue::Int(value) = request.request else {
                    panic!()
                };
                recovered
                    .answer(
                        &policy(),
                        &request.bind_answer(ArgumentValue::Int(value * 10)),
                    )
                    .unwrap();
                status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
                assert!(host.calls[dispatched_before..]
                    .iter()
                    .all(|(site, _)| *site > request.site));
            }
            let cleanup_in_doubt = matches!(status, ContinuationStatus::CleanupInDoubt { .. });
            if cleanup_in_doubt {
                assert!(!format!("{status:?}").contains(&EXPECTED.to_string()));
                cleanups_in_doubt += 1;
                recovered.confirm_cleanup().unwrap();
                status = recovered.status();
            }
            assert_eq!(settled_value(&status), ArgumentValue::Int(EXPECTED));
            let mut sites: Vec<u32> = host.calls.iter().map(|(site, _)| *site).collect();
            sites.sort_unstable();
            sites.dedup();
            assert_eq!(
                sites.len(),
                host.calls.len(),
                "repeat dispatch at {at}/{after}"
            );
            if cleanup_in_doubt {
                assert!(cleanup.0 <= 1, "repeat cleanup at {at}/{after}");
            } else {
                assert_eq!(cleanup.0, 1, "cleanup count at {at}/{after}");
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 2 * FULL_RUN_RECORDS);
    // After each Dispatched, and before each Answered, for three sites; after
    // CleanupStarted and before CleanupSettled.
    assert_eq!((answers_in_doubt, cleanups_in_doubt), (6, 2));
}

// --- hostile answers ---------------------------------------------------------

#[test]
fn replayed_stale_and_misbound_answers_refuse_without_journal_change() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let other_program = program_with_other_digest();
    let mut invocation = start(&scratch, &key, &program, "inv-hostile").unwrap();
    let first = invocation.dispatch(&policy()).unwrap();
    let good_first = first.bind_answer(ArgumentValue::Int(30));
    invocation.answer(&policy(), &good_first).unwrap();
    let second = invocation.dispatch(&policy()).unwrap();
    let journal = scratch.journal("inv-hostile");
    let before = fs::read(&journal).unwrap();

    let mut wrong_program = second.bind_answer(ArgumentValue::Int(1));
    wrong_program.program_digest = *derive_source_effect_signature(&other_program, "app.ask")
        .unwrap()
        .plan_identity();
    let mut wrong_invocation = second.bind_answer(ArgumentValue::Int(1));
    wrong_invocation.invocation_id = "inv-other".into();
    let mut wrong_site = second.bind_answer(ArgumentValue::Int(1));
    wrong_site.site = 2;
    let mut stale = second.bind_answer(ArgumentValue::Int(1));
    stale.envelope_digest = first.envelope_digest;
    let wrong_type = second.bind_answer(ArgumentValue::Bool(true));
    for (answer, expected) in [
        (&good_first, "ReplayedAnswer"),
        (&wrong_program, "ProgramMismatch"),
        (&wrong_invocation, "InvocationMismatch"),
        (&wrong_site, "SiteMismatch"),
        (&stale, "StaleEnvelope"),
        (&wrong_type, "AnswerTypeMismatch"),
    ] {
        let error = invocation.answer(&policy(), answer).unwrap_err();
        assert_eq!(format!("{error:?}"), expected);
    }
    assert!(matches!(
        invocation.answer(
            &CapabilityPolicy::none(),
            &second.bind_answer(ArgumentValue::Int(1))
        ),
        Err(ContinuationError::CapabilityDenied)
    ));
    assert_eq!(fs::read(&journal).unwrap(), before);

    // The genuine answer still settles the site exactly once.
    let genuine = second.bind_answer(ArgumentValue::Int(600));
    invocation.answer(&policy(), &genuine).unwrap();
    assert!(matches!(
        invocation.answer(&policy(), &genuine),
        Err(ContinuationError::ReplayedAnswer)
    ));
    // A replayed answer after restart is refused by the recovered tail too.
    drop(invocation);
    let mut recovered = recover(
        &scratch,
        &key,
        &program,
        "inv-hostile",
        TornTailPolicy::Refuse,
    )
    .unwrap();
    assert!(matches!(
        recovered.answer(&policy(), &genuine),
        Err(ContinuationError::ReplayedAnswer)
    ));
    // Dispatch is capability-gated as well; the site stays undispatched.
    assert!(matches!(
        recovered.dispatch(&CapabilityPolicy::none()),
        Err(ContinuationError::CapabilityDenied)
    ));
    assert!(matches!(
        recovered.status(),
        ContinuationStatus::AwaitingDispatch(ContinuationRequest { site: 2, .. })
    ));
}

fn program_with_other_digest() -> ResolvedProgram {
    program(&SOURCE.replace("seed + 1", "seed + 9"))
}

#[test]
fn recovery_facts_must_match_the_started_record_exactly() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    drop(start(&scratch, &key, &program, "inv-facts").unwrap());
    let drifted = program_with_other_digest();
    let recover_with = |program: &ResolvedProgram, arguments: &[ArgumentValue], epoch: u64| {
        DurableInvocation::recover(
            &scratch.dir(),
            &key,
            program,
            "app.ask",
            arguments,
            "inv-facts",
            epoch,
            STEPS,
            TornTailPolicy::Refuse,
        )
        .err()
        .map(|error| format!("{error:?}"))
    };
    assert_eq!(
        recover_with(&drifted, &args(), 7).as_deref(),
        Some("ProgramMismatch")
    );
    assert_eq!(
        recover_with(&program, &[ArgumentValue::Int(3)], 7).as_deref(),
        Some("ArgumentsMismatch")
    );
    assert_eq!(
        recover_with(&program, &args(), 8).as_deref(),
        Some("PolicyEpochMismatch")
    );
    assert_eq!(recover_with(&program, &args(), 7), None);
    // The step budget is part of the journaled facts.
    assert!(matches!(
        DurableInvocation::recover(
            &scratch.dir(),
            &key,
            &program,
            "app.ask",
            &args(),
            "inv-facts",
            7,
            STEPS + 1,
            TornTailPolicy::Refuse,
        ),
        Err(ContinuationError::BudgetMismatch)
    ));
    // Another key cannot read (or forge) the journal.
    let other_key = SourceCheckpointKey::new([0x24; 32]);
    assert!(matches!(
        recover(
            &scratch,
            &other_key,
            &program,
            "inv-facts",
            TornTailPolicy::Refuse
        ),
        Err(ContinuationError::TamperedJournal)
    ));
}

// --- hostile journals and directories ----------------------------------------

#[test]
fn tampered_record_is_refused_and_never_truncated_away() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let mut invocation = start(&scratch, &key, &program, "inv-tamper").unwrap();
    let request = invocation.dispatch(&policy()).unwrap();
    invocation
        .answer(&policy(), &request.bind_answer(ArgumentValue::Int(30)))
        .unwrap();
    drop(invocation);
    let path = scratch.journal("inv-tamper");
    let original = fs::read_to_string(&path).unwrap();
    // Change the settled answer 30 -> 31 in place.
    let tampered = original.replacen("\"value\":30", "\"value\":31", 1);
    assert_ne!(tampered, original);
    for torn in [
        TornTailPolicy::Refuse,
        TornTailPolicy::TruncateUnacknowledged,
    ] {
        fs::write(&path, &tampered).unwrap();
        assert!(matches!(
            recover(&scratch, &key, &program, "inv-tamper", torn),
            Err(ContinuationError::TamperedJournal)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), tampered);
    }
    // Reordering or dropping a middle record breaks the chain.
    let lines: Vec<&str> = original.lines().collect();
    let mut reordered = lines.clone();
    reordered.swap(1, 2);
    let mut dropped = lines.clone();
    dropped.remove(2);
    for body in [reordered, dropped] {
        fs::write(&path, format!("{}\n", body.join("\n"))).unwrap();
        assert!(matches!(
            recover(
                &scratch,
                &key,
                &program,
                "inv-tamper",
                TornTailPolicy::Refuse
            ),
            Err(ContinuationError::TamperedJournal)
        ));
    }
}

#[test]
fn torn_tail_is_refused_or_truncated_exactly_by_explicit_policy() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let mut invocation = start(&scratch, &key, &program, "inv-torn").unwrap();
    let request = invocation.dispatch(&policy()).unwrap();
    drop(invocation);
    let path = scratch.journal("inv-torn");
    let acknowledged = fs::read(&path).unwrap();
    // A half-written Answered record: no trailing newline, never acknowledged.
    let mut torn = acknowledged.clone();
    torn.extend_from_slice(b"{\"schema\":\"semaprax.resumable-journal.v1\",\"seq\":3,\"pr");
    fs::write(&path, &torn).unwrap();
    assert!(matches!(
        recover(&scratch, &key, &program, "inv-torn", TornTailPolicy::Refuse),
        Err(ContinuationError::TornTail)
    ));
    assert_eq!(fs::read(&path).unwrap(), torn);
    let recovered = recover(
        &scratch,
        &key,
        &program,
        "inv-torn",
        TornTailPolicy::TruncateUnacknowledged,
    )
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), acknowledged);
    // The last acknowledged record was Dispatched: in doubt, not redispatched.
    assert_eq!(
        recovered.status(),
        ContinuationStatus::AwaitingAnswer {
            request,
            in_doubt: true
        }
    );
    drop(recovered);
    // A journal torn inside its Started record has no acknowledged record:
    // the invocation never began, so recovery acknowledges Started afresh.
    fs::write(&path, &acknowledged[..20]).unwrap();
    assert!(matches!(
        recover(&scratch, &key, &program, "inv-torn", TornTailPolicy::Refuse),
        Err(ContinuationError::TornTail)
    ));
    let restarted = recover(
        &scratch,
        &key,
        &program,
        "inv-torn",
        TornTailPolicy::TruncateUnacknowledged,
    )
    .unwrap();
    assert!(matches!(
        restarted.status(),
        ContinuationStatus::AwaitingDispatch(ContinuationRequest { site: 0, .. })
    ));
    assert_eq!(line_count(&path), 2);
}

#[test]
fn foreign_directories_and_journals_are_refused() {
    let (key, program) = (key(), program(SOURCE));
    // Group/other-accessible directory.
    let shared = Scratch::new(0o755);
    assert!(matches!(
        JournalDirectory::open(&shared.0),
        Err(ContinuationError::ForeignDirectory)
    ));
    // A symlink to an owner-private directory is not followed.
    let private = Scratch::new(0o700);
    let link = shared.0.with_extension("link");
    let _ = fs::remove_file(&link);
    symlink(&private.0, &link).unwrap();
    assert!(matches!(
        JournalDirectory::open(&link),
        Err(ContinuationError::ForeignDirectory)
    ));
    fs::remove_file(&link).unwrap();
    // A regular file is not a directory capability.
    let file = private.0.join("plain");
    fs::write(&file, b"x").unwrap();
    assert!(matches!(
        JournalDirectory::open(&file),
        Err(ContinuationError::ForeignDirectory)
    ));
    // A journal copied in from another invocation is refused by its binding.
    drop(start(&private, &key, &program, "inv-a").unwrap());
    let foreign = Scratch::new(0o700);
    fs::copy(private.journal("inv-a"), foreign.journal("inv-b")).unwrap();
    fs::set_permissions(foreign.journal("inv-b"), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        recover(&foreign, &key, &program, "inv-b", TornTailPolicy::Refuse),
        Err(ContinuationError::InvocationMismatch)
    ));
    // A journal symlinked in from elsewhere is not followed.
    let linked = Scratch::new(0o700);
    symlink(private.journal("inv-a"), linked.journal("inv-a")).unwrap();
    assert!(matches!(
        recover(&linked, &key, &program, "inv-a", TornTailPolicy::Refuse),
        Err(ContinuationError::ForeignDirectory)
    ));
    // A group-readable journal is refused.
    fs::set_permissions(private.journal("inv-a"), fs::Permissions::from_mode(0o640)).unwrap();
    assert!(matches!(
        recover(&private, &key, &program, "inv-a", TornTailPolicy::Refuse),
        Err(ContinuationError::ForeignDirectory)
    ));
}

#[test]
fn single_site_and_non_resumable_functions_are_refused_before_storage() {
    let scratch = Scratch::new(0o700);
    let key = key();
    let single = program(
        r#"module test.durable_continuation;
@id("app.ask") fn ask(seed: i64) -> i64 yields i64 -> i64 { yield seed }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    assert!(matches!(
        start(&scratch, &key, &single, "inv-single"),
        Err(ContinuationError::UnsupportedProfile)
    ));
    let full = program(SOURCE);
    assert!(matches!(
        DurableInvocation::start(
            &scratch.dir(),
            &key,
            &full,
            "app.main",
            &[],
            "inv-main",
            7,
            STEPS
        ),
        Err(ContinuationError::Admission(_))
    ));
    assert!(matches!(
        DurableInvocation::start(
            &scratch.dir(),
            &key,
            &full,
            "app.ask",
            &args(),
            "inv-budget",
            7,
            0
        ),
        Err(ContinuationError::InvalidBudget)
    ));
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
}

#[test]
fn one_live_writer_per_journal_even_when_poisoned() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let live = start(&scratch, &key, &program, "inv-busy").unwrap();
    assert!(matches!(
        recover(&scratch, &key, &program, "inv-busy", TornTailPolicy::Refuse),
        Err(ContinuationError::JournalBusy)
    ));
    // The torn-tail truncation path is behind the same lock.
    assert!(matches!(
        recover(
            &scratch,
            &key,
            &program,
            "inv-busy",
            TornTailPolicy::TruncateUnacknowledged
        ),
        Err(ContinuationError::JournalBusy)
    ));
    drop(live);
    let mut recovered =
        recover(&scratch, &key, &program, "inv-busy", TornTailPolicy::Refuse).unwrap();
    // A poisoned instance still holds the journal until it is dropped.
    arm(0, false);
    // Re-arming affects only journals opened after this point, so poison the
    // live instance through its own fault slot.
    recovered.journal.fault = armed_fault();
    assert!(matches!(
        recovered.dispatch(&policy()),
        Err(ContinuationError::Storage)
    ));
    assert!(matches!(
        recovered.dispatch(&policy()),
        Err(ContinuationError::Poisoned)
    ));
    assert!(matches!(
        recover(&scratch, &key, &program, "inv-busy", TornTailPolicy::Refuse),
        Err(ContinuationError::JournalBusy)
    ));
    drop(recovered);
    let again = recover(&scratch, &key, &program, "inv-busy", TornTailPolicy::Refuse).unwrap();
    assert!(matches!(
        again.status(),
        ContinuationStatus::AwaitingDispatch(ContinuationRequest { site: 0, .. })
    ));
}

#[test]
fn a_hard_linked_journal_is_refused() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    drop(start(&scratch, &key, &program, "inv-linked").unwrap());
    fs::hard_link(scratch.journal("inv-linked"), scratch.0.join("alias")).unwrap();
    assert!(matches!(
        recover(
            &scratch,
            &key,
            &program,
            "inv-linked",
            TornTailPolicy::Refuse
        ),
        Err(ContinuationError::ForeignDirectory)
    ));
}
