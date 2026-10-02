//! Local executable fixtures only: never invokes Claude or a network provider.
use super::*;
use std::os::unix::fs::PermissionsExt;

fn envelope(document: &str) -> String {
    json!({"type":"result","subtype":"success","is_error":false,"num_turns":1,
        "stop_reason":"end_turn","terminal_reason":"completed","queued_turn_count":0,
        "result_index":0,"permission_denials":[],"subagent_stats":{"spawned":0},
        "result":document,"usage":{"input_tokens":1,"output_tokens":1},
        "modelUsage":{"claude-haiku-4-5":{"canonicalModel":"claude-haiku-4-5","provider":"firstParty","webSearchRequests":0}}}).to_string()
}

#[test]
fn repair_v3_native_claude_wire_runs_checked_feedback_and_zero_dispatch_resume() {
    let fixture = Fixture::new();
    let source = APP
        .replace("fake.local", "anthropic")
        .replace("fake-basic", crate::claude_host::MODEL);
    let manifest = write_project(&fixture, &source).canonicalize().unwrap();
    let digest = schema_digest(&source);
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"repair the checked candidate").unwrap();
    let v1 = write_config(
        &fixture,
        &repair_config_value(&manifest, &task_path, &digest, "test.repair.claude.v3"),
    );
    let config = v2_config(&fixture, &v1);
    let mut value: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["schema"] = json!(CONFIG_SCHEMA_V3);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let executable = fixture.0.join("claude-fixture");
    let count = fixture.0.join("calls");
    let script = format!("#!/bin/sh\ncase \"${{17}}\" in *'\"proposal_schema_digest\":\"{digest}\"'*) ;; *) exit 43 ;; esac\nprintf 'call\\n' >> '{}'\nfor arg; do prompt=\"$arg\"; done\ncase \"$prompt\" in\n *'\"turn\":0,'*) printf '%s' '{}' ;;\n *'\"turn\":1,'*) printf '%s' '{}' ;;\n *) exit 42 ;;\nesac\n", count.display(), envelope(&proposal(&digest, "0", "0")), envelope(&proposal(&digest, "7", "1")));
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let checkpoint = fixture.0.join("checkpoint");
    let before = project_tree(&fixture.0.join("project"));
    let command = |verb: &str| {
        Command::parse(&[
            verb.into(),
            config.to_string_lossy().into_owned(),
            checkpoint.to_string_lossy().into_owned(),
            "--claude".into(),
            executable.to_string_lossy().into_owned(),
            "--scratch".into(),
            scratch.to_string_lossy().into_owned(),
        ])
        .unwrap()
    };
    let receipt: Value =
        serde_json::from_str(&execute_with_runner(command("run"), ProcessOpenCodeRunner).unwrap())
            .unwrap();
    assert_eq!(receipt["schema"], RECEIPT_SCHEMA_V3);
    assert_eq!(receipt["status"], "complete");
    assert_eq!(receipt["model_dispatches"], 2);
    assert_eq!(receipt["effect_dispatches"], 2);
    assert_eq!(receipt["selected_profile"]["provider_id"], "anthropic");
    assert_eq!(
        receipt["selected_profile"]["model_id"],
        crate::claude_host::MODEL
    );
    assert_eq!(
        receipt["selected_profile"]["config_schema"],
        CONFIG_SCHEMA_V3
    );
    assert_eq!(receipt["source_mutation"], false);
    assert_eq!(receipt["publication_authority"], false);
    assert_eq!(fs::read_to_string(&count).unwrap(), "call\ncall\n");
    let journal = fs::read(checkpoint.join("checkpoint.json")).unwrap();
    let resumed: Value = serde_json::from_str(
        &execute_with_runner(command("resume"), ProcessOpenCodeRunner).unwrap(),
    )
    .unwrap();
    assert_eq!(resumed["status"], "complete");
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(resumed["candidate_digest"], Value::Null);
    assert_eq!(fs::read_to_string(&count).unwrap(), "call\ncall\n");
    assert_eq!(
        fs::read(checkpoint.join("checkpoint.json")).unwrap(),
        journal
    );
    assert_eq!(project_tree(&fixture.0.join("project")), before);
    // The old V2 schema cannot silently select this transport.
    value["schema"] = json!(CONFIG_SCHEMA_V2);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        execute_with_runner(command("resume"), ProcessOpenCodeRunner)
            .unwrap_err()
            .reason,
        "repair Claude configuration requires --claude ABS --scratch EMPTY_ABS"
    );
    assert_eq!(fs::read_to_string(&count).unwrap(), "call\ncall\n");
}
