#!/bin/sh
# Regenerate capi/include/khcore.h from the Rust source (needs: cargo install cbindgen --locked).
# `sh capi/gen-header.sh --check` fails if the committed header is stale.
set -e
cd "$(dirname "$0")"
if [ "$1" = "--check" ]; then
  cbindgen --config cbindgen.toml --crate khcore-capi --output /tmp/khcore.h.check
  diff -u include/khcore.h /tmp/khcore.h.check && echo "khcore.h is up to date"
else
  cbindgen --config cbindgen.toml --crate khcore-capi --output include/khcore.h
fi
