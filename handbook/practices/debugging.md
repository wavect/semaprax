# Debugging diagnostics

A diagnostic has a stable `SPX-...` code, a message, a `file:line:column`
location and usually a `help:` line with the fix. After this page you can find
the cause of an error and the smallest fix for it. Match on the code, not the
wording: wording changes, codes do not.

```text
error[SPX-P106]: `return` is not admitted; a block's value is its final expression at bad.spx:6:5
  help: delete `return` and the trailing `;` so the value is the block's last expression
```

## The loop

1. `semaprax fmt <file>`. Many "errors" are layout the formatter fixes.
2. `semaprax check <file>`. Add `--json` for one JSON object per diagnostic
   (`code`, `severity`, `message`, `path`, `location`, `help`).
3. Fix the first diagnostic at its location. Later ones are often knock-on.
4. Still unclear? Look up the code (case-sensitive):

```sh
semaprax help diagnostic SPX-T208     # what you wrote, and the fix
semaprax help diagnostic codes        # every indexed code
semaprax explain SPX-T208 [--json]    # the installed explanation
```

Exit codes: `1` for compile or run failures, `2` for a bad command line. A
warning such as `SPX-S103` (a function with no `@id`) still exits `0`.

## Check the project, not one file

A module without `main` is a library module. Checking it alone fails with
`SPX-T105`, `SPX-G172` or similar. Run `semaprax check .` from the project.
Single-file commands (`run`, `doc`, `properties`, `package report`, ...) need a
file with `fn main() -> i64`.

## Apply a one-step repair

Some fixes have a plan. `fix --plan` lists the repair operations your binary
knows. For a function missing its `@id` (`SPX-S103`):

```sh
semaprax fix --plan
semaprax fix app.spx assign-function-id <automatic-function-id> --plan
semaprax repairs app.spx assign-function-id <automatic-function-id>
semaprax repair app.spx <repair-id> --persistent-id my.namespace.main
```

`--plan` changes nothing. `repair` is the step that writes source.

## The top fixes

| You wrote | Code | Fix |
| --- | --- | --- |
| `return 42;` | `SPX-P106` | Tail expression: `42` |
| `else if` | `SPX-P106` | `else { if ... }` |
| `while` body ending in an assignment | `SPX-P203` | End with the continuation condition |
| `for i in 0..n` | `SPX-P106` | `while` with a `let mut` counter |
| `f(x);` as a statement | `SPX-P106` | `let _ = f(x);` |
| `let t = (1, 2);` | `SPX-P106` | No tuples; declare a `record` |
| `i = i + 1` on an immutable `i` | `SPX-U101` | `let mut i = ...` |
| `let x = 1; let x = ...` | `SPX-T209` | No shadowing; new name |
| `index + 1` with `index: usize` | `SPX-T208` | `index + 1usize` (no mixed types) |
| `let a: i32 = 5` | `SPX-T232` | `5i32` (literals default to `i64`) |
| `"a" + "b"` | `SPX-T250` | `string_concat("a", "b")` |
| `Some(1)` / `None` | `SPX-T203` | `Option<i64>::Some { value: 1 }` / `Option<i64>::None {}` |
| `Some(b) =>` in a pattern | `SPX-P106` | `Option::Some { value: b } =>` |
| `f("abc")` for `borrow str` | `SPX-T205` | Bind, then `f(string_as_str(s))` |
| `string_as_str("lit")` | `SPX-T266` | Bind the literal first |
| `point.get()`, `s.len()` | `SPX-T203` | Only classes have methods: `get(point)`, `string_len(s)` |
| Second use after an `own` move | `SPX-O101` | Callee takes `borrow`, or pass a fresh value |
| `fn main() -> bool` | `SPX-T104` | `main` returns `i64`; `0` is success |
| Missing `permit` or `uses` | `SPX-E101`, `SPX-E102` | Declare the effect at module and function level |
| Last field or arm without `,` | `SPX-P106` | Trailing comma everywhere |
| Non-canonical manifest | `SPX-J100` | `help` names the first differing line |

`SPX-F102` (interpreter does not admit the program) is not a source error: use
`run --native`.

## Environment and command errors

| You see | Do this |
| --- | --- |
| `SPX-B101 failed to start clang` | Install Clang and put it on `PATH`. |
| `SPX-I001 cannot read ...` | Wrong path or working directory. |
| `SPX-I307` | The build output path exists. Choose a new one. |
| `SPX-J102 cannot inspect ... semaprax.toml` | No manifest in that directory. |
| `SPX-J102` on `fmt` | A path alias. Use the real path. |
| `unknown command ...; did you mean ...?` | Use the suggested command. |
| `doctor`: `failed profile: an explicit offline profile is required` | Expected. Pass `--profile <id>`. See [Targets](../projects/targets.md#check-the-environment). |

Every rejected command prints `hint: run semaprax <command> --help`. Use it.

## Ask a smaller question

```sh
semaprax help language topics                  # then: help language ownership
semaprax help shapes record                    # smallest declaration example
semaprax help library std.core.compare         # one stdlib entry (~200 bytes)
semaprax skills get language                   # packaged language guidance
```

The full card is the
[Agent quick reference](https://github.com/wavect/semaprax/blob/main/docs/AGENT-QUICK-REFERENCE.md),
printed offline by `semaprax help language`. The [diagnostics reference](../reference/diagnostics.md)
lists codes by family.
