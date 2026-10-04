import pathlib
s = pathlib.Path("notes/store.py").read_text()
assert "json" not in s, "store.py still touches json directly"
assert "load_notes" in s and "save_notes" in s, "store.py does not use the persist module"
print("persist structure ok")
