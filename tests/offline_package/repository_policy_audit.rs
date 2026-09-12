use std::path::Path;
use std::process::Command;

/// `scripts/audit-repository-policy.py` is the read-only script issue #169
/// asks for: it compares the live rules GitHub evaluates for `main`
/// (`rules/branches/main`) against the ruleset
/// `docs/CI-REQUIRED-CHECKS-V1.md`'s "Proposed rule" table documents as the
/// desired state, and must distinguish three states that call for different
/// maintainer action: not yet applied (exit 2), applied but drifted from the
/// documented rule types or required context (exit 1, the drift this script
/// exists to catch automatically), and matching (exit 0). This test drives
/// `audit()` and `main()` directly over synthetic rule lists, so it needs no
/// network access and no GitHub credentials.
const POLICY_AUDIT_CHECKS: &str = r#"
import contextlib
import io
import runpy

audit = runpy.run_path('scripts/audit-repository-policy.py')


def run(rules, required_context='Release gate'):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = audit['main'](
            ['--required-context', required_context],
            fetch=lambda endpoint: rules,
        )
    return code, out.getvalue(), err.getvalue()


DESIRED = [
    {'type': 'deletion'},
    {'type': 'non_fast_forward'},
    {
        'type': 'required_status_checks',
        'parameters': {
            'required_status_checks': [
                {'context': 'Release gate', 'integration_id': 15368}
            ],
            'strict_required_status_checks_policy': False,
        },
    },
]

# Not yet applied: no rule at all. This is the state the live repository is
# in as of this writing -- it must be reported distinctly from both "matches"
# and "drifted", not folded into either.
code, out, err = run([])
assert code == 2, (code, out, err)
assert 'DRIFT' not in err, err
assert 'not yet applied' in out, out

# Fully matches the documented desired state.
code, out, err = run(DESIRED)
assert code == 0, (code, out, err)
assert err == '', err
assert 'matches the documented ruleset' in out, out

# Applied, but missing the force-push rule -- the single highest-value rule
# per the doc's "Proposed rule" table. Must be reported, not silently ignored.
missing_non_fast_forward = [r for r in DESIRED if r['type'] != 'non_fast_forward']
code, out, err = run(missing_non_fast_forward)
assert code == 1, (code, out, err)
assert 'non_fast_forward' in err, err

# Applied, required_status_checks present, but pointed at the wrong context --
# exactly the failure mode a matrix-label rename could cause silently.
wrong_context = [
    {'type': 'deletion'},
    {'type': 'non_fast_forward'},
    {
        'type': 'required_status_checks',
        'parameters': {'required_status_checks': [{'context': 'Some other check'}]},
    },
]
code, out, err = run(wrong_context)
assert code == 1, (code, out, err)
assert 'Release gate' in err, err

# A malformed (non-list) response must be reported as an audit finding, never
# silently read as "not yet applied" (exit 2) or "matches" (exit 0).
code, out, err = run({'unexpected': 'shape'})
assert code == 1, (code, out, err)
assert 'expected a JSON array' in err, err

# A non-dict entry in the rule list must not crash the audit; it is simply
# not counted toward any required rule type.
code, out, err = run([{'type': 'deletion'}, 'not-a-dict', {'type': 'non_fast_forward'}])
assert code == 1, (code, out, err)
assert 'required_status_checks' in err, err

# Token-shaped substrings in a fetch failure must never reach stdout/stderr.
def _raise(endpoint):
    raise RuntimeError('gho_abcdefghijklmnopqrstuvwxyz0123 leaked from gh api')


out, err = io.StringIO(), io.StringIO()
with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
    code = audit['main'](['--required-context', 'Release gate'], fetch=_raise)
assert code == 3, (code, out.getvalue(), err.getvalue())
assert 'gho_' not in err.getvalue(), err.getvalue()
assert 'REDACTED' in err.getvalue(), err.getvalue()

print('audit-repository-policy checks passed')
"#;

#[test]
fn policy_audit_distinguishes_unapplied_drifted_and_matching_states() {
    let output = Command::new("python3")
        .args(["-B", "-c", POLICY_AUDIT_CHECKS])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .output()
        .expect("python3 must run the policy-audit checks");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "audit-repository-policy checks passed"
    );
}

/// The script must stay read-only by construction, not merely by convention:
/// no argument list anywhere in it may request a mutating HTTP verb.
#[test]
fn policy_audit_script_issues_no_mutating_request() {
    let script = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/audit-repository-policy.py"),
    )
    .expect("audit-repository-policy.py must be readable");
    for mutating in [
        "--method POST",
        "--method PUT",
        "--method PATCH",
        "--method DELETE",
    ] {
        assert!(
            !script.contains(mutating),
            "audit-repository-policy.py must stay read-only; found {mutating}"
        );
    }
    assert!(
        script.contains("gh api"),
        "script should read through `gh api`, matching this repo's other CI-facing scripts"
    );
}
