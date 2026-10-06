# See Semaprax in action

You will see a file checked and run, queried by meaning, and a project tested
and built for the web. Run the commands from a repository checkout, or copy
the examples from the [examples folder](https://github.com/wavect/semaprax/tree/main/examples).
[Install](install.md) first.

![Animated terminal walkthrough: check, run, query, test, build, and verify](../assets/demo/first-steps.gif)

The animation and still image replay [recorded output](../assets/demo/transcript.txt)
from an earlier release (v0.6.0). The commands below are unchanged.

## Check and run a file

```sh
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

`check` prints a source revision. `run` prints `42`.

![Terminal still showing Semaprax check and run output](../assets/demo/check-and-run.png)

## Ask what the compiler knows

```sh
semaprax query examples/meaning.spx
```

```text
function    math.add    fn add(left: i64, right: i64) -> i64
function    app.main    fn main() -> i64
```

`query` lists declarations by stable ID. Other views:

| Command | Answers |
| --- | --- |
| `semaprax doc examples/meaning.spx` | What are the contracts? |
| `semaprax context examples/meaning.spx math.add --depth 1` | What surrounds one declaration? |

[Agent workflow](../practices/agents.md) explains when to use each.

## Test a project and build for the web

```sh
semaprax test examples/calculator-project/semaprax.toml
semaprax build examples/calculator-project/semaprax.toml \
  --target web -o target/handbook-demo-web
```

Results: `project tests passed`, then `built project web package
target/handbook-demo-web`. The package holds `app.wasm`, JavaScript bindings,
TypeScript declarations, and an export descriptor. The output directory must
not exist yet; pick a new one when you rebuild.

With Node.js 22 or newer, call an export by stable ID:

```sh
node --input-type=module <<'JS'
import { readFile } from 'node:fs/promises';
import { instantiateBytes } from './target/handbook-demo-web/semaprax.bindings.js';

const runtime = await instantiateBytes(
  await readFile('target/handbook-demo-web/app.wasm')
);
console.log(runtime.call('calculator.add', 19n, 23n));
JS
```

```text
{ ok: true, value: 42n }
```

The `n` marks a JavaScript `BigInt`, used for `i64`. The result reports
failure as data instead of throwing.

## Next

[Write your first program](first-program.md), or
[scaffold a project](first-project.md).
