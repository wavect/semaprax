# Lint repairs after 9211 current-head CI

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

[Build job 117568211080](https://github.com/wavect/semaprax/actions/runs/39091075618/job/117568211080) reported an unused certified-status re-export, two cloned-reference slice constructions, and an identity conversion of an existing string slice. The status aliases at both receiving layers now exist only for their historical test callers. The certified receiver body remains unchanged and compiled. The arena builder still clones every inserted key; only two redundant outer temporary clones are removed. The hostile schema string value is identical.

`edits.json` records exact whole-source inverses after formatting. Every original assertion and measurement closure is unchanged. Source metadata retains all old current views, 444 logical rows, 121 successor subjects and 268 owner records; reader caps and compiler budgets are unchanged. The complete original source-role verifier and all six hostile controls, 67 strict/bootstrap refusals, ten successor controls and five browser controls are source-only evidence. Full current-head compiler/CI, browser and Lean gates remain required.
