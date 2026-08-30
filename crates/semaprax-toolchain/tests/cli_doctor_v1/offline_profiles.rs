//! Profile admission is an injected authority boundary, not a sandbox emulator.
use super::*;
use std::cell::Cell;

struct BorrowedHost<'a>(&'a FakeHost);

impl DoctorHost for BorrowedHost<'_> {
    fn os(&self) -> &str {
        self.0.os()
    }
    fn arch(&self) -> &str {
        self.0.arch()
    }
    fn resolve_tool(&self, name: &str) -> Result<PathBuf, DoctorError> {
        self.0.resolve_tool(name)
    }
    fn run_version(&self, path: &Path) -> Result<String, DoctorError> {
        self.0.run_version(path)
    }
}

struct Profiles {
    profiles: BTreeMap<String, FakeHost>,
    acquired: RefCell<Vec<String>>,
    facts: Cell<usize>,
    returned_selector: Option<&'static str>,
}

impl Profiles {
    fn new() -> Self {
        let first = FakeHost::healthy();
        let mut second = FakeHost::healthy();
        second
            .versions
            .insert(tool_path("node"), Ok("v24.1.0\n".to_owned()));
        Self {
            profiles: BTreeMap::from([("first-v1".into(), first), ("second-v1".into(), second)]),
            acquired: RefCell::new(Vec::new()),
            facts: Cell::new(0),
            returned_selector: None,
        }
    }

    fn assert_no_tools(&self) {
        for profile in self.profiles.values() {
            assert!(profile.calls.borrow().is_empty());
        }
    }
}

impl doctor::OfflineProfileHost for Profiles {
    fn os(&self) -> &str {
        self.facts.set(self.facts.get() + 1);
        "test-os"
    }
    fn arch(&self) -> &str {
        self.facts.set(self.facts.get() + 1);
        "test-arch"
    }
    fn acquire(&self, selector: &str) -> Result<doctor::AdmittedProfile<'_>, DoctorError> {
        self.acquired.borrow_mut().push(selector.to_owned());
        let host = self
            .profiles
            .get(selector)
            .ok_or_else(|| DoctorError::new("profile unavailable"))?;
        Ok(doctor::AdmittedProfile {
            selector: self.returned_selector.unwrap_or(selector).to_owned(),
            host: Box::new(BorrowedHost(host)),
        })
    }
}

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn checks(outcome: &doctor::DoctorOutcome) -> Vec<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(&outcome.output).unwrap();
    assert_eq!(value["schema"], "semaprax.doctor.v1");
    value["checks"].as_array().unwrap().clone()
}

#[test]
fn malformed_profile_requests_precede_every_host_callback() {
    let too_long = "a".repeat(65);
    for selector in [
        "",
        "../first-v1",
        "/first-v1",
        "C:\\first-v1",
        "@first-v1",
        "First",
        "0first",
        "a.b",
        "a_b",
        "a b",
        "a\n",
        "a\0b",
        "é",
        too_long.as_str(),
    ] {
        let host = Profiles::new();
        assert!(
            doctor::run_with_profile_host(&arguments(&["--profile", selector]), &host).is_err()
        );
        assert_eq!(host.facts.get(), 0, "{selector:?}");
        assert!(host.acquired.borrow().is_empty());
        host.assert_no_tools();
        assert!(
            doctor::inspect_profile(&host, DoctorTarget::All, true, false, Some(selector)).is_err()
        );
        assert_eq!(host.facts.get(), 0);
    }
    for args in [
        vec!["--profile"],
        vec!["--profile", "first-v1", "--profile", "second-v1"],
        vec!["--profile", "first-v1", "--json", "--json"],
        vec!["--profile", "first-v1", "--target", "unknown"],
        vec!["--profile", "first-v1", "extra"],
    ] {
        let host = Profiles::new();
        assert!(doctor::run_with_profile_host(&arguments(&args), &host).is_err());
        assert_eq!(host.facts.get(), 0);
        assert!(host.acquired.borrow().is_empty());
        host.assert_no_tools();
    }
}

#[test]
fn missing_and_unavailable_profiles_fail_required_checks_without_tool_access() {
    for selector in [None, Some("unavailable-v1")] {
        let host = Profiles::new();
        let outcome =
            doctor::inspect_profile(&host, DoctorTarget::All, true, false, selector).unwrap();
        assert_eq!(outcome.exit_code, 1);
        let rows = checks(&outcome);
        assert_eq!(
            rows.iter()
                .map(|row| row["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["semaprax", "os", "arch", "release", "profile", "clang", "node", "rust"]
        );
        for row in &rows[..4] {
            assert_eq!(row["status"], "ok");
        }
        for row in &rows[4..] {
            assert_eq!(row["required"], true);
            assert_eq!(row["status"], "failed");
        }
        assert_eq!(
            *host.acquired.borrow(),
            selector.into_iter().map(str::to_owned).collect::<Vec<_>>()
        );
        host.assert_no_tools();
    }
}

#[test]
fn exact_selector_boundary_and_target_order_use_one_admitted_profile() {
    for selector in ["a".to_owned(), "a".repeat(64), "first-v1".to_owned()] {
        for (target, tools) in [
            (DoctorTarget::Contributor, vec!["rustc"]),
            (DoctorTarget::Native, vec!["clang"]),
            (DoctorTarget::Web, vec!["node"]),
            (DoctorTarget::All, vec!["clang", "node", "rustc"]),
        ] {
            let mut host = Profiles::new();
            host.profiles.insert(selector.clone(), FakeHost::healthy());
            let outcome =
                doctor::inspect_profile(&host, target, true, true, Some(&selector)).unwrap();
            assert_eq!(outcome.exit_code, 0);
            let rows = checks(&outcome);
            assert_eq!(rows[4]["id"], "profile");
            assert_eq!(rows[4]["status"], "ok");
            assert!(rows[4]["detail"].as_str().unwrap().contains(&selector));
            assert_eq!(*host.acquired.borrow(), [selector.clone()]);
            let expected = tools
                .into_iter()
                .flat_map(|tool| [format!("resolve:{tool}"), version_call(tool)])
                .collect::<Vec<_>>();
            assert_eq!(*host.profiles[&selector].calls.borrow(), expected);
            for (id, profile) in &host.profiles {
                if id != &selector {
                    assert!(profile.calls.borrow().is_empty());
                }
            }
        }
    }
}

#[test]
fn admitted_profile_identity_and_host_mismatch_fail_before_tools() {
    for fault in ["selector", "os", "arch"] {
        let mut host = Profiles::new();
        match fault {
            "selector" => host.returned_selector = Some("second-v1"),
            "os" => host.profiles.get_mut("first-v1").unwrap().os = "other-os",
            "arch" => host.profiles.get_mut("first-v1").unwrap().arch = "other-arch",
            _ => unreachable!(),
        }
        assert!(
            doctor::inspect_profile(&host, DoctorTarget::All, true, false, Some("first-v1"))
                .is_err()
        );
        assert_eq!(*host.acquired.borrow(), ["first-v1"]);
        host.assert_no_tools();
    }
}

#[test]
fn profile_versions_never_cross_select_and_failures_do_not_acquire_another_profile() {
    for (selector, version) in [("first-v1", "v22.14.0"), ("second-v1", "v24.1.0")] {
        let host = Profiles::new();
        let outcome = doctor::run_with_profile_host(
            &arguments(&["--target", "web", "--profile", selector, "--json"]),
            &host,
        )
        .unwrap();
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(checks(&outcome)[5]["detail"], version);
        assert_eq!(*host.acquired.borrow(), [selector]);
    }
    let mut host = Profiles::new();
    host.profiles.get_mut("first-v1").unwrap().versions.insert(
        tool_path("node"),
        Err(DoctorError::new("profile version failure")),
    );
    let outcome =
        doctor::inspect_profile(&host, DoctorTarget::Web, false, false, Some("first-v1")).unwrap();
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome.output.contains("profile version failure"));
    assert_eq!(*host.acquired.borrow(), ["first-v1"]);
    assert!(host.profiles["second-v1"].calls.borrow().is_empty());
}
