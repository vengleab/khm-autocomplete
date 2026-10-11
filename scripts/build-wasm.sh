#!/bin/sh
# Build the WebAssembly engine and its JS glue.
#   needs: rustup target add wasm32-unknown-unknown ; cargo install wasm-bindgen-cli --version <the wasm-bindgen in Cargo.lock>
#   out:   web/pkg/            ES module glue   (browsers served over http, Node tests)
#          web/pkg-nomodules/  classic-script glue (the single-file demo web/index-wasm.html, works from file://)
set -e
cd "$(dirname "$0")/.."
cargo build -p khcore-wasm --target wasm32-unknown-unknown --profile wasm
WASM=target/wasm32-unknown-unknown/wasm/khcore_wasm.wasm
rm -rf web/pkg web/pkg-nomodules
wasm-bindgen --target web --out-dir web/pkg "$WASM"
wasm-bindgen --target no-modules --out-dir web/pkg-nomodules "$WASM"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Os web/pkg/khcore_wasm_bg.wasm -o web/pkg/khcore_wasm_bg.wasm
  cp web/pkg/khcore_wasm_bg.wasm web/pkg-nomodules/khcore_wasm_bg.wasm
fi
ls -l web/pkg/*.wasm
