use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const SHARDS: [&str; 7] = [
    "unit",
    "integration-0",
    "integration-1",
    "integration-2",
    "integration-3",
    "integration-4",
    "integration-5",
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
    assert_eq!(output.stdout, b"MSRV integration-0: 2 workspace targets\n");
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
        "shard: [unit, unit-heavy, integration-0, integration-1, integration-2, integration-3, integration-4, integration-5]",
        "python3 scripts/ci-msrv.py --label \"Rust $RUNNER_OS\" --shard \"${{ matrix.shard }}\"",
        "python3 scripts/ci-msrv.py --label \"Rust Windows\" --shard \"${{ matrix.shard }}\" --exclude-package semaprax-native-rust-interop --nocapture @split",
        "if ('${{ matrix.shard }}' -eq 'integration-3') { $split = @('--split-windows-agent-runtime') }",
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
    assert!(release.contains("      - unix-source-repair\n"));
    assert!(release.contains("      - windows-source-repair\n"));
    assert!(release.contains("      - windows-agent-runtime-rest\n"));
    let router = std::fs::read_to_string(root().join("scripts/ci-msrv.py")).unwrap();
    assert!(router.contains(
        "HEAVY_UNIT_FILTERS = (\"kernel_zero::differential::\", \"workspace_graph::tests::\")"
    ));
    assert!(router.contains("if args.shard == HEAVY_UNIT_SHARD:"));
    assert!(router.contains("if args.shard == \"unit\":"));
    assert!(router
        .contains("test_arguments.extend((\"--skip\", \"source_live_cli::repair::tests::\"))"));
    let repair = workflow
        .split_once("\n  unix-source-repair:\n")
        .unwrap()
        .1
        .split_once("\n  windows-agent-runtime-rest:\n")
        .unwrap()
        .0;
    // The exact repair command now lives in the router, which deals the
    // listed cases across the matrix shards.
    for required in [
        "\"cargo\", \"test\", \"--locked\", \"--offline\", \"-p\", \"semaprax-toolchain\", \"--all-features\", \"--lib\",",
        "\"semaprax/unstable-native-host-internal,semaprax/unstable-wit-component-harness,\"",
        "\"semaprax/unstable-workflow-profiling\",",
        "REPAIR_FILTER = \"source_live_cli::repair::tests::\"",
        "REPAIR_TEST + [REPAIR_FILTER, \"--\", \"--list\", \"--format\", \"terse\"],",
        "REPAIR_TEST + [\"--\", \"--exact\", \"--test-threads=1\", *names],",
    ] {
        assert!(router.contains(required), "missing repair router contract: {required}");
    }
    assert!(repair.contains("name: Rust source repair (${{ matrix.os }}, ${{ matrix.shard }}/2)"));
    assert!(repair.contains("os: [ubuntu-latest, macos-latest]"));
    assert!(repair.contains("shard: [0, 1]"));
    assert!(repair.contains(
        "run: python3 scripts/ci-msrv.py --label \"Rust source repair\" --repair-shard \"${{ matrix.shard }}/2\""
    ));
    assert!(repair.contains("fail-fast: false"));
    assert!(!repair.contains("continue-on-error"));
    let windows_repair = workflow
        .split_once("\n  windows-source-repair:\n")
        .unwrap()
        .1
        .split_once("\n  desktop-native-product:\n")
        .unwrap()
        .0;
    assert!(windows_repair.contains("name: Rust Windows source repair"));
    assert!(windows_repair.contains("runs-on: windows-latest"));
    assert!(windows_repair.contains(
        "run: python3 scripts/ci-msrv.py --label \"Rust Windows source repair\" --repair-shard 0/1"
    ));
    assert!(!windows_repair.contains("continue-on-error"));
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
    let dedicated = workflow
        .split_once("\n  windows-agent-runtime-rest:\n")
        .unwrap()
        .1
        .split_once("\n  desktop-native-product:\n")
        .unwrap()
        .0;
    assert!(dedicated.contains("cargo test --locked --offline --workspace --all-features --exclude semaprax-native-rust-interop --test agent_runtime_v1 -- --skip execution_revision::typed::"));
    assert!(!dedicated.contains("continue-on-error"));
}

const ROUTER_FAILURES: &str = r#"
import contextlib
import copy
import io
import json
from pathlib import Path
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
assert router['macos_test_git']({}, lambda name: sys.executable) == {
    'SEMAPRAX_TEST_GIT': str(Path(sys.executable).resolve())
}
try:
    router['macos_test_git']({'SEMAPRAX_TEST_GIT': 'git'}, lambda name: None)
except ValueError as error:
    assert 'absolute Git file' in str(error), str(error)
else:
    raise AssertionError('relative Git executable was accepted')
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
assert [len(shard['targets']) for shard in plan['shards']] == [4, 2, 1, 1, 1, 1, 1]
assert {'package': 'two', 'kind': 'example', 'name': 'embedding-api'} in plan['shards'][0]['targets']
assert '--examples' in plan['shards'][0]['command']
assert router['plan'](dict(metadata, packages=list(reversed(metadata['packages'])))) == plan
split = copy.deepcopy(plan['shards'][4])
split['targets'].append({'package': 'one', 'kind': 'test', 'name': 'agent_runtime_v1'})
split['command'].extend(['--test', 'agent_runtime_v1'])
assert router['without_dedicated_windows_agent_runtime'](split) == plan['shards'][4]['command']
try:
    router['without_dedicated_windows_agent_runtime'](plan['shards'][4])
except ValueError as error:
    assert 'exact integration-3 target' in str(error), str(error)
else:
    raise AssertionError('missing dedicated target was accepted')
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
for harness in ('project', 'project_candidate'):
    git_metadata = copy.deepcopy(metadata)
    git_metadata['packages'][0]['targets'][-1]['name'] = harness
    git_plan = router['plan'](git_metadata)
    git_shard = next(shard for shard in git_plan['shards']
                     if any(row['name'] == harness for row in shard['targets']))
    selected_git = []
    def select_git(environment):
        selected_git.append(True)
        environment['SEMAPRAX_TEST_GIT'] = sys.executable
    with patch('sys.platform', 'darwin'), patch.dict(
        router['main'].__globals__, {'macos_test_git': select_git}
    ), patch('subprocess.run', side_effect=[
        subprocess.CompletedProcess([], 0, stdout=json.dumps(git_metadata)),
        subprocess.CompletedProcess([], 0),
    ]) as run:
        with contextlib.redirect_stdout(io.StringIO()):
            assert router['main'](['--label', 'Rust macOS', '--shard', git_shard['name']]) == 0
        assert selected_git == [True], harness
        assert run.call_args_list[1].kwargs['env']['SEMAPRAX_TEST_GIT'] == sys.executable
        assert run.call_args_list[1].args[0][-2:] == ['--', '--test-threads=1']
with patch('subprocess.run', side_effect=[
    subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
    subprocess.CompletedProcess([], 0),
    subprocess.CompletedProcess([], 0),
]) as run:
    with contextlib.redirect_stdout(io.StringIO()):
        assert router['main'](['--shard', 'unit-heavy']) == 0
    assert [call.args[0][7] for call in run.call_args_list[1:]] == list(router['HEAVY_UNIT_FILTERS'])
with patch('subprocess.run', side_effect=[
    subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
    subprocess.CompletedProcess([], 0),
]) as run:
    with contextlib.redirect_stdout(io.StringIO()):
        assert router['main'](['--shard', 'unit']) == 0
    command = run.call_args_list[1].args[0]
    assert command[:len(plan['shards'][0]['command'])] == plan['shards'][0]['command']
    assert all(command[command.index(test_filter) - 1] == '--skip' for test_filter in router['HEAVY_UNIT_FILTERS'])
repair_filter = 'source_live_cli::repair::tests::'
for platform, label, dedicated in (
    ('linux', 'Rust Linux', True),
    ('darwin', 'Rust macOS', True),
    ('win32', 'Rust Windows', True),
    ('linux', 'MSRV', False),
):
    with patch('sys.platform', platform), patch('subprocess.run', side_effect=[
        subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata)),
        subprocess.CompletedProcess([], 0),
    ]) as run:
        with contextlib.redirect_stdout(io.StringIO()):
            assert router['main'](['--shard', 'unit', '--label', label]) == 0
        command = run.call_args_list[1].args[0]
        assert (repair_filter in command) == dedicated, (platform, label, command)
        if dedicated:
            assert command[command.index(repair_filter) - 1] == '--skip'
listing = ''.join(f'{repair_filter}{name}: test\n' for name in 'edcba') + '5 tests, 0 benchmarks\n'
select = router['repair_shard_names']
assert select(listing, 0, 1) == [repair_filter + name for name in 'abcde']
shards = [select(listing, index, 2) for index in range(2)]
assert shards == [[repair_filter + n for n in 'ace'], [repair_filter + n for n in 'bd']]
assert sorted(sum(shards, [])) == select(listing, 0, 1)
for index, count, message in ((2, 2, 'out of range'), (-1, 2, 'out of range'), (0, 0, 'out of range'), (5, 6, 'no case')):
    try:
        select(listing, index, count)
    except ValueError as error:
        assert message in str(error), str(error)
    else:
        raise AssertionError('invalid repair shard accepted')
for bad, message in (('', 'no case'), ('other::test: test\n', 'escaped')):
    try:
        select(bad, 0, 1)
    except ValueError as error:
        assert message in str(error), str(error)
    else:
        raise AssertionError('invalid repair listing accepted')
with patch('subprocess.run', side_effect=[
    subprocess.CompletedProcess([], 0, stdout=listing),
    subprocess.CompletedProcess([], 101),
]) as run:
    with contextlib.redirect_stdout(io.StringIO()):
        assert router['main'](['--label', 'Rust source repair', '--repair-shard', '1/2']) == 101
    assert run.call_args_list[0].args[0] == router['REPAIR_TEST'] + [repair_filter, '--', '--list', '--format', 'terse']
    assert run.call_args_list[0].kwargs['check'] is True
    assert run.call_args_list[1].args[0] == router['REPAIR_TEST'] + ['--', '--exact', '--test-threads=1', *shards[1]]
for arguments in (['--repair-shard', '1'], ['--repair-shard', 'a/2']):
    with patch('subprocess.run') as run:
        try:
            router['main'](arguments)
        except ValueError as error:
            assert 'not <index>/<count>' in str(error), str(error)
        else:
            raise AssertionError('malformed repair shard accepted')
        run.assert_not_called()
with patch('subprocess.run') as run:
    with contextlib.redirect_stderr(io.StringIO()):
        try:
            router['main'](['--repair-shard', '0/2', '--shard', 'unit'])
        except SystemExit as error:
            assert error.code == 2
        else:
            raise AssertionError('repair shard combined with a workspace shard')
    run.assert_not_called()
sys.stdout.buffer.write(router_log.getvalue().encode('utf-8'))
"#;
