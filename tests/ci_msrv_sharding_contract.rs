use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const SHARDS: [&str; 4] = ["unit", "integration-0", "integration-1", "integration-2"];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn json_output(command: &mut Command) -> Value {
    let output = command.current_dir(root()).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn key(target: &Value) -> (String, String, String) {
    let [package, kind, name] =
        ["package", "kind", "name"].map(|field| target[field].as_str().unwrap().to_owned());
    (package, kind, name)
}

#[test]
fn msrv_shards_select_every_actual_workspace_target_exactly_once() {
    let metadata = json_output(Command::new(env!("CARGO")).args([
        "metadata",
        "--locked",
        "--no-deps",
        "--all-features",
        "--format-version",
        "1",
    ]));
    let members = metadata["workspace_members"].as_array().unwrap();
    let mut expected = BTreeSet::new();
    for package in metadata["packages"].as_array().unwrap() {
        if !members.contains(&package["id"]) {
            continue;
        }
        for target in package["targets"].as_array().unwrap() {
            let kinds = target["kind"].as_array().unwrap();
            assert_eq!(kinds.len(), 1);
            let kind = kinds[0].as_str().unwrap();
            assert!(["lib", "bin", "test"].contains(&kind));
            assert!(expected.insert((
                package["id"].as_str().unwrap().to_owned(),
                kind.to_owned(),
                target["name"].as_str().unwrap().to_owned(),
            )));
        }
    }
    let plan = json_output(Command::new("python3").args(["scripts/ci-msrv.py", "--plan-only"]));
    let inventory = plan["inventory"].as_array().unwrap();
    assert_eq!(inventory.iter().map(key).collect::<BTreeSet<_>>(), expected);
    assert_eq!(inventory.len(), expected.len());
    let shards = plan["shards"].as_array().unwrap();
    assert_eq!(shards.len(), SHARDS.len());
    let mut visited = BTreeSet::new();
    for (index, shard) in shards.iter().enumerate() {
        assert_eq!(shard["name"], SHARDS[index]);
        let command: Vec<_> = shard["command"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect();
        assert_eq!(
            &command[..5],
            ["cargo", "test", "--locked", "--workspace", "--all-features"]
        );
        let selected: BTreeSet<_> = if index == 0 {
            assert_eq!(&command[5..], ["--lib", "--bins"]);
            expected
                .iter()
                .filter(|(_, kind, _)| kind != "test")
                .cloned()
                .collect()
        } else {
            let (pairs, remainder) = command[5..].as_chunks::<2>();
            assert!(remainder.is_empty());
            let names: BTreeSet<_> = pairs
                .iter()
                .map(|pair| {
                    assert_eq!(pair[0], "--test");
                    pair[1]
                })
                .collect();
            assert_eq!(names.len(), (command.len() - 5) / 2);
            assert!(!names.is_empty());
            for name in &names {
                assert!(expected
                    .iter()
                    .any(|(_, kind, n)| kind == "test" && n == name));
            }
            expected
                .iter()
                .filter(|(_, kind, name)| kind == "test" && names.contains(name.as_str()))
                .cloned()
                .collect()
        };
        let reported = shard["targets"].as_array().unwrap();
        assert_eq!(reported.iter().map(key).collect::<BTreeSet<_>>(), selected);
        assert_eq!(reported.len(), selected.len());
        assert!(!selected.is_empty());
        for target in selected {
            assert!(
                visited.insert(target),
                "workspace target appears in multiple shards"
            );
        }
    }
    assert_eq!(visited, expected);
}

#[test]
fn msrv_router_fails_closed_and_propagates_the_first_cargo_failure() {
    let output = Command::new("python3")
        .args(["-B", "-c", ROUTER_FAILURES])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"MSRV integration-0: 2 workspace targets\n");
}

#[test]
fn msrv_matrix_preserves_checks_timeout_fail_fast_and_release_dependency() {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let msrv = workflow
        .split_once("\n  msrv:\n")
        .unwrap()
        .1
        .split_once("\n  release-gate:\n")
        .unwrap()
        .0;
    for required in [
        "name: Rust 1.88 minimum (${{ matrix.shard }})",
        "timeout-minutes: 20",
        "fail-fast: true",
        "shard: [unit, integration-0, integration-1, integration-2]",
        "toolchain: \"1.88\"",
        "run: cargo fetch --locked",
        "run: cargo check --locked --workspace --all-targets --all-features",
        "run: python3 scripts/ci-msrv.py --shard \"${{ matrix.shard }}\"",
    ] {
        assert!(msrv.contains(required), "missing MSRV contract: {required}");
    }
    assert!(msrv.find("cargo check").unwrap() < msrv.find("python3 scripts/ci-msrv.py").unwrap());
    for forbidden in [
        "continue-on-error",
        "--no-fail-fast",
        "--exclude",
        "--skip",
        "actions/cache",
    ] {
        assert!(
            !msrv.contains(forbidden),
            "MSRV coverage bypass: {forbidden}"
        );
    }
    let release = workflow
        .split_once("\n  release-gate:\n")
        .unwrap()
        .1
        .split_once("\n  release-artifacts:\n")
        .unwrap()
        .0;
    assert!(release.contains("if: ${{ success() }}"));
    assert!(release.contains("      - msrv\n"));
}

const ROUTER_FAILURES: &str = r#"
import contextlib
import copy
import io
import json
import runpy
import subprocess
from unittest.mock import patch
router = runpy.run_path('scripts/ci-msrv.py')
def target(kind, name):
    return {'kind': [kind], 'name': name}
metadata = {'workspace_members': ['one', 'two'], 'packages': [
    {'id': 'one', 'targets': [target('lib', 'one'), target('test', 'a'), target('test', 'b')]},
    {'id': 'two', 'targets': [target('bin', 'two'), target('test', 'a'), target('test', 'c')]},
    {'id': 'external', 'targets': [target('example', 'not_in_workspace')]},
]}
plan = router['plan'](metadata)
assert [len(shard['targets']) for shard in plan['shards']] == [2, 2, 1, 1]
assert router['plan'](dict(metadata, packages=list(reversed(metadata['packages'])))) == plan
for mutation, message in [
    (lambda m: m['packages'][0]['targets'].append(target('example', 'future')), 'unrouted'),
    (lambda m: m['packages'][0]['targets'].append(target('test', 'a')), 'duplicate'),
    (lambda m: m['packages'].pop(1), 'incomplete'),
    (lambda m: m['packages'][1]['targets'].pop(), 'empty shard'),
]:
    bad = copy.deepcopy(metadata)
    mutation(bad)
    try:
        router['plan'](bad)
    except ValueError as error:
        assert message in str(error), str(error)
    else:
        raise AssertionError('invalid inventory was accepted')
with patch('subprocess.run') as run:
    with contextlib.redirect_stderr(io.StringIO()):
        try:
            router['main'](['--shard', 'unknown'])
        except SystemExit as error:
            assert error.code == 2
        else:
            raise AssertionError('unknown shard accepted')
    run.assert_not_called()
with patch('subprocess.run', side_effect=[
    subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
    subprocess.CompletedProcess([], 101),
]) as run:
    assert router['main'](['--shard', 'integration-0']) == 101
    assert run.call_count == 2
    assert run.call_args_list[0].kwargs['check'] is True
    assert run.call_args_list[1].args[0] == plan['shards'][1]['command']
"#;
