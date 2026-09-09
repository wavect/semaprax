use super::*;

fn request(stdout_max: usize, stderr_max: usize) -> ProcessRequest {
    let mut argv = Vec::new();
    argv.extend_from_slice(&2u32.to_le_bytes());
    argv.extend_from_slice(&3u32.to_le_bytes());
    argv.extend_from_slice(b"run");
    argv.extend_from_slice(&2u32.to_le_bytes());
    argv.extend_from_slice(b"ok");
    ProcessRequest::from_wire(7, &argv, argv.len(), b"in", 2, 9, stdout_max, stderr_max).unwrap()
}

#[test]
fn request_decodes_exact_non_nul_wire() {
    let request = request(9, 8);
    assert_eq!(request.tool(), 7);
    assert_eq!(request.argument(0), Some(&b"run"[..]));
    assert_eq!(request.argument(1), Some(&b"ok"[..]));
    assert_eq!(request.stdin(), b"in");
    assert_eq!(request.input_bytes(), 19);
}

#[test]
fn request_refuses_invalid_wire_and_capacities() {
    assert_eq!(
        ProcessRequest::from_wire(0, &[0, 0, 0, 0, 9], 5, &[], 0, 1, 0, 0),
        Err(ProcessFailure::InvalidInput)
    );
    assert_eq!(
        ProcessRequest::from_wire(0, &[1, 0, 0, 0, 1, 0, 0, 0, 0], 9, &[], 0, 1, 0, 0),
        Err(ProcessFailure::InvalidInput)
    );
    assert_eq!(
        ProcessRequest::from_wire(0, &[0, 0, 0, 0], 3, &[], 0, 1, 0, 0),
        Err(ProcessFailure::InvalidInput)
    );
    assert_eq!(
        ProcessRequest::from_wire(0, &[0, 0, 0, 0], 4, &[], 0, 0, 0, 0),
        Err(ProcessFailure::InvalidInput)
    );
}

#[test]
fn output_wire_round_trips_and_authenticates_limits() {
    let output = ProcessOutput {
        termination: ProcessTermination::Signalled(9),
        stdout: b"yes".to_vec(),
        stderr: b"no".to_vec(),
    };
    let request = request(3, 2);
    let wire = output.encode(&request).unwrap();
    assert_eq!(ProcessOutput::decode(&wire, 3, 2), Ok(output));
    assert_eq!(
        ProcessOutput::decode(&wire, 2, 2),
        Err(ProcessFailure::CapacityExceeded)
    );
    let mut malformed = wire;
    malformed[8] = 2;
    assert_eq!(
        ProcessOutput::decode(&malformed, 3, 2),
        Err(ProcessFailure::InvalidInput)
    );
}

#[test]
fn fixture_and_budget_are_deterministic_and_non_refunding() {
    let expected = request(4, 4);
    let output = ProcessOutput {
        termination: ProcessTermination::Exited(0),
        stdout: b"done".to_vec(),
        stderr: Vec::new(),
    };
    let mut fixture = FixtureProcessProvider::new([FixtureProcessStep {
        request: expected.clone(),
        response: Ok(output.clone()),
    }]);
    assert_eq!(fixture.run(&expected), Ok(output));
    assert_eq!(fixture.remaining(), 0);
    assert_eq!(fixture.settle(), Ok(()));
    assert_eq!(fixture.settlements(), 1);

    let mut budget = ProcessInvocationBudget::new();
    assert_eq!(budget.reserve(&expected), Ok(()));
    assert_eq!(budget.runs(), 1);
    assert_eq!(budget.total_bytes(), expected.input_bytes() + 40);
}

#[test]
fn budget_rejects_the_first_over_capacity_without_debiting() {
    let request = request(0, 0);
    let mut budget = ProcessInvocationBudget::new();
    for _ in 0..MAX_RUNS {
        budget.reserve(&request).unwrap();
    }
    let before = budget;
    assert_eq!(
        budget.reserve(&request),
        Err(ProcessFailure::CapacityExceeded)
    );
    assert_eq!(budget, before);
}
