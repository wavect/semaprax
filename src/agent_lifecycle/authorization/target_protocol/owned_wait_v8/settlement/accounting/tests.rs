//! Real target exchanges, not a two-turn source Agent or loop acceptance gate.
use super::*;
struct Host {
    bytes: Vec<u8>,
    calls: usize,
    fail: bool,
}
impl TargetHostHandler for Host {
    fn dispatch(
        &mut self,
        _: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        if self.fail {
            return Err(TargetHostError::Failed);
        }
        let _ = sink.write(&self.bytes);
        Ok(())
    }
}
fn limits() -> TargetLimits {
    TargetLimits {
        max_calls: 3,
        max_request_bytes: 10000,
        max_result_bytes: 10000,
        max_total_bytes: 20000,
        max_fuel: 3,
    }
}
fn exchange(
    ledger: &mut TargetAccounting,
    turn: u64,
    limits: TargetLimits,
    fail: bool,
    oversized: bool,
) -> (Vec<u8>, TargetDispatch, usize) {
    let operation = TargetOperation::new(
        "component.op",
        "component.effect",
        "component.arg",
        "component.result",
    )
    .unwrap();
    let argument = TypedCarrier::new("component.arg", vec![1]).unwrap();
    let grant = TargetGrant {
        grant_id: format!("sha256:{}", "1".repeat(64)),
        authorization_binding: format!("sha256:{}", "2".repeat(64)),
        operation: operation.clone(),
        argument_digest: super::super::super::argument_digest(&argument),
        turn,
        granted_budget: 3,
    };
    let request = TargetHostRequest {
        grant_id: grant.grant_id.clone(),
        authorization_binding: grant.authorization_binding.clone(),
        operation,
        turn,
        argument: argument.clone(),
        fuel: 1,
    }
    .canonical_wire();
    let mut host = Host {
        bytes: if oversized {
            vec![0; 20000]
        } else {
            TypedCarrier::new("component.result", vec![9])
                .unwrap()
                .encode()
        },
        calls: 0,
        fail,
    };
    let run = crate::agent_lifecycle::authorization::target_protocol::dispatch(
        grant,
        argument,
        1,
        limits,
        ledger,
        &AgentCancellation::new(),
        &mut host,
    );
    (request, run, host.calls)
}
fn checked(
    previous: Option<&CheckedTargetAccountingV8>,
    request: &[u8],
    run: &TargetDispatch,
    limits: TargetLimits,
) -> Result<CheckedTargetAccountingV8, Error> {
    let result = run.result().map(TypedCarrier::encode);
    run.evidence()
        .replay_exchange_wire(request, result.as_deref())
        .unwrap();
    verify(
        reserve(previous, request, limits)?,
        limits,
        run.evidence(),
        result.as_deref(),
    )
}
#[test]
fn owned_wait_accounting_two_real_exchanges_preserve_predecessor_and_all_dimensions() {
    for failed_first in [false, true] {
        let mut ledger = TargetAccounting::default();
        let (request, first, calls) = exchange(&mut ledger, 0, limits(), failed_first, false);
        assert_eq!(calls, 1);
        let prefix = checked(None, &request, &first, limits()).unwrap();
        assert_eq!(*prefix.total(), ledger);
        let (next, second, calls) = exchange(&mut ledger, 1, limits(), false, false);
        assert_eq!(calls, 1);
        let proof = checked(Some(&prefix), &next, &second, limits()).unwrap();
        assert_eq!(*proof.total(), ledger);
        assert_eq!(ledger.calls(), 2);
        assert_eq!(ledger.fuel(), 2);
        assert_eq!(ledger.request_bytes(), (request.len() + next.len()) as u64);
        assert_eq!(
            ledger.result_bytes(),
            first.result().map_or(0, |r| r.encode().len()) as u64
                + second.result().unwrap().encode().len() as u64
        );
        assert!(
            checked(None, &next, &second, limits()).is_err(),
            "missing predecessor/reset refused"
        );
        assert!(
            checked(Some(&proof), &request, &first, limits()).is_err(),
            "swapped/decreasing predecessor refused"
        );
        for dimension in 0..4 {
            let mut altered = second.evidence().clone();
            match dimension {
                0 => altered.accounting.calls -= 1,
                1 => altered.accounting.request_bytes -= 1,
                2 => altered.accounting.result_bytes -= 1,
                _ => altered.accounting.fuel -= 1,
            }
            altered.digest = altered.compute_digest();
            assert!(verify(
                reserve(Some(&prefix), &next, limits()).unwrap(),
                limits(),
                &altered,
                second.result().map(TypedCarrier::encode).as_deref()
            )
            .is_err());
        }
        let mut reminted = second.evidence().clone();
        reminted.accounting = *prefix.total();
        reminted.digest = reminted.compute_digest();
        assert!(verify(
            reserve(Some(&prefix), &next, limits()).unwrap(),
            limits(),
            &reminted,
            second.result().map(TypedCarrier::encode).as_deref()
        )
        .is_err());
    }
}
#[test]
fn owned_wait_accounting_remaining_total_exhaustion_and_exact_overflow_sentinel() {
    let mut ledger = TargetAccounting::default();
    let (request, first, _) = exchange(&mut ledger, 0, limits(), false, false);
    let prefix = checked(None, &request, &first, limits()).unwrap();
    let bound = TargetLimits {
        max_result_bytes: 1024,
        max_total_bytes: ledger.request_bytes() + ledger.result_bytes(),
        ..limits()
    };
    assert!(
        reserve(Some(&prefix), &request, bound).is_err(),
        "prior result bytes remain spent before next host"
    );
    let overflow_limits = TargetLimits {
        max_result_bytes: 1024,
        ..limits()
    };
    let (next, second, calls) = exchange(&mut ledger, 1, overflow_limits, false, true);
    assert_eq!(calls, 1);
    assert_eq!(second.evidence().settlement(), Settlement::ResultBudget);
    let proof = checked(Some(&prefix), &next, &second, overflow_limits).unwrap();
    assert_eq!(
        proof.total().result_bytes(),
        prefix.total().result_bytes() + 1025
    );
    let mut reminted = second.evidence().clone();
    reminted.accounting.result_bytes -= 1;
    reminted.digest = reminted.compute_digest();
    assert!(verify(
        reserve(Some(&prefix), &next, overflow_limits).unwrap(),
        overflow_limits,
        &reminted,
        None
    )
    .is_err());
}
