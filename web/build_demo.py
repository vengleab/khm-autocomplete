#!/usr/bin/env python3
"""Embed data.json, lexicon.json and english.json into template.html.

  python3 web/build_demo.py [data.json]          -> web/index.html (JS engine, standalone) and web/artifact.html (fragment)
  python3 web/build_demo.py [data.json] --wasm   -> also web/index-wasm.html: the same page running the Rust engine
                                                    (needs `sh scripts/build-wasm.sh` first; works from file://)
"""
import base64, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "..", "data")
args = [a for a in sys.argv[1:] if not a.startswith("--")]
want_wasm = "--wasm" in sys.argv


def read(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


data = read(args[0] if args else os.path.join(DATA, "data.json"))
tpl = read(os.path.join(HERE, "template.html"))
lex = read(os.path.join(DATA, "lexicon.json"))
en = read(os.path.join(DATA, "english.json"))
frag = tpl.replace("__DATA__", data).replace("__LEX__", lex).replace("__EN__", en)
open(os.path.join(HERE, "artifact.html"), "w", encoding="utf-8").write(frag)
HEAD = ('<!doctype html>\n<html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        '<style>:root{color-scheme:light}body{margin:0}[hidden]{display:none!important}</style></head><body>\n')
wrap = HEAD + frag + "\n</body></html>\n"
open(os.path.join(HERE, "index.html"), "w", encoding="utf-8").write(wrap)
print("wrote index.html and artifact.html", len(wrap), "bytes")

stale = os.path.join(HERE, "index-wasm.html")
if not want_wasm and os.path.exists(stale):
    os.remove(stale)  # never test or ship a WASM page that was built from an older template
if want_wasm:
    pkg = os.path.join(HERE, "pkg-nomodules")
    try:
        glue = read(os.path.join(pkg, "khcore_wasm.js"))
        with open(os.path.join(pkg, "khcore_wasm_bg.wasm"), "rb") as f:
            wasm = base64.b64encode(f.read()).decode("ascii")
    except FileNotFoundError:
        sys.exit("web/pkg-nomodules is missing: run `sh scripts/build-wasm.sh` first")
    if "</script" in glue:
        sys.exit("glue contains </script, cannot inline")
    boot = ('const b = atob("%s"), u = new Uint8Array(b.length); for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i);'
            'wasm_bindgen.initSync({ module: u });' % wasm)
    scripts = ("<script>\n" + glue + "\n</script>\n<script>\n" + boot + "\n</script>\n<script>\n"
               + read(os.path.join(HERE, "wasm-adapter.js")) + "\n</script>\n")
    out = HEAD + scripts + frag + "\n</body></html>\n"
    open(os.path.join(HERE, "index-wasm.html"), "w", encoding="utf-8").write(out)
    print("wrote index-wasm.html", len(out), "bytes")
