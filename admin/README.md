# Admin pages: mapping and dictionary

A local database and two web pages to edit the language data of the keyboard, without touching code.

```
python3 admin/server.py          # then open http://127.0.0.1:8765/     (npm run admin)
```
Standard library only (Python 3.9+, SQLite). The server listens on 127.0.0.1 only. The database `data/khmer.db` is created from
`data/*.json` the first time (it is not committed). Nothing is sent anywhere.

| page | what it edits |
|---|---|
| `/mapping.html` | the **consonant and vowel mapping**: each consonant's series (A first, O second), Latin onset and final, which can take ៉ / ៊; each vowel sign's readings after an A and after an O consonant; extra Latin onsets (`g`, `j`...); Latin finals (`b` → ប); independent vowels. A "Try it" panel runs the real engine code with your unsaved tables: type `leab` and see the rule guesses, or a Khmer word and see how it is spelled. Save validates (same rules as the engine), keeps the last 30 versions, and can load an older one or the built-in default. |
| `/dictionary.html` | the **word dictionary**: for each word its Khmerlish spellings (the lexicon, space separated) and English translations (comma separated), and its frequency. Search, filter (no spelling / no translation / added here), edit rows, add words, remove words you added, try what a typed Khmerlish or English word converts to. A word of the original vocabulary can only have its spellings and translations cleared (the bigram data refers to words by position); words you add go to the end of the vocabulary. |

## From the database to the engine
The engine reads JSON, not the database. **Export** (on any page, or `python3 admin/khdb.py export`) writes
`data/mapping.json`, `lexicon.json`, `english.json` and `data.json` (byte-identical to the originals when nothing changed). Then:

```
# compiled data for the apps (the mapping travels inside khdata.bin; the default charts add nothing to the file)
cargo run --release -p khcore --bin khbuild -- data/data.json data/lexicon.json khdata.bin data/english.json data/mapping.json
# the demo page (embeds data/mapping.json)
python3 web/build_demo.py data/data.json          # or: npm run build:wasm
```
C API: `kh_engine_open_with_mapping(data, lexicon, english, mapping)`; WASM: `new WasmEngine(data, lexicon, english, mapping)`;
Rust: `Model::from_json_full(data, lexicon, Some(english), Some(mapping))`.

After exporting changed data, regenerate the golden vectors (`node tests/golden/gen.mjs`): the older ones are made from `data/`.
The mapping tests do not depend on your edits: they use `admin/default_mapping.json` (the built-in charts, never overwritten by Export).

## What the mapping changes (and what it does not)
- Khmer → Latin (`romanize`): the generated spelling of every word, which is part of the sound index (`shown` spelling and the generated keys).
  Uses onsets, finals, the first reading of each vowel, independent vowels. (Only a vowel sign alone, or with ំ / ះ, is looked up; a pair row
  such as ុះ is used for Latin → Khmer only.)
- Latin → Khmer (`compose`, the rule guesses in tier 1.8): onsets (+ extras), finals, all readings, the series switch flags.
- Not changed: `romKey` (the sound key that folds spellings like `ou/oo/u`) is code, not data. Changing it changes every match.
- A reading is lowercase a–z; the engine refuses a mapping that is missing a vowel sign it needs (the inherent vowel, every sign ា … ៅ, ំ, ះ).

## Files
```
admin/khdb.py             database layer + CLI: seed, export, stats; mapping validation; word CRUD (also usable as a library)
admin/server.py           the local server and JSON API (see the docstring)
admin/static/             index.html, mapping.html, dictionary.html, admin.css, admin.js
admin/default_mapping.json   the built-in charts as a mapping file (equals core/src/tables.rs; a Rust test checks it)
admin/test_admin.py       python3 -m unittest discover -s admin -v
tests/admin-check.mjs     the pages in a real browser against a real server (npm run test:admin)
```
Safety: no login, so do not expose the port. The server refuses other Host headers, cross-site writes and non-JSON writes, and does not
log query strings (they contain what you searched for).
