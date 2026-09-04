# Standard library catalog

Status: generated from `std/` by `tests/project.rs::standard_library`; edit the sources, then regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`.

Audience: agents and humans choosing a standard-library declaration.

Every declaration below is verified, canonical, and executed by its package's conformance module on the interpreter, native C11, and Core Wasm lanes. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns the contract; `std/catalog.json` is the same catalog for tools.

## `std.core`

Package `std/core`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.core.ordering.less`

```semaprax
fn ordering_less() -> i64
    ensures result == -1
```

### `std.core.ordering.equal`

```semaprax
fn ordering_equal() -> i64
    ensures result == 0
```

### `std.core.ordering.greater`

```semaprax
fn ordering_greater() -> i64
    ensures result == 1
```

### `std.core.compare`

```semaprax
fn compare(left: i64, right: i64) -> i64
    ensures result >= -1 && result <= 1
    ensures result != 0 || left == right
    ensures result == 0 || left != right
```

### `std.core.min`

```semaprax
fn min(left: i64, right: i64) -> i64
    ensures result <= left && result <= right
    ensures result == left || result == right
```

### `std.core.max`

```semaprax
fn max(left: i64, right: i64) -> i64
    ensures result >= left && result >= right
    ensures result == left || result == right
```

### `std.core.clamp`

```semaprax
fn clamp(value: i64, low: i64, high: i64) -> i64
    requires low <= high
    ensures result >= low && result <= high
```

### `std.core.in_range`

```semaprax
fn in_range(value: i64, low: i64, high: i64) -> bool
    requires low <= high
    ensures result == (value >= low && value <= high)
```

### `std.core.bool_to_i64`

```semaprax
fn bool_to_i64(value: bool) -> i64
    ensures result == 0 || result == 1
```

### `std.core.i64_to_bool`

```semaprax
fn i64_to_bool(value: i64) -> bool
    ensures result == (value != 0)
```

### `std.core.xor`

```semaprax
fn xor(left: bool, right: bool) -> bool
    ensures result == (left != right)
```

### `std.core.implies`

```semaprax
fn implies(premise: bool, conclusion: bool) -> bool
    ensures result == (!premise || conclusion)
```

## `std.num`

Package `std/num`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.num.i64_min`

```semaprax
fn i64_min() -> i64
    ensures result == -9223372036854775807 - 1
```

### `std.num.i64_max`

```semaprax
fn i64_max() -> i64
    ensures result == 9223372036854775807
```

### `std.num.sign`

```semaprax
fn sign(value: i64) -> i64
    ensures result >= -1 && result <= 1
    ensures result == 0 || value != 0
```

### `std.num.abs`

```semaprax
fn abs(value: i64) -> i64
    requires value != -9223372036854775807 - 1
    ensures result >= 0
    ensures result == value || result == 0 - value
```

### `std.num.is_even`

```semaprax
fn is_even(value: i64) -> bool
    ensures result == (value % 2 == 0)
```

### `std.num.is_odd`

```semaprax
fn is_odd(value: i64) -> bool
    ensures result == (value % 2 != 0)
```

### `std.num.div_euclid`

```semaprax
fn div_euclid(dividend: i64, divisor: i64) -> i64
    requires divisor != 0
    requires dividend != -9223372036854775807 - 1 || divisor != -1
```

### `std.num.rem_euclid`

```semaprax
fn rem_euclid(dividend: i64, divisor: i64) -> i64
    requires divisor != 0
    requires dividend != -9223372036854775807 - 1 || divisor != -1
    ensures result >= 0
```

### `std.num.gcd`

```semaprax
fn gcd(left: i64, right: i64) -> i64
    requires left >= 0 && right >= 0
    ensures result >= 0
```

## `std.num.overflow`

Package `std/num-overflow`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.num.overflow.add_overflows`

```semaprax
fn add_overflows(left: i64, right: i64) -> bool
```

### `std.num.overflow.sub_overflows`

```semaprax
fn sub_overflows(left: i64, right: i64) -> bool
```

### `std.num.overflow.neg_overflows`

```semaprax
fn neg_overflows(value: i64) -> bool
    ensures result == (value == -9223372036854775807 - 1)
```

### `std.num.overflow.mul_overflows`

```semaprax
fn mul_overflows(left: i64, right: i64) -> bool
```

### `std.num.overflow.wrapping_add`

```semaprax
fn wrapping_add(left: i64, right: i64) -> i64
```

### `std.num.overflow.wrapping_sub`

```semaprax
fn wrapping_sub(left: i64, right: i64) -> i64
```

### `std.num.overflow.wrapping_neg`

```semaprax
fn wrapping_neg(value: i64) -> i64
```

### `std.num.overflow.saturating_add`

```semaprax
fn saturating_add(left: i64, right: i64) -> i64
```

### `std.num.overflow.saturating_sub`

```semaprax
fn saturating_sub(left: i64, right: i64) -> i64
```

### `std.num.overflow.saturating_neg`

```semaprax
fn saturating_neg(value: i64) -> i64
```

### `std.num.overflow.saturating_abs`

```semaprax
fn saturating_abs(value: i64) -> i64
    ensures result >= 0
```

### `std.num.overflow.saturating_mul`

```semaprax
fn saturating_mul(left: i64, right: i64) -> i64
```
