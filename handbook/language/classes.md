# Classes and inheritance

After this page you can attach methods to data with a `class`, extend a class
with a subclass, and bind a protocol to a record. Classes are the only values
that answer `value.method()` calls.

Use a [record](types.md) and free functions by default. Use a class when the
methods belong to the value, such as a counter or a builder.

The examples on this page build records and classes, so run them with
`semaprax run file.spx --native`. `check` needs no flag.

## Add methods

<!-- native-checked: {"stdout":"42\n"} -->
```semaprax
module example.counter;

@id("example.counter")
class Counter {
    @id("example.counter.value")
    value: i64,

    @id("example.counter.get")
    fn get(self: Counter) -> i64
{
        self.value
    }

    @id("example.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter
{
        Counter { value: self.value + amount }
    }
}

@id("app.main")
fn main() -> i64
{
    let base = Counter { value: 40 };
    let next = base.bumped(2);
    if next.value == 42 && base.get() == 40 { next.value } else { 0 }
}
```

- The first parameter is `self: Counter`, written out.
- A method usually returns a changed copy. `base` stays at 40.
- A class literal names every field. A field changes with `counter.value = …`
  on a `let mut` binding.
- Records have no methods: `point.get()` is `SPX-T203`. Calling a method on a
  number or string is the same error: use `string_len(s)`.

## Extend a class

<!-- native-checked: {"stdout":"6\n"} -->
```semaprax
module example.inheritance;

@id("example.animal")
class Animal {
    @id("example.animal.legs")
    legs: i64,

    @id("example.animal.speak")
    fn speak(self: Animal) -> i64
{
        self.legs
    }
}

@id("example.dog")
class Dog : Animal {
    @id("example.dog.bark_count")
    bark_count: i64,

    @id("example.dog.speak")
    fn speak(self: Dog) -> i64
{
        super.speak() + self.bark_count
    }
}

@id("example.main")
fn main() -> i64
{
    let d = Dog { legs: 4, bark_count: 2 };
    let a: Animal = d;
    if d.speak() == 6 && a.speak() == 4 { d.speak() } else { 0 }
}
```

- `class Dog : Animal` inherits the fields and methods of `Animal`. A `Dog`
  literal names all fields, inherited ones too.
- `super.speak()` calls the parent's method.
- A `Dog` is an `Animal`: `let a: Animal = d;` converts it, and calls through
  `a` use the parent's methods.
- A method with the same name overrides the parent's for the subclass.

Keep the tree shallow. Every extra level adds fields to every literal. To reuse
behavior without substituting types, hold the other value in a field.

## Bind a protocol to a record

A `protocol` lists the functions a type must have. An `impl` binds each one to
a function you already wrote, by `@id`. The compiler checks that every required
function is bound exactly once. The binding is checked and then erased: it
adds no dispatch, no protocol value, and no runtime cost.

<!-- native-checked: {"stdout":"7\n"} -->
```semaprax
module geometry.app;

@id("geometry.read-x")
protocol ReadX {
    @id("geometry.read-x.get")
    fn get(self: Self) -> i64;
}

@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
}

@id("geometry.point.read-x")
impl "geometry.read-x" for "geometry.point" {
    "geometry.read-x.get" = "geometry.point.get";
}

@id("geometry.point.get")
fn get(point: Point) -> i64
{
    point.x
}

@id("app.main")
fn main() -> i64
{
    get(Point { x: 7 })
}
```

The receiver must be a local record with an `@id`. Bound functions are
top-level, non-generic, and not `main`. Projects import protocols with
`use protocol @id("…") from module as name;`.

Exact rules: [Class Inheritance v1](https://github.com/wavect/semaprax/blob/main/docs/CLASS-INHERITANCE-V1.md),
[Static Protocol Conformance v1](https://github.com/wavect/semaprax/blob/main/docs/STATIC-PROTOCOL-CONFORMANCE-V1.md).
