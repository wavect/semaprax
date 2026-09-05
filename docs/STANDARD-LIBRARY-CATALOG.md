# Standard library catalog

Status: generated from `std/` through the `semaprax doc` documentation model by `tests/project.rs::standard_library`; edit the sources, then regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`.

Audience: agents and humans choosing a standard-library declaration.

Every declaration below is verified, canonical, and executed by its package's conformance module on the interpreter, native C11, and Core Wasm lanes. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns the contract; `std/catalog.json` is the same catalog for tools.

Consume a package from an installed compiler by adding its dependency line to the extensible manifest, then importing the selected stable identity: `[dependencies] std.num = "^0.1.0"` and `use function @id("std.num.abs") from std.num as abs;`. Set `[package] profile` to the package's required profile below; `scalar` means omit the profile key. The compiler supplies the closed bundled package without a source checkout, cache, or network access.

## `std.async`

Package `std/async`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.async = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.async.clamp_wait_ms`

```semaprax
fn clamp_wait_ms(timeout_ms: usize) -> usize
    ensures result <= 30000usize
```

### `std.async.next_timeout_ms`

```semaprax
fn next_timeout_ms(attempt: usize, base_ms: usize, cap_ms: usize) -> usize
    requires base_ms >= 1usize && base_ms <= cap_ms && cap_ms <= 30000usize
    ensures result >= base_ms && result <= cap_ms
```

### `std.async.should_retry`

```semaprax
fn should_retry(state: usize, attempts: usize, max_attempts: usize) -> bool
```

### `std.async.next_handle`

```semaprax
fn next_handle(current: usize, count: usize) -> usize
    requires count >= 1usize && count <= 8usize && current >= 1usize && current <= count
    ensures result >= 1usize && result <= count
```

### `std.async.remaining_ms`

```semaprax
fn remaining_ms(elapsed_ms: usize, budget_ms: usize) -> usize
    ensures result <= budget_ms
```

### `std.async.stream_ended`

```semaprax
fn stream_ended(chunk: borrow Slice<u8>) -> bool
```

## `std.bytes`

Package `std/bytes`, tier `core`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.bytes = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

Package `std/core`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.core = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

## `std.data.csv`

Package `std/data-csv`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.csv = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.csv.field_count`

```semaprax
fn field_count(record: borrow Slice<u8>) -> usize
    ensures result >= 1usize
    ensures result <= byte_len(record) + 1usize
```

### `std.data.csv.has_balanced_quotes`

```semaprax
fn has_balanced_quotes(record: borrow Slice<u8>) -> bool
```

### `std.data.csv.is_well_formed_record`

```semaprax
fn is_well_formed_record(record: borrow Slice<u8>) -> bool
```

## `std.data.toml`

Package `std/data-toml`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.toml = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.toml.is_bare_key`

```semaprax
fn is_bare_key(value: borrow Slice<u8>) -> bool
```

### `std.data.toml.is_blank`

```semaprax
fn is_blank(line: borrow Slice<u8>) -> bool
```

### `std.data.toml.is_comment`

```semaprax
fn is_comment(line: borrow Slice<u8>) -> bool
```

### `std.data.toml.assignment_index`

```semaprax
fn assignment_index(line: borrow Slice<u8>) -> i64
    ensures result >= -1
```

## `std.encoding`

Package `std/encoding`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.encoding = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

## `std.http`

Package `std/http`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.http = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.http.digit_value`

```semaprax
fn digit_value(byte: u8) -> i64
    ensures result >= -1 && result <= 9
```

### `std.http.digit_at`

```semaprax
fn digit_at(view: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 9
```

### `std.http.byte_is`

```semaprax
fn byte_is(view: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.http.lower`

```semaprax
fn lower(byte: u8) -> u8
```

### `std.http.method_is_valid`

```semaprax
fn method_is_valid(method: borrow Slice<u8>) -> bool
```

### `std.http.status_code`

```semaprax
fn status_code(response: borrow Slice<u8>) -> i64
    ensures result >= -1 && result <= 999
```

### `std.http.is_success`

```semaprax
fn is_success(code: i64) -> bool
```

### `std.http.terminator`

```semaprax
fn terminator(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.has_header_end`

```semaprax
fn has_header_end(response: borrow Slice<u8>) -> bool
```

### `std.http.header_end`

```semaprax
fn header_end(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.body_len`

```semaprax
fn body_len(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.name_at`

```semaprax
fn name_at(response: borrow Slice<u8>, index: usize) -> bool
```

### `std.http.length_start`

```semaprax
fn length_start(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.skip_blanks`

```semaprax
fn skip_blanks(view: borrow Slice<u8>, cursor: usize) -> usize
    ensures result >= cursor
```

### `std.http.decimal_at`

```semaprax
fn decimal_at(view: borrow Slice<u8>, start: usize) -> i64
    ensures result >= -1
```

### `std.http.content_length`

```semaprax
fn content_length(response: borrow Slice<u8>) -> i64
    ensures result >= -1
```

## `std.net`

Package `std/net`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.net = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.net.port_is_valid`

```semaprax
fn port_is_valid(port: usize) -> bool
```

### `std.net.wait_is_timeout`

```semaprax
fn wait_is_timeout(state: usize) -> bool
```

### `std.net.wait_is_readable`

```semaprax
fn wait_is_readable(state: usize) -> bool
```

### `std.net.wait_is_closed`

```semaprax
fn wait_is_closed(state: usize) -> bool
```

### `std.net.is_label_byte`

```semaprax
fn is_label_byte(byte: u8) -> bool
```

### `std.net.host_is_valid`

```semaprax
fn host_is_valid(host: borrow Slice<u8>) -> bool
```

### `std.net.digit_or_ten`

```semaprax
fn digit_or_ten(byte: u8) -> usize
    ensures result <= 10usize
```

### `std.net.is_ipv4`

```semaprax
fn is_ipv4(host: borrow Slice<u8>) -> bool
```

## `std.num`

Package `std/num`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.num = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

Package `std/num-overflow`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.num.overflow = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

## `std.path`

Package `std/path`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.path = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.path.is_absolute`

```semaprax
fn is_absolute(path: borrow Slice<u8>) -> bool
```

### `std.path.has_trailing_separator`

```semaprax
fn has_trailing_separator(path: borrow Slice<u8>) -> bool
```

### `std.path.segment_count`

```semaprax
fn segment_count(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

### `std.path.file_name_start`

```semaprax
fn file_name_start(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

### `std.path.parent_end`

```semaprax
fn parent_end(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

### `std.path.extension_start`

```semaprax
fn extension_start(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

## `std.random`

Package `std/random`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.random = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

## `std.test`

Package `std/test`, tier `test`, status partial. Required project profile: `scalar`. Dependency: `std.test = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.test.equal_i64`

```semaprax
fn equal_i64(actual: i64, expected: i64) -> bool
    ensures result == (actual == expected)
```

### `std.test.equal_bool`

```semaprax
fn equal_bool(actual: bool, expected: bool) -> bool
    ensures result == (actual == expected)
```

### `std.test.failure_unless`

```semaprax
fn failure_unless(condition: bool) -> i64
    ensures result == 0 || result == 1
```

### `std.test.failure_if`

```semaprax
fn failure_if(condition: bool) -> i64
    ensures result == 0 || result == 1
```

### `std.test.failure_bit_unless`

```semaprax
fn failure_bit_unless(condition: bool, failure_bit: i64) -> i64
    requires failure_bit > 0
    ensures result == 0 || result == failure_bit
```

## `std.text`

Package `std/text`, tier `core`, status partial. Required project profile: `useful-text-consumer.v1`. Dependency: `std.text = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

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

## `std.time`

Package `std/time`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.time = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.time.milliseconds`

```semaprax
fn milliseconds(seconds: i64) -> i64
    requires seconds >= 0 && seconds <= 9223372036854775
    ensures result >= 0
    ensures result % 1000 == 0
```

### `std.time.seconds_floor`

```semaprax
fn seconds_floor(milliseconds: i64) -> i64
    requires milliseconds >= 0
    ensures result >= 0
```

### `std.time.seconds_ceil`

```semaprax
fn seconds_ceil(milliseconds: i64) -> i64
    requires milliseconds >= 0
    ensures result >= 0
```

### `std.time.subsecond_milliseconds`

```semaprax
fn subsecond_milliseconds(milliseconds: i64) -> i64
    requires milliseconds >= 0
    ensures result >= 0 && result < 1000
```

### `std.time.deadline_reached`

```semaprax
fn deadline_reached(now_milliseconds: i64, deadline_milliseconds: i64) -> bool
    requires now_milliseconds >= 0 && deadline_milliseconds >= 0
    ensures result == now_milliseconds >= deadline_milliseconds
```

### `std.time.remaining_milliseconds`

```semaprax
fn remaining_milliseconds(now_milliseconds: i64, deadline_milliseconds: i64) -> i64
    requires now_milliseconds >= 0 && deadline_milliseconds >= 0
    ensures result >= 0
```

### `std.time.elapsed_milliseconds`

```semaprax
fn elapsed_milliseconds(start_milliseconds: i64, end_milliseconds: i64) -> i64
    requires start_milliseconds >= 0 && end_milliseconds >= start_milliseconds
    ensures result >= 0
```

### `std.time.saturating_add_milliseconds`

```semaprax
fn saturating_add_milliseconds(left: i64, right: i64) -> i64
    requires left >= 0 && right >= 0
    ensures result >= left && result >= right
```

## `std.url`

Package `std/url`, tier `portable`, status partial. Required project profile: `scalar`. Dependency: `std.url = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.url.is_scheme_start`

```semaprax
fn is_scheme_start(byte: u8) -> bool
```

### `std.url.is_scheme_continue`

```semaprax
fn is_scheme_continue(byte: u8) -> bool
```

### `std.url.is_unreserved`

```semaprax
fn is_unreserved(byte: u8) -> bool
```

### `std.url.is_percent_triplet`

```semaprax
fn is_percent_triplet(marker: u8, high: u8, low: u8) -> bool
```

### `std.url.decode_percent_triplet`

```semaprax
fn decode_percent_triplet(marker: u8, high: u8, low: u8) -> i64
    ensures result >= -1 && result <= 255
```
