use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const SHARDS: [&str; 6] = [
    "unit",
    "integration-0",
    "integration-1",
    "integration-2",
    "integration-3",
    "integration-4",
];

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
            assert!(["lib", "bin", "test", "example", "bench", "custom-build"].contains(&kind));
            if kind == "bench" {
                // bench targets are for `cargo bench` (criterion) and are
                // inventoried via cargo metadata but excluded from `cargo test`
                // sharding; see scripts/ci-msrv.py
                continue;
            }
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
            assert_eq!(&command[5..], ["--lib", "--bins", "--examples"]);
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
    let shard_for = |name: &str| {
        shards
            .iter()
            .position(|shard| {
                shard["targets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|target| target["name"] == name)
            })
            .unwrap()
    };
    assert_ne!(shard_for("project"), shard_for("agent_runtime_v1"));
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
    assert_eq!(output.stdout, b"MSRV integration-0: 3 workspace targets\n");
}

#[test]
fn msrv_check_covers_all_targets_once_and_remains_a_release_dependency() {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let msrv = workflow
        .split_once("\n  msrv:\n")
        .unwrap()
        .1
        .split_once("\n  release-gate:\n")
        .unwrap()
        .0;
    for required in [
        "name: Rust 1.88 minimum",
        "timeout-minutes: 180",
        "toolchain: \"1.88\"",
        "run: cargo fetch --locked",
        "run: cargo check --locked --workspace --all-targets --all-features",
    ] {
        assert!(msrv.contains(required), "missing MSRV contract: {required}");
    }
    assert_eq!(msrv.matches("run: cargo check ").count(), 1);
    assert!(!msrv.contains("python3 scripts/ci-msrv.py --shard"));
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
    // The aggregate must run even when the MSRV check fails so it can publish a
    // failing required-check conclusion instead of a skipped one.
    assert!(release.contains("if: ${{ always() }}"));
    assert!(!release.contains("if: ${{ success() }}"));
    assert!(release.contains("      - msrv\n"));
}

#[test]
fn current_rust_matrix_reuses_the_exact_inventory_in_parallel_platform_shards() {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let tests = workflow
        .split_once("\n  verify-tests:\n")
        .unwrap()
        .1
        .split_once("\n  desktop-native-product:\n")
        .unwrap()
        .0;
    for required in [
        "name: Rust tests ${{ matrix.os }} (${{ matrix.shard }})",
        "fail-fast: false",
        "os: [ubuntu-latest, macos-latest, windows-latest]",
        "shard: [unit, integration-0, integration-1, integration-2, integration-3, integration-4]",
        "python3 scripts/ci-msrv.py --label \"Rust $RUNNER_OS\" --shard \"${{ matrix.shard }}\"",
        "python3 scripts/ci-msrv.py --label \"Rust Windows\" --shard \"${{ matrix.shard }}\" --exclude-package semaprax-native-rust-interop --nocapture",
    ] {
        assert!(tests.contains(required), "missing Rust shard contract: {required}");
    }
    for forbidden in ["continue-on-error", "--no-fail-fast", "actions/cache"] {
        assert!(!tests.contains(forbidden), "Rust shard bypass: {forbidden}");
    }
    let verify = workflow
        .split_once("\n  verify:\n")
        .unwrap()
        .1
        .split_once("\n  verify-tests:\n")
        .unwrap()
        .0;
    assert!(!verify.contains("cargo test --locked --workspace --all-targets --all-features"));
    assert!(!verify.contains("cargo test --locked --workspace --exclude"));
    let release = workflow
        .split_once("\n  release-gate:\n")
        .unwrap()
        .1
        .split_once("\n  release-artifacts:\n")
        .unwrap()
        .0;
    assert!(release.contains("      - verify-tests\n"));
}

#[test]
fn windows_typed_agent_corpus_moves_without_losing_coverage() {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let agent_job = workflow
        .split_once("\n  agent-proposal-clients:\n")
        .unwrap()
        .1
        .split_once("\n  gen05b-generic-instance-closure:\n")
        .unwrap()
        .0;
    assert!(agent_job.contains("if: runner.os == 'Windows'"));
    assert!(agent_job.contains(
        "cargo test --locked --offline -p semaprax --all-features --test agent_runtime_v1 execution_revision::typed::"
    ));
    let router = std::fs::read_to_string(root().join("scripts/ci-msrv.py")).unwrap();
    assert!(router.contains("args.label == \"Rust Windows\" and args.shard == \"integration-3\""));
    assert!(router.contains("test_arguments.extend((\"--skip\", \"execution_revision::typed::\"))"));
}

const ROUTER_FAILURES: &str = r#"
import contextlib
import copy
import io
import json
import runpy
import subprocess
import sys
from unittest.mock import patch
# Exercise Windows text translation even on Unix; forward asserted bytes below.
sys.stdout.reconfigure(newline="\r\n")
router = runpy.run_path('scripts/ci-msrv.py')
fallback_env = router['cargo_environment']({}, sys.executable)
assert fallback_env == {'SEMAPRAX_TEST_PYTHON': sys.executable}
explicit_env = router['cargo_environment']({'SEMAPRAX_TEST_PYTHON': sys.executable}, '/ignored')
assert explicit_env == {'SEMAPRAX_TEST_PYTHON': sys.executable}
try:
    router['cargo_environment']({'SEMAPRAX_TEST_PYTHON': 'python3'}, sys.executable)
except ValueError as error:
    assert 'absolute Python file' in str(error), str(error)
else:
    raise AssertionError('relative explicit Python was accepted')
def target(kind, name):
    return {'kind': [kind], 'name': name}
metadata = {'workspace_members': ['one', 'two'], 'packages': [
    {'id': 'one', 'name': 'one', 'targets': [target('lib', 'one'), target('custom-build', 'build-script-build'), target('test', 'a'), target('test', 'b'), target('test', 'd'), target('test', 'e'), target('test', 'f')]},
    {'id': 'two', 'name': 'two', 'targets': [target('bin', 'two'), target('example', 'embedding-api'), target('test', 'a'), target('test', 'c')]},
    {'id': 'external', 'name': 'external', 'targets': [target('example', 'not_in_workspace')]},
]}
plan = router['plan'](metadata)
assert [len(shard['targets']) for shard in plan['shards']] == [4, 3, 1, 1, 1, 1]
assert {'package': 'two', 'kind': 'example', 'name': 'embedding-api'} in plan['shards'][0]['targets']
assert '--examples' in plan['shards'][0]['command']
assert router['plan'](dict(metadata, packages=list(reversed(metadata['packages'])))) == plan
excluded = copy.deepcopy(metadata)
excluded['packages'][0]['targets'].append(target('test', 'g'))
excluded_plan = router['plan'](excluded, ['two'])
assert all(row['package'] == 'one' for row in excluded_plan['inventory'])
assert all(command_part not in ('two',) for shard in excluded_plan['shards'] for command_part in shard['command'][7:])
assert all(shard['command'][5:7] == ['--exclude', 'two'] for shard in excluded_plan['shards'])
try:
    router['plan'](metadata, ['missing'])
except ValueError as error:
    assert 'unknown excluded workspace package' in str(error), str(error)
else:
    raise AssertionError('unknown excluded package was accepted')
for mutation, message in [
    (lambda m: m['packages'][0]['targets'].append(target('unknown', 'future')), 'unrouted'),
    (lambda m: m['packages'][0]['targets'].append(target('test', 'a')), 'duplicate'),
    (lambda m: m['packages'].pop(1), 'incomplete'),
    (lambda m: (m['packages'][0]['targets'].pop(), m['packages'][1]['targets'].pop()), 'empty shard'),
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
router_log = io.StringIO()
with patch('subprocess.run', side_effect=[
    subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
    subprocess.CompletedProcess([], 101),
]) as run:
    with contextlib.redirect_stdout(router_log):
        assert router['main'](['--shard', 'integration-0']) == 101
    assert run.call_count == 2
    assert run.call_args_list[0].kwargs['check'] is True
    assert run.call_args_list[0].kwargs['env']['SEMAPRAX_TEST_PYTHON'] == sys.executable
    assert run.call_args_list[1].args[0] == plan['shards'][1]['command']
    assert run.call_args_list[1].kwargs['env']['SEMAPRAX_TEST_PYTHON'] == sys.executable
with patch('subprocess.run', side_effect=[
    subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
    subprocess.CompletedProcess([], 101),
]) as run:
    with contextlib.redirect_stdout(io.StringIO()):
        assert router['main'](['--shard', 'integration-0', '--nocapture']) == 101
    assert run.call_args_list[1].args[0] == plan['shards'][1]['command'] + ['--', '--nocapture']
sys.stdout.buffer.write(router_log.getvalue().encode('utf-8'))
"#;
