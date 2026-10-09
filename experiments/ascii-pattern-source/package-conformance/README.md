# `std.pattern` package API conformance example

This small consumer package imports the public `std.pattern` matcher type and
every public function. Its entrypoint compiles a one-capture byte pattern,
matches a separate input view, checks packet validity/status/diagnostics/work,
and reads both capture endpoints. The package dependency is declared in the
manifest; it does not copy or import implementation helpers.

This source is pending the owning standard-library package and compiler
registration batch. No compiler or runtime check is claimed here.
