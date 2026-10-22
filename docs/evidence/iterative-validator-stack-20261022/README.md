# Iterative validator stack boundaries

An actual normal compiler reproduction of the original complete `INDEXED_LOOP`
source aborts during public HIR validation on a 128 KiB thread. The original
unoptimized iterative dispatcher reserves approximately 288 KiB. This public
case is distinct from the private 512-deep admission test.

The same 43 frame arms and 37 expression arms now use 16 local dispatch functions
with explicit stack boundaries. The complete interface/import loop also has a
separate stack boundary; its maps remain owned by the original validate driver.
All original arm tokens, diagnostics, literals, traversal order and Frame layout
declarations are preserved. The two original outer-loop continues become
successful phase returns followed by the next driver iteration. The entire
original validate driver is unchanged apart from that exact loop extraction.
The source remains within its existing 8,812-line recorded limit.

Both complete original source-role domains and all six original hostile controls
pass for the current source. Every prior source, class, role, exact acquisition
edge and attribute remains. The original codec composes all 697 existing rows in
a 254,402-byte page within the unchanged 262,144-byte limit. Complete published
predecessors remain exact immutable source views.

All three original source-domain executable gates pass, including the complete
Main44 preimage domain and the compiled lookup's EOF/foreign/duplicate controls.
The source harness passes Clippy with warnings denied. The latter audit first
reverses only the four exact later declarations and four exact comment packings;
every original source binding, fixed span and whole-predecessor check is retained.

The current actual normal Root library build/Clippy and public 128 KiB execution
remain pending. The paused local native attempt was released for device RAM;
hosted checks must qualify the complete compiler. The private 512-deep test and current-head hosted CI are also
pending; these focused checks do not qualify the full suite. No compiler
byte/work limit, acceptance floor or test stack limit was changed. Local work
monitors physical device RAM and swap; process RSS is telemetry only.
