# Remaining integration digest formatting correction

The actual 23e8 unit compile exposed unsupported LowerHex on the upgraded digest output. A bounded source audit found the same two expressions in the new shared native/Wasm/interpreter record-field-update integration helper, not yet reached by that failed compilation. Use the existing public allocation-free semaprax digest adapter for both. Every expected digest, source input, child inventory and assertion remains exact.

The full immediate old helper and one unique full-source inverse are retained and verified. This is source-only prevention of the identical type error in the next selected harness stage; no integration compile or runtime success is claimed. Owning gates are the already-selected #836 native/Wasm/interpreter integration controls.
