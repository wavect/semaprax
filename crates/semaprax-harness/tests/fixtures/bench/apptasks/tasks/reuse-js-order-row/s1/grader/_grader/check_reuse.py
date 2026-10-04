import pathlib, re
src = pathlib.Path("app/orderRow.js").read_text()
for need in ("kebab", "readDay", "dayLabel", "cashLabel"):
    assert re.search(r"\b%s\b" % need, src), f"does not reuse existing {need}"
for forbid in ("toLowerCase", "padStart", "MONTHS", "Math.floor", ".replace("):
    assert forbid not in src, f"reimplements existing helper logic ({forbid})"
print("reuse ok")
