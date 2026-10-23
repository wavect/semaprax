# Exact calculator graph captures after the type cursor change

Status: Native browser, TypeScript, 230 Rust controls and all focused source audits pass.
Audience: Compiler and browser-gate maintainers reviewing exact graph fingerprints.

The complete hosted captures come from main 573623043c, Linux Rust 1.88,
run 40031634129, job 120681907600. The complete native captures use the
cached genuine default-cap production compiler library and freshly linked
unchanged CLI described in native-provenance.json. That library is not a fresh
current-head compiler build. Every prior graph byte is restored by changing
only the observed builder bill and self-digest. The earlier whole captures and
all original profile, drift, cap, role, and historical bridge tests remain.
