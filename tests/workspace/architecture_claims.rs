//! Architecture Claims v1 integration: `forbid_reaches` evaluated end to end
//! against a real compiled `ProjectRevision`, not just the pure graph model
//! exercised by `semaprax::architecture_claims`'s own unit tests.
//!
//! The one property worth proving here that the pure unit tests cannot: a
//! claim result is derived from checked facts, so it goes stale loudly. When
//! the exact same claim is re-evaluated after source changes so that a new
//! call edge exists, the result flips from `held` to `violated` and the
//! bound `project_revision` digest changes with it. Nothing here renders a
//! claim from anything other than the compiler's own retained HIR.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::architecture_claims::{ArchitectureClaim, ArchitectureClaimSet};
use semaprax::project::{with_authenticated_project, ProjectRevision};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

const MANIFEST: &str = concat!(
    "schema = \"semaprax.project.v1\"\n",
    "name = \"archclaims\"\n",
    "entry = \"archclaims.app\"\n",
    "sources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\n",
    "web_exports = [\"archclaims.c\"]\n",
    "tests = [\"archclaims.tests\"]\n",
);

const TESTS: &str = concat!(
    "module archclaims.tests;\n\n",
    "@id(\"archclaims.tests.main\")\n",
    "fn main() -> i64\n",
    "{\n",
    "    0\n",
    "}\n",
);

const APP: &str = concat!(
    "module archclaims.app;\n",
    "use function @id(\"archclaims.a\") from archclaims.core as a;\n\n",
    "@id(\"archclaims.app.main\")\n",
    "fn main() -> i64\n",
    "{\n",
    "    a()\n",
    "}\n",
);

fn core(b_calls_c: bool) -> String {
    let b_body = if b_calls_c { "c()" } else { "1" };
    format!(
        "module archclaims.core;\n\n\
@id(\"archclaims.a\")\n\
fn a() -> i64\n\
{{\n\
    b()\n\
}}\n\n\
@id(\"archclaims.b\")\n\
fn b() -> i64\n\
{{\n\
    {b_body}\n\
}}\n\n\
@id(\"archclaims.c\")\n\
fn c() -> i64\n\
{{\n\
    2\n\
}}\n"
    )
}

impl Fixture {
    fn new(b_calls_c: bool) -> Self {
        Self::with_core(core(b_calls_c))
    }

    fn with_core(raw_core: String) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-architecture-claims-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        let app_program = semaprax::parse(APP, "src/app.spx").unwrap();
        std::fs::write(
            root.join("src/app.spx"),
            semaprax::format::canonical(&app_program),
        )
        .unwrap();
        let core_program = semaprax::parse(&raw_core, "src/core.spx").unwrap();
        std::fs::write(
            root.join("src/core.spx"),
            semaprax::format::canonical(&core_program),
        )
        .unwrap();
        let tests_program = semaprax::parse(TESTS, "src/tests.spx").unwrap();
        std::fs::write(
            root.join("src/tests.spx"),
            semaprax::format::canonical(&tests_program),
        )
        .unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        self.try_revision().unwrap()
    }

    fn try_revision(&self) -> Result<Arc<ProjectRevision>, Vec<semaprax::diagnostic::Diagnostic>> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn no_a_to_c() -> ArchitectureClaimSet {
    ArchitectureClaimSet::new(vec![ArchitectureClaim::forbid_reaches(
        "no-a-to-c",
        "archclaims.a",
        "archclaims.c",
    )
    .unwrap()])
    .unwrap()
}

fn value(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

#[test]
fn forbid_reaches_holds_over_a_real_compiled_revision_with_no_call_edge() {
    let fixture = Fixture::new(false);
    let revision = fixture.revision();
    let result = no_a_to_c().evaluate(&revision).unwrap();
    let payload = value(result.to_json());
    assert_eq!(
        payload["schema"],
        "semaprax.architecture-claim-set-result.v1"
    );
    assert_eq!(payload["claims"][0]["status"], "held");
    assert_eq!(payload["project_revision"], revision.project_revision());
}

#[test]
fn forbid_reaches_over_a_real_compiled_revision_goes_stale_loudly_not_silently() {
    // Same claim, same claim set constructor, two revisions of the same
    // module that differ only in whether `b` calls `c`. The claim result
    // must track the checked fact, not remain frozen at its first answer.
    let held_fixture = Fixture::new(false);
    let held_revision = held_fixture.revision();
    let held_result = no_a_to_c().evaluate(&held_revision).unwrap();
    let held_payload = value(held_result.to_json());
    assert_eq!(held_payload["claims"][0]["status"], "held");

    let violated_fixture = Fixture::new(true);
    let violated_revision = violated_fixture.revision();
    let violated_result = no_a_to_c().evaluate(&violated_revision).unwrap();
    let violated_payload = value(violated_result.to_json());
    assert_eq!(violated_payload["claims"][0]["status"], "violated");
    let path = violated_payload["claims"][0]["path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["stable_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(path, vec!["archclaims.a", "archclaims.b", "archclaims.c"]);

    // The claim is bound to the exact revision it was checked against: two
    // source variants that differ in checked behavior never share a
    // project_revision digest, so the flip from held to violated cannot be
    // mistaken for the same fact re-read twice.
    assert_ne!(
        held_payload["project_revision"],
        violated_payload["project_revision"]
    );
}

#[test]
fn claim_set_result_is_byte_identical_across_two_independent_builds_of_the_same_source() {
    let first = Fixture::new(false);
    let second = Fixture::new(false);
    let first_result = no_a_to_c().evaluate(&first.revision()).unwrap();
    let second_result = no_a_to_c().evaluate(&second.revision()).unwrap();
    assert_eq!(first_result.to_json(), second_result.to_json());
}

/// Issue #297: `protocol_order_bound` over a real compiled revision whose
/// `core` module declares a session protocol realized by its own functions.
fn core_with_protocol(transitions: &str) -> String {
    format!(
        "{}\n@id(\"archclaims.protocol\")\n\
session protocol \"archclaims-order-v1\" {{\n\
    states {{ Ready, Done }}\n\
    initial Ready;\n\
    terminal Done cleanup {{ release }}\n\
{transitions}}}\n",
        core(false)
    )
}

const BOUND: &str = "    on Ready first: send Unit via \"archclaims.b\" -> Done;\n    on Ready abort: fail Unit via \"archclaims.c\" -> Done;\n";

fn order_claim() -> ArchitectureClaimSet {
    ArchitectureClaimSet::new(vec![ArchitectureClaim::protocol_order_bound(
        "order-bound",
        "archclaims.protocol",
    )
    .unwrap()])
    .unwrap()
}

#[test]
fn protocol_order_bound_holds_when_every_via_is_a_checked_call_graph_node() {
    let fixture = Fixture::with_core(core_with_protocol(BOUND));
    let revision = fixture.revision();
    let result = order_claim().evaluate(&revision).unwrap();
    let payload = value(result.to_json());
    let claim = &payload["claims"][0];
    assert_eq!(claim["operator"], "protocol_order_bound");
    assert_eq!(claim["status"], "held");
    assert_eq!(claim["authority"], "none");
    assert_eq!(claim["protocol_name"], "archclaims-order-v1");
    assert_eq!(claim["missing"], serde_json::Value::Null);
    let via = claim["via"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| edge["via"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(via, vec!["archclaims.b", "archclaims.c"]);
    assert_eq!(payload["project_revision"], revision.project_revision());
    // Deterministic across two independent builds of the same source.
    let again = Fixture::with_core(core_with_protocol(BOUND));
    assert_eq!(
        order_claim().evaluate(&again.revision()).unwrap().to_json(),
        result.to_json()
    );
}

#[test]
fn protocol_order_bound_is_unevaluable_without_any_via_binding() {
    let fixture = Fixture::with_core(core_with_protocol(
        "    on Ready first: send Unit -> Done;\n    on Ready abort: fail Unit -> Done;\n",
    ));
    let payload = value(
        order_claim()
            .evaluate(&fixture.revision())
            .unwrap()
            .to_json(),
    );
    assert_eq!(payload["claims"][0]["status"], "unevaluable");
}

#[test]
fn a_via_outside_the_declaring_module_fails_the_build_and_an_unknown_protocol_is_refused() {
    let fixture = Fixture::with_core(core_with_protocol(
        "    on Ready first: send Unit via \"archclaims.app.main\" -> Done;\n    on Ready abort: fail Unit -> Done;\n",
    ));
    // The Project build itself runs the session-protocol source checks, so a
    // `via` outside the declaring module never reaches claim evaluation.
    let error = fixture.try_revision().err().unwrap();
    assert!(
        error.iter().any(|diagnostic| diagnostic.code == "SPX-K104"),
        "{error:?}"
    );

    let bound = Fixture::with_core(core_with_protocol(BOUND));
    let unknown = ArchitectureClaimSet::new(vec![ArchitectureClaim::protocol_order_bound(
        "unknown",
        "archclaims.no-such-protocol",
    )
    .unwrap()])
    .unwrap();
    assert_eq!(
        unknown.evaluate(&bound.revision()).unwrap_err()[0].code,
        "SPX-AC601"
    );
}
