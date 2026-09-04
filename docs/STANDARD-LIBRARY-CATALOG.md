# Standard library catalog

Status: generated from `std/` by `tests/project.rs::standard_library`; edit the sources, then regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`.

Audience: agents and humans choosing a standard-library declaration.

Every declaration below is verified, canonical, and executed by its package's conformance module on the interpreter, native C11, and Core Wasm lanes. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns the contract; `std/catalog.json` is the same catalog for tools.

## `std.bytes`

Package `std/bytes`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.bytes.byte_to_i64`

```semaprax
fn byte_to_i64(byte: u8) -> i64
    ensures result >= 0 && result <= 255
```

### `std.bytes.get_or`

```semaprax
fn get_or(view: borrow Slice<u8>, index: usize, fallback: i64) -> i64
    requires fallback >= -1 && fallback <= 255
    ensures result >= -1 && result <= 255
```

### `std.bytes.index_of`

```semaprax
fn index_of(view: borrow Slice<u8>, needle: u8) -> i64
    ensures result >= -1
```

### `std.bytes.position_of`

```semaprax
fn position_of(next_index: usize) -> i64
    requires next_index >= 1usize
    ensures result >= 0
```

### `std.bytes.count`

```semaprax
fn count(view: borrow Slice<u8>, needle: u8) -> usize
    ensures result <= byte_len(view)
```

### `std.bytes.is_ascii`

```semaprax
fn is_ascii(view: borrow Slice<u8>) -> bool
```

### `std.bytes.equals`

```semaprax
fn equals(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.bytes.starts_with`

```semaprax
fn starts_with(view: borrow Slice<u8>, prefix: borrow Slice<u8>) -> bool
```

### `std.bytes.ends_with`

```semaprax
fn ends_with(view: borrow Slice<u8>, suffix: borrow Slice<u8>) -> bool
```

### `std.bytes.read_u16_le`

```semaprax
fn read_u16_le(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 2usize <= byte_len(view)
    ensures result >= 0 && result <= 65535
```

### `std.bytes.read_u16_be`

```semaprax
fn read_u16_be(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 2usize <= byte_len(view)
    ensures result >= 0 && result <= 65535
```

### `std.bytes.read_u32_le`

```semaprax
fn read_u32_le(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 4usize <= byte_len(view)
    ensures result >= 0 && result <= 4294967295
```

### `std.bytes.read_u32_be`

```semaprax
fn read_u32_be(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 4usize <= byte_len(view)
    ensures result >= 0 && result <= 4294967295
```

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

## `std.encoding`

Package `std/encoding`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.encoding.byte_value`

```semaprax
fn byte_value(byte: u8) -> i64
    ensures result >= 0 && result <= 255
```

### `std.encoding.hex_value`

```semaprax
fn hex_value(byte: u8) -> i64
    ensures result >= -1 && result <= 15
```

### `std.encoding.is_hex_digit`

```semaprax
fn is_hex_digit(byte: u8) -> bool
    ensures result == (byte >= 48u8 && byte <= 57u8 || byte >= 65u8 && byte <= 70u8 || byte >= 97u8 && byte <= 102u8)
```

### `std.encoding.decode_hex_byte`

```semaprax
fn decode_hex_byte(high: u8, low: u8) -> i64
    ensures result >= -1 && result <= 255
```

### `std.encoding.encode_hex_lower`

```semaprax
fn encode_hex_lower(value: i64) -> i64
    requires value >= 0 && value <= 15
    ensures result >= 48 && result <= 102
```

### `std.encoding.encode_hex_upper`

```semaprax
fn encode_hex_upper(value: i64) -> i64
    requires value >= 0 && value <= 15
    ensures result >= 48 && result <= 70
```

### `std.encoding.base64_value`

```semaprax
fn base64_value(byte: u8) -> i64
    ensures result >= -1 && result <= 63
```

### `std.encoding.is_base64_digit`

```semaprax
fn is_base64_digit(byte: u8) -> bool
    ensures result == (byte >= 65u8 && byte <= 90u8 || byte >= 97u8 && byte <= 122u8 || byte >= 48u8 && byte <= 57u8 || byte == 43u8 || byte == 47u8)
```

### `std.encoding.encode_base64_digit`

```semaprax
fn encode_base64_digit(value: i64) -> i64
    requires value >= 0 && value <= 63
    ensures result >= 43 && result <= 122
```

### `std.encoding.decode_base64_quad`

```semaprax
fn decode_base64_quad(first: u8, second: u8, third: u8, fourth: u8) -> i64
    ensures result >= -1 && result <= 16777215
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

### `std.num.pow`

```semaprax
fn pow(base: i64, exponent: i64) -> i64
    requires exponent >= 0
    ensures exponent > 0 || result == 1
```

### `std.num.isqrt`

```semaprax
fn isqrt(value: i64) -> i64
    requires value >= 0
    ensures result >= 0 && result <= 3037000499
    ensures result * result <= value
```

### `std.num.digit_count`

```semaprax
fn digit_count(value: i64) -> i64
    ensures result >= 1 && result <= 19
```

### `std.num.is_power_of_two`

```semaprax
fn is_power_of_two(value: i64) -> bool
    ensures !result || value > 0
```

### `std.num.log2_floor`

```semaprax
fn log2_floor(value: i64) -> i64
    requires value > 0
    ensures result >= 0 && result <= 62
```

### `std.num.log10_floor`

```semaprax
fn log10_floor(value: i64) -> i64
    requires value > 0
    ensures result >= 0 && result <= 18
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

## `std.random`

Package `std/random`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.random.normalize_seed`

```semaprax
fn normalize_seed(value: i64) -> i64
    ensures result >= 1 && result <= 2147483646
```

### `std.random.next_seed`

```semaprax
fn next_seed(seed: i64) -> i64
    requires seed >= 1 && seed <= 2147483646
    ensures result >= 1 && result <= 2147483646
```

### `std.random.advance`

```semaprax
fn advance(seed: i64, steps: i64) -> i64
    requires seed >= 1 && seed <= 2147483646
    requires steps >= 0 && steps <= 100000
    ensures result >= 1 && result <= 2147483646
```

### `std.random.sample_below`

```semaprax
fn sample_below(seed: i64, upper: i64) -> i64
    requires seed >= 1 && seed <= 2147483646
    requires upper > 0 && upper <= 2147483647
    ensures result >= 0 && result < upper
```

## `std.text`

Package `std/text`, tier `core`, status partial. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.text.byte_len`

```semaprax
fn text_byte_len(value: borrow str) -> i64
```

### `std.text.contains`

```semaprax
fn contains(value: borrow str, needle: borrow str) -> bool
```

### `std.text.equals`

```semaprax
fn equals(left: borrow str, right: borrow str) -> bool
```

### `std.text.is_empty`

```semaprax
fn is_empty(value: borrow str) -> bool
```

### `std.text.starts_with`

```semaprax
fn starts_with(value: borrow str, prefix: borrow str) -> bool
```
