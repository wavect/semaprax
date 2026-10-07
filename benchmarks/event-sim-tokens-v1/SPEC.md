# ShiftSim: deterministic service scheduling

ShiftSim is a small event-driven engine for scheduling service jobs across a
fixed set of workers. It is a pure function: one JSON request in, one JSON
report out. The same frozen contract and acceptance corpus apply to the
TypeScript and SEMAPRAX arms.

## Request

The request is one JSON object with exactly these keys:

```json
{"servers":["S1","S2"],"patients":[{"id":"P1","arrival":0,"service":5,"priority":1,"deadline":8}]}
```

- `servers` is an array of 0 to 8 unique ASCII identifiers matching
  `[A-Za-z0-9_-]{1,16}`. Server identifiers are compared bytewise.
- `patients` is an array of at most 256 objects. Each object has exactly
  `id`, `arrival`, `service`, `priority`, and `deadline`.
- Patient identifiers are unique ASCII identifiers matching
  `[A-Za-z0-9_-]{1,16}`.
- `arrival` and `deadline` are integers from 0 through 1,000,000.
- `service` is an integer from 1 through 100,000.
- `priority` is an integer from 0 through 9; lower numbers are more urgent.
- `deadline` is an integer completion deadline. A job is late only when its
  completion time is greater than its deadline.
- Empty server and patient arrays are valid. A non-empty patient array with
  no servers is invalid.
- JSON whitespace may appear before, between, and after the request tokens.
  This specification imposes no raw-input byte limit; implementations must
  accept valid requests even when permitted whitespace makes the input exceed
  65,536 bytes. The semantic array and field bounds above still apply.

All integer fields are JSON integers, not booleans or floating point values.
Unknown or missing keys, duplicate identifiers, invalid types, and out-of-range
values are rejected. A valid request must produce exactly one JSON object and
one trailing newline on stdout. An invalid request exits with status 2, writes
one diagnostic line to stderr, and writes nothing to stdout.

## Scheduling rules

Workers are non-preemptive and each handles at most one patient at a time.
Simulation advances directly from one arrival or completion time to the next;
it does not iterate over every time unit.

At each event time, process all completions first, then add every patient
arriving at that time, then dispatch work. Before dispatch, record the number
of waiting patients for the peak-queue metric. Dispatch repeatedly while a
worker is free and a patient is waiting:

1. Select the waiting patient by priority ascending, arrival ascending, then
   patient identifier bytewise ascending.
2. Assign the free worker with the bytewise-smallest identifier.
3. Start the job at the current event time and finish it after `service`
   integer time units.

This rule defines simultaneous arrivals and completions completely. A worker
finishing at time `t` can take a job arriving at `t`. Jobs arriving while every
worker is busy remain queued. The simulation ends when every patient has
completed.

## Report

The report has exactly two keys, in this order: `assignments`, then `metrics`.
Assignments appear in dispatch order and contain exactly these fields:

```json
{"id":"P1","server":"S1","arrival":0,"start":0,"finish":5,"wait":0,"late":false}
```

`wait` is `start - arrival`. `late` is true exactly when `finish > deadline`.
Metrics contain, in this order:

- `patients`: number of patients completed.
- `total_wait`: sum of all patient wait times.
- `max_wait`: greatest wait, or 0 when there are no patients.
- `late`: number of late patients.
- `busy_time`: sum of service times across all workers.
- `makespan`: last completion time, or 0 when there are no patients.
- `peak_queue`: greatest waiting count measured after arrivals and before
  dispatch, or 0 when no patient ever waited.
- `utilization_ppm`: floor of `busy_time * 1,000,000 / (server_count * makespan)`;
  it is 0 when there are no patients or `makespan` is 0.

The output contains no timestamps from the host, randomness, floating-point
values, or additional fields. JSON strings use ordinary JSON escaping and the
object and array order shown above.

## Acceptance

The shared corpus in `acceptance/corpus.json` covers empty input, simultaneous
arrivals, priority and identifier tie-breaking, simultaneous completion and
arrival, multiple workers, idle gaps, deadline boundaries, and a longer
queue. An implementation is accepted only when it matches every expected
report and both invalid-input cases. The corpus and oracle are test material;
the live task prompt should direct agents to the request, this specification,
and the public interface only.
