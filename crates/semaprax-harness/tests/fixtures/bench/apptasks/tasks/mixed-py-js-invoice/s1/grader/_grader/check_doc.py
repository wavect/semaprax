import pathlib
d = pathlib.Path("docs/contract.md").read_text()
assert "dueDate" in d, "contract doc does not mention dueDate"
print("doc ok")
