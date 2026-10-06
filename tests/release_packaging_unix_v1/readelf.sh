#!/bin/sh
# Stand-in ELF inspector. The packaging fixture's "binaries" are scripts, so
# the glibc baseline gate reads this report instead: one fixed old requirement
# plus FAKE_GLIBC, and two DT_NEEDED entries printed out of order.
set -eu
case "$1" in
    --version-info)
        printf '  0x0010: Name: GLIBC_2.17  Flags: none  Version: 3\n'
        printf '  0x0020: Name: GLIBC_%s  Flags: none  Version: 2\n' "$FAKE_GLIBC"
        ;;
    -d)
        printf ' 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]\n'
        printf ' 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]\n'
        ;;
    *) exit 9 ;;
esac
