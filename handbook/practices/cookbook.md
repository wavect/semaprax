# Cookbook

Copy-paste recipes for the jobs every program needs. Each is a complete,
runnable module — save it as a `.spx` file and `run` it.

## Print a greeting

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

`stdout_write` returns the byte count — assert it so a truncated write fails
loudly instead of printing half a line.

## Sum digits with a loop

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
(`Result::Ok`). Callers handle both arms — the compiler enforces it.

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
every field.

## Count bytes in a string

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
