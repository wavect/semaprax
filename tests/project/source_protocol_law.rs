use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::assurance_manifest::law_set::{
    strict::{self, RequiredLawEvidence, StrictLawPolicy},
    EvidenceRequirement, LawDefinition, LawModule, LawSelector, LawSet,
};
use semaprax::assurance_manifest::model_checking::source_protocol::{
    check_authenticated_snapshot, check_project_source_protocol, replay, ProtocolSafetyOutcome,
};
use semaprax::assurance_manifest::model_checking::Bounds;
use semaprax::project::with_authenticated_project;
use std::collections::BTreeMap;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"payment-machine\"\nentry = \"payment.machine\"\nsources = [\"src/helper.spx\", \"src/machine.spx\"]\nweb_exports = [\"payment.step\"]\ntests = [\"payment.tests\"]\n";
const BOUNDS: Bounds = Bounds {
    max_states: 64,
    max_depth: 32,
    max_transitions: 128,
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Fixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-source-protocol-law-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        let helper = semaprax::parse(
            "module payment.tests;\n@id(\"payment.tests.main\") fn main() -> i64 { 0 }\n",
            Path::new("src/helper.spx"),
        )
        .unwrap();
        std::fs::write(
            root.join("src/helper.spx"),
            semaprax::format::canonical(&helper),
        )
        .unwrap();
        let program = semaprax::parse(source, Path::new("src/machine.spx")).unwrap();
        std::fs::write(
            root.join("src/machine.spx"),
            semaprax::format::canonical(&program),
        )
        .unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

fn source(repeated_charge: bool) -> String {
    let mut branches = vec![
        ("state == 0 && event == 0", 3),
        ("state == 1 && event == 1", 4),
        ("state == 1 && event == 2", 6),
        ("state == 3 && event == 3", 0),
        ("state == 3 && event == 4", 8),
    ];
    if repeated_charge {
        branches.push(("state == 2 && event == 5", 3));
    }
    branches.push((
        if repeated_charge {
            "state == 0 && event == 6"
        } else {
            "state == 0 && event == 5"
        },
        8,
    ));
    branches.push((
        if repeated_charge {
            "state == 1 && event == 7"
        } else {
            "state == 1 && event == 6"
        },
        8,
    ));
    if repeated_charge {
        branches.push(("state == 2 && event == 8", 8));
    }
    let mut dispatch = "-1".to_owned();
    for (condition, result) in branches.into_iter().rev() {
        dispatch = format!("if {condition} {{ {result} }} else {{ {dispatch} }}");
    }
    let succeeded_terminal = if repeated_charge {
        ""
    } else {
        "terminal Succeeded cleanup { release_receipt }"
    };
    let extra_transition = if repeated_charge {
        "on Succeeded charge: call ChargeCommand via \"payment.dispatch\" -> Pending;"
    } else {
        ""
    };
    let succeeded_escape = if repeated_charge {
        "on Succeeded escaped: fail Unit via \"payment.dispatch\" -> Failed;"
    } else {
        ""
    };
    format!(
        r#"module payment.machine;
@id("payment.dispatch") fn dispatch(state: i64, event: i64) -> i64 {{
    {dispatch}
}}
@id("payment.main") fn main() -> i64 {{ 0 }}
@id("payment.step") fn step(state: i64, event: i64) -> i64 {{ dispatch(state, event) }}
@id("payment.protocol") session protocol "payment-command-v1" {{
    states {{ Idle, Pending, Succeeded, Retry, Failed }}
    initial Idle;
    {succeeded_terminal}
    terminal Failed cleanup {{ release_receipt }}
    on Idle charge: call ChargeCommand via "payment.dispatch" -> Pending;
    on Pending success: receive ChargeAccepted via "payment.dispatch" -> Succeeded;
    on Pending failure: receive ChargeRejected via "payment.dispatch" -> Retry;
    on Retry retry: call RetryDecision via "payment.dispatch" -> Idle;
    on Retry abort: fail Unit via "payment.dispatch" -> Failed;
    {extra_transition}
    on Idle cancel: cancel Unit via "payment.dispatch" -> Failed;
    on Pending timeout: timeout Unit via "payment.dispatch" -> Failed;
    {succeeded_escape}
}}
"#
    )
}

fn checked(
    fixture: &Fixture,
    bounds: Bounds,
) -> semaprax::assurance_manifest::model_checking::source_protocol::SourceProtocolReport {
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let report = check_project_source_protocol(
            &snapshot.retain_revision(),
            "payment.protocol",
            "payment.dispatch",
            "payment.step",
            "Succeeded",
            "charge",
            bounds,
        )
        .map_err(|error| vec![error])?;
        Ok(report)
    })
    .unwrap()
}

#[test]
fn legal_retry_model_is_source_bound_and_finitely_checked() {
    let fixture = Fixture::new(&source(false));
    let report = checked(&fixture, BOUNDS);
    assert_eq!(report.outcome, ProtocolSafetyOutcome::ModelChecked);
    assert!(report.model_checked());
    assert_eq!(report.coverage.len(), 7);
    assert_eq!(report.state_domain.len(), 5);
    assert_eq!(report.initial_state, "Idle");
    assert_eq!(report.fairness, "none");
    assert!(report.explored_states >= 5);
    let view: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(view["status"], "model_checked");
    assert_eq!(
        view["claim"],
        "finite_pure_dispatcher_safety_only_no_external_exactly_once"
    );
    let diagnostic = with_authenticated_project(&fixture.manifest(), |snapshot| {
        check_authenticated_snapshot(
            snapshot,
            "payment.protocol",
            "payment.dispatch",
            "payment.step",
            "Succeeded",
            "charge",
            BOUNDS,
        )
    })
    .unwrap();
    assert_eq!(diagnostic, report.to_json());
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&report, &snapshot.retain_revision()).map_err(|error| vec![error])
    })
    .unwrap();

    let tiny = checked(
        &fixture,
        Bounds {
            max_states: 1,
            max_depth: 1,
            max_transitions: 1,
        },
    );
    assert_eq!(tiny.outcome, ProtocolSafetyOutcome::BoundsExhausted);
    assert!(!tiny.model_checked());
    let mut stale = report.clone();
    stale.bounds.max_depth = 1;
    let refusal = with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&stale, &snapshot.retain_revision()).map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP407");
    let mut abstract_trace = report.clone();
    abstract_trace.outcome = ProtocolSafetyOutcome::AbstractCounterexample { trace: vec![] };
    let view: serde_json::Value = serde_json::from_str(&abstract_trace.to_json()).unwrap();
    assert_eq!(view["trace_replay"], "abstract_only");
    assert_eq!(view["status"], "violated");
    let refusal = with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&abstract_trace, &snapshot.retain_revision()).map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP407");
}

#[test]
fn repeated_charge_has_minimal_source_replayed_counterexample() {
    let fixture = Fixture::new(&source(true));
    let report = checked(&fixture, BOUNDS);
    let ProtocolSafetyOutcome::ConcreteCounterexample { trace } = &report.outcome else {
        panic!("expected concrete counterexample: {:?}", report.outcome)
    };
    assert_eq!(trace.len(), 3);
    assert_eq!(
        trace
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        vec!["charge", "success", "charge"]
    );
    assert_eq!(trace.last().unwrap().from, "Succeeded");
    assert!(trace.last().unwrap().charge_command);
    assert_eq!(trace.last().unwrap().via, "payment.dispatch");
    let view: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(view["trace_replay"], "concrete_source_replay");
    assert_eq!(view["status"], "violated");
    assert!(!report.model_checked());
}

#[test]
fn source_mutation_and_missing_protocol_coverage_refuse() {
    let legal = Fixture::new(&source(false));
    let verified = checked(&legal, BOUNDS);
    let changed_source =
        Fixture::new(&source(false).replace("event == 1 { 4 }", "event == 1 { 6 }"));
    let refusal = with_authenticated_project(&changed_source.manifest(), |snapshot| {
        replay(&verified, &snapshot.retain_revision()).map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP406");
    let changed_initial = Fixture::new(&source(false).replace("initial Idle;", "initial Retry;"));
    let refusal = with_authenticated_project(&changed_initial.manifest(), |snapshot| {
        replay(&verified, &snapshot.retain_revision()).map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP407");
    let out_of_domain =
        Fixture::new(&source(false).replace("event == 1 { 4 }", "event == 1 { 100 }"));
    let refusal = with_authenticated_project(&out_of_domain.manifest(), |snapshot| {
        check_project_source_protocol(
            &snapshot.retain_revision(),
            "payment.protocol",
            "payment.dispatch",
            "payment.step",
            "Succeeded",
            "charge",
            BOUNDS,
        )
        .map(|_| ())
        .map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP405");
    let missing = Fixture::new(&source(false).replace(
        "on Retry retry: call RetryDecision via \"payment.dispatch\" -> Idle;",
        "",
    ));
    let refusal = with_authenticated_project(&missing.manifest(), |snapshot| {
        check_project_source_protocol(
            &snapshot.retain_revision(),
            "payment.protocol",
            "payment.dispatch",
            "payment.step",
            "Succeeded",
            "charge",
            BOUNDS,
        )
        .map(|_| ())
        .map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP406");
}

#[test]
fn caller_bypass_or_reordered_arguments_refuse_source_association() {
    for (altered, expected_code) in [
        (
            source(false).replace(
                "@id(\"payment.step\")",
                "@id(\"payment.bypass\") fn bypass(state: i64, event: i64) -> i64 { dispatch(state, event) }\n@id(\"payment.step\")",
            ),
            "SPX-LP408",
        ),
        (
            source(false)
            .replace(
                "@id(\"payment.step\")",
                "@id(\"payment.generic-bypass\") fn bypass<T>(state: i64, event: i64, unused: T) -> i64 { dispatch(state, event) }\n@id(\"payment.step\")",
            )
            .replace(
                "@id(\"payment.main\") fn main() -> i64 { 0 }",
                "@id(\"payment.main\") fn main() -> i64 { bypass(0, 0, 0) }",
            ),
            "SPX-W115",
        ),
        (
            source(false).replace("dispatch(state, event) }", "dispatch(event, state) }"),
            "SPX-LP408",
        ),
    ] {
        let fixture = Fixture::new(&altered);
        let refusal = with_authenticated_project(&fixture.manifest(), |snapshot| {
            check_project_source_protocol(
                &snapshot.retain_revision(),
                "payment.protocol",
                "payment.dispatch",
                "payment.step",
                "Succeeded",
                "charge",
                BOUNDS,
            )
            .map(|_| ())
            .map_err(|error| vec![error])
        })
        .unwrap_err();
        assert_eq!(refusal[0].code, expected_code, "{refusal:?}");
    }
}

#[test]
fn private_wrapper_in_export_closure_is_not_a_selected_public_caller() {
    let source = source(false).replace(
        "@id(\"payment.step\")",
        "@id(\"payment.other\") fn other(state: i64, event: i64) -> i64 { step(state, event) }\n@id(\"payment.step\")",
    );
    let fixture = Fixture::new(&source);
    std::fs::write(
        fixture.manifest(),
        MANIFEST.replace(
            "web_exports = [\"payment.step\"]",
            "web_exports = [\"payment.other\"]",
        ),
    )
    .unwrap();
    let refusal = with_authenticated_project(&fixture.manifest(), |snapshot| {
        check_project_source_protocol(
            &snapshot.retain_revision(),
            "payment.protocol",
            "payment.dispatch",
            "payment.step",
            "Succeeded",
            "charge",
            BOUNDS,
        )
        .map(|_| ())
        .map_err(|error| vec![error])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP408", "{refusal:?}");
}

fn selected_law(bounds: Bounds) -> LawModule {
    LawModule {
        module_id: "payment.laws".into(),
        source_path: "src/machine.spx".into(),
        assumptions: vec![],
        laws: vec![LawDefinition {
            law_id: "payment.no-charge-after-success".into(),
            selector: LawSelector::SourceProtocolSafety {
                protocol_id: "payment.protocol".into(),
                dispatcher_id: "payment.dispatch".into(),
                caller_id: "payment.step".into(),
                success_state: "Succeeded".into(),
                charge_label: "charge".into(),
                bounds,
            },
            assumption_ids: vec![],
            requires_laws: vec![],
            evidence: EvidenceRequirement::ModelChecked,
        }],
    }
}

fn strict_policy(laws: &LawSet, digest: String) -> StrictLawPolicy {
    StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "payment.no-charge-after-success".into(),
            RequiredLawEvidence::SourceProtocolSafety {
                evidence_digest: digest,
                minimum_states: BOUNDS.max_states,
                minimum_depth: BOUNDS.max_depth,
                minimum_transitions: BOUNDS.max_transitions,
            },
        )]),
    )
    .unwrap()
}

#[test]
fn protected_source_protocol_law_requires_exact_replayed_source_and_bounds() {
    let fixture = Fixture::new(&source(false));
    let model = checked(&fixture, BOUNDS);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let laws = LawSet::derive(&revision, "checked-v1", vec![selected_law(BOUNDS)])?;
        let policy = strict_policy(&laws, model.evidence_digest.clone());
        let report = strict::derive(&revision, &laws, &policy, &[])?;
        strict::require(&report, &revision, &laws, &policy, &[])?;
        let view: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(view["accepted"], true);
        assert_eq!(view["laws"][0]["evidence"]["status"], "model_checked");

        let wrong = strict_policy(&laws, "sha256:wrong".into());
        let refused = strict::derive(&revision, &laws, &wrong, &[])?;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&refused).unwrap()["accepted"],
            false
        );
        assert_eq!(
            strict::require(&refused, &revision, &laws, &wrong, &[]).unwrap_err()[0].code,
            "SPX-LW130"
        );
        let tiny = LawSet::derive(
            &revision,
            "checked-v1",
            vec![selected_law(Bounds {
                max_states: 1,
                max_depth: 1,
                max_transitions: 1,
            })],
        )?;
        let policy = strict_policy(&tiny, model.evidence_digest.clone());
        let refused = strict::derive(&revision, &tiny, &policy, &[])?;
        let view: serde_json::Value = serde_json::from_str(&refused).unwrap();
        assert_eq!(view["accepted"], false);
        assert_eq!(
            view["laws"][0]["failure"],
            "law_missing_unsupported_or_open"
        );
        let mut assumed = selected_law(BOUNDS);
        assumed.assumptions = vec!["payment.provider-accepts".into()];
        assumed.laws[0].assumption_ids = assumed.assumptions.clone();
        let assumed = LawSet::derive(&revision, "checked-v1", vec![assumed])?;
        let policy = strict_policy(&assumed, model.evidence_digest.clone());
        let refused = strict::derive(&revision, &assumed, &policy, &[])?;
        let view: serde_json::Value = serde_json::from_str(&refused).unwrap();
        assert_eq!(view["accepted"], false);
        assert_eq!(
            view["laws"][0]["failure"],
            "law_missing_unsupported_or_open"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn via_binding_alone_cannot_satisfy_source_protocol_strict_method() {
    let fixture = Fixture::new(&source(false));
    let model = checked(&fixture, BOUNDS);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let mut module = selected_law(BOUNDS);
        module.laws[0].selector = LawSelector::ProtocolRealizersBound {
            claim_id: "payment-via-only".into(),
            protocol_id: "payment.protocol".into(),
        };
        module.laws[0].evidence = EvidenceRequirement::CompilerProved;
        let laws = LawSet::derive(&revision, "checked-v1", vec![module])?;
        let policy = strict_policy(&laws, model.evidence_digest.clone());
        let report = strict::derive(&revision, &laws, &policy, &[])?;
        let view: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(view["accepted"], false);
        assert_eq!(
            view["laws"][0]["failure"],
            "source_protocol_scope_does_not_match_law"
        );
        Ok(())
    })
    .unwrap();
}
