"""Candidate: map sentinels in an owned byte buffer and compute the
one-based positional checksum of the transformed bytes. Unchanged between
the public and hidden phases.

`input` is copied into a private, owned `bytearray` before the sentinel
mapping is applied in place, mirroring the Rust reference's
`input.to_vec()` and the TypeScript reference's `new Uint8Array(input)`.
"""


def sentinel_checksum(input_bytes: bytes) -> int:
    transformed = bytearray(input_bytes)
    for index in range(len(transformed)):
        byte = transformed[index]
        if byte == 0xFF:
            transformed[index] = 0x00
        elif byte == 0x00:
            transformed[index] = 0xFF
        else:
            transformed[index] = 0x01
    checksum = 0
    for index in range(len(transformed)):
        checksum += (index + 1) * transformed[index]
    return checksum
