//! Target-neutral deterministic model of structured scoped concurrency.
//!
//! This module deliberately contains no threads, no async runtime, no scheduler
//! integration, no language syntax, and no compiler/backend wiring. It fixes
//! the bounded proof data that a future structured-concurrency implementation
//! must preserve: a bounded task DAG inside a strict scope tree, deterministic
//! sequential scheduling in canonical stable-id order, sticky cancellation
//! propagation, children-before-parents cleanup on scope exit, first-failure
//! stickiness with sibling draining, and closed per-task `Sendable`/
//! `Shareable` annotations.
//!
//! Like the callable-v3 settlement model, everything here is evidence, not
//! authority: a `ScopedTaskRun` records what a conforming implementation MUST
//! do; it never executes user work, spawns nothing, and grants no concurrency
//! capability. It performs no `Sendable` checking of real programs.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;
use crate::hir::DeclarationId;

pub const SCOPED_TASKS_MODEL_V1: &str = "semaprax.scoped-tasks-model.v1";
pub const SCOPED_TASKS_TRACE_V1: &str = "semaprax.scoped-tasks-trace.v1";

pub const MAX_SCOPES: usize = 4_096;
pub const MAX_TASKS: usize = 4_096;
pub const MAX_DEPENDENCIES: usize = 65_536;
const MAX_WORK_UNITS: u64 = 1_000_000;
const MODEL_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.scoped-tasks-model-fingerprint.v1\0";
const TRACE_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.scoped-tasks-trace-fingerprint.v1\0";

/// Closed per-task `Sendable` classification. This is a declared annotation
/// only; the model performs no cross-thread transfer analysis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SendableMark {
    Sendable,
    NotSendable,
}

/// Closed per-task `Shareable` classification. This is a declared annotation
/// only; the model performs no aliasing analysis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ShareableMark {
    Shareable,
    NotShareable,
}

/// Closed scripted outcome of one modeled task body.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TaskOutcome {
    Succeed,
    Fail(FailureKind),
}

/// Closed failure vocabulary. A physical failure code must be nonzero; zero is
/// reserved against the settlement-model convention that zero means success.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FailureKind {
    Semantic,
    Physical(u32),
}

/// One scope in the strict containment tree. The root scope has no parent;
/// every other scope names exactly one existing parent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeSpec {
    id: DeclarationId,
    parent: Option<DeclarationId>,
}

impl ScopeSpec {
    #[must_use]
    pub fn root(id: impl Into<String>) -> Self {
        Self {
            id: DeclarationId::new(id),
            parent: None,
        }
    }

    #[must_use]
    pub fn child(id: impl Into<String>, parent: impl Into<String>) -> Self {
        Self {
            id: DeclarationId::new(id),
            parent: Some(DeclarationId::new(parent)),
        }
    }

    #[must_use]
    pub fn id(&self) -> &DeclarationId {
        &self.id
    }

    #[must_use]
    pub fn parent(&self) -> Option<&DeclarationId> {
        self.parent.as_ref()
    }
}

/// One modeled task: identity, owning scope, closed lifetime annotations, and
/// the scripted outcome applied when the deterministic scheduler reaches it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSpec {
    id: DeclarationId,
    scope: DeclarationId,
    sendable: SendableMark,
    shareable: ShareableMark,
    outcome: TaskOutcome,
}

impl TaskSpec {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        scope: impl Into<String>,
        sendable: SendableMark,
        shareable: ShareableMark,
        outcome: TaskOutcome,
    ) -> Self {
        Self {
            id: DeclarationId::new(id),
            scope: DeclarationId::new(scope),
            sendable,
            shareable,
            outcome,
        }
    }

    #[must_use]
    pub fn id(&self) -> &DeclarationId {
        &self.id
    }

    #[must_use]
    pub fn scope(&self) -> &DeclarationId {
        &self.scope
    }

    #[must_use]
    pub const fn sendable(&self) -> SendableMark {
        self.sendable
    }

    #[must_use]
    pub const fn shareable(&self) -> ShareableMark {
        self.shareable
    }

    #[must_use]
    pub const fn outcome(&self) -> &TaskOutcome {
        &self.outcome
    }
}

/// Declared structured join: `waiter` (the parent scope body) waits for the
/// complete drain of its direct child `target` before the waiter may exit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeJoin {
    waiter: DeclarationId,
    target: DeclarationId,
}

impl ScopeJoin {
    #[must_use]
    pub fn new(waiter: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            waiter: DeclarationId::new(waiter),
            target: DeclarationId::new(target),
        }
    }

    #[must_use]
    pub fn waiter(&self) -> &DeclarationId {
        &self.waiter
    }

    #[must_use]
    pub fn target(&self) -> &DeclarationId {
        &self.target
    }
}

/// Declared dependency: `dependent` may start only after `prerequisite`
/// completed. The prerequisite scope must be the dependent's own scope or an
/// ancestor of it; anything else is an escaping reference and is rejected at
/// model construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyEdge {
    prerequisite: DeclarationId,
    dependent: DeclarationId,
}

impl DependencyEdge {
    #[must_use]
    pub fn new(prerequisite: impl Into<String>, dependent: impl Into<String>) -> Self {
        Self {
            prerequisite: DeclarationId::new(prerequisite),
            dependent: DeclarationId::new(dependent),
        }
    }

    #[must_use]
    pub fn prerequisite(&self) -> &DeclarationId {
        &self.prerequisite
    }

    #[must_use]
    pub fn dependent(&self) -> &DeclarationId {
        &self.dependent
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopeEntry {
    parent: Option<DeclarationId>,
    depth: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TaskEntry {
    scope: DeclarationId,
    sendable: SendableMark,
    shareable: ShareableMark,
    outcome: TaskOutcome,
}

/// Immutable certified structure: the bounded scope tree, task inventory,
/// dependency DAG, and required parent joins. Construction rejects every
/// structural ambiguity and computes a domain-separated fingerprint. All
/// inventories are canonically ordered, so input order cannot change any
/// projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedTaskModel {
    schema: &'static str,
    root: DeclarationId,
    scopes: BTreeMap<DeclarationId, ScopeEntry>,
    tasks: BTreeMap<DeclarationId, TaskEntry>,
    dependencies: BTreeSet<(DeclarationId, DeclarationId)>,
    joins: BTreeMap<DeclarationId, DeclarationId>,
    children: BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    fingerprint: [u8; 32],
}

impl ScopedTaskModel {
    /// Build and fully validate one bounded model.
    pub fn try_new(
        scopes: Vec<ScopeSpec>,
        tasks: Vec<TaskSpec>,
        dependencies: Vec<DependencyEdge>,
        joins: Vec<ScopeJoin>,
    ) -> Result<Self, ScopedTasksError> {
        if scopes.is_empty() {
            return Err(ScopedTasksError::MissingRoot);
        }
        if scopes.len() > MAX_SCOPES {
            return Err(ScopedTasksError::ScopeBoundExceeded);
        }
        if tasks.len() > MAX_TASKS {
            return Err(ScopedTasksError::TaskBoundExceeded);
        }
        if dependencies.len() > MAX_DEPENDENCIES {
            return Err(ScopedTasksError::DependencyBoundExceeded);
        }
        let work = (tasks.len() as u64)
            .checked_mul(dependencies.len() as u64 + 1)
            .ok_or(ScopedTasksError::WorkBudgetExceeded)?
            .checked_add(scopes.len() as u64)
            .ok_or(ScopedTasksError::WorkBudgetExceeded)?;
        if work > MAX_WORK_UNITS {
            return Err(ScopedTasksError::WorkBudgetExceeded);
        }

        let mut parents = BTreeMap::<DeclarationId, Option<DeclarationId>>::new();
        for scope in &scopes {
            validate_identity(scope.id.as_str())?;
            if parents
                .insert(scope.id.clone(), scope.parent.clone())
                .is_some()
            {
                return Err(ScopedTasksError::DuplicateScope);
            }
        }
        match scopes.iter().filter(|scope| scope.parent.is_none()).count() {
            0 => return Err(ScopedTasksError::MissingRoot),
            1 => {}
            _ => return Err(ScopedTasksError::MultipleRoots),
        }
        for scope in &scopes {
            if let Some(parent) = &scope.parent {
                if scope.id == *parent {
                    return Err(ScopedTasksError::ScopeCycle);
                }
                if !parents.contains_key(parent) {
                    return Err(ScopedTasksError::UnknownScope);
                }
            }
        }
        let depths = canonical_depths(&parents)?;
        let mut scope_entries = BTreeMap::new();
        for (id, parent) in &parents {
            scope_entries.insert(
                id.clone(),
                ScopeEntry {
                    parent: parent.clone(),
                    depth: depths[id],
                },
            );
        }
        let root = scopes
            .iter()
            .find(|scope| scope.parent.is_none())
            .map(|scope| scope.id.clone())
            .expect("exactly one root was validated");
        let mut children = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
        for (id, entry) in &scope_entries {
            if let Some(parent) = &entry.parent {
                children
                    .entry(parent.clone())
                    .or_default()
                    .insert(id.clone());
            }
        }

        let mut task_entries = BTreeMap::<DeclarationId, TaskEntry>::new();
        for task in &tasks {
            validate_identity(task.id.as_str())?;
            if !scope_entries.contains_key(&task.scope) {
                return Err(ScopedTasksError::UnknownScope);
            }
            if let TaskOutcome::Fail(FailureKind::Physical(code)) = task.outcome {
                if code == 0 {
                    return Err(ScopedTasksError::InvalidFailureCode);
                }
            }
            if task_entries.contains_key(&task.id) {
                return Err(ScopedTasksError::DuplicateTask);
            }
            task_entries.insert(
                task.id.clone(),
                TaskEntry {
                    scope: task.scope.clone(),
                    sendable: task.sendable,
                    shareable: task.shareable,
                    outcome: task.outcome.clone(),
                },
            );
        }

        let mut dependency_set = BTreeSet::new();
        for edge in &dependencies {
            let Some(prerequisite_entry) = task_entries.get(&edge.prerequisite) else {
                return Err(ScopedTasksError::UnknownTask);
            };
            let Some(dependent_entry) = task_entries.get(&edge.dependent) else {
                return Err(ScopedTasksError::UnknownTask);
            };
            if edge.prerequisite == edge.dependent {
                return Err(ScopedTasksError::SelfDependency);
            }
            if !is_self_or_ancestor(
                &scope_entries,
                &dependent_entry.scope,
                &prerequisite_entry.scope,
            ) {
                return Err(ScopedTasksError::EscapingDependency);
            }
            if !dependency_set.insert((edge.prerequisite.clone(), edge.dependent.clone())) {
                return Err(ScopedTasksError::DuplicateDependency);
            }
        }
        validate_acyclic(&task_entries, &dependency_set)?;

        if joins.len() != scopes.len() - 1 {
            return Err(ScopedTasksError::UnjoinedChildScope);
        }
        let mut join_map = BTreeMap::<DeclarationId, DeclarationId>::new();
        for join in &joins {
            if !scope_entries.contains_key(&join.waiter)
                || !scope_entries.contains_key(&join.target)
            {
                return Err(ScopedTasksError::UnknownScope);
            }
            if join.target == root {
                return Err(ScopedTasksError::OrphanJoin);
            }
            let actual_parent = scope_entries[&join.target]
                .parent
                .clone()
                .expect("non-root targets were validated");
            if join.waiter != actual_parent {
                return Err(ScopedTasksError::OrphanJoin);
            }
            if join_map
                .insert(join.target.clone(), join.waiter.clone())
                .is_some()
            {
                return Err(ScopedTasksError::DoubleJoin);
            }
        }
        for (id, entry) in &scope_entries {
            if entry.parent.is_some() && !join_map.contains_key(id) {
                return Err(ScopedTasksError::UnjoinedChildScope);
            }
        }

        let mut model = Self {
            schema: SCOPED_TASKS_MODEL_V1,
            root,
            scopes: scope_entries,
            tasks: task_entries,
            dependencies: dependency_set,
            joins: join_map,
            children,
            fingerprint: [0; 32],
        };
        model.fingerprint =
            fingerprint(MODEL_FINGERPRINT_DOMAIN, model.canonical_json().as_bytes());
        Ok(model)
    }

    #[must_use]
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    #[must_use]
    pub fn root(&self) -> &DeclarationId {
        &self.root
    }

    pub fn scopes(&self) -> impl Iterator<Item = (&DeclarationId, Option<&DeclarationId>)> {
        self.scopes
            .iter()
            .map(|(id, entry)| (id, entry.parent.as_ref()))
    }

    pub fn tasks(
        &self,
    ) -> impl Iterator<
        Item = (
            &DeclarationId,
            &DeclarationId,
            SendableMark,
            ShareableMark,
            &TaskOutcome,
        ),
    > {
        self.tasks.iter().map(|(id, entry)| {
            (
                id,
                &entry.scope,
                entry.sendable,
                entry.shareable,
                &entry.outcome,
            )
        })
    }

    pub fn dependencies(&self) -> impl Iterator<Item = (&DeclarationId, &DeclarationId)> {
        self.dependencies.iter().map(|edge| (&edge.0, &edge.1))
    }

    pub fn joins(&self) -> impl Iterator<Item = (&DeclarationId, &DeclarationId)> {
        self.joins.iter()
    }

    #[must_use]
    pub const fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    #[must_use]
    pub fn canonical_json(&self) -> String {
        let scopes = self
            .scopes
            .iter()
            .map(|(id, entry)| {
                let parent = entry
                    .parent
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |parent| quote_json(parent.as_str()));
                format!(
                    "{{\"id\":{},\"parent\":{}}}",
                    quote_json(id.as_str()),
                    parent
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let tasks = self
            .tasks
            .iter()
            .map(|(id, entry)| {
                format!(
                    "{{\"id\":{},\"scope\":{},\"sendable\":{},\"shareable\":{},\"outcome\":{}}}",
                    quote_json(id.as_str()),
                    quote_json(entry.scope.as_str()),
                    quote_json(sendable_name(entry.sendable)),
                    quote_json(shareable_name(entry.shareable)),
                    outcome_json(&entry.outcome),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let dependencies = self
            .dependencies
            .iter()
            .map(|(prerequisite, dependent)| {
                format!(
                    "[{},{}]",
                    quote_json(prerequisite.as_str()),
                    quote_json(dependent.as_str())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let joins = self
            .joins
            .iter()
            .map(|(target, waiter)| {
                format!(
                    "{{\"waiter\":{},\"target\":{}}}",
                    quote_json(waiter.as_str()),
                    quote_json(target.as_str())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"root\":{},\"scopes\":[{}],\"tasks\":[{}],\"dependencies\":[{}],\"joins\":[{}]}}",
            quote_json(self.schema),
            quote_json(self.root.as_str()),
            scopes,
            tasks,
            dependencies,
            joins,
        )
    }

    /// Prepare the linear deterministic run bound to this exact model.
    #[must_use]
    pub fn prepare_run(&self) -> ScopedTaskRun<'_> {
        let states = self
            .tasks
            .keys()
            .cloned()
            .map(|id| (id, TaskRuntime::pending()))
            .collect::<BTreeMap<_, _>>();
        let mut pending_dependencies = BTreeMap::<DeclarationId, usize>::new();
        let mut dependents = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
        for id in self.tasks.keys() {
            pending_dependencies.insert(id.clone(), 0);
        }
        for (prerequisite, dependent) in &self.dependencies {
            *pending_dependencies
                .get_mut(dependent)
                .expect("endpoints validated") += 1;
            dependents
                .entry(prerequisite.clone())
                .or_default()
                .insert(dependent.clone());
        }
        ScopedTaskRun {
            model_fingerprint: self.fingerprint,
            model: self,
            states,
            pending_dependencies,
            dependents,
            cancelled_scopes: BTreeSet::new(),
            announced_cancellations: BTreeSet::new(),
            exited_scopes: BTreeSet::new(),
            finalized_tasks: BTreeSet::new(),
            scope_failures: BTreeMap::new(),
            scope_outcomes: BTreeMap::new(),
            completion_seq: 0,
            events: Vec::new(),
            running: None,
            totals: RunTotals::default(),
            complete: false,
        }
    }
}

fn validate_identity(name: &str) -> Result<(), ScopedTasksError> {
    if name.is_empty() || name.as_bytes().contains(&0) {
        return Err(ScopedTasksError::InvalidIdentity);
    }
    Ok(())
}

fn is_self_or_ancestor(
    scopes: &BTreeMap<DeclarationId, ScopeEntry>,
    start: &DeclarationId,
    candidate: &DeclarationId,
) -> bool {
    let mut current = Some(start);
    while let Some(scope) = current {
        if scope == candidate {
            return true;
        }
        current = scopes[scope].parent.as_ref();
    }
    false
}

fn canonical_depths(
    parents: &BTreeMap<DeclarationId, Option<DeclarationId>>,
) -> Result<BTreeMap<DeclarationId, u32>, ScopedTasksError> {
    let mut depths = BTreeMap::<DeclarationId, u32>::new();
    for id in parents.keys() {
        let mut stack = Vec::new();
        let mut cursor = id.clone();
        let base = loop {
            if let Some(known) = depths.get(&cursor) {
                break *known;
            }
            match parents[&cursor].clone() {
                None => break 0,
                Some(parent) => {
                    stack.push(cursor);
                    if stack.len() > parents.len() {
                        return Err(ScopedTasksError::ScopeCycle);
                    }
                    cursor = parent;
                }
            }
        };
        let mut running = base;
        for name in stack.iter().rev() {
            running = running.checked_add(1).ok_or(ScopedTasksError::ScopeCycle)?;
            depths.insert(name.clone(), running);
        }
        depths.entry(cursor).or_insert(base);
    }
    Ok(depths)
}

fn validate_acyclic(
    tasks: &BTreeMap<DeclarationId, TaskEntry>,
    dependencies: &BTreeSet<(DeclarationId, DeclarationId)>,
) -> Result<(), ScopedTasksError> {
    let mut indegree = tasks
        .keys()
        .cloned()
        .map(|id| (id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    for (prerequisite, dependent) in dependencies {
        *indegree.get_mut(dependent).expect("endpoints validated") += 1;
        dependents
            .entry(prerequisite.clone())
            .or_default()
            .insert(dependent.clone());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut processed = 0_usize;
    while let Some(id) = ready.pop_first() {
        processed += 1;
        for dependent in dependents.get(&id).into_iter().flatten() {
            let degree = indegree.get_mut(dependent).expect("endpoints validated");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    if processed != tasks.len() {
        return Err(ScopedTasksError::DependencyCycle);
    }
    Ok(())
}

/// Closed observable scheduler event. Events are evidence of what a conforming
/// implementation must do; they perform no work themselves.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TaskEvent {
    Started {
        task: DeclarationId,
    },
    Completed {
        task: DeclarationId,
    },
    Failed {
        task: DeclarationId,
        failure: FailureKind,
    },
    Cancelled {
        task: DeclarationId,
    },
    ScopeCancelled {
        scope: DeclarationId,
    },
    Finalized {
        task: DeclarationId,
    },
    ScopeExited {
        scope: DeclarationId,
        outcome: ScopeExitOutcome,
    },
}

impl TaskEvent {
    #[must_use]
    pub fn canonical_json(&self) -> String {
        match self {
            Self::Started { task } => {
                format!(
                    "{{\"kind\":\"started\",\"task\":{}}}",
                    quote_json(task.as_str())
                )
            }
            Self::Completed { task } => format!(
                "{{\"kind\":\"completed\",\"task\":{}}}",
                quote_json(task.as_str())
            ),
            Self::Failed { task, failure } => format!(
                "{{\"kind\":\"failed\",\"task\":{},\"failure\":{}}}",
                quote_json(task.as_str()),
                failure_json(*failure)
            ),
            Self::Cancelled { task } => format!(
                "{{\"kind\":\"cancelled\",\"task\":{}}}",
                quote_json(task.as_str())
            ),
            Self::ScopeCancelled { scope } => format!(
                "{{\"kind\":\"scope_cancelled\",\"scope\":{}}}",
                quote_json(scope.as_str())
            ),
            Self::Finalized { task } => format!(
                "{{\"kind\":\"finalized\",\"task\":{}}}",
                quote_json(task.as_str())
            ),
            Self::ScopeExited { scope, outcome } => format!(
                "{{\"kind\":\"scope_exited\",\"scope\":{},\"outcome\":{}}}",
                quote_json(scope.as_str()),
                scope_exit_outcome_json(outcome)
            ),
        }
    }
}

/// Terminal outcome recorded when one scope exits. Sticky first failure beats
/// cancellation: a drained failure stays observable even if the scope was also
/// cancelled before its exit.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScopeExitOutcome {
    Success,
    Failed {
        task: DeclarationId,
        failure: FailureKind,
    },
    Cancelled,
}

/// Closed observable lifecycle phase of one task inside a run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskPhase {
    Pending,
    Started,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunTotals {
    pub started: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
}

/// Immutable terminal evidence of one complete run: root outcome and totals.
/// Cloning it has no scheduling effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeRunSummary {
    root_outcome: ScopeExitOutcome,
    totals: RunTotals,
}

impl ScopeRunSummary {
    #[must_use]
    pub const fn root_outcome(&self) -> &ScopeExitOutcome {
        &self.root_outcome
    }

    #[must_use]
    pub const fn totals(&self) -> RunTotals {
        self.totals
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskState {
    Pending,
    Started,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Eq, PartialEq)]
struct TaskRuntime {
    state: TaskState,
    completion_seq: Option<u64>,
}

impl TaskRuntime {
    const fn pending() -> Self {
        Self {
            state: TaskState::Pending,
            completion_seq: None,
        }
    }
}

/// Linear deterministic execution of one prepared model. The run is not
/// cloneable: it models exactly one sequential schedule. `cancel_scope` may be
/// injected between steps; every step is deterministic in canonical stable-id
/// order.
#[derive(Eq, PartialEq)]
pub struct ScopedTaskRun<'a> {
    model_fingerprint: [u8; 32],
    model: &'a ScopedTaskModel,
    states: BTreeMap<DeclarationId, TaskRuntime>,
    pending_dependencies: BTreeMap<DeclarationId, usize>,
    dependents: BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    cancelled_scopes: BTreeSet<DeclarationId>,
    announced_cancellations: BTreeSet<DeclarationId>,
    exited_scopes: BTreeSet<DeclarationId>,
    finalized_tasks: BTreeSet<DeclarationId>,
    scope_failures: BTreeMap<DeclarationId, (u64, DeclarationId, FailureKind)>,
    scope_outcomes: BTreeMap<DeclarationId, ScopeExitOutcome>,
    completion_seq: u64,
    events: Vec<TaskEvent>,
    running: Option<DeclarationId>,
    totals: RunTotals,
    complete: bool,
}

impl<'a> ScopedTaskRun<'a> {
    #[must_use]
    pub const fn model_fingerprint(&self) -> [u8; 32] {
        self.model_fingerprint
    }

    #[must_use]
    pub fn events(&self) -> &[TaskEvent] {
        &self.events
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    #[must_use]
    pub fn totals(&self) -> RunTotals {
        self.totals
    }

    #[must_use]
    pub fn task_phase(&self, task: &str) -> Option<TaskPhase> {
        let runtime = self.states.get(&DeclarationId::new(task))?;
        Some(match runtime.state {
            TaskState::Pending => TaskPhase::Pending,
            TaskState::Started => TaskPhase::Started,
            TaskState::Completed => TaskPhase::Completed,
            TaskState::Failed => TaskPhase::Failed,
            TaskState::Cancelled => TaskPhase::Cancelled,
        })
    }

    #[must_use]
    pub fn is_scope_cancelled(&self, scope: &str) -> bool {
        self.cancelled_scopes.contains(&DeclarationId::new(scope))
    }

    /// Propagate sticky cancellation to one scope and all of its descendants.
    /// The mark takes effect before any sibling starts new work: subsequent
    /// steps announce the cancelled scopes and cancel their pending tasks
    /// before starting anything else. Repeating an identical cancellation is
    /// effect-free.
    pub fn cancel_scope(&mut self, scope: &str) -> Result<bool, ScopedTasksError> {
        self.authenticate()?;
        if self.complete {
            return Err(ScopedTasksError::RunAlreadyComplete);
        }
        let id = DeclarationId::new(scope);
        if !self.model.scopes.contains_key(&id) {
            return Err(ScopedTasksError::UnknownScope);
        }
        let targets = self.descendants_including(&id);
        let mut inserted = false;
        for candidate in targets {
            inserted |= self.cancelled_scopes.insert(candidate);
        }
        Ok(inserted)
    }

    /// Advance one deterministic scheduler step. Returns `Ok(None)` once the
    /// root scope exited; repeated calls stay quiescent. Step precedence:
    /// drain the running task, materialize cancellations, start the smallest
    /// ready task, abandon permanently blocked tasks, finalize in reverse
    /// completion order, then exit finished scopes deepest-first.
    pub fn step(&mut self) -> Result<Option<TaskEvent>, ScopedTasksError> {
        self.authenticate()?;
        if self.complete {
            return Ok(None);
        }
        if let Some(event) = self.apply_running_outcome() {
            return Ok(Some(self.record(event)));
        }
        if let Some(event) = self.materialize_scope_cancelled() {
            return Ok(Some(self.record(event)));
        }
        if let Some(event) = self.materialize_task_cancelled() {
            return Ok(Some(self.record(event)));
        }
        if let Some(event) = self.start_ready_task() {
            return Ok(Some(self.record(event)));
        }
        if let Some(event) = self.abandon_blocked_task() {
            return Ok(Some(self.record(event)));
        }
        if let Some(event) = self.finalize_one_task() {
            return Ok(Some(self.record(event)));
        }
        if let Some(event) = self.exit_one_scope() {
            let event = self.record(event);
            if let TaskEvent::ScopeExited { scope, .. } = &event {
                if *scope == self.model.root {
                    self.complete = true;
                }
            }
            return Ok(Some(event));
        }
        self.complete = true;
        Ok(None)
    }

    /// Consume the terminal summary. Requires a completed run.
    pub fn finish(&self) -> Result<ScopeRunSummary, ScopedTasksError> {
        self.authenticate()?;
        if !self.complete {
            return Err(ScopedTasksError::RunNotComplete);
        }
        let root_outcome = self
            .scope_outcomes
            .get(&self.model.root)
            .cloned()
            .ok_or(ScopedTasksError::RunNotComplete)?;
        Ok(ScopeRunSummary {
            root_outcome,
            totals: self.totals,
        })
    }

    /// Canonical JSON projection of the trace so far, bound to the model
    /// fingerprint and the terminal root outcome once present.
    #[must_use]
    pub fn trace_canonical_json(&self) -> String {
        let events = self
            .events
            .iter()
            .map(TaskEvent::canonical_json)
            .collect::<Vec<_>>()
            .join(",");
        let outcome = self
            .scope_outcomes
            .get(&self.model.root)
            .map_or_else(|| "null".to_owned(), scope_exit_outcome_json);
        let first_failure = self.scope_failures.get(&self.model.root).map_or_else(
            || "null".to_owned(),
            |(_, task, failure)| {
                format!(
                    "{{\"task\":{},\"failure\":{}}}",
                    quote_json(task.as_str()),
                    failure_json(*failure)
                )
            },
        );
        format!(
            "{{\"schema\":{},\"model_fingerprint\":\"{}\",\"events\":[{}],\"root_outcome\":{},\"first_failure\":{}}}",
            quote_json(SCOPED_TASKS_TRACE_V1),
            hex(&self.model_fingerprint),
            events,
            outcome,
            first_failure,
        )
    }

    /// Domain-separated SHA-256 digest over the canonical trace projection.
    #[must_use]
    pub fn trace_digest(&self) -> [u8; 32] {
        fingerprint(
            TRACE_FINGERPRINT_DOMAIN,
            self.trace_canonical_json().as_bytes(),
        )
    }

    /// First sticky failure observed in the given scope's subtree.
    #[must_use]
    pub fn first_failure(&self, scope: &str) -> Option<(&DeclarationId, FailureKind)> {
        let (_, task, failure) = self.scope_failures.get(&DeclarationId::new(scope))?;
        Some((task, *failure))
    }

    fn authenticate(&self) -> Result<(), ScopedTasksError> {
        if self.model_fingerprint != self.model.fingerprint {
            return Err(ScopedTasksError::FrameBindingMismatch);
        }
        Ok(())
    }

    fn record(&mut self, event: TaskEvent) -> TaskEvent {
        match &event {
            TaskEvent::Started { .. } => self.totals.started += 1,
            TaskEvent::Completed { .. } => self.totals.completed += 1,
            TaskEvent::Failed { .. } => self.totals.failed += 1,
            TaskEvent::Cancelled { .. } => self.totals.cancelled += 1,
            TaskEvent::ScopeCancelled { .. }
            | TaskEvent::Finalized { .. }
            | TaskEvent::ScopeExited { .. } => {}
        }
        self.events.push(event.clone());
        event
    }

    fn apply_running_outcome(&mut self) -> Option<TaskEvent> {
        let task = self.running.clone()?;
        self.running = None;
        let outcome = self
            .model
            .tasks
            .get(&task)
            .expect("running task exists")
            .outcome
            .clone();
        let runtime = self.states.get_mut(&task).expect("running task exists");
        runtime.completion_seq = Some(self.completion_seq);
        self.completion_seq += 1;
        match outcome {
            TaskOutcome::Succeed => {
                runtime.state = TaskState::Completed;
                self.settle_completion_edges(&task);
                Some(TaskEvent::Completed { task })
            }
            TaskOutcome::Fail(failure) => {
                runtime.state = TaskState::Failed;
                self.record_failure(&task, failure);
                Some(TaskEvent::Failed { task, failure })
            }
        }
    }

    fn settle_completion_edges(&mut self, task: &DeclarationId) {
        for dependent in self.dependents.get(task).cloned().unwrap_or_default() {
            if let Some(count) = self.pending_dependencies.get_mut(&dependent) {
                *count -= 1;
            }
        }
    }

    fn record_failure(&mut self, task: &DeclarationId, failure: FailureKind) {
        let mut scope = self
            .model
            .tasks
            .get(task)
            .map(|entry| entry.scope.clone())
            .expect("task exists");
        loop {
            self.scope_failures.entry(scope.clone()).or_insert((
                self.completion_seq,
                task.clone(),
                failure,
            ));
            match self
                .model
                .scopes
                .get(&scope)
                .and_then(|entry| entry.parent.clone())
            {
                Some(parent) => scope = parent,
                None => break,
            }
        }
    }

    fn materialize_scope_cancelled(&mut self) -> Option<TaskEvent> {
        let scope = self
            .cancelled_scopes
            .iter()
            .find(|scope| !self.announced_cancellations.contains(*scope))?
            .clone();
        self.announced_cancellations.insert(scope.clone());
        Some(TaskEvent::ScopeCancelled { scope })
    }

    fn task_in_cancelled_scope(&self, task: &DeclarationId) -> bool {
        let Some(entry) = self.model.tasks.get(task) else {
            return false;
        };
        let mut current = Some(entry.scope.clone());
        while let Some(scope) = current {
            if self.cancelled_scopes.contains(&scope) {
                return true;
            }
            current = self
                .model
                .scopes
                .get(&scope)
                .and_then(|entry| entry.parent.clone());
        }
        false
    }

    fn materialize_task_cancelled(&mut self) -> Option<TaskEvent> {
        let task = self
            .states
            .iter()
            .filter(|(_, runtime)| runtime.state == TaskState::Pending)
            .find(|(id, _)| self.task_in_cancelled_scope(id))
            .map(|(id, _)| id.clone())?;
        let runtime = self.states.get_mut(&task).expect("task exists");
        runtime.state = TaskState::Cancelled;
        Some(TaskEvent::Cancelled { task })
    }

    fn is_ready(&self, task: &DeclarationId) -> bool {
        self.pending_dependencies
            .get(task)
            .copied()
            .unwrap_or_default()
            == 0
    }

    fn start_ready_task(&mut self) -> Option<TaskEvent> {
        let task = self
            .states
            .iter()
            .filter(|(_, runtime)| runtime.state == TaskState::Pending)
            .find(|(id, _)| self.is_ready(id) && !self.task_in_cancelled_scope(id))
            .map(|(id, _)| id.clone())?;
        let runtime = self.states.get_mut(&task).expect("task exists");
        runtime.state = TaskState::Started;
        self.running = Some(task.clone());
        Some(TaskEvent::Started { task })
    }

    fn abandon_blocked_task(&mut self) -> Option<TaskEvent> {
        let task = self
            .states
            .iter()
            .filter(|(_, runtime)| runtime.state == TaskState::Pending)
            .find(|(id, _)| !self.is_ready(id))
            .map(|(id, _)| id.clone())?;
        let runtime = self.states.get_mut(&task).expect("task exists");
        runtime.state = TaskState::Cancelled;
        Some(TaskEvent::Cancelled { task })
    }

    /// Deepest scope whose tasks are all terminal, whose children all exited,
    /// and which has not itself exited. Ties resolve to the canonical smallest
    /// identity.
    fn exit_candidate(&self) -> Option<DeclarationId> {
        self.model
            .scopes
            .iter()
            .filter(|(id, _)| !self.exited_scopes.contains(*id))
            .filter(|(id, _)| {
                self.states
                    .iter()
                    .filter(|(task, _)| {
                        self.model.tasks.get(task).map(|entry| &entry.scope) == Some(id)
                    })
                    .all(|(_, runtime)| {
                        matches!(
                            runtime.state,
                            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
                        )
                    })
            })
            .filter(|(id, _)| {
                self.model
                    .children
                    .get(id)
                    .map(|children| {
                        children
                            .iter()
                            .all(|child| self.exited_scopes.contains(child))
                    })
                    .unwrap_or(true)
            })
            .max_by(|(left, left_entry), (right, right_entry)| {
                left_entry
                    .depth
                    .cmp(&right_entry.depth)
                    .then_with(|| right.cmp(left))
            })
            .map(|(id, _)| id.clone())
    }

    fn unfinalized_ran_tasks<'scope>(
        &'scope self,
        scope: &'scope DeclarationId,
    ) -> impl Iterator<Item = (&'scope DeclarationId, u64)> + 'scope {
        self.states
            .iter()
            .filter(move |(task, runtime)| {
                matches!(runtime.state, TaskState::Completed | TaskState::Failed)
                    && !self.finalized_tasks.contains(*task)
                    && self.model.tasks.get(*task).map(|entry| &entry.scope) == Some(scope)
            })
            .map(|(task, runtime)| (task, runtime.completion_seq.unwrap_or_default()))
    }

    fn finalize_one_task(&mut self) -> Option<TaskEvent> {
        let scope = self.exit_candidate()?;
        let task = self
            .unfinalized_ran_tasks(&scope)
            .max_by_key(|(_, seq)| *seq)
            .map(|(task, _)| task.clone())?;
        self.finalized_tasks.insert(task.clone());
        Some(TaskEvent::Finalized { task })
    }

    fn exit_one_scope(&mut self) -> Option<TaskEvent> {
        let scope = self.exit_candidate()?;
        if self.unfinalized_ran_tasks(&scope).next().is_some() {
            return None;
        }
        let outcome = self.scope_exit_outcome_for(&scope);
        self.exited_scopes.insert(scope.clone());
        self.scope_outcomes.insert(scope.clone(), outcome.clone());
        Some(TaskEvent::ScopeExited { scope, outcome })
    }

    fn scope_exit_outcome_for(&self, scope: &DeclarationId) -> ScopeExitOutcome {
        if let Some((_, task, failure)) = self.scope_failures.get(scope) {
            return ScopeExitOutcome::Failed {
                task: task.clone(),
                failure: *failure,
            };
        }
        if self.cancelled_scopes.contains(scope) {
            return ScopeExitOutcome::Cancelled;
        }
        ScopeExitOutcome::Success
    }

    fn descendants_including(&self, scope: &DeclarationId) -> Vec<DeclarationId> {
        let mut collected = vec![scope.clone()];
        let mut index = 0;
        while index < collected.len() {
            let current = collected[index].clone();
            index += 1;
            if let Some(children) = self.model.children.get(&current) {
                for child in children {
                    if !collected.contains(child) {
                        collected.push(child.clone());
                    }
                }
            }
        }
        collected.sort();
        collected.dedup();
        collected
    }
}

impl fmt::Debug for ScopedTaskRun<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopedTaskRun")
            .field("model_fingerprint", &hex(&self.model_fingerprint))
            .field("events", &self.events.len())
            .field("complete", &self.complete)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopedTasksError {
    MissingRoot,
    MultipleRoots,
    InvalidIdentity,
    DuplicateScope,
    DuplicateTask,
    UnknownScope,
    UnknownTask,
    ScopeCycle,
    ScopeBoundExceeded,
    TaskBoundExceeded,
    DependencyBoundExceeded,
    WorkBudgetExceeded,
    SelfDependency,
    DuplicateDependency,
    EscapingDependency,
    DependencyCycle,
    UnjoinedChildScope,
    OrphanJoin,
    DoubleJoin,
    InvalidFailureCode,
    RunAlreadyComplete,
    RunNotComplete,
    FrameBindingMismatch,
}

impl fmt::Display for ScopedTasksError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingRoot => "scoped-task model has no root scope",
            Self::MultipleRoots => "scoped-task model has more than one root scope",
            Self::InvalidIdentity => "scoped-task identity is empty or contains NUL",
            Self::DuplicateScope => "scoped-task model declares a duplicate scope identity",
            Self::DuplicateTask => "scoped-task model declares a duplicate task identity",
            Self::UnknownScope => "scoped-task model references an unknown scope",
            Self::UnknownTask => "scoped-task model references an unknown task",
            Self::ScopeCycle => "scoped-task scope tree contains a cycle",
            Self::ScopeBoundExceeded => "scoped-task scope count is outside bounds",
            Self::TaskBoundExceeded => "scoped-task task count is outside bounds",
            Self::DependencyBoundExceeded => "scoped-task dependency count is outside bounds",
            Self::WorkBudgetExceeded => "scoped-task model work budget is exceeded",
            Self::SelfDependency => "scoped-task task depends on itself",
            Self::DuplicateDependency => "scoped-task model declares a duplicate dependency",
            Self::EscapingDependency => "scoped-task dependency escapes its scope lineage",
            Self::DependencyCycle => "scoped-task dependency graph contains a cycle",
            Self::UnjoinedChildScope => "scoped-task child scope lacks exactly one parent join",
            Self::OrphanJoin => "scoped-task join does not name a direct child scope",
            Self::DoubleJoin => "scoped-task child scope is joined more than once",
            Self::InvalidFailureCode => "scoped-task physical failure code must be nonzero",
            Self::RunAlreadyComplete => "scoped-task run is already complete",
            Self::RunNotComplete => "scoped-task run has not reached quiescence",
            Self::FrameBindingMismatch => "scoped-task run binding does not match its model",
        })
    }
}

impl Error for ScopedTasksError {}

fn fingerprint(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

const fn sendable_name(mark: SendableMark) -> &'static str {
    match mark {
        SendableMark::Sendable => "sendable",
        SendableMark::NotSendable => "not_sendable",
    }
}

const fn shareable_name(mark: ShareableMark) -> &'static str {
    match mark {
        ShareableMark::Shareable => "shareable",
        ShareableMark::NotShareable => "not_shareable",
    }
}

fn failure_json(failure: FailureKind) -> String {
    match failure {
        FailureKind::Semantic => "{\"kind\":\"semantic\"}".to_owned(),
        FailureKind::Physical(code) => format!("{{\"kind\":\"physical\",\"code\":{code}}}"),
    }
}

fn outcome_json(outcome: &TaskOutcome) -> String {
    match outcome {
        TaskOutcome::Succeed => "{\"kind\":\"succeed\"}".to_owned(),
        TaskOutcome::Fail(failure) => {
            format!(
                "{{\"kind\":\"fail\",\"failure\":{}}}",
                failure_json(*failure)
            )
        }
    }
}

fn scope_exit_outcome_json(outcome: &ScopeExitOutcome) -> String {
    match outcome {
        ScopeExitOutcome::Success => "{\"kind\":\"success\"}".to_owned(),
        ScopeExitOutcome::Cancelled => "{\"kind\":\"cancelled\"}".to_owned(),
        ScopeExitOutcome::Failed { task, failure } => format!(
            "{{\"kind\":\"failed\",\"task\":{},\"failure\":{}}}",
            quote_json(task.as_str()),
            failure_json(*failure)
        ),
    }
}

#[cfg(test)]
#[path = "scoped_tasks/tests.rs"]
mod tests;
