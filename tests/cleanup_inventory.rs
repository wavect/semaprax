use std::path::Path;

use semaprax::cleanup::{
    CleanupStorageId, CleanupStorageOrigin, FieldLivenessShape, CLEANUP_INVENTORY_SCHEMA_V1,
};
use semaprax::hir;
use semaprax::{codegen, parse, wasm};

const SOURCE: &str = r#"module test.cleanup_inventory;
permit { io.release }

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("file.type")
resource File {
    @id("file.drop")
    drop import "file.finalize";
}

@id("inner.type")
record Inner {
    @id("inner.token")
    token: Token,
    @id("inner.count")
    count: i64,
}

@id("outer.type")
record Outer {
    @id("outer.inner")
    inner: Inner,
    @id("outer.file")
    file: File,
}

@id("file.host")
interface FileHost permits { io.release } {
    @id("file.finalize")
    import fn finalize(file: own File) -> unit
        effects { io.release }
        failure infallible
        consumes file always;
}

@id("outer.identity")
fn identity(value: own Outer) -> Outer uses { io.release }
{
    value
}

@id("outer.pipeline")
fn pipeline(value: own Outer) -> Outer uses { io.release }
{
    let moved = identity(value);
    moved
}

@id("outer.choose")
fn choose(
    left_token: own Token,
    left_file: own File,
    right_token: own Token,
    right_file: own File
) -> Outer uses { io.release }
{
    if true {
        Outer {
            inner: Inner { token: left_token, count: 1 },
            file: left_file,
        }
    } else {
        Outer {
            inner: Inner { token: right_token, count: 2 },
            file: right_file,
        }
    }
}

@id("outer.take_file")
fn take_file(token: own Token, file: own File) -> File uses { io.release }
{
    Outer {
        inner: Inner { token: token, count: 1 },
        file: file,
    }.file
}

@id("outer.inspect")
fn inspect(value: borrow Outer) -> i64
{
    value.inner.count
}

@id("outer.shared")
fn shared_view(value: shared Outer) -> i64
{
    value.inner.count
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn resolved() -> hir::ResolvedProgram {
    let program = parse(SOURCE, Path::new("cleanup-inventory.spx")).unwrap();
    hir::resolve(&program).unwrap()
}

#[test]
fn inventory_catalogs_recursive_resource_leaves_and_entry_ownership() {
    let program = resolved();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "outer.pipeline")
        .unwrap();
    let inventory = &function.cleanup;
    assert_eq!(inventory.schema, CLEANUP_INVENTORY_SCHEMA_V1);
    assert_eq!(
        inventory.entry_state.live_owned_parameters,
        [CleanupStorageId(0)]
    );
    assert_eq!(inventory.slots.len(), 5);
    assert_eq!(inventory.flags.len(), 10);
    assert!(matches!(
        &inventory.slots[0].origin,
        CleanupStorageOrigin::Parameter {
            parameter_index: 0,
            ..
        }
    ));
    assert!(matches!(
        &inventory.slots[1].origin,
        CleanupStorageOrigin::Temporary { expression }
            if expression.as_str().contains("body.s0.value")
    ));
    assert!(matches!(
        &inventory.slots[2].origin,
        CleanupStorageOrigin::Binding { .. }
    ));
    assert!(matches!(
        &inventory.slots[3].origin,
        CleanupStorageOrigin::Temporary { expression }
            if expression.as_str().contains(":body")
    ));
    assert!(matches!(
        &inventory.slots[4].origin,
        CleanupStorageOrigin::ProvisionalResult { .. }
    ));
    for (index, slot) in inventory.slots.iter().enumerate() {
        assert_eq!(slot.id, CleanupStorageId(index as u32));
        assert_eq!(slot.discovery_index, index as u32);
        let FieldLivenessShape::Record {
            declaration,
            fields,
        } = &slot.shape
        else {
            panic!("Outer storage must retain its recursive record shape")
        };
        assert_eq!(declaration.as_str(), "outer.type");
        assert_eq!(fields[0].field.as_str(), "outer.inner");
        assert_eq!(fields[0].field_index, 0);
        assert_eq!(fields[1].field.as_str(), "outer.file");
        assert_eq!(fields[1].field_index, 1);
    }
    assert_eq!(
        inventory.flags[0]
            .place
            .projections
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ["outer.inner", "inner.token"]
    );
    assert_eq!(inventory.flags[0].lifecycle.as_str(), "token.drop");
    assert_eq!(
        inventory.flags[1]
            .place
            .projections
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ["outer.file"]
    );
    assert_eq!(inventory.flags[1].lifecycle.as_str(), "file.drop");
}

#[test]
fn borrow_shared_and_scalar_functions_have_no_cleanup_storage() {
    let program = resolved();
    for id in ["outer.inspect", "outer.shared", "app.main"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        assert!(function.cleanup.slots.is_empty(), "unexpected slot in {id}");
        assert!(function.cleanup.flags.is_empty(), "unexpected flag in {id}");
        assert!(function
            .cleanup
            .entry_state
            .live_owned_parameters
            .is_empty());
    }
}

#[test]
fn inventory_covers_branch_construction_and_owned_temporary_projection() {
    let program = resolved();
    let choose = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "outer.choose")
        .unwrap();
    assert_eq!(
        choose.cleanup.entry_state.live_owned_parameters,
        [
            CleanupStorageId(0),
            CleanupStorageId(1),
            CleanupStorageId(2),
            CleanupStorageId(3),
        ]
    );
    assert_eq!(choose.cleanup.slots.len(), 13);
    assert_eq!(
        choose
            .cleanup
            .slots
            .iter()
            .filter(|slot| matches!(slot.origin, CleanupStorageOrigin::Temporary { .. }))
            .count(),
        8
    );
    assert!(matches!(
        choose.cleanup.slots.last().unwrap().origin,
        CleanupStorageOrigin::ProvisionalResult { .. }
    ));

    let take_file = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "outer.take_file")
        .unwrap();
    assert_eq!(take_file.cleanup.slots.len(), 7);
    assert_eq!(take_file.cleanup.flags.len(), 8);
    assert!(matches!(
        &take_file.cleanup.slots[4].origin,
        CleanupStorageOrigin::Temporary { expression }
            if expression.as_str().contains("body.tail")
    ));
    assert_eq!(
        take_file.cleanup.slots[4].ty.identity_key(),
        "nominal:9:file.type:0:"
    );
}

#[test]
fn inventory_is_deterministic_and_hostile_mutations_fail_before_backend_gates() {
    let program = resolved();
    assert_eq!(program, resolved());

    let mut missing_slot = program.clone();
    missing_slot
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "outer.pipeline")
        .unwrap()
        .cleanup
        .slots
        .pop();
    assert_eq!(hir::validate(&missing_slot).unwrap_err().code, "SPX-H006");
    assert_eq!(
        codegen::emit_hir_c(&missing_slot).unwrap_err().code,
        "SPX-H006"
    );
    assert_eq!(
        wasm::emit_resolved_module(&missing_slot).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_entry = program.clone();
    wrong_entry
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "outer.inspect")
        .unwrap()
        .cleanup
        .entry_state
        .live_owned_parameters
        .push(CleanupStorageId(0));
    assert_eq!(hir::validate(&wrong_entry).unwrap_err().code, "SPX-H006");

    let mut wrong_lifecycle = program;
    let pipeline = wrong_lifecycle
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "outer.pipeline")
        .unwrap();
    pipeline.cleanup.flags[0].lifecycle = pipeline.cleanup.flags[1].lifecycle.clone();
    assert_eq!(
        hir::validate(&wrong_lifecycle).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn inventory_uses_semantic_ids_not_nominal_display_names() {
    let original = resolved();
    let renamed_source = SOURCE.replace("Token", "Capability");
    let renamed_ast = parse(&renamed_source, Path::new("cleanup-inventory-renamed.spx")).unwrap();
    let renamed = hir::resolve(&renamed_ast).unwrap();
    let inventory = |program: &hir::ResolvedProgram| {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "outer.pipeline")
            .unwrap()
            .cleanup
            .clone()
    };
    assert_eq!(inventory(&original), inventory(&renamed));
}
