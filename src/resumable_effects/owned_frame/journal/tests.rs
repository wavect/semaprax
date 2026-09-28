use super::super::store::OwnedFrameStoreRegistration;
use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::resumable::owned_frame::{
    durable::DurableOwner, OwnedFrameBudget, OwnedFrameInputField, OwnedFrameInputValue,
};
use crate::interpreter::ArgumentValue;
use std::fs::File;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(in crate::resumable_effects::owned_frame) struct Directory(PathBuf);
impl Directory {
    pub(in crate::resumable_effects::owned_frame) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "spx-owned-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    pub(in crate::resumable_effects::owned_frame) fn file(&self) -> File {
        File::open(&self.0).unwrap()
    }
    pub(in crate::resumable_effects::owned_frame) fn identity(&self) -> (u64, u64) {
        let m = std::fs::metadata(&self.0).unwrap();
        (m.dev(), m.ino())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(in crate::resumable_effects::owned_frame) fn fixture() -> (
    CheckedOwnedFramePlan,
    OwnedFrameInput,
    SourceCheckpointScope,
    SourceCheckpointKey,
) {
    let source = r#"module fixture.owned_journal;
@id("fixture.state") record State {
@id("fixture.state.z") first:Bytes,
@id("fixture.state.a") second:Bytes,
@id("fixture.state.m") third:Bytes,
@id("fixture.state.b") budget:i64,
}
@id("fixture.park") fn park(state:own State)->State yields i64->i64 {
let answer=yield state.budget; state
}
@id("fixture.main") fn main()->i64 {0}
"#;
    let program =
        crate::hir::resolve(&crate::parse(source, Path::new("owned-journal.spx")).unwrap())
            .unwrap();
    let plan =
        super::super::compile_owned_frame_plan(&program, &DeclarationId::new("fixture.park"))
            .unwrap();
    let input = OwnedFrameInput {
        declaration: DeclarationId::new("fixture.state"),
        fields: vec![
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.z"),
                value: OwnedFrameInputValue::Bytes(vec![0, 7]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.a"),
                value: OwnedFrameInputValue::Bytes(vec![]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.m"),
                value: OwnedFrameInputValue::Bytes(vec![0]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.b"),
                value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(4)),
            },
        ],
    };
    (
        plan,
        input,
        SourceCheckpointScope::new("sha256:program", "owned-journal", 7).unwrap(),
        SourceCheckpointKey::new([7; 32]),
    )
}
fn created(state: &State, input: &OwnedFrameInput) -> Record {
    let argument = codec::input(&state.plan, input).unwrap();
    Record::new(Kind::Created,json!({"profile":super::super::plan::PROFILE,"scope":codec::scope(&state.scope).unwrap(),"function":state.plan.function().id.as_str(),"plan_digest":state.plan.binding(),"signature":codec::signature(&state.plan),"argument_digest":codec::fact_digest(b"semaprax.source-owned-frame-arguments.v1\0",&argument),"argument":argument,"max_steps":state.max_steps,"max_reserved_fuel":state.max_reserved_fuel,"limits":fold::limits()})).unwrap()
}
fn committed(state: &State) -> Record {
    Record::new(Kind::ArgumentCommitted,json!({"argument_digest":state.argument_digest().unwrap(),"storage":codec::storage(&state.plan.liveness().storage).unwrap(),"leaf_flags":codec::leaf_flags(&state.plan)})).unwrap()
}
#[test]
fn owned_frame_journal_restoration_requires_commit_held_lease_and_once_per_history() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope).unwrap();
    let identity = lease.identity();
    let state = State::new(plan.clone(), scope.clone(), 100, 1000, identity).unwrap();
    let mut journal = Journal::fresh(lease, &key, state).unwrap();
    journal
        .append(created(&journal.state, &input), &[])
        .unwrap();
    assert!(
        journal.restore_permit().is_err(),
        "Created is not a committed owner"
    );
    journal.append(committed(&journal.state), &[]).unwrap();
    drop(journal);
    let registration =
        OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true).unwrap();
    let lease = RegisteredJournalLease::recover(directory.file(), registration, &scope).unwrap();
    let state = State::new(plan.clone(), scope.clone(), 100, 1000, identity).unwrap();
    let mut journal = Journal::reopen(lease, &key, state).unwrap();
    let owner = DurableOwner::restore(&plan, journal.restore_permit().unwrap()).unwrap();
    assert!(
        journal.restore_permit().is_err(),
        "a second live logical root must not be materialized"
    );
    assert_eq!(
        codec::input(&plan, &owner.input().unwrap()).unwrap(),
        codec::input(&plan, &input).unwrap()
    );
    let registration =
        OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true).unwrap();
    assert_eq!(
        RegisteredJournalLease::recover(directory.file(), registration, &scope).err(),
        Some(Error::Busy)
    );
    drop(owner);
}
#[test]
fn owned_frame_journal_repeated_interrupted_replay_charges_and_requires_latest_ack() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope).unwrap();
    let state = State::new(plan.clone(), scope.clone(), 100, 400, lease.identity()).unwrap();
    let mut journal = Journal::fresh(lease, &key, state).unwrap();
    journal
        .append(created(&journal.state, &input), &[])
        .unwrap();
    journal.append(committed(&journal.state), &[]).unwrap();
    let owner = DurableOwner::restore(&plan, journal.restore_permit().unwrap()).unwrap();
    let basis = journal.state.basis().unwrap();
    for total in [100, 200] {
        journal
            .append(
                Record::new(
                    Kind::ReplayReserved,
                    json!({"basis":basis,"reservation":100,"reserved_total":total}),
                )
                .unwrap(),
                &[],
            )
            .unwrap();
    }
    assert_eq!(journal.state.reserved_total, 200);
    assert_eq!(
        journal.state.consumed_total, 0,
        "unobserved interrupted work is not inferred"
    );
    let stale = Record::new(
        Kind::ReplayValidated,
        json!({"basis":basis,"reservation_sequence":2,"consumed_steps":3}),
    )
    .unwrap();
    assert_eq!(journal.append(stale, &[]), Err(Error::Binding));
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let facts = owner.replay_start(&mut budget);
    assert_eq!(facts.request(), Some(&ArgumentValue::Int(4)));
    journal
        .append(
            Record::new(
                Kind::ReplayValidated,
                json!({"basis":basis,"reservation_sequence":3,"consumed_steps":budget.consumed()}),
            )
            .unwrap(),
            &[],
        )
        .unwrap();
    journal
        .append(
            Record::new(
                Kind::StartReserved,
                json!({"causal_sequence":4,"reservation":100,"reserved_total":300}),
            )
            .unwrap(),
            &[],
        )
        .unwrap();
    assert_eq!(journal.state.reservation_count, 3);
    assert_eq!(journal.state.consumed_total, budget.consumed() as u64);
    assert_eq!(
        journal.append(
            Record::new(
                Kind::StartReserved,
                json!({"causal_sequence":5,"reservation":100,"reserved_total":400})
            )
            .unwrap(),
            &[]
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        journal.state.reserved_total, 300,
        "rejected transition has no charge or write"
    );
}
