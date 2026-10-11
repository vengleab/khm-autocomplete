#!/usr/bin/env python3
"""Embed data.json and lexicon.json into template.html -> index.html (standalone) and artifact.html (fragment)."""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "..", "data")
data = open(sys.argv[1] if len(sys.argv) > 1 else os.path.join(DATA, "data.json"), encoding="utf-8").read()
tpl = open(os.path.join(HERE, "template.html"), encoding="utf-8").read()
lex = open(os.path.join(DATA, "lexicon.json"), encoding="utf-8").read()
frag = tpl.replace("__DATA__", data).replace("__LEX__", lex)
open(os.path.join(HERE, "artifact.html"), "w", encoding="utf-8").write(frag)
wrap = ('<!doctype html>\n<html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        '<style>:root{color-scheme:light}body{margin:0}[hidden]{display:none!important}</style></head><body>\n'
        + frag + "\n</body></html>\n")
open(os.path.join(HERE, "index.html"), "w", encoding="utf-8").write(wrap)
print("wrote index.html and artifact.html", len(wrap), "bytes")
