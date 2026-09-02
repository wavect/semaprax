use super::*;

fn succeed(id: &str, scope: &str) -> TaskSpec {
    TaskSpec::new(
        id,
        scope,
        SendableMark::Sendable,
        ShareableMark::NotShareable,
        TaskOutcome::Succeed,
    )
}

fn drain(run: &mut ScopedTaskRun<'_>) -> Vec<TaskEvent> {
    let mut events = Vec::new();
    while let Some(event) = run.step().expect("model steps are valid") {
        events.push(event);
    }
    events
}

#[test]
fn constructor_rejects_every_structural_ambiguity() {
    let scopes = || vec![ScopeSpec::root("r")];
    assert_eq!(
        ScopedTaskModel::try_new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        Err(ScopedTasksError::MissingRoot)
    );
    for name in ["", "r\0x"] {
        assert_eq!(
            ScopedTaskModel::try_new(
                vec![ScopeSpec::root(name)],
                Vec::new(),
                Vec::new(),
                Vec::new()
            ),
            Err(ScopedTasksError::InvalidIdentity)
        );
        assert_eq!(
            ScopedTaskModel::try_new(
                scopes(),
                vec![TaskSpec::new(
                    name,
                    "r",
                    SendableMark::NotSendable,
                    ShareableMark::NotShareable,
                    TaskOutcome::Succeed
                )],
                Vec::new(),
                Vec::new()
            ),
            Err(ScopedTasksError::InvalidIdentity)
        );
    }
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![ScopeSpec::root("r"), ScopeSpec::root("r")],
            Vec::new(),
            Vec::new(),
            vec![ScopeJoin::new("r", "r")]
        ),
        Err(ScopedTasksError::DuplicateScope)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            scopes(),
            vec![succeed("t", "r"), succeed("t", "r")],
            Vec::new(),
            Vec::new()
        ),
        Err(ScopedTasksError::DuplicateTask)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![ScopeSpec::root("r"), ScopeSpec::root("q")],
            Vec::new(),
            Vec::new(),
            Vec::new()
        ),
        Err(ScopedTasksError::MultipleRoots)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![ScopeSpec::root("r"), ScopeSpec::child("a", "ghost")],
            Vec::new(),
            Vec::new(),
            vec![ScopeJoin::new("r", "a")]
        ),
        Err(ScopedTasksError::UnknownScope)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![ScopeSpec::child("r", "r")],
            Vec::new(),
            Vec::new(),
            Vec::new()
        ),
        Err(ScopedTasksError::MissingRoot)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![
                ScopeSpec::root("r"),
                ScopeSpec::child("a", "b"),
                ScopeSpec::child("b", "a")
            ],
            Vec::new(),
            Vec::new(),
            vec![ScopeJoin::new("r", "a"), ScopeJoin::new("a", "b")]
        ),
        Err(ScopedTasksError::ScopeCycle)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            scopes(),
            vec![succeed("t", "ghost")],
            Vec::new(),
            Vec::new()
        ),
        Err(ScopedTasksError::UnknownScope)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            scopes(),
            Vec::new(),
            vec![DependencyEdge::new("x", "y")],
            Vec::new()
        ),
        Err(ScopedTasksError::UnknownTask)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            scopes(),
            vec![succeed("t", "r")],
            vec![DependencyEdge::new("t", "t")],
            Vec::new()
        ),
        Err(ScopedTasksError::SelfDependency)
    );
    let duplicate_edge = |edges| {
        ScopedTaskModel::try_new(
            scopes(),
            vec![succeed("a", "r"), succeed("b", "r")],
            edges,
            Vec::new(),
        )
    };
    assert_eq!(
        duplicate_edge(vec![
            DependencyEdge::new("a", "b"),
            DependencyEdge::new("a", "b")
        ]),
        Err(ScopedTasksError::DuplicateDependency)
    );
    assert_eq!(
        duplicate_edge(vec![
            DependencyEdge::new("a", "b"),
            DependencyEdge::new("b", "a")
        ]),
        Err(ScopedTasksError::DependencyCycle)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![ScopeSpec::root("r"), ScopeSpec::child("s", "r")],
            Vec::new(),
            Vec::new(),
            vec![ScopeJoin::new("s", "s")]
        ),
        Err(ScopedTasksError::OrphanJoin)
    );
}

#[test]
fn joins_must_cover_each_direct_child_exactly_once() {
    let scopes = vec![
        ScopeSpec::root("r"),
        ScopeSpec::child("s1", "r"),
        ScopeSpec::child("s2", "s1"),
    ];
    assert_eq!(
        ScopedTaskModel::try_new(scopes.clone(), Vec::new(), Vec::new(), Vec::new()),
        Err(ScopedTasksError::UnjoinedChildScope)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            scopes.clone(),
            Vec::new(),
            Vec::new(),
            vec![ScopeJoin::new("r", "s1"), ScopeJoin::new("s2", "s1")]
        ),
        Err(ScopedTasksError::OrphanJoin)
    );
    assert_eq!(
        ScopedTaskModel::try_new(
            scopes.clone(),
            Vec::new(),
            Vec::new(),
            vec![ScopeJoin::new("r", "s1"), ScopeJoin::new("r", "s1")]
        ),
        Err(ScopedTasksError::DoubleJoin)
    );
    assert!(ScopedTaskModel::try_new(
        scopes,
        Vec::new(),
        Vec::new(),
        vec![ScopeJoin::new("r", "s1"), ScopeJoin::new("s1", "s2")]
    )
    .is_ok());
}

#[test]
fn dependencies_must_stay_inside_the_scope_lineage() {
    let scopes = vec![
        ScopeSpec::root("r"),
        ScopeSpec::child("left", "r"),
        ScopeSpec::child("right", "r"),
    ];
    let joins = vec![ScopeJoin::new("r", "left"), ScopeJoin::new("r", "right")];
    let sibling_escape = ScopedTaskModel::try_new(
        scopes.clone(),
        vec![succeed("p", "left"), succeed("q", "right")],
        vec![DependencyEdge::new("p", "q")],
        joins.clone(),
    );
    assert_eq!(sibling_escape, Err(ScopedTasksError::EscapingDependency));
    let root_prerequisite = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r"), ScopeSpec::child("s", "r")],
        vec![succeed("base", "r"), succeed("leaf", "s")],
        vec![DependencyEdge::new("base", "leaf")],
        vec![ScopeJoin::new("r", "s")],
    );
    assert!(root_prerequisite.is_ok());
    let same_scope = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![succeed("a", "r"), succeed("b", "r")],
        vec![DependencyEdge::new("a", "b")],
        Vec::new(),
    );
    assert!(same_scope.is_ok());
}

#[test]
fn physical_failure_code_zero_is_never_admitted() {
    assert_eq!(
        ScopedTaskModel::try_new(
            vec![ScopeSpec::root("r")],
            vec![TaskSpec::new(
                "t",
                "r",
                SendableMark::NotSendable,
                ShareableMark::NotShareable,
                TaskOutcome::Fail(FailureKind::Physical(0))
            )],
            Vec::new(),
            Vec::new()
        ),
        Err(ScopedTasksError::InvalidFailureCode)
    );
}

#[test]
fn bounds_and_work_budget_fail_closed() {
    let many_scopes = (0..=MAX_SCOPES)
        .map(|index| {
            if index == 0 {
                ScopeSpec::root("r")
            } else {
                ScopeSpec::child(format!("s{index}"), "r")
            }
        })
        .collect::<Vec<_>>();
    let joins = (1..=MAX_SCOPES)
        .map(|index| ScopeJoin::new("r", format!("s{index}")))
        .collect::<Vec<_>>();
    assert_eq!(
        ScopedTaskModel::try_new(many_scopes, Vec::new(), Vec::new(), joins),
        Err(ScopedTasksError::ScopeBoundExceeded)
    );
    let scopes = vec![ScopeSpec::root("r")];
    let many_tasks = (0..=MAX_TASKS)
        .map(|index| succeed(&format!("t{index:05}"), "r"))
        .collect::<Vec<_>>();
    assert_eq!(
        ScopedTaskModel::try_new(scopes.clone(), many_tasks, Vec::new(), Vec::new()),
        Err(ScopedTasksError::TaskBoundExceeded)
    );
    let budget_tasks = 1_000_usize;
    let tasks = (0..budget_tasks)
        .map(|index| succeed(&format!("t{index:04}"), "r"))
        .collect::<Vec<_>>();
    let over_budget = (0..=MAX_DEPENDENCIES.min(1_001))
        .map(|index| {
            DependencyEdge::new(
                format!("t{:04}", index % budget_tasks),
                format!("t{:04}", (index + 1) % budget_tasks),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ScopedTaskModel::try_new(scopes, tasks, over_budget, Vec::new()),
        Err(ScopedTasksError::WorkBudgetExceeded)
    );
}

#[test]
fn canonical_json_is_valid_and_order_insensitive() {
    let build = |reverse: bool| {
        let mut scopes = vec![ScopeSpec::root("r"), ScopeSpec::child("inner", "r")];
        let mut tasks = vec![
            TaskSpec::new(
                "t\"quoted\\task",
                "inner",
                SendableMark::Sendable,
                ShareableMark::Shareable,
                TaskOutcome::Succeed,
            ),
            succeed("plain", "r"),
        ];
        let mut joins = vec![ScopeJoin::new("r", "inner")];
        if reverse {
            scopes.reverse();
            tasks.reverse();
            joins.reverse();
        }
        ScopedTaskModel::try_new(scopes, tasks, Vec::new(), joins).unwrap()
    };
    let forward = build(false);
    let backward = build(true);
    assert_eq!(forward.canonical_json(), backward.canonical_json());
    assert_eq!(forward.fingerprint(), backward.fingerprint());
    let parsed: serde_json::Value =
        serde_json::from_str(&forward.canonical_json()).expect("canonical JSON parses");
    assert_eq!(parsed["schema"], SCOPED_TASKS_MODEL_V1);
    assert_eq!(parsed["tasks"][1]["sendable"], "sendable");
    assert_eq!(parsed["tasks"][1]["shareable"], "shareable");
}

#[test]
fn run_hostile_operations_reject_or_drain_idempotently() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![succeed("a", "r")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    assert_eq!(
        run.cancel_scope("ghost"),
        Err(ScopedTasksError::UnknownScope)
    );
    assert_eq!(run.finish(), Err(ScopedTasksError::RunNotComplete));
    drain(&mut run);
    assert!(run.is_complete());
    assert_eq!(
        run.cancel_scope("r"),
        Err(ScopedTasksError::RunAlreadyComplete)
    );
    assert_eq!(run.step().unwrap(), None);
    let summary = run.finish().unwrap();
    assert!(matches!(summary.root_outcome(), ScopeExitOutcome::Success));
    assert_eq!(summary.totals().started, 1);
    assert_eq!(summary.totals().completed, 1);
    assert_eq!(run.task_phase("a"), Some(TaskPhase::Completed));
    assert_eq!(run.task_phase("missing"), None);
}
