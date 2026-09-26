# Classes and inheritance

Classes are records with methods — the only values that answer dot-calls.
Inheritance is single, explicit, and checked.

## Methods

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

- `self` is an explicitly typed first parameter (`self: Counter`).
- Methods typically return a changed **copy**; the original is untouched.
- Construction names every field: `Counter { value: 40 }`.
- Records have no methods: `point.get()` fails (`SPX-T203`) — call a free
  function `get(point)` or use a class.

## Inheritance

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

- `class Dog : Animal` inherits fields and methods. Construction names
  **all** fields, inherited ones included.
- `super.speak()` dispatches to the parent implementation.
- A subclass value **is** its parent type: `let a: Animal = d;` upcasts, and
  calls through `a` use the parent's view.
- Overriding replaces the method for the subclass; the parent's other
  methods are inherited unchanged.

## Best practices

1. **Records + free functions by default; classes for behavior.** If the
   methods genuinely belong to the value (counters, builders, handles),
   make it a class.
2. **Keep hierarchies shallow.** One level of inheritance covers most designs;
   deeper trees get hard to construct (every field, every level) and hard
   to query.
3. **Prefer composition for reuse, inheritance for substitution.** Inherit
   when callers should accept the parent type; otherwise hold a field.

Exact rules: [Class Inheritance v1](https://github.com/wavect/semaprax/blob/main/docs/CLASS-INHERITANCE-V1.md).
