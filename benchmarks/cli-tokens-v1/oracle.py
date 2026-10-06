"""Reference oracle for LogLens: writes sample.log and the golden outputs.

    python3 oracle.py            # regenerate sample.log, expected.txt, expected.json
    python3 oracle.py FILE [--top N] [--json]   # analyse FILE like the spec

The sample is generated from a fixed seed so it is reproducible. Benchmark
agents must not read this file.
"""
import random
import re
import sys
from pathlib import Path

LINE = re.compile(
    r'^(\S+) - - \[(\d\d)/([A-Z][a-z]{2})/(\d{4}):(\d\d):(\d\d):(\d\d) [+-]\d{4}\] '
    r'"([A-Z]+) (/\S*) (\S+)" (\d{3}) (\d+|-)$'
)


def analyse(text, top):
    lines = [line for line in text.split("\n") if line != ""]
    requests, ips, status, paths, hours, total = 0, set(), [0, 0, 0, 0], {}, {}, 0
    for line in lines:
        m = LINE.match(line)
        if not m or not ("00" <= m.group(5) <= "23") or not (100 <= int(m.group(11)) <= 599):
            continue
        requests += 1
        ips.add(m.group(1))
        code = int(m.group(11))
        if 200 <= code < 600:
            status[code // 100 - 2] += 1
        paths[m.group(9)] = paths.get(m.group(9), 0) + 1
        hours[m.group(5)] = hours.get(m.group(5), 0) + 1
        total += 0 if m.group(12) == "-" else int(m.group(12))
    errors = status[2] + status[3]
    rate = (errors * 1000 * 2 + requests) // (2 * requests) if requests else 0
    ranked = sorted(paths.items(), key=lambda kv: (-kv[1], kv[0].encode()))[:top]
    busiest = min(hours, key=lambda h: (-hours[h], h)) if hours else "-"
    return {
        "lines": len(lines),
        "requests": requests,
        "malformed": len(lines) - requests,
        "unique_ips": len(ips),
        "status": dict(zip(["2xx", "3xx", "4xx", "5xx"], status)),
        "error_rate": f"{rate // 10}.{rate % 10}",
        "bytes": total,
        "avg_bytes": total // requests if requests else 0,
        "top_paths": ranked,
        "hours": sorted(hours.items()),
        "busiest_hour": busiest,
    }


def text(r):
    out = [f"lines: {r['lines']}", f"requests: {r['requests']}", f"malformed: {r['malformed']}",
           f"unique_ips: {r['unique_ips']}",
           "status: " + " ".join(f"{k}={v}" for k, v in r["status"].items()),
           f"error_rate: {r['error_rate']}%", f"bytes: {r['bytes']}", f"avg_bytes: {r['avg_bytes']}",
           "top_paths:"]
    out += [f"  {i}. {p} {c}" for i, (p, c) in enumerate(r["top_paths"], 1)]
    out += ["hours:"] + [f"  {h} {c}" for h, c in r["hours"]]
    out.append(f"busiest_hour: {r['busiest_hour']}")
    return "\n".join(out) + "\n"


def json_line(r):
    status = ",".join(f'"{k}":{v}' for k, v in r["status"].items())
    paths = ",".join(f'{{"path":"{p}","count":{c}}}' for p, c in r["top_paths"])
    hours = ",".join(f'"{h}":{c}' for h, c in r["hours"])
    return (f'{{"lines":{r["lines"]},"requests":{r["requests"]},"malformed":{r["malformed"]},'
            f'"unique_ips":{r["unique_ips"]},"status":{{{status}}},"error_rate":{r["error_rate"]},'
            f'"bytes":{r["bytes"]},"avg_bytes":{r["avg_bytes"]},"top_paths":[{paths}],'
            f'"hours":{{{hours}}},"busiest_hour":"{r["busiest_hour"]}"}}\n')


def sample():
    rng = random.Random(20261007)
    ips = [f"203.0.113.{n}" for n in (7, 9, 21, 42, 77, 101)] + ["198.51.100.4", "2001:db8::1"]
    paths = ["/", "/index.html", "/api/items", "/api/items/42", "/login", "/static/app.js",
             "/static/app.css", "/search?q=logs", "/admin", "/health"]
    weights = [6, 9, 7, 3, 4, 5, 5, 2, 1, 6]
    statuses = [200] * 30 + [201, 204, 301, 302, 304, 304, 400, 401, 403, 404, 404, 404, 500, 502, 503, 101]
    methods = ["GET"] * 8 + ["POST", "PUT", "DELETE", "HEAD"]
    months = ["Oct"]
    out = []
    for i in range(240):
        hour = rng.choice([8, 9, 9, 10, 10, 10, 11, 13, 14, 14, 15, 17, 22, 23, 0])
        size = "-" if rng.random() < 0.08 else str(rng.randint(0, 9000))
        line = (f'{rng.choice(ips)} - - [{rng.randint(1, 28):02d}/{months[0]}/2026:{hour:02d}:'
                f'{rng.randint(0, 59):02d}:{rng.randint(0, 59):02d} +0000] '
                f'"{rng.choice(methods)} {rng.choices(paths, weights)[0]} HTTP/1.1" '
                f'{rng.choice(statuses)} {size}')
        out.append(line)
        if i % 37 == 5:
            out.append("")
    malformed = [
        '203.0.113.7 - - [10/Oct/2026:24:00:00 +0000] "GET / HTTP/1.1" 200 12',
        '203.0.113.7 - - [10/Oct/2026:10:00:00 +0000] "get / HTTP/1.1" 200 12',
        '203.0.113.7 - - [10/Oct/2026:10:00:00 +0000] "GET index.html HTTP/1.1" 200 12',
        '203.0.113.7 - - [10/Oct/2026:10:00:00 +0000] "GET / HTTP/1.1" 600 12',
        '203.0.113.7 - - [10/Oct/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12kb',
        '203.0.113.7 - - [10/Oct/2026:10:00:00 +0000] "GET /a b HTTP/1.1" 200 12',
        '203.0.113.7 - [10/Oct/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12',
        'not a log line at all',
        '203.0.113.7 - - [10/Oct/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12 extra',
    ]
    for n, line in enumerate(malformed):
        out.insert(17 + n * 23, line)
    return "\n".join(out) + "\n"


def main():
    here = Path(__file__).parent
    if len(sys.argv) == 1:
        data = sample()
        (here / "sample.log").write_text(data)
        (here / "expected.txt").write_text(text(analyse(data, 5)))
        (here / "expected.json").write_text(json_line(analyse(data, 3)))
        return
    args = sys.argv[1:]
    top = int(args[args.index("--top") + 1]) if "--top" in args else 5
    r = analyse(Path(args[0]).read_text(), top)
    sys.stdout.write(json_line(r) if "--json" in args else text(r))


if __name__ == "__main__":
    main()
