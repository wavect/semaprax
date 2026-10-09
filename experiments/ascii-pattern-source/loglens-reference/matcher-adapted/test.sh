#!/bin/sh
set -eu
cd "$(dirname "$0")"
./build.sh
node tests.mjs
