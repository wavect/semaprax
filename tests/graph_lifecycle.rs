use std::path::Path;

use semaprax::{graph, parse};

const SOURCE: &str = r#"
module examples.lifecycle_graph;
permit { filesystem.handle.release }
@id("io.file")
resource File { @id("io.file.drop") drop import "io.file.finalize"; }
@id("io.lock")
resource Lock { @id("io.lock.drop") drop import "io.lock.finalize"; }
@id("io.file.host")
interface FileHost permits { filesystem.handle.release } {
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes file always;
    @id("io.file.close")
    import fn close(file: own File) -> unit
        effects { filesystem.handle.release }
        failure status "io.error.v1"
        consumes file always;
    @id("io.lock.close")
    import fn close_lock(lock: own Lock) -> unit
        effects { filesystem.handle.release }
        failure status "io.lock.error.v1"
        consumes lock always;
}
@id("io.lock.host")
interface LockHost permits { filesystem.handle.release } {
    @id("io.lock.finalize")
    import fn finalize(lock: own Lock) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes lock always;
}
@id("io.file.inspect")
fn inspect(file: borrow File) -> i64 { 1 }
@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn lifecycle_graph_v10_exposes_contract_and_context_closure() {
    let program = parse(
        include_str!("../examples/lifecycle.spx"),
        Path::new("lifecycle-graph.spx"),
    )
    .unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(
        json.trim(),
        include_str!("snapshots/lifecycle.graph.json").trim()
    );
    assert!(json.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(json.contains("\"id\":\"platform.token.drop\",\"kind\":\"resource_drop\""));
    assert!(json.contains("\"strategy\":\"imported\",\"import\":\"platform.token.finalize\""));
    assert!(json.contains("\"id\":\"platform.token.host\",\"kind\":\"interface\""));
    assert!(json.contains("\"id\":\"platform.token.finalize\",\"kind\":\"import\""));
    assert!(json.contains("\"out_slot_initialization\":\"success_only\""));
    assert!(!json.contains("\"normalization\":\"semaprax.status.v1\""));

    let context_program = parse(SOURCE, Path::new("lifecycle-context.spx")).unwrap();
    let context = graph::context_json(&context_program, "io.file.inspect", 0)
        .unwrap()
        .unwrap();
    for id in [
        "io.file",
        "io.file.drop",
        "io.file.host",
        "io.file.finalize",
        "io.file.close",
        "io.lock",
        "io.lock.drop",
        "io.lock.close",
        "io.lock.host",
        "io.lock.finalize",
    ] {
        assert!(
            context.contains(&format!("\"id\":\"{id}\"")),
            "missing {id}"
        );
    }
    assert!(context.contains(
        "\"failure\":{\"kind\":\"status\",\"domain_id\":\"io.error.v1\",\"normalization\":\"semaprax.status.v1\"}"
    ));
    assert!(context.contains(
        "\"cleanup\":{\"kind\":\"cleanup_plan\",\"schema\":\"semaprax.cleanup-plan.v2\""
    ));
    assert!(!context.contains("\"id\":\"app.main\",\"kind\":\"function\""));
}
