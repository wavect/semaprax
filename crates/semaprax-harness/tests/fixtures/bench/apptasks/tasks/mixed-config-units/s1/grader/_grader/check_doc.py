import pathlib
d = pathlib.Path("docs/schedule.md").read_text()
assert "interval_ms" in d and "jitter_ms" in d and "milliseconds" in d, "docs/schedule.md not updated"
print("doc ok")
