# Remaining receiver lexical source epoch

Status: Source-test repair; full compiler CI remains required.
Audience: Compiler maintainers reviewing complete historical source custody.

The original receiver test still acquires all three sources, applies every
original inverse, and compares every complete original. The lexical caller now
passes through the existing authenticated lexical832 and LexicalOperands
controllers before two exact generic paid-call ABI hooks are inverted. Whole
input and output SHA-256 checks reject drift and already inverted input.
No compiler behavior, budget, source fixture, old assertion, or diagnostic changes.
