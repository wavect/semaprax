# Residual payload and complete HIR layout

Status: local prior-head layout and focused test evidence.
Audience: compiler and CI maintainers.

Rust 1.97.1 on aarch64-apple-darwin reports the complete retained Try payload at 128 bytes and the current payload at 96 bytes. The residual type itself changes from 40 to 8 bytes. The former enum uses a niche and occupies 128 bytes. The current enum has a one-byte discriminator with seven bytes of padding and occupies 104 bytes. Its complete saving is therefore 24 bytes. The expression, statement, field initializer, match arm and closure capture each save the same 24 bytes.

The owning test now checks the exact 32-byte typed payload saving, the actual aligned enum tag envelopes, and the corresponding exact whole-container deltas. The complete predecessor remains unchanged. Zero/exact/short construction and drop checks remain unchanged. The actual updated test body executes against the real normal compiler HIR types and complete retained predecessor; this does not claim a full Root cfg(test) run. Compiler layout diagnostics are explanatory evidence, not authority or a substitute for executable checks.

The existing finite source replacement had not previously changed this member's base current bytes. Its original base endpoint is retained under the mandatory whole-pre-overlay-v2-current view as well as the new named predecessor view. No replacement/addition member, codec, cap or source role changes.
