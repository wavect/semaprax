//! Textual authority gates include every model-wait route under the same seam.

#[test]
fn the_authorization_value_has_exactly_one_mint_site_in_the_crate() {
    let authorization_joined = [
        include_str!("../authorization.rs"),
        include_str!("../authorization/owned_wait_v8.rs"),
        include_str!("../authorization/owned_wait_v8/journal.rs"),
    ]
    .join("\n");
    let authorization = authorization_joined.as_str();
    let lifecycle = include_str!("../../agent_lifecycle.rs");
    let stages = include_str!("../stages.rs");
    let durable = include_str!("../durable.rs");
    let checkpoint = include_str!("../durable/checkpoint.rs");
    let journal = include_str!("../durable/journal.rs");
    let target_joined = [
        include_str!("../authorization/target_protocol/owned_wait_v8.rs"),
        include_str!("../authorization/target_protocol/owned_wait_v8/settlement.rs"),
        include_str!("../authorization/target_protocol/owned_wait_v8/physical.rs"),
    ]
    .join("\n");
    let model_wait_sources = [
        (
            "live_invocation/source_journal/owned_wait_v8/capacity/reduce.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/capacity/reduce.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/fold/reduce.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/fold/reduce.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/live_upstream.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/observe.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/observe.rs"
            ),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/wait.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/wait.rs"
            ),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/model.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/model.rs"
            ),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/authorize.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/authorize.rs"
            ),
        ),
        (
            "interpreter/resumable/owned_frame/registered_stage/live_run/resume.rs",
            include_str!(
                "../../interpreter/resumable/owned_frame/registered_stage/live_run/resume.rs"
            ),
        ),
        (
            "interpreter/resumable/owned_frame/registered_stage/live_run/authorize.rs",
            include_str!(
                "../../interpreter/resumable/owned_frame/registered_stage/live_run/authorize.rs"
            ),
        ),
        (
            "provider_adapter_sdk/source_bridge/dispatch.rs",
            include_str!("../../provider_adapter_sdk/source_bridge/dispatch.rs"),
        ),
        (
            "provider_adapter_sdk/source_bridge/owned_wait_v8.rs",
            include_str!("../../provider_adapter_sdk/source_bridge/owned_wait_v8.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_fold.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_fold.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_inventory.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_inventory.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_model.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_model.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_wire.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_wire.rs"),
        ),
        (
            "resumable_effects/owned_frame/v2/reduce_wire.rs",
            include_str!("../../resumable_effects/owned_frame/v2/reduce_wire.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/capacity/effect.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/capacity/effect.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/effect_fold.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/effect_fold.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/ready_commitment.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/ready_commitment.rs"),
        ),
        (
            "authorization/owned_wait_v8/journal.rs",
            include_str!("../authorization/owned_wait_v8/journal.rs"),
        ),
        (
            "iterative/effects/live/owned_wait_v8.rs",
            include_str!("../iterative/effects/live/owned_wait_v8.rs"),
        ),
        (
            "authorization/target_protocol/owned_wait_v8.rs",
            target_joined.as_str(),
        ),
        (
            "iterative/effects/live/model_wait.rs",
            include_str!("../iterative/effects/live/model_wait.rs"),
        ),
        (
            "iterative/source_live/model_wait.rs",
            include_str!("../iterative/source_live/model_wait.rs"),
        ),
        (
            "iterative/source_live/session/model_wait.rs",
            include_str!("../iterative/source_live/session/model_wait.rs"),
        ),
        (
            "iterative/model_wait.rs",
            include_str!("../iterative/model_wait.rs"),
        ),
        (
            "execution_revision/typed_durable/model_wait.rs",
            include_str!("../../execution_revision/typed_durable/model_wait.rs"),
        ),
    ];

    // The struct literal that builds the value exists exactly once, and its
    // fields are private to the authorization module, so no other module can
    // name them even inside this crate.
    assert_eq!(authorization.matches("\n    Authorized {").count(), 1);
    assert_eq!(lifecycle.matches("Authorized {").count(), 0);
    assert_eq!(stages.matches("Authorized {").count(), 0);
    // The durable path adds no second route: it never names the struct, never
    // names the mint, and reaches an authorization only by running the same
    // validated authorizing transition once.
    for (name, source) in [
        ("durable.rs", durable),
        ("durable/checkpoint.rs", checkpoint),
        ("durable/journal.rs", journal),
    ]
    .into_iter()
    .chain(model_wait_sources)
    {
        assert_eq!(source.matches("Authorized {").count(), 0, "{name}");
        assert_eq!(source.matches("mint(").count(), 0, "{name}");
    }
    assert_eq!(durable.matches("run_authorize_stage(").count(), 1);
    assert!(authorization.contains("pub struct Authorized {\n    binding: String,"));

    // The mint is private and is called exactly once, from the function that
    // runs the validated authorizing transition.
    assert!(authorization.contains("\nfn mint("));
    assert!(!authorization.contains("pub fn mint("));
    assert!(!authorization.contains("pub(super) fn mint("));
    assert!(!authorization.contains("pub(crate) fn mint("));
    assert_eq!(
        authorization.matches("mint(binding, budget, seal)").count(),
        1
    );
    let (_, after) = authorization
        .split_once("pub(super) fn run_authorize_stage(")
        .expect("the mint's only caller is the authorize-stage runner");
    assert!(after.contains("mint(binding, budget, seal)"));

    // The value derives nothing: it is not `Clone`, not `Copy`, and has no
    // `Default`, so one grant admits at most one effect.
    // Audit the authorization carriers, not the copyable backend selector.
    let carriers = authorization
        .split_once("/// The single mint site")
        .unwrap()
        .0;
    assert!(!carriers.contains("#[derive"));
    assert!(!authorization.contains("impl Clone for Authorized"));
    assert!(!authorization.contains("impl Default for Authorized"));

    // Nothing in the lifecycle reaches a host, a process or the environment.
    for forbidden in [
        "std::net::",
        "std::env::",
        "TcpStream",
        "Command::new",
        "fs::write",
        "fs::read",
        "File::create",
    ] {
        for (name, source) in [
            ("authorization.rs", authorization),
            ("agent_lifecycle.rs", lifecycle),
            ("stages.rs", stages),
            ("durable.rs", durable),
            ("durable/checkpoint.rs", checkpoint),
            ("durable/journal.rs", journal),
        ]
        .into_iter()
        .chain(model_wait_sources)
        {
            assert!(!source.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}

#[test]
fn the_stage_executor_seam_has_exactly_three_implementations_and_one_dispatch_route() {
    let authorization_joined = [
        include_str!("../authorization.rs"),
        include_str!("../authorization/owned_wait_v8.rs"),
        include_str!("../authorization/owned_wait_v8/journal.rs"),
    ]
    .join("\n");
    let authorization = authorization_joined.as_str();
    let native_executor = include_str!("../authorization/native_executor.rs");
    let wasm_executor = include_str!("../authorization/wasm_executor.rs");
    let lifecycle = include_str!("../../agent_lifecycle.rs");
    let stages = include_str!("../stages.rs");
    let durable = include_str!("../durable.rs");
    let checkpoint = include_str!("../durable/checkpoint.rs");
    let journal = include_str!("../durable/journal.rs");
    let target_joined = [
        include_str!("../authorization/target_protocol/owned_wait_v8.rs"),
        include_str!("../authorization/target_protocol/owned_wait_v8/settlement.rs"),
        include_str!("../authorization/target_protocol/owned_wait_v8/physical.rs"),
    ]
    .join("\n");
    let model_wait_sources = [
        (
            "live_invocation/source_journal/owned_wait_v8/capacity/reduce.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/capacity/reduce.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/fold/reduce.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/fold/reduce.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/live_upstream.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/observe.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/observe.rs"
            ),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/wait.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/wait.rs"
            ),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/model.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/model.rs"
            ),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/live_upstream/authorize.rs",
            include_str!(
                "../../live_invocation/source_journal/owned_wait_v8/live_upstream/authorize.rs"
            ),
        ),
        (
            "interpreter/resumable/owned_frame/registered_stage/live_run/resume.rs",
            include_str!(
                "../../interpreter/resumable/owned_frame/registered_stage/live_run/resume.rs"
            ),
        ),
        (
            "interpreter/resumable/owned_frame/registered_stage/live_run/authorize.rs",
            include_str!(
                "../../interpreter/resumable/owned_frame/registered_stage/live_run/authorize.rs"
            ),
        ),
        (
            "provider_adapter_sdk/source_bridge/dispatch.rs",
            include_str!("../../provider_adapter_sdk/source_bridge/dispatch.rs"),
        ),
        (
            "provider_adapter_sdk/source_bridge/owned_wait_v8.rs",
            include_str!("../../provider_adapter_sdk/source_bridge/owned_wait_v8.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_fold.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_fold.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_inventory.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_inventory.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_model.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_model.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/reduce_wire.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/reduce_wire.rs"),
        ),
        (
            "resumable_effects/owned_frame/v2/reduce_wire.rs",
            include_str!("../../resumable_effects/owned_frame/v2/reduce_wire.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/capacity/effect.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/capacity/effect.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/effect_fold.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/effect_fold.rs"),
        ),
        (
            "live_invocation/source_journal/owned_wait_v8/ready_commitment.rs",
            include_str!("../../live_invocation/source_journal/owned_wait_v8/ready_commitment.rs"),
        ),
        (
            "authorization/owned_wait_v8/journal.rs",
            include_str!("../authorization/owned_wait_v8/journal.rs"),
        ),
        (
            "iterative/effects/live/owned_wait_v8.rs",
            include_str!("../iterative/effects/live/owned_wait_v8.rs"),
        ),
        (
            "authorization/target_protocol/owned_wait_v8.rs",
            target_joined.as_str(),
        ),
        (
            "iterative/effects/live/model_wait.rs",
            include_str!("../iterative/effects/live/model_wait.rs"),
        ),
        (
            "iterative/source_live/model_wait.rs",
            include_str!("../iterative/source_live/model_wait.rs"),
        ),
        (
            "iterative/source_live/session/model_wait.rs",
            include_str!("../iterative/source_live/session/model_wait.rs"),
        ),
        (
            "iterative/model_wait.rs",
            include_str!("../iterative/model_wait.rs"),
        ),
        (
            "execution_revision/typed_durable/model_wait.rs",
            include_str!("../../execution_revision/typed_durable/model_wait.rs"),
        ),
    ];
    let rich_stage = include_str!("../rich_stage.rs");
    let driver = include_str!("../iterative/driver.rs");
    let live = include_str!("../iterative/driver/live.rs");
    let target_live_joined = [
        include_str!("../iterative/effects/live.rs"),
        include_str!("../iterative/effects/live/owned_wait_v8.rs"),
    ]
    .join("\n");
    let target_live = target_live_joined.as_str();
    let target_metered = include_str!("../iterative/effects/metered.rs");

    // `StageExecutor` is implemented exactly three times in the whole tree:
    // the interpreter-backed executor here in `authorization.rs`, the
    // native C11 executor (#142) in `authorization/native_executor.rs`, and
    // the Core Wasm executor (#143) in `authorization/wasm_executor.rs`.
    // Extending this count from one to three is the deliberate, reviewed
    // outcome of admitting those two backends -- not a regression of the
    // seal. `sealed::Sealed`, defined once in a private `mod sealed` nested
    // in `authorization.rs`, is visible only to `authorization.rs` and the
    // two submodules it declares, so nothing outside this module's own tree
    // could add a fourth implementation even if it tried; the compiler, not
    // this scan, is what actually enforces that (see the `compile_fail`
    // doctest on `StageExecutor` itself).
    assert_eq!(authorization.matches("impl StageExecutor for").count(), 1);
    assert_eq!(native_executor.matches("impl StageExecutor for").count(), 1);
    assert_eq!(wasm_executor.matches("impl StageExecutor for").count(), 1);
    assert_eq!(authorization.matches("mod sealed").count(), 1);
    for (name, source) in [
        ("agent_lifecycle.rs", lifecycle),
        ("stages.rs", stages),
        ("durable.rs", durable),
        ("durable/checkpoint.rs", checkpoint),
        ("durable/journal.rs", journal),
        ("rich_stage.rs", rich_stage),
        ("iterative/driver.rs", driver),
        ("iterative/driver/live.rs", live),
        ("iterative/effects/live.rs", target_live),
        ("iterative/effects/metered.rs", target_metered),
    ]
    .into_iter()
    .chain(model_wait_sources)
    {
        assert_eq!(
            source.matches("impl StageExecutor for").count(),
            0,
            "{name}"
        );
        assert_eq!(source.matches("mod sealed").count(), 0, "{name}");
    }
    for (name, source) in [
        ("authorization/native_executor.rs", native_executor),
        ("authorization/wasm_executor.rs", wasm_executor),
    ] {
        assert_eq!(source.matches("mod sealed").count(), 0, "{name}");
    }

    // Every stage dispatch this crate's `src/agent_lifecycle/**` file lease
    // can reach now calls the sealed `dispatch`/`dispatch_on` instead of
    // `evaluate_retained_call` directly. The Rich Proposal binder used to
    // call it twice (authorize and reduce); it now calls it zero times. The
    // native and Wasm executors never call it at all -- they compile and
    // run the stage body through their own backend instead.
    assert_eq!(authorization.matches("evaluate_retained_call(").count(), 1);
    for (name, source) in [
        ("rich_stage.rs", rich_stage),
        ("durable.rs", durable),
        ("iterative/driver.rs", driver),
        ("iterative/driver/live.rs", live),
        ("iterative/effects/live.rs", target_live),
        ("iterative/effects/metered.rs", target_metered),
        ("stages.rs", stages),
        ("authorization/native_executor.rs", native_executor),
        ("authorization/wasm_executor.rs", wasm_executor),
    ]
    .into_iter()
    .chain(model_wait_sources)
    {
        assert_eq!(
            source.matches("evaluate_retained_call(").count(),
            0,
            "{name}"
        );
    }

    // The module-root `CompiledAgentLifecycle::evaluate` was the last
    // bypass: it called `evaluate_retained_call` directly, outside the
    // seam. It now routes through `authorization::dispatch` like every
    // other stage execution, so the count here is zero. If it ever becomes
    // non-zero again a second, unsealed execution route has reappeared.
    assert_eq!(lifecycle.matches("evaluate_retained_call(").count(), 0);
}
