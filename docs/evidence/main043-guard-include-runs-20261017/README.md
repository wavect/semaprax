# Strict additive include runs

The original Main043 line-hunk parser refuses all three actual a097 inventories: multiple separated include insertions share one long physical line. This repair changes only `fragments` in `guard-extensions.py`. The complete old script is preserved under `before/`; `verify_guard`, required/maximum checks, unique-fragment checks, record binding, original controls and main entry remain byte-exact.

The replacement lexes complete admitted include macros as indivisible tokens. Every other non-whitespace character is an exact original anchor. Between anchors, old whitespace must remain byte-exact after removing one complete added include run. The longest exact retained prefix chooses whitespace ownership deterministically. No whitespace/content normalization, line-hunk projection, original token removal/reorder, source rewriting, or generic source adapter occurs. Every fragment must satisfy the unchanged include grammar and occur exactly once, and removing all fragments must restore the WHOLE original string exactly. The existing outer verifier repeats full restoration and all required/maximum/foreign checks.

Against exact a097 raw files and a82e original files, all three whole originals restore exactly:6/7/5 unique runs containing89/124/94 includes. Closure agent's exact ef3c allowed manifest admits those counts with no missing or extra paths. This branch does not edit that manifest or any current authority bindings.

Source-only controls passed:9 positive cases (multiple separated same-line runs, different whitespace ownership, Unicode, LF/CRLF);13 synthetic hostile edits;18 mutations of the actual complete raw guards;3 independently exercised maximum-count violations; and the unchanged original controls'9 missing/duplicate/foreign refusals. Tests also reproduce all3 old parser failures. Original controls run with exact raw checkout files but generated binding records in a disposable SSD unit fixture; no common authority or checkout metadata is refreshed. `proof.json` binds both script versions and the new controls. No Cargo or Rust test/build ran.

Run after integrating the separately owned allowed/recipe changes:

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 /opt/homebrew/bin/python3 docs/evidence/main043-guard-include-runs-20261017/controls.py --output /Volumes/Semaprax/provenance/main043-include-runs-next-controls
```

The recorded private run adds `--allowed-ref ef3c5b3c117fd01f3121ec0199d0566cbfb5202f` to bind the closure agent's independently reviewed manifest/recipe bytes. Parent must integrate this parser before its one common authority refresh. These source checks are not current-head compiled evidence.
