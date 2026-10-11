#!/bin/sh
# Build the C API library and run a real C program against it (header + shared library).
#   sh capi/tests/c-smoke.sh [debug|release]
set -e
PROFILE=${1:-debug}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
if [ "$PROFILE" = release ]; then cargo build --release -p khcore-capi; else cargo build -p khcore-capi; fi
OUT=${TMPDIR:-/tmp}/khcore-c-smoke
mkdir -p "$OUT"
CC=${CC:-cc}
# shared library
$CC -Wall -Wextra -Werror -std=c99 -Icapi/include capi/tests/smoke.c -Ltarget/$PROFILE -lkhcore_capi \
    -Wl,-rpath,"$ROOT/target/$PROFILE" -o "$OUT/smoke-shared"
"$OUT/smoke-shared" data/data.json data/lexicon.json
# static library (what the macOS app links)
$CC -Wall -Wextra -Werror -std=c99 -Icapi/include capi/tests/smoke.c target/$PROFILE/libkhcore_capi.a \
    -lpthread -ldl -lm -o "$OUT/smoke-static"
"$OUT/smoke-static" data/data.json data/lexicon.json
# the header must also compile as C++
printf '#include "khcore.h"\nint main(){return kh_abi_version()==KH_ABI_VERSION?0:1;}\n' > "$OUT/h.cpp"
${CXX:-c++} -Icapi/include "$OUT/h.cpp" target/$PROFILE/libkhcore_capi.a -lpthread -ldl -lm -o "$OUT/h-cpp" && "$OUT/h-cpp" && echo "header compiles as C++"
