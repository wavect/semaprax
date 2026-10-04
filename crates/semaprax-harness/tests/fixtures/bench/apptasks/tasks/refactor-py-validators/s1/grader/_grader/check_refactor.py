import sys, pathlib
root = pathlib.Path(".")
v = (root / "app/validators.py").read_text()
assert "def normalize_email" in v, "validators.normalize_email missing"
for m in ("users", "invites", "newsletter"):
    src = (root / f"app/{m}.py").read_text()
    assert "normalize_email" in src, f"{m} does not use normalize_email"
    assert ".strip().lower()" not in src, f"{m} still duplicates normalization"
print("refactor structure ok")
