# RTK 0.51.0 research for command.view/v1 (HP-09)

Everything below was observed against the real pinned binary on macOS arm64 (2026-10-04) with
`env -i`, a throwaway `HOME`, and `RTK_*` redirections. Source reading is the v0.51.0 tag
(`rtk-ai/rtk` commit `e001f77`, tarball via `gh api repos/rtk-ai/rtk/tarball/v0.51.0`). Tests in `test/`
re-prove the load-bearing claims against the binary; run them with `python3 -m unittest discover -s test`
(set `RTK_BIN` to another path; tests skip when the binary is absent).

## 1. Explicit pinned preparation (not an automatic install)

```
gh release download v0.51.0 -R rtk-ai/rtk -p rtk-aarch64-apple-darwin.tar.gz -p checksums.txt -D <dir>
shasum -a 256 rtk-aarch64-apple-darwin.tar.gz
  8817d8b71afc02ac8bf06eb24bcc41c306592ab735b68e8fee9db1ba0de7cb59   == checksums.txt entry
tar -xzf ... -C /private/tmp/claude-501/hp-tools/rtk-0.51.0/
shasum -a 256 rtk   -> 02866e65968c0359b19495a5dc2d3ec643a1553fdd1c83c0ad33a8ad33bac3c9  (Mach-O arm64, 8507616 bytes)
rtk --version       -> rtk 0.51.0
```

Not on PATH; `rtk init` never run; no file under the real `~/.claude`, shell rc or `~/Library` touched.
`rtk --version` and `rtk pipe` write nothing at all (verified with an empty HOME). License Apache-2.0.
Identity check used by the adapter: `--version` output is exactly `rtk 0.51.0` (the name `rtk` is shared with an
unrelated "Rust Type Kit" binary, whose version string differs).

## 2. Subcommands (from `rtk --help`, ~90 total)

Relevant: `git`, `ls`, `read`, `grep`, `rg`, `find`, `cargo`, `pytest`, `npm`, `test`, `err`, `summary`, `run`,
`proxy`, `rewrite`, `recall`, `pipe`, `config`, `telemetry`, `gain`, `hook`, `init`, `verify`, `trust`.

- `rtk <tool> ...` wrappers run the real tool and print a filtered view (`rtk git status|diff|log`, `rtk cargo test`,
  `rtk pytest`, `rtk ls`, `rtk rg`, `rtk grep`, `rtk find`, `rtk read`).
- `rtk err|test|summary <cmd...>` run any command and keep only errors / failures / a summary.
- `rtk run <argv>` and `rtk proxy <cmd>` run unfiltered (proxy only tracks). `rtk run -c <str> [--shell]` uses a shell.
- `rtk pipe [-f <filter>] [--passthrough]`: **reads stdin, applies a named filter, prints**. This is a genuine
  post-execution transform. Filters (`src/cmds/system/pipe_cmd.rs`): cargo-test|cargo, pytest, go-test, go-build, ctest,
  tsc, vitest, grep|rg, find|fd, git-log, git-diff, git-status, log, mypy, ruff-check, ruff-format, sqlfluff-lint,
  prettier, phpunit, pest|paratest|php-test, ecs, phpstan, pint. No `-f` auto-detects (identity if unknown).
- `rtk rewrite <string>`: prints the "RTK equivalent" **as a shell string**; exit 0 allow, 1 no equivalent, 2 deny,
  3 "no explicit allow rule" (observed 3 with an empty HOME for every rewritten command). It consults `~/.claude`
  permission rules, so it reads agent configuration; the adapter never calls it. Observed divergences that make it
  unusable as authority (all pinned in `test_wrapper.py`):
  `python -m pytest -q` -> `rtk pytest -q` (drops `python -m`), `head -n 3 a.txt` -> `rtk read a.txt --head-lines 3`
  (different tool), `cargo test --message-format=json` -> rewritten (machine output), `cargo test && git push` ->
  `rtk cargo test && rtk git push` (shell pipeline). For allowlisted argv, `rewrite` agrees with `["rtk", *argv]`.

## 3. Wrapper semantics (`rtk <argv>`)

Measured wrapped vs unwrapped (`test_wrapper.py`, repo of 40 files / 30 commits, fake `cargo` printing libtest format
because no cargo builds are allowed here; real `pytest`, `git`, `rg`, `ls`):

| family | success rc | nonzero rc | native stderr kept | notes |
| --- | --- | --- | --- | --- |
| git diff / log | equal (0) | equal (128 for bad rev) | yes | `--exit-code` rc 1 preserved |
| rg -n | equal 0 / 1 (no match) / 2 (bad regex) | equal | yes | |
| ls | equal | equal (1) | yes | |
| cargo test (fake) | equal 0 | equal 101 | partly | see below |
| pytest | equal 0 | equal 1 | n/a | |
| **find** | equal 0 | **rtk exits 0, no output, no stderr where `find nosuchdir` exits 1 and prints an error** | **no** | exit status lost: **no wrapper mapping for find** |

- Streams are not preserved: `rtk err|test|cargo` merge stdout+stderr, and print the filtered failure detail on
  **stderr** and the summary on **stdout** regardless of origin (fake cargo fail: assertion text was on native stdout,
  wrapped it is on stderr). "Stderr distinction" is therefore a property of native errors only (git/ls/rg messages survive).
- Signals: group-SIGINT/SIGTERM on `rtk cargo test` (fake sleeper) -> rtk dies by the same signal (rc -2 / -15, identical to
  the native process), child gone. Signal to rtk alone: rtk dies by that signal too; `rtk run` kept waiting for its child
  (hung until killed). `rtk err` prints `[FAIL] Command failed (exit code: 130)` on stdout when group-interrupted.
- Raw decoding is lossy and capped: line reader replaces invalid UTF-8 with U+FFFD; `RAW_CAP` = 10 MiB.
  `rtk recall --full` of a malformed-UTF-8 run returns `U+FFFD`, not the original bytes.
- `rtk <cmd>` resolves the program through `PATH` by bare name (`Command::new("git")`), so the host-authorized
  executable path is not what runs unless PATH resolution is pinned equal. The adapter returns `resolves_via: "PATH"`
  and refuses wrapper plans for argv[0] containing `/`.
- Savings measured (my own byte counts): `rtk git diff` 34980 -> 13967, `rtk rg -n` 77240 -> 10050, `rtk ls` on a
  2-file dir **12 -> 20 bytes (negative)**, `rtk read big.txt` 193890 -> 193890 (0).

## 4. Raw recovery (tee / recall) - the deciding finding

Config `[retriever] mode = sqlite|tee|disabled` (default **sqlite**), `rtk config recall <mode>`.
`rtk config` prints the defaults: `tee_on_success = false`, `max_entry_bytes = 10 MiB`, `max_entries = 200`,
`retention_days = 30`, `tee_max_file_size = 1 MiB`, `tracking.enabled = true`, `telemetry.enabled = false`.

- Store: `recall.db` (sqlite). Env **`RTK_RECALL_DB`** picks the file; **`RTK_TEE_DIR`** picks the legacy tee directory
  (tee mode only); **`RTK_DB_PATH`** picks the tracking `history.db`. With all three set and `HOME` empty, nothing was
  written under HOME (`find $HOME` empty; files only under the retention dir, mode 0600).
- Handles: `[full output: rtk recall <12 hex>]` or `[+N hidden: rtk recall <12 hex>]` printed in the view.
  `rtk recall <hash> [--full|--from N|--lines N|--grep RE]`, `rtk recall --list`. Recall never re-executes (counter test:
  3 recalls, command ran once).
- **When raw is stored** (src/core/tee.rs, confirmed by running):
  - sqlite mode, **failure only** (nonzero exit) and output >= `MIN_FAILURE_BYTES`; stdout and stderr are merged into
    one blob (`rtk err ./gen.sh 0` -> exit 0 -> **nothing stored**, planted ERROR line shown but 177 KB raw lost).
  - Plus per-filter "tail/elision" stores when a filter itself hides lines **and** emits a hint: `rg/grep` (one entry per
    elided file + one remainder entry; union of entries equals raw lines but **order is not preserved**), `ls`, `find`.
  - **Not stored on success**: `git diff` (34980 -> 13967, no hint, `recall --list` empty), `git log`, `git status`,
    `cargo test`, `rtk test`, `rtk err`, `rtk summary`, `rtk read`.
  - tee mode with `tee_on_success = true` would archive successes, but the setting lives only in `config.toml`.
- **Config cannot be redirected without changing HOME**: path is `dirs::config_dir()/rtk/config.toml`
  (`$HOME/Library/Application Support/rtk` on macOS; `$XDG_CONFIG_HOME` or `~/.config` on Linux); there is no
  `RTK_CONFIG`. Setting HOME for the wrapped process would also change cargo/git/pip behaviour, and editing the user's
  config is forbidden, so success retention for wrappers is not obtainable. **=> every wrapper route fails the
  "raw recoverable without re-execution" rule for successful runs and defaults to bypass.**

## 5. `rtk pipe` as the primary route

`rtk pipe -f <filter>` sees only bytes the host already captured, so exit status, signals, stderr identity, execution
count and raw retention are all host-owned (the host retains the streams; adapter `view` takes `stdout_b64|stdout_path`).
Facts: invalid UTF-8 on stdin -> `rtk: Failed to read stdin: stream did not contain valid UTF-8`, exit 1 (the adapter
decodes lossily first and reports `lossless=false`, `omissions>=1`); input >10 MiB refused; a "never worse" guard
(estimated tokens = bytes/4) returns raw if the filter would enlarge; no tracking/recall writes. Filters are
format-specific: `git-diff` on `git diff --name-only` output returns **empty** (data loss), `git-log` on `--format=%H`
indents lines, `git-status` is ~0% on human output (expects porcelain), `grep` needs `file:line:content`
(-n). The adapter's allowlist therefore pins exact flag shapes and bypasses everything else.

Measured (adapter `view`, bytes counted by the tests, `min_bytes=0`): git diff 34980 -> 4711; git log 3852 -> 147;
rg -n 77240 -> 17242; grep -rn 77240 -> 17242; find 520 -> 148; fake cargo test 8499 -> 193; pytest (400 ok, 1 failing)
1312 -> 552; **negative: tiny `rg -n` stdout + stderr 56 -> 66 bytes** (stream-distinction header). Planted failures
(`CRITICAL-PLANTED-7731`, `CRITICAL-PY-9921`) survive; a planted error line inside 6000 lines of stderr noise
(`CRITICAL-STDERR-55`) was dropped by `cargo-test` and is re-attached by the adapter's critical-line preserve step.

## 6. Telemetry and tracking

- Telemetry: off by default (`consent: never asked`); needs `rtk telemetry enable`; **`RTK_TELEMETRY_DISABLED=1`**
  blocks it regardless. Adapter sets it on every rtk process.
- Tracking: wrapper commands write `history.db` (`tracking.enabled = true` default) under
  `$HOME/Library/Application Support/rtk` unless `RTK_DB_PATH` is set. `rtk gain` reports its byte/token estimates; these
  are vendor estimates only. `RTK_NO_TOML=1` disables TOML filters (project `.rtk/filters.toml`); the adapter sets it.
- Other env: `RTK_RECALL=0|RTK_TEE=0` disable recovery, `RTK_HOOK_AUDIT=1`, `RTK_REWRITE_HOST=<agent>` (turns a default
  "ask" rewrite exit 3 into 0), `RTK_DISABLED=1 <cmd>` is the hook-bypass prefix (shell syntax -> bypass).

## 7. Hook installation and read-only detection

`rtk init` (never run here) installs: Claude Code `$CLAUDE_CONFIG_DIR|~/.claude/settings.json`
`hooks.PreToolUse[matcher Bash].hooks[].command = "rtk hook claude"` (legacy `~/.claude/hooks/rtk-rewrite.sh`, plus
`RTK.md` and an `@RTK.md` line in `CLAUDE.md`); Codex `~/.codex/hooks.json` (`rtk hook codex`); Cursor `~/.cursor`
(`rtk hook cursor`); Factory Droid `~/.factory/hooks.json`; Antigravity `.agents/plugins/rtk/hooks.json` or
`~/.gemini/config/plugins/rtk/hooks.json`; Vibe `~/.vibe/hooks.toml`; OpenCode `~/.config/opencode/plugins/rtk.ts`;
Pi/OMP `~/.pi/agent/extensions/rtk.ts`; Hermes `~/.hermes/plugins/rtk-rewrite/`; Gemini `~/.gemini`; Copilot
`.github/hooks/rtk-rewrite.json`; Cline/Windsurf rules files (prompt-level). Detection is a read of those files; the
adapter reads none of them (no ambient authority). A host holding an explicit grant passes the text to
`hook_detect.detect_rtk_hook` or sets `external_hooks` / `lineage:["rtk"]` in `plan`, which then bypasses with
`external-hook-owns-rewrite`.

## 8. HN-12 findings: automatic use at development boundaries (rtk 0.51.0, 2026-10-04)

Measured with the real binary through the adapter and `rtk pipe`; synthetic output is stated as synthetic.

- **Version qualification.** `rtk_families.QUALIFIED_VERSIONS` is an explicit table (`{"0.51.0": FAMILIES}`).
  `plan` bypasses with `rtk-version-unqualified` and `view` reports `unavailable`/`rtk-version` for any other version,
  including a newer one, until a row is added together with re-measured tests for every family it lists. The
  descriptor's `upstream.versions` stays the host-side qualifier; the two must change together.
- **Truncated multiline failures.** The `cargo-test` pipe filter cuts long failure blocks (a nested `left:`/`right:`
  panic message loses its tail; only a line matching the critical regex was previously re-attached). The adapter now
  restores the whole `---- t stdout ----` (libtest) or `___ t ___` (pytest) block when any of its lines is absent from
  the view (at most 5 blocks, 60 lines and 6000 bytes), then re-attaches remaining critical lines. Tests:
  `test_nested_multiline_failure_block_is_kept_whole`, long paths/negations, Unicode, critical stderr, totals,
  no-match (rg exit 1, empty stdout stays empty and lossless) and error status (rg exit 2, find on a missing dir).
- **Admitted: `ctest`** (bare `ctest` only; `--output-on-failure` matches the machine-flag rule and `-V`/`-j` are
  unverified): 10494 -> 83 bytes on 150 passes + 1 failure with the failing test name and `1 failed` kept. No `ctest`
  binary exists here, so this family is proven on synthetic output through the real filter only.
- **Not admitted (measured, kept bypassed):** `go test` (the `go-test` filter expects `-json` events: plain output
  became `Go test: No tests found`, i.e. total data loss); `mypy` (identity, 12524 -> 12524, no gain); `ruff check`
  (the filter expects JSON, elides the tail: the planted `E999` line and the summary were lost); `tsc` (identity,
  6642 -> 6642, no gain).
- **Delivered-to-model accounting** is host-side (`command_view::measure`): the count covers the text the model is
  shown plus its recovery reference, compared with the host's own raw view; a view that does not shrink is replaced by
  raw. Vendor `rtk gain` figures are never used.
