//! One request lineage and revision bound to every step of a run. The id is a
//! digest of (snapshot revision, task, lock), so an identical restart finds the
//! same lineage (and its journal) while any changed input starts a new one.

use crate::contract::ProjectBinding;
use crate::json::digest;
use serde_json::json;
use std::cell::{Cell, RefCell};

#[derive(Debug)]
pub struct Lineage {
    pub id: String,
    pub project: ProjectBinding,
    pub lock_digest: String,
    pub task_digest: String,
    counter: Cell<u32>,
    issued: RefCell<Vec<String>>,
}

impl Lineage {
    pub fn new(project: ProjectBinding, lock_digest: &str, task_digest: &str) -> Self {
        let d = digest(
            "semaprax.harness-lineage.v1",
            &json!({"revision": project.revision, "task": task_digest, "lock": lock_digest}),
        );
        Self {
            id: format!("wf-{}", &d["sha256:".len()..][..16]),
            project,
            lock_digest: lock_digest.into(),
            task_digest: task_digest.into(),
            counter: Cell::new(0),
            issued: RefCell::default(),
        }
    }

    /// Next invocation id in this lineage (`inv-<n>`, unique per lineage).
    pub fn next_invocation(&self) -> String {
        let n = self.counter.get() + 1;
        self.counter.set(n);
        let id = format!("inv-{}-{n:06}", &self.id[3..]);
        self.issued.borrow_mut().push(id.clone());
        id
    }

    /// Invocation ids issued before the most recent one (the request `lineage` array).
    pub fn parents(&self) -> Vec<String> {
        let issued = self.issued.borrow();
        issued[..issued.len().saturating_sub(1)]
            .iter()
            .rev()
            .take(8)
            .rev()
            .cloned()
            .collect()
    }

    pub fn invocations_issued(&self) -> u32 {
        self.counter.get()
    }
}
