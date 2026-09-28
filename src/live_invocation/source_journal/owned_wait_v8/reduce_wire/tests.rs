use super::*;
use serde_json::json;

fn step() -> Value {
    json!({"scope":{"profile":"p"},"binding":"b","plan":"b","turn":0,"attempt":0,"stage_reservation":9,"step":{"kind":"fail","code":-2}})
}
#[test]
fn owned_reduce_commitment_has_independent_canonical_known_answer() {
    // Expected SHA-256 independently computed over lexical JSON without LF.
    assert_eq!(
        recipe_digest(ReduceRecipeV8::Step, &step()).unwrap(),
        "sha256:6d87fa8a7e50b174071c9997ec6fcc2a859bcf3a0e62ec63cf43289bea917638"
    );
    let mut changed = step();
    changed["stage_reservation"] = 10.into();
    assert_ne!(
        recipe_digest(ReduceRecipeV8::Step, &step()).unwrap(),
        recipe_digest(ReduceRecipeV8::Step, &changed).unwrap()
    );
}
#[test]
fn owned_reduce_recipes_refuse_open_key_sets_and_floating_payloads() {
    let mut value = step();
    value["extra"] = true.into();
    assert!(recipe_digest(ReduceRecipeV8::Step, &value).is_err());
    value.as_object_mut().unwrap().remove("extra");
    value.as_object_mut().unwrap().remove("plan");
    assert!(recipe_digest(ReduceRecipeV8::Step, &value).is_err());
    value = step();
    value["step"]["code"] = json!(1.5);
    assert!(recipe_digest(ReduceRecipeV8::Step, &value).is_err());
    assert!(recipe_digest(ReduceRecipeV8::Basis, &step()).is_err());
    assert!(recipe_digest(ReduceRecipeV8::Transfer, &step()).is_err());
}
#[test]
fn owned_reduce_transfer_mapping_order_is_committed_without_repair() {
    let mut value = json!({"scope":{},"binding":"b","plan":"b","turn":0,"attempt":0,"reserved":12,"case":"continue","mapping":[["a","x"],["b","y"]],"target":{"kind":"continue","state":{}}});
    let original = recipe_digest(ReduceRecipeV8::Transfer, &value).unwrap();
    value["mapping"].as_array_mut().unwrap().reverse();
    assert_ne!(
        original,
        recipe_digest(ReduceRecipeV8::Transfer, &value).unwrap()
    );
    let basis = json!({"scope":{},"binding":"b","plan":"b","turn":0,"attempt":0,"stage_reservation":9,"basis":{"kind":"initial_failure","status":{}}});
    assert!(recipe_digest(ReduceRecipeV8::Basis, &basis).is_ok());
}
