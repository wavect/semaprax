import re, pathlib
core = pathlib.Path("src/core.spx").read_text()
ids = set(re.findall(r'@id\("(calculator\.[a-z-]+)"\)', core))
want = {"calculator.add","calculator.subtract","calculator.multiply","calculator.divide","calculator.is-negative","calculator.not"}
assert want <= ids, f"missing declarations: {sorted(want - ids)}"
t = pathlib.Path("src/tests.spx").read_text()
assert "is_negative(" in t and "divide(84, 2) == 42" in t, "tests module was weakened"
print("declarations and tests intact")
