use super::*;
use crate::assurance_manifest::law_set::{
    strict::{RequiredLawEvidence, StrictLawPolicy},
    LawSet,
};

fn native_fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-host-law-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/native-law-project");
    for file in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
        "src/LAWS.spx",
    ] {
        std::fs::copy(example.join(file), root.join(file)).unwrap();
    }
    root.canonicalize().unwrap()
}

fn install_selected_native_policy(root: &Path) {
    let manifest = root.join(MANIFEST_FILE);
    let revision =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    let laws =
        LawSet::derive(&revision, "native-host-v1", revision.law_modules().to_vec()).unwrap();
    let policy = StrictLawPolicy::new(
        laws,
        BTreeMap::from([
            (
                "native-law.add.right-nonnegative".into(),
                RequiredLawEvidence::PinnedLeanSource {
                    toolchain: crate::proof_export::PINNED_TOOLCHAIN.into(),
                    accepted_assumptions: crate::proof_export::ASSUMPTIONS
                        .iter()
                        .map(|(id, _)| (*id).into())
                        .collect(),
                    accepted_axioms: crate::proof_export::kernel_report::STANDARD_AXIOMS
                        .iter()
                        .map(|id| (*id).into())
                        .collect(),
                },
            ),
            (
                "native-law.order.total".into(),
                RequiredLawEvidence::SmtSource,
            ),
        ]),
    )
    .unwrap();
    install_host_strict_law_policy(&manifest, &policy, vec![]).unwrap();
}

#[test]
fn selected_host_policy_closes_generic_revision_escape_and_keeps_inspection_open() {
    let root = native_fixture();
    let manifest = root.join(MANIFEST_FILE);
    install_selected_native_policy(&root);
    let error = with_authenticated_project(&manifest, |snapshot| {
        let retained = snapshot.retain_revision();
        retained.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-LW150");
    with_authenticated_project_inspection(&manifest, |inspection| {
        inspection.check()?;
        assert_eq!(inspection.law_modules().len(), 1);
        assert!(inspection
            .semantic_graph()
            .contains("native-law.order.total"));
        Ok(())
    })
    .unwrap();
    let error = with_strict_authenticated_project(&manifest, &[], &[], |session| {
        session.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-LW130");
    assert_eq!(
        with_authenticated_project(&manifest, |_| Ok(())).unwrap_err()[0].code,
        "SPX-LW150"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn selected_host_policy_refuses_raw_workspace_apply_before_active_pivot() {
    let root = native_fixture();
    let source = canonical_source(
        "helper.spx",
        "module host_aux; @id(\"host-aux.value\") fn value() -> i64 { 1 } @id(\"host-aux.main\") fn main() -> i64 { value() }",
    );
    std::fs::write(root.join("helper.spx"), &source).unwrap();
    let second = canonical_source(
        "helper2.spx",
        "module host_aux2; @id(\"host-aux2.value\") fn value() -> i64 { 2 } @id(\"host-aux2.main\") fn main() -> i64 { value() }",
    );
    std::fs::write(root.join("helper2.spx"), &second).unwrap();
    let path_set = root.join("paths.json");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"helper.spx\"},{\"path\":\"helper2.spx\"}]}\n",
    )
    .unwrap();
    let base = crate::workspace::initialize(&root, &path_set).unwrap();
    let patch = root.join("change.wspatch");
    let child = format!(
        "base {}\nrename host-aux.value to answer\n",
        crate::graph::revision(&crate::parse(&source, "helper.spx").unwrap())
    );
    let second_child = format!(
        "base {}\nrename host-aux2.value to answer\n",
        crate::graph::revision(&crate::parse(&second, "helper2.spx").unwrap())
    );
    std::fs::write(
        &patch,
        format!(
            "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{base}\",\"files\":[{{\"path\":\"helper.spx\",\"patch\":{}}},{{\"path\":\"helper2.spx\",\"patch\":{}}}]}}\n",
            serde_json::to_string(&child).unwrap(),
            serde_json::to_string(&second_child).unwrap()
        ),
    )
    .unwrap();
    let active = root.join(".semaprax-workspace/ACTIVE");
    let before = std::fs::read(&active).unwrap();
    install_selected_native_policy(&root);
    let diagnostics = crate::workspace::apply(&root, &patch).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-LW150");
    assert_eq!(std::fs::read(&active).unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}
