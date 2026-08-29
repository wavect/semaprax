# Quickstart

Audience: new SEMAPRAX users and contributors.

Status: pre-alpha bounded calculator workflow; not a production-readiness claim.

This quickstart uses the installed `semaprax` CLI to create and exercise the
built-in calculator Project v1 template. It does not access a network,
initialize Git, or install dependencies.

From a directory where `first-semaprax` does not already exist, run:

```sh
semaprax new first-semaprax
cd first-semaprax
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax graph src/app.spx
semaprax build semaprax.toml --target web -o dist/web
```

The run command prints `42`. The graph command emits deterministic JSON for
the generated application module. The final command creates the single
missing `dist` parent and publishes the Web package at `dist/web`.

SEMAPRAX remains pre-alpha. This flow demonstrates the bounded calculator
project contract; it is not a production-readiness or broader ecosystem claim.
