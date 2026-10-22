# Preserve extensionless source evidence on Windows checkout

The 179 content-addressed archives in `e/` and one in `docs/evidence/w/`
lack an extension-specific line-ending rule. With `core.autocrlf=true`, Git
smudged their LF bytes to CRLF. An actual checkout-filter reproduction changed
one 48,210-byte source image into 49,429 bytes and broke its unchanged SHA256.

Mark these two archive namespaces `-text`, preserving every stored byte.
Actual Git checkout filters with the Windows setting now reproduce all 180
original registered lengths and SHA256 values exactly. No hash pin, reader,
source assertion, budget, floor, or semantic implementation changes. This Git
filter check is portable; it does not claim execution of the Windows suite.
