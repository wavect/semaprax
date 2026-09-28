//! Independent ordinary AST execution on copied inert fixture values. No
//! oracle value is admitted back into the physical owned route.
use super::*;
use crate::interpreter::resumable::{ResumableChannelValue, Resumption};
use crate::interpreter::{
    Evaluator, FunctionLookup, OwnedBytesValue, OwnedRecordValue, PreparedCancellation,
};
use std::collections::BTreeMap;
use std::sync::Arc;
impl LiveContinuedParkedStateV8<'_> {
    pub(crate) fn test_ordinary_start(&self) -> (serde_json::Value, ResumableChannelValue, usize) {
        let actual = &self.parked.parked;
        let plan = &actual.plan;
        let Value::Record(record) = actual.root.as_ref().expect("actual root") else {
            panic!("State")
        };
        let root = Value::Record(Arc::new(OwnedRecordValue {
            record: record.record.clone(),
            fields: record
                .fields
                .iter()
                .map(|(id, value)| {
                    let inert = match value {
                        Value::Bytes(bytes) => Value::Bytes(OwnedBytesValue {
                            allocation: bytes.allocation,
                            bytes: Arc::from(bytes.bytes.to_vec()),
                        }),
                        other => crate::interpreter::resumable::clone_scalar(other)
                            .expect("actual scalar"),
                    };
                    (id.clone(), inert)
                })
                .collect(),
        }));
        let state = super::super::super::root_facts(plan, &root).unwrap();
        let copy = crate::interpreter::resumable::owned_frame::channel_v2::value_of_copy(
            &plan.program().declarations,
            &plan.function().params[1].ty,
            actual.request(),
        )
        .unwrap();
        let admitted = BTreeMap::new();
        let mut evaluator = Evaluator::new_prepared(
            FunctionLookup::Borrowed(&admitted),
            BTreeMap::new(),
            &plan.program().declarations,
            self.predecessor.test_fuel(),
            0,
            PreparedCancellation::Never,
        );
        evaluator.resumption = Resumption::Fresh {
            parked: None,
            parked_site: None,
            parked_environment: None,
        };
        let result = evaluator.call_frame(
            plan.function(),
            vec![
                (plan.function().params[0].id.clone(), root),
                (plan.function().params[1].id.clone(), copy),
            ],
            0,
        );
        assert!(matches!(
            result,
            Err(crate::interpreter::Flow::Guard(
                crate::interpreter::resumable::SUSPENDED_AT_YIELD
            ))
        ));
        let Resumption::Fresh {
            parked: Some(request),
            ..
        } = &evaluator.resumption
        else {
            panic!("ordinary park")
        };
        let channel =
            crate::interpreter::resumable::channel_of(&plan.program().declarations, request)
                .unwrap();
        (state, channel, evaluator.steps)
    }
}
