---
# Hand-written to the agentskills.io specification fields (not an upstream file).
name: pdf-processing
description: >-
  Extract text and tables from PDF files, fill forms, merge documents.
  Use when handling PDFs or when the user mentions forms or extraction.
license: Apache-2.0
compatibility: "Needs python3 and network access for font downloads"
metadata:
  author: example-org
  version: "1.0"
allowed-tools: Bash(git add:*) Bash(pdftotext:*) Read
argument-hint: "[file.pdf]"
x-vendor.priority: high
tags: [pdf, documents]
---
# PDF processing

See [the reference](references/REFERENCE.md) for the form-field catalog.
Run `scripts/extract.py` only when the host grants it; it is never run for you.
