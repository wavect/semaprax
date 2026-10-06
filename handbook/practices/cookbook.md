# Cookbook

Copy-paste recipes. After this page you can do the common jobs: print text,
loop, match, handle errors, and run the everyday project commands. Language
recipes are complete modules: save one as `app.spx` and `semaprax run app.spx`.
A program that exits `0` prints `0` after its own output.

## Print a greeting

<!-- handbook-smoke: {"stdout":"hello, world0\n"} -->
```semaprax
module app.greet;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let greeting = string_concat("hello, ", "world");
    let view = string_as_str(greeting);
    let written = stdout_write(str_as_bytes(view));
    if written == 12usize { 0 } else { 1 }
}
```

`stdout_write` returns the byte count. Assert it so a truncated write fails
loudly. The module needs `permit { process.stdout.write }` and `uses` on `main`
exactly as above (`SPX-E101`, `SPX-E102` otherwise).

## Sum digits with a loop

<!-- handbook-smoke: {"stdout":"0\n"} -->
```semaprax
module app.sum;

@id("flow.digit_sum")
fn digit_sum(value: i64) -> i64
    requires value >= 0
    ensures result >= 0
{
    let mut remaining = value;
    let mut total = 0;
    while remaining > 0 {
        total = total + remaining % 10;
        remaining = remaining / 10;
        remaining > 0
    }
    total
}

@id("app.main")
fn main() -> i64
{
    if digit_sum(98765) == 35 { 0 } else { 1 }
}
```

The loop pattern: `let mut` state, `while` with scalar updates, continuation
condition as the body's last line, pure scalar result.

## Classify with match

<!-- handbook-smoke: {"stdout":"0\n"} -->
```semaprax
module app.sign;

@id("flow.classify")
fn classify(value: i64) -> i64
{
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, _ => 1, }
}

@id("app.main")
fn main() -> i64
{
    if classify(-2) == -9 { 0 } else { 1 }
}
```

Order arms from specific to general; the final `_` catch-all is mandatory.

## Safe division with Result

```semaprax
module app.divide;

@id("data.checked_div")
fn checked_div(left: i64, right: i64) -> Result<i64, i64>
{
    if right == 0 { Result<i64, i64>::Err { error: 1 } } else { Result<i64, i64>::Ok { value: left / right } }
}

@id("app.main")
fn main() -> i64
{
    let ok = match checked_div(8, 2) { Result::Ok { value: v } => v, Result::Err { error: code } => code, };
    let err = match checked_div(8, 0) { Result::Ok { value: v } => v, Result::Err { error: code } => code, };
    if ok == 4 && err == 1 { 0 } else { 1 }
}
```

Construct **with** type arguments (`Result<i64, i64>::Ok`), match **without**
(`Result::Ok`). Callers handle both arms; the compiler enforces it. The
interpreter does not admit this program (`SPX-F102`), so run it with
`semaprax run app.spx --native`.

## Update a record immutably

```semaprax
module app.move_point;

@id("data.point")
record Point {
    @id("data.point.x")
    x: i64,
    @id("data.point.y")
    y: i64,
}

@id("app.main")
fn main() -> i64
{
    let origin = Point { x: 1, y: 2 };
    let moved = origin with { y: 10 };
    if moved.x == 1 && moved.y == 10 { 0 } else { 1 }
}
```

`with` builds a new value; the original is untouched. Construction must name
every field. Run it with `--native` (`SPX-F102` in the interpreter).

## Count bytes in a string

<!-- handbook-smoke: {"stdout":"0\n"} -->
```semaprax
module app.count;

@id("bytes.count_a")
fn count_a(text: borrow str) -> usize
{
    let view = str_as_bytes(text);
    let length = byte_len(view);
    let mut index = 0usize;
    let mut hits = 0usize;
    while index < length {
        hits = match byte_get(view, index) { Option::Some { value: byte } => if byte == 97u8 { hits + 1usize } else { hits }, Option::None {} => hits, };
        index = index + 1usize;
        index < length
    }
    hits
}

@id("app.main")
fn main() -> i64
{
    let word = "banana";
    if count_a(string_as_str(word)) == 3usize { 0 } else { 1 }
}
```

The byte pattern: borrow the string, view it as bytes, walk with `usize`
indices, destructure `byte_get`'s `Option<u8>`. Byte `97u8` is `'a'`.

## Start a project and run its checks

```sh
semaprax new my-app && cd my-app
semaprax fmt . && semaprax check . && semaprax test . && semaprax run .
```

`fmt` first, `check` second. Stop at the first failure.

## Add a test

Add a function to your test module and list that module under `tests`:

```text
@id("my_app.tests.test_add")
fn test_add() -> i64
{
    if add(2, 2) == 4 { 0 } else { 1 }
}
```

`semaprax test .` runs every zero-argument `test_*` function and names failures.

## Add a dependency

```sh
semaprax add . std.num "^0.1.0"
semaprax help library std.num      # exact signatures and stable IDs
```

## Find who calls a function

```sh
semaprax query . --calls my-app.add          # callers
semaprax context . my-app.add --direction both --depth 1 --max-bytes 4096
```

## Rename safely

```sh
semaprax change preview . rename-display-name my-app.add sum
```

Read the preview. It writes nothing. See [Shipping](../projects/shipping.md#change-with-review).

## Ship a web package

```sh
semaprax build . --target web -o dist/web
semaprax lock . --write
```

## Gate CI on interface breaks

```sh
semaprax fmt . --check && semaprax check . && semaprax test .
semaprax lock . --compare base.lock      # exits 1 when breaking
```

## Run a network command offline

```sh
semaprax network-run . --fixture http.fixture.json --arg https://example.test/
```

This uses a recorded fixture; no real connection opens. See
[Profiles](../projects/profiles.md#command-io).
