//! Closed inert §23 nested bodies. These do not carry a runtime owner or ACK.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ReduceBasisV8 {
    InitialFailure {
        status: Value,
    },
    PartialFailure {
        status: Value,
        constructor: String,
        case: String,
        transfer_prefix: Vec<String>,
        active_flags: Vec<u32>,
    },
    ProvisionalFailure {
        status: Value,
        constructor: String,
        case: String,
        active_flags: Vec<u32>,
    },
    Success {
        staged: u32,
        constructor: String,
        case: String,
        active_flags: Vec<u32>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ReduceCleanupV8 {
    CompilerEmpty,
    Observed { started: u32, settled: u32 },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ReduceTargetV8 {
    Continue { state: Value },
    Suspend { state: Value },
    Complete { report: Value },
    Fail { code: i64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn owned_reduce_nested_unions_refuse_extra_missing_and_incompatible_fields() {
        for value in [
            json!({"kind":"compiler_empty","started":1}),
            json!({"kind":"observed","started":1}),
            json!({"kind":"observed","started":null,"settled":2}),
        ] {
            assert!(serde_json::from_value::<ReduceCleanupV8>(value).is_err());
        }
        assert!(
            serde_json::from_value::<ReduceCleanupV8>(json!({"kind":"compiler_empty"})).is_ok()
        );
        assert!(
            serde_json::from_value::<ReduceTargetV8>(json!({"kind":"fail","code":i64::MIN}))
                .is_ok()
        );
        for value in [
            json!({"kind":"fail","code":null}),
            json!({"kind":"fail","code":u64::MAX}),
            json!({"kind":"complete","report":{},"state":{}}),
        ] {
            assert!(serde_json::from_value::<ReduceTargetV8>(value).is_err());
        }
        assert!(serde_json::from_value::<ReduceBasisV8>(json!({"kind":"success","staged":1,"constructor":"e","case":"c","active_flags":[],"status":null})).is_err());
    }
}
