# Public workspace wire accounting repair

Status: Preserved local and CI capture evidence; focused verification passed.
Audience: Compiler maintainers and CI reviewers.

Complete normal compiler API/CLI captures from macOS and complete Linux CI payloads preserve every semantic field. The current native builder debit is 2,232 bytes smaller on both hosts; macOS retains its independently measured 144-byte layout difference. Reversing only this debit and recomputing authenticated dependent digests reproduces all eighteen previous wire known answers exactly.

Run `python3 docs/evidence/public-wire-accounting-20261026/verify.py` to check the complete preserved bytes. This is an offline comparison of captured evidence. Public runtime documents remain unmodified and the tests still check exact raw hashes, counters, API/CLI parity, getters, locking and hostile inputs. Both original public tests passed against the complete normal compiler and CLI; all 3,139 production dependency inputs remained unchanged.
