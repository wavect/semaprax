# Language shapes catalog

Status: generated from `examples/*.spx` through the `semaprax doc` documentation model by `tests/projections.rs::shapes_catalog`; edit the examples, then regenerate with `cargo test --locked -p semaprax --test projections -- --ignored shapes_catalog::regenerate_shapes_catalog`.

Audience: agents and humans writing SEMAPRAX declarations from an installed compiler.

Every shape below is the canonical header of a declaration in a committed, verified example, rendered by the same documentation model as `semaprax doc`, so the catalog cannot show a shape the compiler rejects. `semaprax help shapes` prints this document. The [agent quick reference](AGENT-QUICK-REFERENCE.md) explains the rules behind the shapes, and [Documentation Projection v1](DOC-PROJECTION-V1.md) owns the model. Identities are the examples' own `@id` attributes; bodies are omitted.

## Records

### `ledger.account` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.account")
record Account {
    @id("ledger.account.id")
    id: i64,
    @id("ledger.account.balance")
    balance: i64,
}
```

### `byte.type` (`examples/bytes_u8.spx`)

```semaprax
@id("byte.type")
record Sample {
    @id("byte.tag")
    tag: u8,
    @id("byte.weight")
    weight: i64,
}
```

### `glyph.type` (`examples/chars.spx`)

```semaprax
@id("glyph.type")
record Glyph {
    @id("glyph.symbol")
    symbol: char,
    @id("glyph.weight")
    weight: i64,
}
```

### `expr.pair` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.pair")
record Pair {
    @id("expr.pair.left")
    left: i64,
    @id("expr.pair.right")
    right: i64,
}
```

### `geometry.point` (`examples/field_mutation.spx`)

```semaprax
@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
    @id("geometry.point.y")
    y: i64,
    @id("geometry.point.enabled")
    enabled: bool,
}
```

### `geometry.vector` (`examples/floats.spx`)

```semaprax
@id("geometry.vector")
record Vector {
    @id("geometry.vector.x")
    x: f64,
    @id("geometry.vector.y")
    y: f64,
}
```

### `order.line` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.line")
record Line {
    @id("order.line.sku")
    sku: i64,
    @id("order.line.quantity")
    quantity: i64,
    @id("order.line.unit_price")
    unit_price: i64,
}
```

### `geometry.point` (`examples/records.spx`)

```semaprax
@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
    @id("geometry.point.y")
    y: i64,
    @id("geometry.point.enabled")
    enabled: bool,
}
```

### `geometry.line` (`examples/records.spx`)

```semaprax
@id("geometry.line")
record Line {
    @id("geometry.line.start")
    start: Point,
    @id("geometry.line.end")
    end: Point,
}
```

## Variants

### `ledger.tx_kind` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.tx_kind")
variant TxKind {
    @id("ledger.tx_kind.deposit")
    Deposit,
    @id("ledger.tx_kind.withdraw")
    Withdraw,
    @id("ledger.tx_kind.transfer")
    Transfer,
}
```

### `expr.op` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.op")
variant Op {
    @id("expr.op.add")
    Add,
    @id("expr.op.subtract")
    Subtract,
    @id("expr.op.multiply")
    Multiply,
    @id("expr.op.divide")
    Divide,
}
```

### `expr.unary` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.unary")
variant UnaryOp {
    @id("expr.unary.negate")
    Negate,
    @id("expr.unary.double")
    Double,
}
```

### `order.status` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.status")
variant Status {
    @id("order.status.created")
    Created,
    @id("order.status.paid")
    Paid,
    @id("order.status.shipped")
    Shipped,
    @id("order.status.delivered")
    Delivered,
    @id("order.status.cancelled")
    Cancelled,
}
```

### `order.decision` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.decision")
variant Decision {
    @id("order.decision.approve")
    Approve,
    @id("order.decision.reject")
    Reject,
    @id("order.decision.hold")
    Hold,
}
```

## Classes

### `ledger.portfolio` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.portfolio")
class Portfolio {
    @id("ledger.portfolio.holdings")
    holdings: i64,
    @id("ledger.portfolio.credits")
    credits: i64,

    @id("ledger.portfolio.total")
    fn total(self: Portfolio) -> i64

    @id("ledger.portfolio.deposited")
    fn deposited(self: Portfolio, amount: i64) -> Portfolio

    @id("ledger.portfolio.charged")
    fn charged(self: Portfolio, amount: i64) -> Portfolio
}
```

### `example.counter` (`examples/classes.spx`)

```semaprax
@id("example.counter")
class Counter {
    @id("example.counter.value")
    value: i64,

    @id("example.counter.get")
    fn get(self: Counter) -> i64

    @id("example.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter
}
```

### `example.counter` (`examples/field_mutation.spx`)

```semaprax
@id("example.counter")
class Counter {
    @id("example.counter.value")
    value: i64,

    @id("example.counter.get")
    fn get(self: Counter) -> i64
}
```

### `example.animal` (`examples/inheritance.spx`)

```semaprax
@id("example.animal")
class Animal {
    @id("example.animal.legs")
    legs: i64,

    @id("example.animal.speak")
    fn speak(self: Animal) -> i64

    @id("example.animal.name")
    fn name(self: Animal) -> string
}
```

### `example.dog` (`examples/inheritance.spx`)

```semaprax
@id("example.dog")
class Dog : Animal {
    @id("example.dog.bark_count")
    bark_count: i64,

    @id("example.dog.speak")
    fn speak(self: Dog) -> i64

    @id("example.dog.name")
    fn name(self: Dog) -> string
}
```

### `example.puppy` (`examples/inheritance.spx`)

```semaprax
@id("example.puppy")
class Puppy : Dog {
    @id("example.puppy.cuteness")
    cuteness: i64,

    @id("example.puppy.total")
    fn total(self: Puppy) -> i64
}
```

### `order.inventory` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.inventory")
class Inventory {
    @id("order.inventory.stock")
    stock: i64,
    @id("order.inventory.reserved")
    reserved: i64,

    @id("order.inventory.available")
    fn available(self: Inventory) -> i64

    @id("order.inventory.reserve")
    fn reserve(self: Inventory, amount: i64) -> Inventory

    @id("order.inventory.commit")
    fn commit(self: Inventory, amount: i64) -> Inventory
}
```

## Methods

### `ledger.portfolio.total` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.portfolio.total")
fn total(self: Portfolio) -> i64
```

### `ledger.portfolio.deposited` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.portfolio.deposited")
fn deposited(self: Portfolio, amount: i64) -> Portfolio
```

### `ledger.portfolio.charged` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.portfolio.charged")
fn charged(self: Portfolio, amount: i64) -> Portfolio
```

### `example.counter.get` (`examples/classes.spx`)

```semaprax
@id("example.counter.get")
fn get(self: Counter) -> i64
```

### `example.counter.bumped` (`examples/classes.spx`)

```semaprax
@id("example.counter.bumped")
fn bumped(self: Counter, amount: i64) -> Counter
```

### `example.counter.get` (`examples/field_mutation.spx`)

```semaprax
@id("example.counter.get")
fn get(self: Counter) -> i64
```

### `example.animal.speak` (`examples/inheritance.spx`)

```semaprax
@id("example.animal.speak")
fn speak(self: Animal) -> i64
```

### `example.animal.name` (`examples/inheritance.spx`)

```semaprax
@id("example.animal.name")
fn name(self: Animal) -> string
```

### `example.dog.speak` (`examples/inheritance.spx`)

```semaprax
@id("example.dog.speak")
fn speak(self: Dog) -> i64
```

### `example.dog.name` (`examples/inheritance.spx`)

```semaprax
@id("example.dog.name")
fn name(self: Dog) -> string
```

### `example.puppy.total` (`examples/inheritance.spx`)

```semaprax
@id("example.puppy.total")
fn total(self: Puppy) -> i64
```

### `order.inventory.available` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.inventory.available")
fn available(self: Inventory) -> i64
```

### `order.inventory.reserve` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.inventory.reserve")
fn reserve(self: Inventory, amount: i64) -> Inventory
```

### `order.inventory.commit` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.inventory.commit")
fn commit(self: Inventory, amount: i64) -> Inventory
```

## Resources

### `platform.token` (`examples/lifecycle.spx`)

```semaprax
@id("platform.token")
resource Token {
    @id("platform.token.drop")
    drop import "platform.token.finalize";
}
```

### `example.token` (`examples/native_callable.spx`)

```semaprax
@id("example.token")
resource Token {
    @id("example.token.drop")
    drop trivial;
}
```

### `buffer.type` (`examples/ownership.spx`)

```semaprax
@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
```

## Interfaces

### `platform.token.host` (`examples/lifecycle.spx`)

```semaprax
@id("platform.token.host")
interface TokenHost
    permits { platform.token.release }
{
    @id("platform.token.finalize")
    import fn finalize(token: own Token) -> unit
        effects { platform.token.release }
        failure infallible
        consumes token always;
}
```

## Functions

### `ledger.is_deposit` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.is_deposit")
fn is_deposit(kind: TxKind) -> bool
```

### `ledger.apply` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.apply")
fn apply(account: Account, kind: TxKind, amount: i64) -> Account
    requires amount >= 0
```

### `ledger.safe_withdraw` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.safe_withdraw")
fn safe_withdraw(account: Account, amount: i64) -> Result<i64, i64>
    requires amount >= 0
```

### `ledger.find_balance` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.find_balance")
fn find_balance(left: Account, right: Account, wanted: i64) -> Option<i64>
```

### `ledger.compound` (`examples/banking_ledger.spx`)

```semaprax
@id("ledger.compound")
fn compound(principal: i64, rate_percent: i64, years: i64) -> i64
    requires principal >= 0
    requires rate_percent >= 0
    requires rate_percent <= 100
    requires years >= 0
    requires years <= 10
```

### `app.main` (`examples/banking_ledger.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `byte.limit` (`examples/bytes_u8.spx`)

```semaprax
@id("byte.limit")
fn limit() -> u8
```

### `byte.make` (`examples/bytes_u8.spx`)

```semaprax
@id("byte.make")
fn make(tag: u8) -> Sample
```

### `byte.saturating_add` (`examples/bytes_u8.spx`)

```semaprax
@id("byte.saturating_add")
fn saturating_add(left: u8, right: u8) -> u8
    requires right <= limit()
```

### `app.main` (`examples/bytes_u8.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `calculator.add` (`examples/calculator.spx`)

```semaprax
@id("calculator.add")
fn add(left: i64, right: i64) -> i64
```

### `calculator.subtract` (`examples/calculator.spx`)

```semaprax
@id("calculator.subtract")
fn subtract(left: i64, right: i64) -> i64
```

### `calculator.multiply` (`examples/calculator.spx`)

```semaprax
@id("calculator.multiply")
fn multiply(left: i64, right: i64) -> i64
```

### `calculator.divide` (`examples/calculator.spx`)

```semaprax
@id("calculator.divide")
fn divide(left: i64, right: i64) -> i64
    requires right != 0
```

### `calculator.is-negative` (`examples/calculator.spx`)

```semaprax
@id("calculator.is-negative")
fn is_negative(value: i64) -> bool
```

### `calculator.not` (`examples/calculator.spx`)

```semaprax
@id("calculator.not")
fn not(value: bool) -> bool
```

### `app.main` (`examples/calculator.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `glyph.initial` (`examples/chars.spx`)

```semaprax
@id("glyph.initial")
fn initial() -> char
```

### `glyph.make` (`examples/chars.spx`)

```semaprax
@id("glyph.make")
fn make(symbol: char) -> Glyph
```

### `glyph.order` (`examples/chars.spx`)

```semaprax
@id("glyph.order")
fn order(left: char, right: char) -> i64
```

### `app.main` (`examples/chars.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `example.main` (`examples/classes.spx`)

```semaprax
@id("example.main")
fn main() -> i64
```

### `flow.choose` (`examples/control_flow.spx`)

```semaprax
@id("flow.choose")
fn choose(flag: bool, base: i64) -> i64
```

### `app.main` (`examples/control_flow.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `clock.logical_tick` (`examples/effects.spx`)

```semaprax
@id("clock.logical_tick")
fn logical_tick(value: i64) -> i64
    uses { clock.read }
    ensures result == value + 1
```

### `app.main` (`examples/effects.spx`)

```semaprax
@id("app.main")
fn main() -> i64
    uses { clock.read }
```

### `mut.accumulator` (`examples/explicit_mutation.spx`)

```semaprax
@id("mut.accumulator")
fn accumulator() -> i64
```

### `mut.checked_steps` (`examples/explicit_mutation.spx`)

```semaprax
@id("mut.checked_steps")
fn checked_steps() -> i64
```

### `main` (`examples/explicit_mutation.spx`)

```semaprax
@id("main")
fn main() -> i64
```

### `expr.is_divide` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.is_divide")
fn is_divide(op: Op) -> bool
```

### `expr.apply_unary` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.apply_unary")
fn apply_unary(value: i64, operation: UnaryOp) -> i64
```

### `expr.evaluate` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.evaluate")
fn evaluate(pair: Pair, op: Op) -> i64
```

### `expr.safe_divide` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.safe_divide")
fn safe_divide(left: i64, right: i64) -> Result<i64, i64>
```

### `expr.safe_evaluate` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.safe_evaluate")
fn safe_evaluate(pair: Pair, op: Op) -> Result<i64, i64>
```

### `expr.chain_scalars` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.chain_scalars")
fn chain_scalars(first: Pair, left_op: Op, second: Pair, right_op: Op, combine: Op) -> i64
```

### `expr.fibonacci` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.fibonacci")
fn fibonacci(nth: i64) -> i64
    requires nth >= 0
    requires nth <= 20
```

### `expr.fold_many` (`examples/expression_evaluator.spx`)

```semaprax
@id("expr.fold_many")
fn fold_many() -> i64
```

### `app.main` (`examples/expression_evaluator.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `fm.shift_x` (`examples/field_mutation.spx`)

```semaprax
@id("fm.shift_x")
fn shift_x(point: Point, step: i64) -> Point
```

### `fm.track` (`examples/field_mutation.spx`)

```semaprax
@id("fm.track")
fn track(flag: bool) -> i64
```

### `main` (`examples/field_mutation.spx`)

```semaprax
@id("main")
fn main() -> i64
```

### `geometry.length_squared` (`examples/floats.spx`)

```semaprax
@id("geometry.length_squared")
fn length_squared(vector: Vector) -> f64
```

### `geometry.inverse_length_squared` (`examples/floats.spx`)

```semaprax
@id("geometry.inverse_length_squared")
fn inverse_length_squared(vector: Vector) -> f64
```

### `geometry.half` (`examples/floats.spx`)

```semaprax
@id("geometry.half")
fn half(value: f32) -> f32
```

### `app.main` (`examples/floats.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `example.main` (`examples/inheritance.spx`)

```semaprax
@id("example.main")
fn main() -> i64
```

### `sum.pair` (`examples/integers_i32.spx`)

```semaprax
@id("sum.pair")
fn sum_pair(left: i32, right: i32) -> i32
```

### `sum.checked` (`examples/integers_i32.spx`)

```semaprax
@id("sum.checked")
fn checked() -> i32
```

### `compare.pair` (`examples/integers_i32.spx`)

```semaprax
@id("compare.pair")
fn compare(left: i32, right: i32) -> i64
```

### `app.main` (`examples/integers_i32.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `app.main` (`examples/lifecycle.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `math.gcd` (`examples/math_algorithms.spx`)

```semaprax
@id("math.gcd")
fn gcd(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result >= 0
```

### `math.lcm` (`examples/math_algorithms.spx`)

```semaprax
@id("math.lcm")
fn lcm(left: i64, right: i64) -> i64
    requires left > 0
    requires right > 0
```

### `math.is_prime` (`examples/math_algorithms.spx`)

```semaprax
@id("math.is_prime")
fn is_prime(value: i64) -> bool
    requires value >= 0
```

### `math.fibonacci` (`examples/math_algorithms.spx`)

```semaprax
@id("math.fibonacci")
fn fibonacci(nth: i64) -> i64
    requires nth >= 0
    requires nth <= 30
```

### `math.factorial` (`examples/math_algorithms.spx`)

```semaprax
@id("math.factorial")
fn factorial(value: i64) -> i64
    requires value >= 0
    requires value <= 12
```

### `math.digital_root` (`examples/math_algorithms.spx`)

```semaprax
@id("math.digital_root")
fn digital_root(value: i64) -> i64
    requires value >= 0
```

### `app.main` (`examples/math_algorithms.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `math.add` (`examples/meaning.spx`)

```semaprax
@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
```

### `app.main` (`examples/meaning.spx`)

```semaprax
@id("app.main")
fn main() -> i64
    ensures result == 42
```

### `example.token.identity` (`examples/native_callable.spx`)

```semaprax
@id("example.token.identity")
fn identity(value: own Token) -> Token
```

### `app.main` (`examples/native_callable.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `net-http-get.fetch` (`examples/net_http_get.spx`)

```semaprax
@id("net-http-get.fetch")
fn fetch() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
```

### `app.main` (`examples/net_http_get.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `order.status_value` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.status_value")
fn status_value(status: Status) -> i64
```

### `order.is_terminal` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.is_terminal")
fn is_terminal(status: Status) -> bool
```

### `order.decide` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.decide")
fn decide(line: Line, inventory: Inventory) -> Decision
```

### `order.decision_value` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.decision_value")
fn decision_value(decision: Decision) -> i64
```

### `order.line_total` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.line_total")
fn line_total(line: Line) -> i64
```

### `order.process_steps` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.process_steps")
fn process_steps(count: i64) -> i64
```

### `order.apply_batch` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.apply_batch")
fn apply_batch(total: i64, items: i64) -> i64
```

### `app.main` (`examples/order_lifecycle.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `order.safe_total` (`examples/order_lifecycle.spx`)

```semaprax
@id("order.safe_total")
fn safe_total(amount: i64) -> Result<i64, i64>
```

### `buffer.inspect` (`examples/ownership.spx`)

```semaprax
@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64
```

### `buffer.consume` (`examples/ownership.spx`)

```semaprax
@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64
```

### `buffer.pipeline` (`examples/ownership.spx`)

```semaprax
@id("buffer.pipeline")
fn pipeline(buffer: own Buffer) -> i64
    ensures result == 2
```

### `app.main` (`examples/ownership.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `geometry.line.shift` (`examples/records.spx`)

```semaprax
@id("geometry.line.shift")
fn shift(line: Line, amount: i64) -> Line
```

### `app.main` (`examples/records.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `refutable.sign_class` (`examples/refutable_match.spx`)

```semaprax
@id("refutable.sign_class")
fn sign_class(value: i64) -> i64
```

### `refutable.digit_name` (`examples/refutable_match.spx`)

```semaprax
@id("refutable.digit_name")
fn digit_name(digit: u8) -> i64
```

### `refutable.route` (`examples/refutable_match.spx`)

```semaprax
@id("refutable.route")
fn route(code: char) -> i64
```

### `main` (`examples/refutable_match.spx`)

```semaprax
@id("main")
fn main() -> i64
```

### `ops.combine` (`examples/string_ops.spx`)

```semaprax
@id("ops.combine")
fn combine(left: string, right: string) -> string
```

### `ops.world` (`examples/string_ops.spx`)

```semaprax
@id("ops.world")
fn world() -> string
```

### `test.main` (`examples/string_ops.spx`)

```semaprax
@id("test.main")
fn main() -> i64
```

### `ops.has_prefix` (`examples/string_ops_v2.spx`)

```semaprax
@id("ops.has_prefix")
fn has_prefix(value: string, prefix: string) -> bool
```

### `ops.holds` (`examples/string_ops_v2.spx`)

```semaprax
@id("ops.holds")
fn holds(value: string) -> i64
```

### `test.main` (`examples/string_ops_v2.spx`)

```semaprax
@id("test.main")
fn main() -> i64
```

### `test.main` (`examples/strings.spx`)

```semaprax
@id("test.main")
fn main() -> i64
```

### `text.count_byte` (`examples/text_analytics.spx`)

```semaprax
@id("text.count_byte")
fn count_byte(text: borrow str, target: u8) -> usize
```

### `text.count_words` (`examples/text_analytics.spx`)

```semaprax
@id("text.count_words")
fn count_words(text: borrow str) -> usize
```

### `text.is_empty_str` (`examples/text_analytics.spx`)

```semaprax
@id("text.is_empty_str")
fn is_empty_str(text: borrow str) -> bool
```

### `text.starts_with_hello` (`examples/text_analytics.spx`)

```semaprax
@id("text.starts_with_hello")
fn starts_with_hello(text: string) -> bool
```

### `text.contains_world` (`examples/text_analytics.spx`)

```semaprax
@id("text.contains_world")
fn contains_world(text: string) -> bool
```

### `text.build_greeting` (`examples/text_analytics.spx`)

```semaprax
@id("text.build_greeting")
fn build_greeting(prefix: string, suffix: string) -> string
```

### `text.palindrome_bytes` (`examples/text_analytics.spx`)

```semaprax
@id("text.palindrome_bytes")
fn palindrome_bytes(text: string) -> bool
```

### `app.main` (`examples/text_analytics.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `example.usize.checksum` (`examples/useful_data_usize_v1.spx`)

```semaprax
@id("example.usize.checksum")
fn checksum(seed: usize, count: usize) -> usize
```

### `app.main` (`examples/useful_data_usize_v1.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```

### `loops.digit_sum` (`examples/while_loops.spx`)

```semaprax
@id("loops.digit_sum")
fn digit_sum(value: i64) -> i64
```

### `loops.factorial` (`examples/while_loops.spx`)

```semaprax
@id("loops.factorial")
fn factorial(value: i64) -> i64
```

### `app.main` (`examples/while_loops.spx`)

```semaprax
@id("app.main")
fn main() -> i64
```
