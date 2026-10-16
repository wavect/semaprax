//! Independent scheduler schema through the existing full direct-codec owner.
use super::*;

#[test]
fn independent_scheduler_nested_request_detaches_owners_and_keeps_typed_refusals() {
    let root = install_example(
        "nested-scheduler-json",
        "examples/nested-scheduler-json-project",
        "ScheduleRequest",
        "scheduler.request",
    );
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/nested-scheduler-json-project/src/tests.consumer.spx");
    std::fs::copy(tests, root.join("src/tests.spx")).unwrap();
    // Reuse the complete order/configuration backend owner with its exact HIR,
    // graph, interpreter, native O0/O2, Wasm validator and strict host checks.
    qualify_named(&root, "scheduler.request");
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        let result = snapshot.execute_test(&options)?;
        assert!(result.command_succeeded());
        assert_eq!(result.cases().len(), 13);
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
