use super::*;

fn rows(outcome: &DoctorOutcome) -> Vec<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(&outcome.output).unwrap()["checks"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn production_adapter_requires_the_opaque_settled_observation() {
    // Signature evidence only: this neither constructs authority nor runs a tool.
    let _: fn(&SettledDoctorObservation, bool) -> DoctorOutcome = super::super::render_settled;
}

#[test]
fn aliased_paths_keep_role_specific_outputs_and_real_clang_path() {
    let tools = [
        (
            DoctorOfflineTool::Clang,
            "/bin/shared",
            Ok(&b"clang version 18.1.0\nTarget: private\n"[..]),
        ),
        (
            DoctorOfflineTool::Node,
            "/bin/shared",
            Ok(&b"v22.14.0\n"[..]),
        ),
        (
            DoctorOfflineTool::Rustc,
            "/bin/shared",
            Ok(&b"rustc 1.88.0 (private)\n"[..]),
        ),
    ];
    let outcome = render_rows(
        "profile-v1",
        DoctorOfflineArchitecture::LinuxX86_64,
        DoctorOfflineTarget::All,
        tools,
        true,
        true,
    );
    assert_eq!(outcome.exit_code, 0);
    let checks = rows(&outcome);
    assert_eq!(
        checks
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["semaprax", "os", "arch", "release", "profile", "clang", "node", "rust"]
    );
    assert_eq!(checks[0]["detail"], env!("CARGO_PKG_VERSION"));
    assert_eq!(checks[1]["detail"], "linux");
    assert_eq!(checks[2]["detail"], "x86_64");
    assert_eq!(checks[3]["detail"], "release");
    assert_eq!(
        checks[4]["detail"],
        "offline profile `profile-v1`; checks describe this profile only"
    );
    assert_eq!(checks[5]["detail"], "/bin/shared (clang version 18.1.0)");
    assert_eq!(checks[6]["detail"], "v22.14.0");
    assert_eq!(checks[7]["detail"], "rustc 1.88.0 (private)");
    assert_eq!(
        outcome.output,
        render_rows(
            "profile-v1",
            DoctorOfflineArchitecture::LinuxX86_64,
            DoctorOfflineTarget::All,
            tools,
            true,
            true
        )
        .output
    );
}

#[test]
fn selected_targets_and_architecture_are_observation_facts() {
    for (target, tool, bytes, id, label) in [
        (
            DoctorOfflineTarget::Contributor,
            DoctorOfflineTool::Rustc,
            &b"rustc 1.88.0\n"[..],
            "rust",
            "contributor",
        ),
        (
            DoctorOfflineTarget::Native,
            DoctorOfflineTool::Clang,
            &b"clang version 18\n"[..],
            "clang",
            "native",
        ),
        (
            DoctorOfflineTarget::Web,
            DoctorOfflineTool::Node,
            &b"v22.0.0\n"[..],
            "node",
            "web",
        ),
    ] {
        let outcome = render_rows(
            "arm-v1",
            DoctorOfflineArchitecture::LinuxAarch64,
            target,
            [(tool, "/bin/tool", Ok(bytes))],
            true,
            false,
        );
        assert_eq!(outcome.exit_code, 0);
        let checks = rows(&outcome);
        assert_eq!(checks.len(), 6);
        assert_eq!(checks[2]["detail"], "aarch64");
        assert_eq!(checks[3]["detail"], "debug");
        assert_eq!(checks[5]["id"], id);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&outcome.output).unwrap()["target"],
            label
        );
    }
}

#[test]
fn captured_failures_have_closed_stable_diagnostics_without_invented_versions() {
    for (error, detail) in [
        (
            DoctorProbeError::Invalid,
            "offline tool invocation was invalid",
        ),
        (
            DoctorProbeError::Unsupported,
            "offline tool invocation is unsupported",
        ),
        (
            DoctorProbeError::Spawn,
            "offline tool supervisor launch failed",
        ),
        (
            DoctorProbeError::Exit,
            "offline tool terminated unsuccessfully",
        ),
        (
            DoctorProbeError::OutputLimit,
            "offline tool output exceeded its limit",
        ),
        (
            DoctorProbeError::Timeout,
            "offline tool execution timed out",
        ),
        (DoctorProbeError::Io, "offline tool capture failed"),
    ] {
        let outcome = render_rows(
            "profile-v1",
            DoctorOfflineArchitecture::LinuxX86_64,
            DoctorOfflineTarget::All,
            [
                (DoctorOfflineTool::Clang, "/bin/shared", Err(error)),
                (
                    DoctorOfflineTool::Node,
                    "/bin/shared",
                    Ok(&b"v22.0.0\n"[..]),
                ),
                (
                    DoctorOfflineTool::Rustc,
                    "/bin/shared",
                    Ok(&b"rustc 1.88.0\n"[..]),
                ),
            ],
            true,
            false,
        );
        assert_eq!(outcome.exit_code, 1);
        let checks = rows(&outcome);
        assert_eq!(checks[5]["status"], "failed");
        assert_eq!(checks[5]["detail"], detail);
        assert_eq!(checks[6]["status"], "ok");
        assert_eq!(checks[7]["status"], "ok");
    }
}

#[test]
fn output_text_and_version_policy_fail_as_tool_rows() {
    for (tool, bytes, detail) in [
        (
            DoctorOfflineTool::Node,
            &b"\xff"[..],
            "tool returned non-UTF-8 version output",
        ),
        (
            DoctorOfflineTool::Node,
            &b""[..],
            "tool returned an invalid version string",
        ),
        (
            DoctorOfflineTool::Node,
            &b"v22.\x010.0\n"[..],
            "tool returned an invalid version string",
        ),
        (
            DoctorOfflineTool::Node,
            &b"v21.0.0\n"[..],
            "v21.0.0 (requires major version 22 or newer; found 21)",
        ),
        (
            DoctorOfflineTool::Node,
            &b"v22.0.0garbage\n"[..],
            "unrecognized Node version `v22.0.0garbage`",
        ),
        (
            DoctorOfflineTool::Rustc,
            &b"rustc 1.87.0\n"[..],
            "rustc 1.87.0 (requires Rust 1.88 or newer; found 1.87)",
        ),
    ] {
        let target = match tool {
            DoctorOfflineTool::Rustc => DoctorOfflineTarget::Contributor,
            _ => DoctorOfflineTarget::Web,
        };
        let outcome = render_rows(
            "profile-v1",
            DoctorOfflineArchitecture::LinuxX86_64,
            target,
            [(tool, "/bin/tool", Ok(bytes))],
            true,
            false,
        );
        assert_eq!(outcome.exit_code, 1);
        let checks = rows(&outcome);
        assert_eq!(checks[5]["status"], "failed");
        assert_eq!(checks[5]["detail"], detail);
    }
}

#[test]
fn human_and_json_escape_observed_text_without_changing_policy() {
    let tools = [(
        DoctorOfflineTool::Clang,
        "/bin/tool",
        Ok(&b"clang \"quoted\" \\version\n"[..]),
    )];
    let human = render_rows(
        "profile-v1",
        DoctorOfflineArchitecture::LinuxX86_64,
        DoctorOfflineTarget::Native,
        tools,
        false,
        false,
    );
    assert_eq!(human.exit_code, 0);
    assert!(human
        .output
        .ends_with("ok clang: /bin/tool (clang \\\"quoted\\\" \\\\version)\n"));
    let json = render_rows(
        "profile-v1",
        DoctorOfflineArchitecture::LinuxX86_64,
        DoctorOfflineTarget::Native,
        tools,
        true,
        false,
    );
    assert_eq!(
        rows(&json)[5]["detail"],
        "/bin/tool (clang \"quoted\" \\version)"
    );
}
