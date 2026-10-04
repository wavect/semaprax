import pathlib, re
src = pathlib.Path("reports/order_row.py").read_text()
for need in ("slugify", "format_cents", "parse_iso", "short_date"):
    assert re.search(r"\b%s\b" % need, src), f"does not reuse existing {need}"
for forbid in ("import re", "strftime", "re.sub", "months", "// 100", "% 100"):
    assert forbid not in src, f"reimplements existing helper logic ({forbid})"
print("reuse ok")
