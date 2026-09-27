use super::*;

pub(super) fn assert_checkpoint_host_refusal() {
    let fixture = Fixture::new();
    let checkpoint = fixture.0.join("checkpoint");
    for attempt in [
        CheckpointDir::fresh(&checkpoint, &fixture.0),
        CheckpointDir::existing(&checkpoint, &fixture.0),
    ] {
        let error = attempt.err().expect("Windows checkpoint host must refuse");
        assert_eq!(
            error.reason,
            "source-live CLI requires a Unix checkpoint host"
        );
        assert!(!checkpoint.exists());
    }
}

#[test]
fn checkpoint_host_refuses_fresh_and_resume_without_creating_state() {
    assert_checkpoint_host_refusal();
}
