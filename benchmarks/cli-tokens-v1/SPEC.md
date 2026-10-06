# LogLens: the command-line benchmark application

The frozen specification every implementation in `benchmarks/cli-tokens-v1/`
builds. Unlike the web benchmarks, this is a plain command-line program:
parsing, string handling, counting by key, sorting, and formatted output.
An implementation is complete when `loglens sample.log` prints exactly
[`expected.txt`](expected.txt), `loglens sample.log --top 3 --json` prints
exactly [`expected.json`](expected.json) (both ending with one newline), and
it meets every rule below. [`sample.log`](sample.log) is the input.

## Invocation

```
loglens <file> [--top N] [--json]
```

- `<file>` is a text file of at most 64 KiB.
- `--top N` sets how many paths the top list shows, `1 <= N <= 50`
  (default 5).
- `--json` prints the JSON form instead of the text report.
- Exit status: 0 on success; 2 for a usage error (no file, an unknown flag,
  a bad or missing `--top` value), with a one-line message on stderr; 1 when
  the file cannot be read, with a one-line message on stderr.

## Input

Each non-empty line is one Common Log Format request:

```
203.0.113.7 - - [10/Oct/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326
```

A line is a request only when it has exactly this shape: a client address
without spaces; ` - - [`; a timestamp `DD/Mon/YYYY:HH:MM:SS +ZZZZ` whose `HH`
is `00`..`23`; `] "`; a method of uppercase letters; one space; a path that
starts with `/` and has no spaces; one space; a protocol without spaces;
`" `; a three-digit status `100`..`599`; one space; and a byte count of
digits or `-` (meaning 0). Anything else is malformed. Empty lines are
ignored and are not counted.

## Report

From the requests compute:

- `lines`: the number of non-empty lines.
- `requests`: the number of well-formed lines; `malformed`: the rest.
- `unique_ips`: the number of distinct client addresses.
- `status`: the request counts for `2xx`, `3xx`, `4xx`, and `5xx`. A 1xx
  status counts in none of them.
- `error_rate`: `(4xx + 5xx) / requests` as a percentage rounded half up to
  one decimal place (`0.0` when there are no requests).
- `bytes`: the sum of byte counts; `avg_bytes`: `bytes / requests` rounded
  down (`0` when there are no requests).
- `top_paths`: the N paths with the most requests, by count descending, ties
  by path ascending (bytewise).
- `hours`: the request count per hour of day, for hours that have requests,
  ascending.
- `busiest_hour`: the hour with the most requests, the earliest on a tie, or
  `-` when there are none.

Text form (`--top 3` shown):

```
lines: 12
requests: 10
malformed: 2
unique_ips: 4
status: 2xx=7 3xx=1 4xx=1 5xx=1
error_rate: 20.0%
bytes: 10450
avg_bytes: 1045
top_paths:
  1. /index.html 4
  2. /api/items 3
  3. /login 2
hours:
  09 3
  10 7
busiest_hour: 10
```

JSON form, one line, keys in this order:

```
{"lines":12,"requests":10,"malformed":2,"unique_ips":4,"status":{"2xx":7,"3xx":1,"4xx":1,"5xx":1},"error_rate":20.0,"bytes":10450,"avg_bytes":1045,"top_paths":[{"path":"/index.html","count":4},{"path":"/api/items","count":3},{"path":"/login","count":2}],"hours":{"09":3,"10":7},"busiest_hour":"10"}
```

`error_rate` always has exactly one decimal digit. Paths need no JSON
escaping: well-formed paths contain no `"` or `\`.

## Tests

Each implementation includes automated tests: at least the two golden
comparisons above and the three exit statuses.
