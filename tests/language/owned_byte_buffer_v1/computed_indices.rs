use super::*;

#[test]
fn a_computed_element_index_is_admitted_and_stores_in_range_on_every_backend() {
    let program = parse(COMPUTED, "owned-byte-buffer-computed.spx").unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "a computed usize element index is admitted source"
    );
    let canonical = format::canonical(&program);
    assert_eq!(
        format::canonical(&parse(&canonical, "owned-byte-buffer-computed-canonical.spx").unwrap()),
        canonical,
        "the computed-index chain round-trips through the canonical formatter"
    );

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    // The bound is a selected operation failure, not a backend accident: each
    // store owns one status source, and every exit still finalizes exactly one
    // slot, so a failed store has the same single destruction path.
    let plan = &main_function(&resolved).cleanup_plan;
    assert_eq!(
        plan.status_sources
            .iter()
            .filter(|source| matches!(
                &source.producer,
                semaprax::cleanup_plan::StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == "core.bytes.set"))
            .count(),
        2,
        "each bytes_set link carries its own element-bound status source"
    );
    for exit in &plan.exits {
        assert!(exit.finalize_in_order.len() <= 1);
    }

    let interpreted = interpret(COMPUTED, "computed-interp");
    assert!(
        interpreted.contains("\"kind\":\"returned\"") && interpreted.contains("\"value\":\"7\""),
        "the reference interpreter stores at the computed offsets: {interpreted}"
    );

    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert_eq!(
        generated.matches("spx_bytes_set_check_v1(spx_ctx,").count(),
        2,
        "the native backend checks the bound once per store"
    );
    assert_eq!(
        generated
            .matches("spx_bytes_set(spx_bytes_move(&spx_bytes_slot_")
            .count(),
        2,
        "each successful checked store commits its one staged owner"
    );
    assert!(generated.contains("semaprax.byte-buffer.v1"));

    // Core-Wasm emits the same check in generated code rather than relying on
    // the host import, and stays deterministic.
    let emitted = wasm::emit_module(&program).unwrap();
    assert_eq!(emitted, wasm::emit_module(&program).unwrap());
    assert!(emitted.starts_with(b"\0asm"));

    if !command_available("clang") {
        return;
    }
    let native = std::env::temp_dir().join(format!(
        "semaprax-owned-byte-buffer-computed-{}.native{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    codegen::build(&program, &native).unwrap();
    let output = Command::new(&native).output().unwrap();
    let _ = std::fs::remove_file(&native);
    assert!(output.status.success(), "native computed-index run failed");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "7",
        "the native backend agrees with the reference interpreter"
    );
}

#[test]
fn an_out_of_range_computed_index_selects_the_same_failure_on_every_backend() {
    let program = parse(COMPUTED_OUT_OF_RANGE, "owned-byte-buffer-past-end.spx").unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "an index the compiler cannot bound is admitted source"
    );
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    // Reference interpreter: the exact normalized status, and no partial write.
    let interpreted = interpret(COMPUTED_OUT_OF_RANGE, "past-end-interp");
    let parsed: serde_json::Value = serde_json::from_str(&interpreted).unwrap();
    let outcome = &parsed["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed", "{interpreted}");
    assert_eq!(outcome["status"]["domain_id"], "semaprax.byte-buffer.v1");
    assert_eq!(outcome["status"]["code"], 1);
    assert_eq!(outcome["status"]["class"], "adapter");

    if !command_available("clang") {
        return;
    }
    // Native C11: the identical domain and code, nothing on stdout, and the
    // buffer released by the exit the plan already owns.
    let native = std::env::temp_dir().join(format!(
        "semaprax-owned-byte-buffer-past-end-{}.native{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    codegen::build(&program, &native).unwrap();
    let output = Command::new(&native).output().unwrap();
    let _ = std::fs::remove_file(&native);
    assert_eq!(
        output.status.code(),
        Some(73),
        "the native run did not select an operation failure"
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "SEMAPRAX operation failure: semaprax.byte-buffer.v1/1",
        "the native backend selects the reference interpreter's exact status"
    );
}
